//! Plugin Handle - Lifecycle Management
//!
//! This module provides the `PluginHandle` which manages a single plugin's lifecycle,
//! including initialization, request handling, health tracking, and shutdown.

use std::sync::Arc;
use std::time::Duration;

use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

use super::health::HealthTracker;
use super::process::{PluginProcess, PluginProcessConfig, ProcessError};
use super::protocol::{
    InitializeParams, MetadataGetParams, MetadataMatchParams, MetadataSearchParams,
    MetadataSearchResponse, PluginBookMetadata, PluginManifest, PluginSeriesMetadata, SearchResult,
    methods,
};
use super::rpc::{RpcClient, RpcError};
use super::secrets::SecretValue;
use super::storage_handler::StorageRequestHandler;

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

    #[error("Plugin spawn failed: {0}")]
    SpawnFailed(String),
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
    /// Admin-level configuration (from plugin settings)
    pub admin_config: Option<Value>,
    /// Per-user configuration (from user plugin settings)
    pub user_config: Option<Value>,
    /// Credentials to pass to plugin (via init message)
    /// Uses SecretValue to prevent logging of sensitive data
    pub credentials: Option<SecretValue>,
}

impl std::fmt::Debug for PluginConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PluginConfig")
            .field("process", &self.process)
            .field("request_timeout", &self.request_timeout)
            .field("shutdown_timeout", &self.shutdown_timeout)
            .field("max_failures", &self.max_failures)
            .field("admin_config", &self.admin_config)
            .field("user_config", &self.user_config)
            .field("credentials", &self.credentials) // SecretValue shows [REDACTED]
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
            admin_config: None,
            user_config: None,
            credentials: None,
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
    /// Optional storage handler for user plugin reverse RPC
    storage_handler: Option<StorageRequestHandler>,
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
            storage_handler: None,
        }
    }

    /// Create a new plugin handle with storage support for user plugins.
    ///
    /// The storage handler enables the plugin to make `storage/*` reverse RPC
    /// calls that are handled by the host using the database.
    pub fn new_with_storage(config: PluginConfig, storage_handler: StorageRequestHandler) -> Self {
        Self {
            health: Arc::new(HealthTracker::new(config.max_failures)),
            config,
            state: Arc::new(RwLock::new(PluginState::Idle)),
            client: Arc::new(RwLock::new(None)),
            manifest: Arc::new(RwLock::new(None)),
            storage_handler: Some(storage_handler),
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

        // Create RPC client (with storage support if configured)
        let mut client = match &self.storage_handler {
            Some(handler) => {
                debug!("Creating RPC client with storage handler for user plugin");
                RpcClient::new_with_storage(process, self.config.request_timeout, handler.clone())
            }
            None => RpcClient::new(process, self.config.request_timeout),
        };
        debug!("RPC client created, sending initialize request");

        // Initialize the plugin
        // Build merged config for backward compat, and send split configs
        let merged_config = match (&self.config.admin_config, &self.config.user_config) {
            (Some(admin), Some(user)) => {
                let mut merged = admin.clone();
                if let (Some(base), Some(overlay)) = (merged.as_object_mut(), user.as_object()) {
                    for (k, v) in overlay {
                        base.insert(k.clone(), v.clone());
                    }
                }
                Some(merged)
            }
            (Some(c), None) | (None, Some(c)) => Some(c.clone()),
            (None, None) => None,
        };
        let init_params = InitializeParams {
            config: merged_config,
            admin_config: self.config.admin_config.clone(),
            user_config: self.config.user_config.clone(),
            credentials: self.config.credentials.as_ref().map(|s| s.inner().clone()),
        };

        debug!(
            has_admin_config = init_params.admin_config.is_some(),
            has_user_config = init_params.user_config.is_some(),
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

        let client_guard = self.client.read().await;
        let client = client_guard.as_ref().ok_or(PluginError::NotInitialized)?;
        match client
            .call::<_, MetadataSearchResponse>(methods::METADATA_SERIES_SEARCH, params)
            .await
        {
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

    /// Get series metadata by external ID
    pub async fn get_series_metadata(
        &self,
        params: MetadataGetParams,
    ) -> Result<PluginSeriesMetadata, PluginError> {
        self.ensure_running().await?;

        let client_guard = self.client.read().await;
        let client = client_guard.as_ref().ok_or(PluginError::NotInitialized)?;
        match client.call(methods::METADATA_SERIES_GET, params).await {
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

    /// Get book metadata by external ID
    pub async fn get_book_metadata(
        &self,
        params: MetadataGetParams,
    ) -> Result<PluginBookMetadata, PluginError> {
        self.ensure_running().await?;

        let client_guard = self.client.read().await;
        let client = client_guard.as_ref().ok_or(PluginError::NotInitialized)?;
        match client.call(methods::METADATA_BOOK_GET, params).await {
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

    /// Search for book metadata
    pub async fn search_book(
        &self,
        params: super::protocol::BookSearchParams,
    ) -> Result<MetadataSearchResponse, PluginError> {
        self.ensure_running().await?;

        let timeout_ms = self.config.request_timeout.as_millis();
        debug!(
            isbn = ?params.isbn,
            query = ?params.query,
            timeout_ms = timeout_ms,
            "Plugin handle: sending book search request"
        );

        let client_guard = self.client.read().await;
        let client = client_guard.as_ref().ok_or(PluginError::NotInitialized)?;
        match client
            .call::<_, MetadataSearchResponse>(methods::METADATA_BOOK_SEARCH, params)
            .await
        {
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

    /// Find best match for a book (ISBN first, then title fallback)
    pub async fn match_book(
        &self,
        params: super::protocol::BookMatchParams,
    ) -> Result<Option<SearchResult>, PluginError> {
        self.ensure_running().await?;

        let client_guard = self.client.read().await;
        let client = client_guard.as_ref().ok_or(PluginError::NotInitialized)?;
        match client.call(methods::METADATA_BOOK_MATCH, params).await {
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

    /// Find best match for a series title
    pub async fn match_series(
        &self,
        params: MetadataMatchParams,
    ) -> Result<Option<SearchResult>, PluginError> {
        self.ensure_running().await?;

        let client_guard = self.client.read().await;
        let client = client_guard.as_ref().ok_or(PluginError::NotInitialized)?;
        match client.call(methods::METADATA_SERIES_MATCH, params).await {
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

    /// Ensure the plugin is in a running state
    async fn ensure_running(&self) -> Result<(), PluginError> {
        let state = self.state.read().await.clone();
        match state {
            PluginState::Running => Ok(()),
            PluginState::Disabled { reason } => Err(PluginError::Disabled { reason }),
            _ => Err(PluginError::NotInitialized),
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

// Note: PluginHandle doesn't need special Drop logic because RpcClient::Drop
// aborts the reader task, releasing the Arc<PluginProcess>. This allows the
// PluginProcess to drop and kill_on_drop(true) to fire, cleaning up the OS process.

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
        assert!(handle.manifest().await.is_none());
    }
}
