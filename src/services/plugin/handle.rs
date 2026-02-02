//! Plugin Handle - Lifecycle Management
//!
//! This module provides the `PluginHandle` which manages a single plugin's lifecycle,
//! including initialization, request handling, health tracking, and shutdown.
//!
//! Note: Some methods and error variants are designed for the complete plugin API
//! but may not be called from external code yet.

// Allow dead code for plugin API methods and error variants that are part of the
// complete API surface but not yet called from external code.
#![allow(dead_code)]

use std::sync::Arc;
use std::time::Duration;

use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

use super::health::{HealthState, HealthTracker};
use super::process::{PluginProcess, PluginProcessConfig, ProcessError};
use super::protocol::{
    methods, InitializeParams, MetadataGetParams, MetadataMatchParams, MetadataSearchParams,
    MetadataSearchResponse, PluginBookMetadata, PluginManifest, PluginSeriesMetadata, SearchResult,
};
use super::rpc::{RpcClient, RpcError};
use super::secrets::SecretValue;

/// Error type for plugin handle operations
#[derive(Debug, thiserror::Error)]
pub enum PluginError {
    #[error("Plugin process error: {0}")]
    Process(#[from] ProcessError),

    #[error("Plugin RPC error: {0}")]
    Rpc(#[from] RpcError),

    #[error("Plugin not initialized")]
    NotInitialized,

    #[error("Plugin is disabled: {reason}")]
    Disabled { reason: String },

    #[error("Plugin health check failed: {0}")]
    HealthCheckFailed(String),

    #[error("Plugin spawn failed: {0}")]
    SpawnFailed(String),

    #[error("Invalid manifest: {0}")]
    InvalidManifest(String),
}

/// Configuration for retry behavior on rate-limited requests
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Maximum number of retries before giving up
    pub max_retries: u32,
    /// Additional delay increment per retry (added to retry_after from API)
    /// Total delay = retry_after + (attempt - 1) * delay_increment
    pub delay_increment: Duration,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 5,
            delay_increment: Duration::from_secs(10),
        }
    }
}

/// Configuration for a plugin handle
///
/// Note: The `credentials` field uses `SecretValue` which implements `Debug`
/// to show `[REDACTED]` instead of actual credential values, preventing
/// accidental exposure in logs.
#[derive(Clone)]
pub struct PluginConfig {
    /// Process configuration
    pub process: PluginProcessConfig,
    /// Request timeout
    pub request_timeout: Duration,
    /// Shutdown timeout
    pub shutdown_timeout: Duration,
    /// Maximum consecutive failures before disabling
    pub max_failures: u32,
    /// Initial configuration to pass to plugin
    pub config: Option<Value>,
    /// Credentials to pass to plugin (via init message)
    /// Uses SecretValue to prevent logging of sensitive data
    pub credentials: Option<SecretValue>,
    /// Retry configuration for rate-limited requests
    pub retry_config: RetryConfig,
}

impl std::fmt::Debug for PluginConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PluginConfig")
            .field("process", &self.process)
            .field("request_timeout", &self.request_timeout)
            .field("shutdown_timeout", &self.shutdown_timeout)
            .field("max_failures", &self.max_failures)
            .field("config", &self.config)
            .field("credentials", &self.credentials) // SecretValue shows [REDACTED]
            .field("retry_config", &self.retry_config)
            .finish()
    }
}

impl Default for PluginConfig {
    fn default() -> Self {
        Self {
            process: PluginProcessConfig::new("echo"),
            request_timeout: Duration::from_secs(30),
            shutdown_timeout: Duration::from_secs(5),
            max_failures: 3,
            config: None,
            credentials: None,
            retry_config: RetryConfig::default(),
        }
    }
}

/// State of the plugin handle
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PluginState {
    /// Not yet started
    Idle,
    /// Process starting
    Starting,
    /// Process running and initialized
    Running,
    /// Being shut down
    ShuttingDown,
    /// Stopped (either gracefully or due to error)
    Stopped,
    /// Disabled due to failures
    Disabled { reason: String },
}

/// Handle for managing a single plugin's lifecycle
pub struct PluginHandle {
    /// Plugin configuration
    config: PluginConfig,
    /// Plugin state
    state: Arc<RwLock<PluginState>>,
    /// RPC client (if running)
    client: Arc<RwLock<Option<RpcClient>>>,
    /// Cached manifest (after initialization)
    manifest: Arc<RwLock<Option<PluginManifest>>>,
    /// Health tracker
    health: Arc<HealthTracker>,
}

impl PluginHandle {
    /// Create a new plugin handle with the given configuration
    pub fn new(config: PluginConfig) -> Self {
        Self {
            health: Arc::new(HealthTracker::new(config.max_failures)),
            config,
            state: Arc::new(RwLock::new(PluginState::Idle)),
            client: Arc::new(RwLock::new(None)),
            manifest: Arc::new(RwLock::new(None)),
        }
    }

    /// Get the current plugin state
    pub async fn state(&self) -> PluginState {
        self.state.read().await.clone()
    }

    /// Get the cached manifest (if initialized)
    pub async fn manifest(&self) -> Option<PluginManifest> {
        self.manifest.read().await.clone()
    }

    /// Get the health state
    pub async fn health_state(&self) -> HealthState {
        self.health.state().await
    }

    /// Check if the plugin is currently running
    pub async fn is_running(&self) -> bool {
        matches!(*self.state.read().await, PluginState::Running)
    }

    /// Check if the plugin is disabled
    pub async fn is_disabled(&self) -> bool {
        matches!(*self.state.read().await, PluginState::Disabled { .. })
    }

    /// Spawn the plugin process and initialize it
    pub async fn start(&self) -> Result<PluginManifest, PluginError> {
        // Check if already running
        {
            let state = self.state.read().await;
            match &*state {
                PluginState::Running => {
                    if let Some(manifest) = self.manifest.read().await.clone() {
                        return Ok(manifest);
                    }
                }
                PluginState::Disabled { reason } => {
                    return Err(PluginError::Disabled {
                        reason: reason.clone(),
                    });
                }
                _ => {}
            }
        }

        // Update state to starting
        {
            let mut state = self.state.write().await;
            *state = PluginState::Starting;
        }

        debug!(
            command = %self.config.process.command,
            args = ?self.config.process.args,
            working_directory = ?self.config.process.working_directory,
            has_credentials = self.config.credentials.is_some(),
            "Starting plugin process"
        );

        // Spawn the process
        let process = match PluginProcess::spawn(&self.config.process).await {
            Ok(p) => {
                debug!("Plugin process spawned, creating RPC client");
                p
            }
            Err(e) => {
                error!(
                    error = %e,
                    command = %self.config.process.command,
                    args = ?self.config.process.args,
                    "Failed to spawn plugin process"
                );
                let mut state = self.state.write().await;
                *state = PluginState::Stopped;
                return Err(PluginError::SpawnFailed(e.to_string()));
            }
        };

        // Create RPC client
        let mut client = RpcClient::new(process, self.config.request_timeout);
        debug!("RPC client created, sending initialize request");

        // Initialize the plugin
        // Convert SecretValue to Value for the init message
        let init_params = InitializeParams {
            config: self.config.config.clone(),
            credentials: self.config.credentials.as_ref().map(|s| s.inner().clone()),
        };

        debug!(
            has_config = init_params.config.is_some(),
            has_credentials = init_params.credentials.is_some(),
            "Sending initialize request to plugin"
        );

        let manifest: PluginManifest = match client.call(methods::INITIALIZE, init_params).await {
            Ok(m) => m,
            Err(e) => {
                error!(error = %e, "Plugin initialization failed");
                let _ = client.shutdown(self.config.shutdown_timeout).await;
                let mut state = self.state.write().await;
                *state = PluginState::Stopped;
                self.health.record_failure().await;
                return Err(PluginError::Rpc(e));
            }
        };

        info!(
            name = %manifest.name,
            version = %manifest.version,
            "Plugin initialized successfully"
        );

        // Store the client and manifest
        {
            let mut client_lock = self.client.write().await;
            *client_lock = Some(client);
        }
        {
            let mut manifest_lock = self.manifest.write().await;
            *manifest_lock = Some(manifest.clone());
        }
        {
            let mut state = self.state.write().await;
            *state = PluginState::Running;
        }

        self.health.record_success().await;
        Ok(manifest)
    }

    /// Stop the plugin gracefully
    pub async fn stop(&self) -> Result<(), PluginError> {
        let current_state = self.state.read().await.clone();

        if !matches!(current_state, PluginState::Running) {
            debug!("Plugin not running, nothing to stop");
            return Ok(());
        }

        // Update state
        {
            let mut state = self.state.write().await;
            *state = PluginState::ShuttingDown;
        }

        debug!("Stopping plugin");

        // Send shutdown message and close client
        let mut client_opt = self.client.write().await;
        if let Some(mut client) = client_opt.take() {
            // Try to send shutdown notification
            if let Err(e) = client.call_no_params::<Value>(methods::SHUTDOWN).await {
                warn!("Plugin shutdown request failed: {}", e);
            }

            // Wait for process to exit
            match client.shutdown(self.config.shutdown_timeout).await {
                Ok(code) => {
                    info!("Plugin process exited with code {}", code);
                }
                Err(e) => {
                    warn!("Plugin shutdown error: {}", e);
                }
            }
        }

        // Update state
        {
            let mut state = self.state.write().await;
            *state = PluginState::Stopped;
        }

        Ok(())
    }

    /// Restart the plugin
    pub async fn restart(&self) -> Result<PluginManifest, PluginError> {
        self.stop().await?;
        self.start().await
    }

    /// Send a ping to check if the plugin is responsive
    pub async fn ping(&self) -> Result<(), PluginError> {
        self.ensure_running().await?;
        let client_guard = self.client.read().await;
        let client = client_guard.as_ref().ok_or(PluginError::NotInitialized)?;
        let _: String = client.call_no_params(methods::PING).await?;
        self.health.record_success().await;
        Ok(())
    }

    /// Search for series metadata
    pub async fn search_series(
        &self,
        params: MetadataSearchParams,
    ) -> Result<MetadataSearchResponse, PluginError> {
        self.ensure_running().await?;

        let timeout_ms = self.config.request_timeout.as_millis();
        debug!(
            query = %params.query,
            timeout_ms = timeout_ms,
            "Plugin handle: sending search request"
        );

        self.call_with_retry("search_series", params, |p| async move {
            let client_guard = self.client.read().await;
            let client = client_guard
                .as_ref()
                .ok_or_else(|| RpcError::InvalidResponse("Client not initialized".to_string()))?;
            client
                .call::<_, MetadataSearchResponse>(methods::METADATA_SERIES_SEARCH, p)
                .await
        })
        .await
    }

    /// Get series metadata by external ID
    pub async fn get_series_metadata(
        &self,
        params: MetadataGetParams,
    ) -> Result<PluginSeriesMetadata, PluginError> {
        self.ensure_running().await?;

        self.call_with_retry("get_series_metadata", params, |p| async move {
            let client_guard = self.client.read().await;
            let client = client_guard
                .as_ref()
                .ok_or_else(|| RpcError::InvalidResponse("Client not initialized".to_string()))?;
            client.call(methods::METADATA_SERIES_GET, p).await
        })
        .await
    }

    /// Get book metadata by external ID (future use)
    #[allow(dead_code)]
    pub async fn get_book_metadata(
        &self,
        params: MetadataGetParams,
    ) -> Result<PluginBookMetadata, PluginError> {
        self.ensure_running().await?;

        // TODO: Change to METADATA_BOOK_GET when book metadata is implemented
        self.call_with_retry("get_book_metadata", params, |p| async move {
            let client_guard = self.client.read().await;
            let client = client_guard
                .as_ref()
                .ok_or_else(|| RpcError::InvalidResponse("Client not initialized".to_string()))?;
            client.call(methods::METADATA_SERIES_GET, p).await
        })
        .await
    }

    /// Find best match for a series title
    pub async fn match_series(
        &self,
        params: MetadataMatchParams,
    ) -> Result<Option<SearchResult>, PluginError> {
        self.ensure_running().await?;

        self.call_with_retry("match_series", params, |p| async move {
            let client_guard = self.client.read().await;
            let client = client_guard
                .as_ref()
                .ok_or_else(|| RpcError::InvalidResponse("Client not initialized".to_string()))?;
            client.call(methods::METADATA_SERIES_MATCH, p).await
        })
        .await
    }

    /// Call an arbitrary method on the plugin
    pub async fn call_method<P, R>(&self, method: &str, params: P) -> Result<R, PluginError>
    where
        P: Serialize,
        R: DeserializeOwned,
    {
        self.ensure_running().await?;
        let client_guard = self.client.read().await;
        let client = client_guard.as_ref().ok_or(PluginError::NotInitialized)?;
        match client.call(method, params).await {
            Ok(response) => {
                self.health.record_success().await;
                Ok(response)
            }
            Err(e) => {
                self.health.record_failure().await;
                self.check_and_disable().await;
                Err(PluginError::Rpc(e))
            }
        }
    }

    /// Re-enable a disabled plugin
    pub async fn enable(&self) -> Result<(), PluginError> {
        let current_state = self.state.read().await.clone();

        if let PluginState::Disabled { reason: _ } = current_state {
            self.health.reset().await;
            {
                let mut state = self.state.write().await;
                *state = PluginState::Idle;
            }
            info!("Plugin re-enabled");
            Ok(())
        } else {
            Ok(()) // Already enabled
        }
    }

    /// Ensure the plugin is in a running state
    async fn ensure_running(&self) -> Result<(), PluginError> {
        let state = self.state.read().await.clone();
        match state {
            PluginState::Running => Ok(()),
            PluginState::Disabled { reason } => Err(PluginError::Disabled { reason }),
            _ => Err(PluginError::NotInitialized),
        }
    }

    /// Check if an RPC error is retryable (rate limited)
    fn is_retryable_error(err: &RpcError) -> Option<u64> {
        match err {
            RpcError::RateLimited {
                retry_after_seconds,
            } => Some(*retry_after_seconds),
            _ => None,
        }
    }

    /// Calculate retry delay: retry_after + (attempt - 1) * delay_increment
    fn calculate_retry_delay(&self, retry_after_seconds: u64, attempt: u32) -> Duration {
        let base = Duration::from_secs(retry_after_seconds);
        let increment = self.config.retry_config.delay_increment * (attempt - 1);
        base + increment
    }

    /// Execute an RPC call with retry logic for rate-limited errors
    async fn call_with_retry<P, R, F, Fut>(
        &self,
        operation_name: &str,
        params: P,
        make_call: F,
    ) -> Result<R, PluginError>
    where
        P: Clone + std::fmt::Debug,
        F: Fn(P) -> Fut,
        Fut: std::future::Future<Output = Result<R, RpcError>>,
    {
        let max_retries = self.config.retry_config.max_retries;
        let mut attempt = 0u32;

        loop {
            attempt += 1;
            let result = make_call(params.clone()).await;

            match result {
                Ok(response) => {
                    self.health.record_success().await;
                    if attempt > 1 {
                        info!(
                            operation = operation_name,
                            attempt = attempt,
                            "Plugin operation succeeded after retry"
                        );
                    }
                    return Ok(response);
                }
                Err(e) => {
                    // Check if this is a retryable error
                    if let Some(retry_after) = Self::is_retryable_error(&e) {
                        if attempt < max_retries {
                            let delay = self.calculate_retry_delay(retry_after, attempt);
                            warn!(
                                operation = operation_name,
                                attempt = attempt,
                                max_retries = max_retries,
                                retry_after_seconds = retry_after,
                                delay_seconds = delay.as_secs(),
                                "Rate limited, will retry after delay"
                            );
                            tokio::time::sleep(delay).await;
                            continue;
                        }
                        // Max retries exhausted
                        error!(
                            operation = operation_name,
                            attempt = attempt,
                            max_retries = max_retries,
                            "Rate limited, max retries exhausted"
                        );
                    }

                    // Non-retryable error or max retries exhausted
                    let health_state = self.health.state().await;
                    error!(
                        operation = operation_name,
                        attempt = attempt,
                        error = %e,
                        error_debug = ?e,
                        health_status = %health_state.status,
                        consecutive_failures = health_state.consecutive_failures,
                        max_failures = self.config.max_failures,
                        "Plugin operation failed"
                    );
                    self.health.record_failure().await;
                    self.check_and_disable().await;
                    return Err(PluginError::Rpc(e));
                }
            }
        }
    }

    /// Check if the plugin should be disabled due to failures
    async fn check_and_disable(&self) {
        if self.health.should_disable().await {
            let mut state = self.state.write().await;
            if matches!(*state, PluginState::Running) {
                let reason = format!(
                    "Disabled after {} consecutive failures",
                    self.config.max_failures
                );
                warn!("{}", reason);
                *state = PluginState::Disabled { reason };
            }
        }
    }
}

// Note: We need a custom impl because RpcClient contains tokio tasks
impl Drop for PluginHandle {
    fn drop(&mut self) {
        // The RpcClient will clean up its tasks when dropped
        // The process will be killed due to kill_on_drop(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_config_default() {
        let config = PluginConfig::default();
        assert_eq!(config.request_timeout, Duration::from_secs(30));
        assert_eq!(config.shutdown_timeout, Duration::from_secs(5));
        assert_eq!(config.max_failures, 3);
    }

    #[test]
    fn test_plugin_state_eq() {
        assert_eq!(PluginState::Idle, PluginState::Idle);
        assert_eq!(PluginState::Running, PluginState::Running);
        assert_ne!(PluginState::Idle, PluginState::Running);

        let disabled1 = PluginState::Disabled {
            reason: "test".to_string(),
        };
        let disabled2 = PluginState::Disabled {
            reason: "test".to_string(),
        };
        assert_eq!(disabled1, disabled2);
    }

    #[tokio::test]
    async fn test_plugin_handle_initial_state() {
        let config = PluginConfig::default();
        let handle = PluginHandle::new(config);

        assert_eq!(handle.state().await, PluginState::Idle);
        assert!(!handle.is_running().await);
        assert!(!handle.is_disabled().await);
        assert!(handle.manifest().await.is_none());
    }

    #[tokio::test]
    async fn test_plugin_handle_enable_when_not_disabled() {
        let config = PluginConfig::default();
        let handle = PluginHandle::new(config);

        // Should be a no-op when not disabled
        handle.enable().await.unwrap();
        assert_eq!(handle.state().await, PluginState::Idle);
    }

    #[test]
    fn test_retry_config_default() {
        let config = RetryConfig::default();
        assert_eq!(config.max_retries, 5);
        assert_eq!(config.delay_increment, Duration::from_secs(10));
    }

    #[test]
    fn test_calculate_retry_delay() {
        let config = PluginConfig::default();
        let handle = PluginHandle::new(config);

        // Test delay calculation: retry_after + (attempt - 1) * delay_increment
        // With default delay_increment of 10s and retry_after of 10s:
        // Attempt 1: 10 + 0*10 = 10s
        // Attempt 2: 10 + 1*10 = 20s
        // Attempt 3: 10 + 2*10 = 30s
        // Attempt 4: 10 + 3*10 = 40s
        // Attempt 5: 10 + 4*10 = 50s

        assert_eq!(handle.calculate_retry_delay(10, 1), Duration::from_secs(10));
        assert_eq!(handle.calculate_retry_delay(10, 2), Duration::from_secs(20));
        assert_eq!(handle.calculate_retry_delay(10, 3), Duration::from_secs(30));
        assert_eq!(handle.calculate_retry_delay(10, 4), Duration::from_secs(40));
        assert_eq!(handle.calculate_retry_delay(10, 5), Duration::from_secs(50));

        // Test with different retry_after values
        assert_eq!(handle.calculate_retry_delay(5, 1), Duration::from_secs(5));
        assert_eq!(handle.calculate_retry_delay(5, 3), Duration::from_secs(25));
    }

    #[test]
    fn test_is_retryable_error() {
        use super::super::rpc::RpcError;

        // Rate limited is retryable
        let rate_limited = RpcError::RateLimited {
            retry_after_seconds: 10,
        };
        assert_eq!(PluginHandle::is_retryable_error(&rate_limited), Some(10));

        // Other errors are not retryable
        let not_found = RpcError::NotFound("test".to_string());
        assert_eq!(PluginHandle::is_retryable_error(&not_found), None);

        let auth_failed = RpcError::AuthFailed("test".to_string());
        assert_eq!(PluginHandle::is_retryable_error(&auth_failed), None);

        let timeout = RpcError::Timeout(Duration::from_secs(30));
        assert_eq!(PluginHandle::is_retryable_error(&timeout), None);

        let api_error = RpcError::ApiError("test".to_string());
        assert_eq!(PluginHandle::is_retryable_error(&api_error), None);
    }

    // Integration tests would require a mock plugin process
    // See tests/integration/plugin_handle.rs for full integration tests
}
