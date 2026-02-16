//! Plugin Manager - Multi-Plugin Coordination
//!
//! This module provides the `PluginManager` which coordinates multiple plugins,
//! handling plugin lifecycle, database synchronization, and request routing.
//!
//! ## Responsibilities
//!
//! - Load plugin configurations from database
//! - Spawn and manage plugin processes (lazy loading)
//! - Route requests to appropriate plugins based on scope
//! - Synchronize health status with database
//! - Handle plugin enable/disable/restart operations
//!
//! ## Architecture
//!
//! ```text
//! ┌───────────────────────────────────────────────────────────────────┐
//! │                        PluginManager                              │
//! │                                                                   │
//! │  plugins: HashMap<Uuid, PluginEntry>                              │
//! │                                                                   │
//! │  ┌─────────────────────────────────────────────────────────────┐  │
//! │  │ PluginEntry                                                 │  │
//! │  │   db_config: plugins::Model  (from database)                │  │
//! │  │   handle: Option<PluginHandle>  (spawned process)           │  │
//! │  └─────────────────────────────────────────────────────────────┘  │
//! │                                                                   │
//! │  Methods:                                                         │
//! │  - load_all()     → Load plugins from DB                          │
//! │  - get_or_spawn() → Lazy spawn plugin on first use                │
//! │  - by_scope()     → Get plugins that support a scope              │
//! │  - shutdown_all() → Graceful shutdown of all plugins              │
//! └───────────────────────────────────────────────────────────────────┘
//! ```
//!
//! Note: Some methods and error variants are part of the complete API
//! surface but not yet called from external code.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use sea_orm::DatabaseConnection;
use tokio::sync::{Mutex, RwLock};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::db::entities::plugins;
use crate::db::entities::user_plugins;
use crate::db::repositories::{
    FailureContext, PluginFailuresRepository, PluginsRepository, UserPluginsRepository,
};
use crate::services::PluginMetricsService;

use crate::services::user_plugin::token_refresh::{self, RefreshResult};

use super::handle::{PluginConfig, PluginError, PluginHandle, PluginState};
use super::process::PluginProcessConfig;
use super::protocol::{
    BookMatchParams, BookSearchParams, MetadataGetParams, MetadataMatchParams,
    MetadataSearchParams, MetadataSearchResponse, OAuthConfig, PluginBookMetadata, PluginManifest,
    PluginScope, PluginSeriesMetadata, SearchResult,
};
use super::secrets::SecretValue;
use super::storage_handler::StorageRequestHandler;

/// Error type for plugin manager operations
#[derive(Debug, thiserror::Error)]
pub enum PluginManagerError {
    #[error("Plugin not found: {0}")]
    PluginNotFound(Uuid),

    #[error("Plugin not enabled: {0}")]
    PluginNotEnabled(Uuid),

    #[error("Plugin error: {0}")]
    Plugin(#[from] PluginError),

    #[error("Database error: {0}")]
    Database(#[from] anyhow::Error),

    #[error("Encryption error: {0}")]
    Encryption(String),

    #[error("Rate limit exceeded for plugin {plugin_id}: {requests_per_minute} requests/minute")]
    RateLimited {
        plugin_id: Uuid,
        requests_per_minute: i32,
    },

    #[error("User plugin not found for user {user_id} and plugin {plugin_id}")]
    UserPluginNotFound { user_id: Uuid, plugin_id: Uuid },

    #[error("OAuth token refresh failed: {0}")]
    TokenRefreshFailed(String),

    #[error("OAuth re-authentication required for user plugin {0}")]
    ReauthRequired(Uuid),
}

impl PluginManagerError {
    /// Check if this error is a rate limit from the plugin's RPC layer
    /// (i.e., the external API returned a rate limit, not our local token bucket)
    ///
    /// Returns the `retry_after_seconds` value if this is an RPC rate limit error.
    pub fn rpc_retry_after_seconds(&self) -> Option<u64> {
        match self {
            PluginManagerError::Plugin(e) => e.rpc_retry_after_seconds(),
            _ => None,
        }
    }
}

/// Context for a user plugin operation
///
/// Tracks the user and their plugin instance, used for scoping
/// storage operations and credential injection.
#[derive(Debug, Clone)]
pub struct UserPluginContext {
    /// The user plugin instance ID (from `user_plugins` table)
    pub user_plugin_id: Uuid,
}

/// Configuration for the plugin manager
#[derive(Debug, Clone)]
pub struct PluginManagerConfig {
    /// Default request timeout for plugins
    pub default_request_timeout: Duration,
    /// Default shutdown timeout for plugins
    pub default_shutdown_timeout: Duration,
    /// Time window for counting failures (in seconds)
    /// Failures outside this window are not counted for auto-disable
    pub failure_window_seconds: i64,
    /// Number of failures within the window to trigger auto-disable
    pub failure_threshold: u32,
    /// How long to keep failure records (in days)
    pub failure_retention_days: i64,
    /// Whether to auto-sync health status to database
    pub auto_sync_health: bool,
    /// Interval between health checks (0 = disabled)
    pub health_check_interval: Duration,
    /// TTL for the plugin cache before refreshing from database
    /// This ensures multi-pod deployments eventually see plugin changes
    pub cache_ttl: Duration,
}

impl Default for PluginManagerConfig {
    fn default() -> Self {
        Self {
            default_request_timeout: Duration::from_secs(30),
            default_shutdown_timeout: Duration::from_secs(5),
            failure_window_seconds: 3600, // 1 hour
            failure_threshold: 3,
            failure_retention_days: 30,
            auto_sync_health: true,
            health_check_interval: Duration::from_secs(60), // Check every minute
            cache_ttl: Duration::from_secs(30),             // Refresh from DB every 30 seconds
        }
    }
}

/// Token bucket rate limiter for per-plugin rate limiting
///
/// Uses atomic operations for thread-safe rate limiting without locks.
/// Tokens refill over time based on the configured rate.
#[derive(Debug)]
pub struct TokenBucketRateLimiter {
    /// Current number of available tokens (scaled by 1000 for precision)
    tokens: AtomicU32,
    /// Last refill time as milliseconds since process start
    last_refill_ms: AtomicU64,
    /// Maximum tokens (bucket capacity)
    capacity: u32,
    /// Tokens to add per second (refill rate)
    tokens_per_second: f64,
    /// Start time for calculating elapsed milliseconds
    start_instant: Instant,
}

impl TokenBucketRateLimiter {
    /// Create a new rate limiter with the given requests per minute limit
    pub fn new(requests_per_minute: i32) -> Self {
        let capacity = requests_per_minute as u32;
        let tokens_per_second = requests_per_minute as f64 / 60.0;

        Self {
            tokens: AtomicU32::new(capacity),
            last_refill_ms: AtomicU64::new(0),
            capacity,
            tokens_per_second,
            start_instant: Instant::now(),
        }
    }

    /// Try to acquire a token. Returns true if successful, false if rate limited.
    pub fn try_acquire(&self) -> bool {
        // Calculate elapsed time since start
        let now_ms = self.start_instant.elapsed().as_millis() as u64;

        // Refill tokens based on elapsed time
        let last_refill = self.last_refill_ms.load(Ordering::Acquire);
        let elapsed_ms = now_ms.saturating_sub(last_refill);

        if elapsed_ms > 0 {
            // Calculate tokens to add
            let tokens_to_add =
                (elapsed_ms as f64 / 1000.0 * self.tokens_per_second).floor() as u32;

            if tokens_to_add > 0 {
                // Try to update last_refill time (CAS to handle concurrent updates)
                let _ = self.last_refill_ms.compare_exchange(
                    last_refill,
                    now_ms,
                    Ordering::Release,
                    Ordering::Relaxed,
                );

                // Add tokens up to capacity
                loop {
                    let current = self.tokens.load(Ordering::Acquire);
                    let new_tokens = (current + tokens_to_add).min(self.capacity);
                    if current == new_tokens {
                        break;
                    }
                    if self
                        .tokens
                        .compare_exchange(current, new_tokens, Ordering::Release, Ordering::Relaxed)
                        .is_ok()
                    {
                        break;
                    }
                }
            }
        }

        // Try to consume a token
        loop {
            let current = self.tokens.load(Ordering::Acquire);
            if current == 0 {
                return false;
            }
            if self
                .tokens
                .compare_exchange(current, current - 1, Ordering::Release, Ordering::Relaxed)
                .is_ok()
            {
                return true;
            }
        }
    }

    /// Get the current number of available tokens
    pub fn available_tokens(&self) -> u32 {
        self.tokens.load(Ordering::Acquire)
    }

    /// Get the bucket capacity
    pub fn capacity(&self) -> u32 {
        self.capacity
    }
}

/// Entry for a managed plugin
struct PluginEntry {
    /// Plugin configuration from database
    db_config: plugins::Model,
    /// Plugin handle (lazily spawned)
    handle: Option<Arc<PluginHandle>>,
    /// Rate limiter (if rate limit is configured)
    rate_limiter: Option<TokenBucketRateLimiter>,
    /// Spawn mutex to prevent concurrent spawn operations for the same plugin.
    /// This prevents a race condition where the write lock is released during
    /// the async `is_running()` check, allowing duplicate processes to spawn.
    spawn_mutex: Arc<Mutex<()>>,
}

impl PluginEntry {
    /// Create a new plugin entry from a database model
    fn new(plugin: plugins::Model) -> Self {
        let rate_limiter = plugin
            .rate_limit_requests_per_minute
            .filter(|&r| r > 0)
            .map(TokenBucketRateLimiter::new);

        Self {
            db_config: plugin,
            handle: None,
            rate_limiter,
            spawn_mutex: Arc::new(Mutex::new(())),
        }
    }

    /// Update the plugin configuration and recreate the rate limiter if needed
    fn update_config(&mut self, plugin: plugins::Model) {
        // Check if rate limit changed
        let old_rate = self.db_config.rate_limit_requests_per_minute;
        let new_rate = plugin.rate_limit_requests_per_minute;

        if old_rate != new_rate {
            tracing::info!(
                plugin_id = %plugin.id,
                plugin_name = %plugin.name,
                old_rate = ?old_rate,
                new_rate = ?new_rate,
                "Rate limit changed, recreating rate limiter"
            );
            self.rate_limiter = new_rate.filter(|&r| r > 0).map(TokenBucketRateLimiter::new);
        }

        self.db_config = plugin;
    }
}

/// Manager for coordinating multiple plugins
pub struct PluginManager {
    /// Database connection
    db: Arc<DatabaseConnection>,
    /// Manager configuration
    config: PluginManagerConfig,
    /// Managed plugins by ID
    plugins: Arc<RwLock<HashMap<Uuid, PluginEntry>>>,
    /// When the plugin cache was last refreshed from database
    /// Used for TTL-based cache invalidation in multi-pod deployments
    cache_loaded_at: RwLock<Option<Instant>>,
    /// Mutex to prevent thundering herd on cache refresh.
    /// Only one task can refresh the cache at a time; others wait for it to complete.
    cache_refresh_mutex: Mutex<()>,
    /// Health check task handle
    health_check_handle: RwLock<Option<tokio::task::JoinHandle<()>>>,
    /// Optional metrics service for recording plugin operation metrics
    metrics_service: Option<Arc<PluginMetricsService>>,
    /// Optional plugin file storage for resolving plugin data directories
    plugin_file_storage: Option<Arc<crate::services::PluginFileStorage>>,
}

impl PluginManager {
    /// Create a new plugin manager
    pub fn new(db: Arc<DatabaseConnection>, config: PluginManagerConfig) -> Self {
        Self {
            db,
            config,
            plugins: Arc::new(RwLock::new(HashMap::new())),
            cache_loaded_at: RwLock::new(None),
            cache_refresh_mutex: Mutex::new(()),
            health_check_handle: RwLock::new(None),
            metrics_service: None,
            plugin_file_storage: None,
        }
    }

    /// Create a new plugin manager with default configuration
    pub fn with_defaults(db: Arc<DatabaseConnection>) -> Self {
        Self::new(db, PluginManagerConfig::default())
    }

    /// Set the metrics service for recording plugin operation metrics
    pub fn with_metrics_service(mut self, metrics_service: Arc<PluginMetricsService>) -> Self {
        self.metrics_service = Some(metrics_service);
        self
    }

    /// Set the plugin file storage for resolving plugin data directories
    pub fn with_plugin_file_storage(
        mut self,
        storage: Arc<crate::services::PluginFileStorage>,
    ) -> Self {
        self.plugin_file_storage = Some(storage);
        self
    }

    /// Load all enabled plugins from database
    pub async fn load_all(&self) -> Result<usize, PluginManagerError> {
        debug!("Loading enabled plugins from database...");
        let enabled_plugins = PluginsRepository::get_enabled(&self.db).await?;
        let count = enabled_plugins.len();
        debug!("Found {} enabled plugins in database", count);

        let mut plugins = self.plugins.write().await;

        // Preserve existing handles - we don't want to kill running plugin processes
        // Just update the db_config for existing entries and add new ones
        let mut existing_handles: HashMap<Uuid, Option<Arc<PluginHandle>>> = HashMap::new();
        for (id, entry) in plugins.drain() {
            existing_handles.insert(id, entry.handle);
        }

        for plugin in enabled_plugins {
            let id = plugin.id;
            debug!("Loading plugin: {} ({})", plugin.name, id);
            let mut entry = PluginEntry::new(plugin);
            // Restore handle if we had one
            if let Some(handle) = existing_handles.remove(&id) {
                entry.handle = handle;
            }
            plugins.insert(id, entry);
        }

        // Stop any handles for plugins that are no longer enabled
        for (_id, handle) in existing_handles {
            if let Some(h) = handle {
                let _ = h.stop().await;
            }
        }

        // Update cache timestamp
        *self.cache_loaded_at.write().await = Some(Instant::now());

        info!("Loaded {} enabled plugins from database", count);
        Ok(count)
    }

    /// Check if the cache is stale and needs refreshing
    fn is_cache_stale(&self, loaded_at: Option<Instant>) -> bool {
        match loaded_at {
            None => true, // Never loaded
            Some(loaded) => loaded.elapsed() > self.config.cache_ttl,
        }
    }

    /// Refresh the plugin cache from database if it's stale
    ///
    /// This is called automatically by `plugins_by_scope` and similar methods
    /// to ensure multi-pod deployments eventually see plugin changes.
    ///
    /// Uses double-checked locking to prevent thundering herd:
    /// 1. Quick check without lock (fast path for fresh cache)
    /// 2. Acquire mutex and re-check (handles concurrent refresh attempts)
    /// 3. Refresh only if still stale after acquiring mutex
    async fn refresh_if_stale(&self) -> Result<(), PluginManagerError> {
        // Fast path: check if cache is stale without acquiring the refresh mutex
        let loaded_at = *self.cache_loaded_at.read().await;
        if !self.is_cache_stale(loaded_at) {
            return Ok(());
        }

        // Slow path: acquire the refresh mutex to prevent thundering herd
        let _refresh_guard = self.cache_refresh_mutex.lock().await;

        // Re-check after acquiring mutex - another task may have refreshed while we waited
        let loaded_at = *self.cache_loaded_at.read().await;
        if self.is_cache_stale(loaded_at) {
            debug!("Plugin cache is stale, refreshing from database");
            self.load_all().await?;
        } else {
            debug!("Plugin cache was refreshed by another task while waiting");
        }

        Ok(())
    }

    /// Reload a specific plugin's configuration from database
    pub async fn reload(&self, plugin_id: Uuid) -> Result<(), PluginManagerError> {
        debug!("Reloading plugin {} from database", plugin_id);

        let plugin = PluginsRepository::get_by_id(&self.db, plugin_id)
            .await?
            .ok_or(PluginManagerError::PluginNotFound(plugin_id))?;

        debug!(
            "Found plugin {} (name={}, enabled={}, scopes={:?})",
            plugin_id, plugin.name, plugin.enabled, plugin.scopes
        );

        let mut plugins = self.plugins.write().await;

        if plugin.enabled {
            // If plugin exists and has a handle, stop it first
            if let Some(entry) = plugins.get_mut(&plugin_id) {
                debug!("Updating existing plugin entry for {}", plugin_id);
                if let Some(handle) = entry.handle.take() {
                    let _ = handle.stop().await;
                }
                entry.update_config(plugin);
            } else {
                debug!("Inserting new plugin entry for {}", plugin_id);
                plugins.insert(plugin_id, PluginEntry::new(plugin));
            }
            debug!("Plugin manager now has {} plugins loaded", plugins.len());
        } else {
            // Plugin is disabled, remove it from managed plugins
            debug!("Plugin {} is disabled, removing from memory", plugin_id);
            if let Some(entry) = plugins.remove(&plugin_id)
                && let Some(handle) = entry.handle
            {
                let _ = handle.stop().await;
            }
        }

        Ok(())
    }

    /// Remove a plugin from memory without fetching from database
    ///
    /// Use this when a plugin has been deleted from the database and you need
    /// to clean up the in-memory state.
    pub async fn remove(&self, plugin_id: Uuid) {
        let mut plugins = self.plugins.write().await;
        if let Some(entry) = plugins.remove(&plugin_id) {
            if let Some(handle) = entry.handle {
                let _ = handle.stop().await;
            }
            debug!("Removed plugin {} from memory", plugin_id);
        }
    }

    /// Get or spawn a plugin, returning a handle for operations
    ///
    /// This method uses a per-plugin spawn mutex to prevent race conditions where
    /// multiple concurrent callers could spawn duplicate plugin processes. The
    /// pattern is:
    /// 1. Check if handle exists and is running (fast path, read lock only)
    /// 2. If not, acquire the spawn mutex to serialize spawn operations
    /// 3. Re-check under mutex in case another caller spawned while we waited
    /// 4. Spawn if still needed
    pub async fn get_or_spawn(
        &self,
        plugin_id: Uuid,
    ) -> Result<Arc<PluginHandle>, PluginManagerError> {
        // Fast path: check with read lock if we already have a running handle
        {
            let plugins = self.plugins.read().await;
            let entry = plugins
                .get(&plugin_id)
                .ok_or(PluginManagerError::PluginNotFound(plugin_id))?;

            if !entry.db_config.enabled {
                return Err(PluginManagerError::PluginNotEnabled(plugin_id));
            }

            if let Some(ref handle) = entry.handle
                && handle.state().await == PluginState::Running
            {
                return Ok(Arc::clone(handle));
            }
        }

        // Slow path: need to potentially spawn the plugin.
        // First, get the spawn mutex to serialize spawn operations for this plugin.
        // This prevents the race condition where multiple callers could see
        // "not running" and all try to spawn.
        let spawn_mutex = {
            let plugins = self.plugins.read().await;
            let entry = plugins
                .get(&plugin_id)
                .ok_or(PluginManagerError::PluginNotFound(plugin_id))?;
            Arc::clone(&entry.spawn_mutex)
        };

        // Hold the spawn mutex while we check again and potentially spawn.
        // This ensures only one caller can spawn at a time.
        let _spawn_guard = spawn_mutex.lock().await;

        // Re-check now that we hold the spawn mutex - another caller may have
        // spawned while we were waiting for the mutex.
        {
            let plugins = self.plugins.read().await;
            let entry = plugins
                .get(&plugin_id)
                .ok_or(PluginManagerError::PluginNotFound(plugin_id))?;

            if !entry.db_config.enabled {
                return Err(PluginManagerError::PluginNotEnabled(plugin_id));
            }

            if let Some(ref handle) = entry.handle
                && handle.state().await == PluginState::Running
            {
                return Ok(Arc::clone(handle));
            }
        }

        // Now get write lock and spawn
        let mut plugins = self.plugins.write().await;

        let entry = plugins
            .get_mut(&plugin_id)
            .ok_or(PluginManagerError::PluginNotFound(plugin_id))?;

        // Final check under write lock (in case of config change)
        if !entry.db_config.enabled {
            return Err(PluginManagerError::PluginNotEnabled(plugin_id));
        }

        // Need to spawn/initialize the plugin
        let handle_config = self.create_plugin_config(&entry.db_config).await?;
        let handle = PluginHandle::new(handle_config);

        // Start the plugin
        match handle.start().await {
            Ok(manifest) => {
                // Serialize manifest for storage
                let manifest_json = serde_json::to_value(&manifest).unwrap_or_default();

                // Update manifest in database
                let _ = PluginsRepository::update_manifest(
                    &self.db,
                    plugin_id,
                    Some(manifest_json.clone()),
                )
                .await;

                // Update manifest in in-memory config so it's available immediately
                // for plugin action queries (which check cached_manifest for capabilities)
                entry.db_config.manifest = Some(manifest_json);

                // Record success
                if self.config.auto_sync_health {
                    let _ = PluginsRepository::record_success(&self.db, plugin_id).await;
                }

                let handle = Arc::new(handle);
                // Store the handle for reuse and health checks
                entry.handle = Some(Arc::clone(&handle));
                Ok(handle)
            }
            Err(e) => {
                // Record failure using time-windowed tracking
                if self.config.auto_sync_health {
                    self.record_failure_and_check_disable(
                        plugin_id,
                        &e.to_string(),
                        Some("INIT_ERROR"),
                        Some("initialize"),
                    )
                    .await;
                }

                Err(PluginManagerError::Plugin(e))
            }
        }
    }

    /// Get all plugins that support a specific scope
    ///
    /// This method automatically refreshes the cache from the database if it's stale,
    /// ensuring multi-pod deployments eventually see plugin changes.
    pub async fn plugins_by_scope(&self, scope: &PluginScope) -> Vec<plugins::Model> {
        // Refresh cache if stale (ignore errors - use stale data if DB is unavailable)
        if let Err(e) = self.refresh_if_stale().await {
            warn!("Failed to refresh plugin cache: {}", e);
        }

        let plugins = self.plugins.read().await;
        plugins
            .values()
            .filter(|entry| entry.db_config.has_scope(scope))
            .map(|entry| entry.db_config.clone())
            .collect()
    }

    /// Get all plugins that support a specific scope AND apply to a specific library
    ///
    /// This filters plugins by:
    /// 1. Scope support
    /// 2. Library filtering (empty library_ids = all libraries, or library must be in the list)
    ///
    /// This method automatically refreshes the cache from the database if it's stale,
    /// ensuring multi-pod deployments eventually see plugin changes.
    pub async fn plugins_by_scope_and_library(
        &self,
        scope: &PluginScope,
        library_id: Uuid,
    ) -> Vec<plugins::Model> {
        // Refresh cache if stale (ignore errors - use stale data if DB is unavailable)
        if let Err(e) = self.refresh_if_stale().await {
            warn!("Failed to refresh plugin cache: {}", e);
        }

        let plugins = self.plugins.read().await;
        plugins
            .values()
            .filter(|entry| {
                entry.db_config.has_scope(scope) && entry.db_config.applies_to_library(library_id)
            })
            .map(|entry| entry.db_config.clone())
            .collect()
    }

    // =========================================================================
    // User Plugin Methods
    // =========================================================================

    /// Get or spawn a plugin handle for a specific user
    ///
    /// This method looks up the user's plugin instance from the database,
    /// decrypts their credentials, and spawns a plugin handle with per-user
    /// credential injection.
    ///
    /// Returns the plugin handle and user plugin context (for storage scoping).
    pub async fn get_user_plugin_handle(
        &self,
        plugin_id: Uuid,
        user_id: Uuid,
        request_timeout: Option<Duration>,
    ) -> Result<(Arc<PluginHandle>, UserPluginContext), PluginManagerError> {
        // Look up the user's plugin instance
        let user_plugin =
            UserPluginsRepository::get_by_user_and_plugin(&self.db, user_id, plugin_id)
                .await?
                .ok_or(PluginManagerError::UserPluginNotFound { user_id, plugin_id })?;

        if !user_plugin.enabled {
            return Err(PluginManagerError::PluginNotEnabled(plugin_id));
        }

        let context = UserPluginContext {
            user_plugin_id: user_plugin.id,
        };

        // Create a plugin config with user-specific credentials
        let mut handle_config = self
            .create_user_plugin_config(plugin_id, &user_plugin)
            .await?;

        // Override request timeout if caller specified one (e.g. background tasks use longer timeouts)
        if let Some(timeout) = request_timeout {
            handle_config.request_timeout = timeout;
        }

        // Create handle with storage support for user plugins
        let storage_handler = StorageRequestHandler::new(self.db.as_ref().clone(), user_plugin.id);
        let handle = PluginHandle::new_with_storage(handle_config, storage_handler);

        // Start the plugin
        match handle.start().await {
            Ok(manifest) => {
                // Record success for the user's instance
                let _ = UserPluginsRepository::record_success(&self.db, user_plugin.id).await;

                // Persist manifest to DB so that userConfigSchema and
                // capabilities are available on the UserPluginDto.
                let manifest_json = serde_json::to_value(&manifest).unwrap_or_default();
                let _ =
                    PluginsRepository::update_manifest(&self.db, plugin_id, Some(manifest_json))
                        .await;

                Ok((Arc::new(handle), context))
            }
            Err(e) => {
                // Record failure for the user's instance
                let _ = UserPluginsRepository::record_failure(&self.db, user_plugin.id).await;
                Err(PluginManagerError::Plugin(e))
            }
        }
    }

    /// Create a PluginConfig for a user plugin instance
    ///
    /// Uses the base plugin definition (command, args, etc.) but injects
    /// per-user credentials from the user_plugins table.
    async fn create_user_plugin_config(
        &self,
        plugin_id: Uuid,
        user_plugin: &user_plugins::Model,
    ) -> Result<PluginConfig, PluginManagerError> {
        // Get the base plugin definition
        let plugin = PluginsRepository::get_by_id(&self.db, plugin_id)
            .await?
            .ok_or(PluginManagerError::PluginNotFound(plugin_id))?;

        // Build process config from the base plugin
        let mut process_config = PluginProcessConfig::new(&plugin.command);
        process_config = process_config
            .plugin_name(&plugin.name)
            .args(plugin.args_vec());

        // Add environment variables from the base plugin config
        for (key, value) in plugin.env_vec() {
            process_config = process_config.env(&key, &value);
        }

        if let Some(wd) = &plugin.working_directory {
            process_config = process_config.working_directory(wd);
        }

        // Inject per-user credentials
        //
        // For user plugins we always deliver credentials via init_message
        // (the JSON-RPC initialize params) because SDK-based plugins read
        // from `params.credentials`. We also honour the legacy env-var path
        // when credential_delivery is "env" or "both".
        let mut credentials: Option<SecretValue> = None;
        let delivery = plugin.credential_delivery.as_str();

        // Try OAuth tokens first, then fall back to simple credentials
        if user_plugin.has_oauth_tokens() {
            // Ensure the OAuth token is still valid, refreshing if needed
            let access_token = self.ensure_fresh_oauth_token(&plugin, user_plugin).await?;

            let cred_value = serde_json::json!({
                "access_token": access_token,
            });

            if matches!(delivery, "env" | "both") {
                process_config = process_config.env("ACCESS_TOKEN", &access_token);
            }

            // Always include in init_message for user plugins
            credentials = Some(SecretValue::new(cred_value));
        } else if user_plugin.has_credentials() {
            // Decrypt simple credentials for the user
            if let Ok(Some(decrypted)) =
                UserPluginsRepository::get_credentials(&self.db, user_plugin.id).await
            {
                if matches!(delivery, "env" | "both")
                    && let Some(obj) = decrypted.as_object()
                {
                    for (key, value) in obj {
                        if let Some(v) = value.as_str() {
                            process_config = process_config.env(key.to_uppercase(), v);
                        }
                    }
                }

                // Always include in init_message for user plugins
                credentials = Some(SecretValue::new(decrypted));
            }
        }

        // Send admin config and user config separately
        let admin_config = Some(plugin.config.clone());
        let user_config = if user_plugin.config.is_object()
            && !user_plugin.config.as_object().is_none_or(|o| o.is_empty())
        {
            Some(user_plugin.config.clone())
        } else {
            None
        };

        // User plugins share the same data directory as their parent system plugin
        let data_dir = self.resolve_plugin_data_dir(&plugin.name).await;

        Ok(PluginConfig {
            process: process_config,
            request_timeout: self.config.default_request_timeout,
            shutdown_timeout: self.config.default_shutdown_timeout,
            max_failures: self.config.failure_threshold,
            admin_config,
            user_config,
            credentials,
            data_dir,
        })
    }

    /// Ensure the user's OAuth access token is fresh, refreshing it if needed.
    ///
    /// Returns the valid access token (either the existing one or a freshly refreshed one).
    /// Returns an error if re-authentication is required or the refresh fails.
    async fn ensure_fresh_oauth_token(
        &self,
        plugin: &plugins::Model,
        user_plugin: &user_plugins::Model,
    ) -> Result<String, PluginManagerError> {
        // Extract OAuth config from the plugin manifest
        let oauth_config = Self::get_oauth_config(plugin);
        let client_id = Self::get_oauth_client_id(plugin);

        // If OAuth config or client_id is missing, skip refresh and just return the
        // current access token (some plugins use non-standard OAuth flows)
        if let (Some(oauth_config), Some(client_id)) = (&oauth_config, &client_id) {
            let client_secret = Self::get_oauth_client_secret(plugin);

            match token_refresh::ensure_valid_token(
                &self.db,
                user_plugin,
                oauth_config,
                client_id,
                client_secret.as_deref(),
            )
            .await
            {
                Ok(RefreshResult::Refreshed { access_token }) => {
                    info!(
                        user_plugin_id = %user_plugin.id,
                        "Using refreshed OAuth token"
                    );
                    return Ok(access_token);
                }
                Ok(RefreshResult::StillValid) => {
                    // Fall through to return the existing token
                }
                Ok(RefreshResult::ReauthRequired) => {
                    return Err(PluginManagerError::ReauthRequired(user_plugin.id));
                }
                Ok(RefreshResult::Failed(reason)) => {
                    return Err(PluginManagerError::TokenRefreshFailed(reason));
                }
                Err(e) => {
                    return Err(PluginManagerError::TokenRefreshFailed(e.to_string()));
                }
            }
        }

        // Token is still valid (or no OAuth config to check against) — decrypt and return
        UserPluginsRepository::get_oauth_access_token(&self.db, user_plugin.id)
            .await
            .map_err(PluginManagerError::Database)?
            .ok_or_else(|| {
                PluginManagerError::TokenRefreshFailed(
                    "Failed to decrypt OAuth access token".to_string(),
                )
            })
    }

    /// Extract OAuth config from a plugin's stored manifest
    fn get_oauth_config(plugin: &plugins::Model) -> Option<OAuthConfig> {
        let manifest_json = plugin.manifest.as_ref()?;
        let manifest: PluginManifest = serde_json::from_value(manifest_json.clone()).ok()?;
        manifest.oauth
    }

    /// Get the OAuth client_id for a plugin (config override > manifest default)
    fn get_oauth_client_id(plugin: &plugins::Model) -> Option<String> {
        // Check plugin config for client_id override
        if let Some(client_id) = plugin
            .config
            .get("oauth_client_id")
            .and_then(|v| v.as_str())
        {
            return Some(client_id.to_string());
        }

        // Fall back to manifest's default client_id
        Self::get_oauth_config(plugin)?.client_id
    }

    /// Get OAuth client_secret from plugin config
    fn get_oauth_client_secret(plugin: &plugins::Model) -> Option<String> {
        plugin
            .config
            .get("oauth_client_secret")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
    }

    /// Check rate limit for a plugin. Returns Ok(plugin_name) if allowed, Err if rate limited.
    ///
    /// This method refreshes the plugin cache if it's stale, ensuring rate limit changes
    /// made via the API are eventually picked up by worker processes.
    async fn check_rate_limit(&self, plugin_id: Uuid) -> Result<String, PluginManagerError> {
        // Refresh cache if stale to pick up rate limit changes from other processes
        if let Err(e) = self.refresh_if_stale().await {
            warn!(
                "Failed to refresh plugin cache before rate limit check: {}",
                e
            );
        }

        let plugins = self.plugins.read().await;
        if let Some(entry) = plugins.get(&plugin_id) {
            let rate_config = entry.db_config.rate_limit_requests_per_minute;
            debug!(
                plugin_id = %plugin_id,
                plugin_name = %entry.db_config.name,
                rate_limit_config = ?rate_config,
                has_rate_limiter = entry.rate_limiter.is_some(),
                "Checking rate limit"
            );

            if let Some(ref rate_limiter) = entry.rate_limiter {
                let available = rate_limiter.available_tokens();
                debug!(
                    plugin_id = %plugin_id,
                    available_tokens = available,
                    capacity = rate_limiter.capacity(),
                    "Rate limiter state before acquire"
                );

                if !rate_limiter.try_acquire() {
                    let rate = entry.db_config.rate_limit_requests_per_minute.unwrap_or(0);
                    let plugin_name = entry.db_config.name.clone();

                    warn!(
                        plugin_id = %plugin_id,
                        plugin_name = %plugin_name,
                        rate_limit = rate,
                        "Rate limit exceeded - request blocked"
                    );

                    // Record rate limit rejection in metrics
                    if let Some(ref metrics) = self.metrics_service {
                        metrics.record_rate_limit(plugin_id, &plugin_name).await;
                    }

                    return Err(PluginManagerError::RateLimited {
                        plugin_id,
                        requests_per_minute: rate,
                    });
                }
            }
            Ok(entry.db_config.name.clone())
        } else {
            Ok(String::new())
        }
    }

    /// Search for series metadata using a specific plugin
    pub async fn search_series(
        &self,
        plugin_id: Uuid,
        params: MetadataSearchParams,
    ) -> Result<MetadataSearchResponse, PluginManagerError> {
        // Check rate limit before making the request
        let plugin_name = self.check_rate_limit(plugin_id).await?;

        let timeout_ms = self.config.default_request_timeout.as_millis();
        debug!(
            plugin_id = %plugin_id,
            plugin_name = %plugin_name,
            query = %params.query,
            timeout_ms = timeout_ms,
            "Starting plugin search request"
        );

        let start = Instant::now();
        let handle = self.get_or_spawn(plugin_id).await?;
        let result = handle.search_series(params.clone()).await;
        let duration_ms = start.elapsed().as_millis() as u64;

        match &result {
            Ok(response) => {
                debug!(
                    plugin_id = %plugin_id,
                    plugin_name = %plugin_name,
                    query = %params.query,
                    duration_ms = duration_ms,
                    result_count = response.results.len(),
                    "Plugin search completed successfully"
                );

                // Update health status on success
                if self.config.auto_sync_health {
                    let _ = PluginsRepository::record_success(&self.db, plugin_id).await;
                }

                // Record success in metrics
                if let Some(ref metrics) = self.metrics_service {
                    metrics
                        .record_success(plugin_id, &plugin_name, "search", duration_ms)
                        .await;
                }
            }
            Err(e) => {
                if e.rpc_retry_after_seconds().is_some() {
                    warn!(
                        plugin_id = %plugin_id,
                        plugin_name = %plugin_name,
                        query = %params.query,
                        duration_ms = duration_ms,
                        error = %e,
                        "Plugin search rate limited by external API"
                    );
                } else {
                    error!(
                        plugin_id = %plugin_id,
                        plugin_name = %plugin_name,
                        query = %params.query,
                        duration_ms = duration_ms,
                        timeout_ms = timeout_ms,
                        error = %e,
                        error_debug = ?e,
                        "Plugin search failed"
                    );
                }

                // Don't record RPC rate limits as failures — the plugin is healthy
                if e.rpc_retry_after_seconds().is_none()
                    && let Some(ref metrics) = self.metrics_service
                {
                    let error_code = self.error_to_code(e);
                    metrics
                        .record_failure(
                            plugin_id,
                            &plugin_name,
                            "search",
                            duration_ms,
                            Some(error_code),
                        )
                        .await;
                }
            }
        }

        Ok(result?)
    }

    /// Get series metadata using a specific plugin
    pub async fn get_series_metadata(
        &self,
        plugin_id: Uuid,
        params: MetadataGetParams,
    ) -> Result<PluginSeriesMetadata, PluginManagerError> {
        // Check rate limit before making the request
        let plugin_name = self.check_rate_limit(plugin_id).await?;

        let start = Instant::now();
        let handle = self.get_or_spawn(plugin_id).await?;
        let result = handle.get_series_metadata(params).await;
        let duration_ms = start.elapsed().as_millis() as u64;

        match &result {
            Ok(_) => {
                // Update health status on success
                if self.config.auto_sync_health {
                    let _ = PluginsRepository::record_success(&self.db, plugin_id).await;
                }

                // Record success in metrics
                if let Some(ref metrics) = self.metrics_service {
                    metrics
                        .record_success(plugin_id, &plugin_name, "get_metadata", duration_ms)
                        .await;
                }
            }
            Err(e) => {
                // Don't record RPC rate limits as failures — the plugin is healthy
                if e.rpc_retry_after_seconds().is_none()
                    && let Some(ref metrics) = self.metrics_service
                {
                    let error_code = self.error_to_code(e);
                    metrics
                        .record_failure(
                            plugin_id,
                            &plugin_name,
                            "get_metadata",
                            duration_ms,
                            Some(error_code),
                        )
                        .await;
                }
            }
        }

        Ok(result?)
    }

    /// Find best series match using a specific plugin
    pub async fn match_series(
        &self,
        plugin_id: Uuid,
        params: MetadataMatchParams,
    ) -> Result<Option<SearchResult>, PluginManagerError> {
        // Check rate limit before making the request
        let plugin_name = self.check_rate_limit(plugin_id).await?;

        let start = Instant::now();
        let handle = self.get_or_spawn(plugin_id).await?;
        let result = handle.match_series(params).await;
        let duration_ms = start.elapsed().as_millis() as u64;

        match &result {
            Ok(_) => {
                // Update health status on success
                if self.config.auto_sync_health {
                    let _ = PluginsRepository::record_success(&self.db, plugin_id).await;
                }

                // Record success in metrics
                if let Some(ref metrics) = self.metrics_service {
                    metrics
                        .record_success(plugin_id, &plugin_name, "match", duration_ms)
                        .await;
                }
            }
            Err(e) => {
                // Don't record RPC rate limits as failures — the plugin is healthy
                if e.rpc_retry_after_seconds().is_none()
                    && let Some(ref metrics) = self.metrics_service
                {
                    let error_code = self.error_to_code(e);
                    metrics
                        .record_failure(
                            plugin_id,
                            &plugin_name,
                            "match",
                            duration_ms,
                            Some(error_code),
                        )
                        .await;
                }
            }
        }

        Ok(result?)
    }

    // =========================================================================
    // Book Metadata Methods
    // =========================================================================

    /// Search for book metadata using a specific plugin
    pub async fn search_book(
        &self,
        plugin_id: Uuid,
        params: BookSearchParams,
    ) -> Result<MetadataSearchResponse, PluginManagerError> {
        // Check rate limit before making the request
        let plugin_name = self.check_rate_limit(plugin_id).await?;

        let start = Instant::now();
        let handle = self.get_or_spawn(plugin_id).await?;
        let result = handle.search_book(params.clone()).await;
        let duration_ms = start.elapsed().as_millis() as u64;

        match &result {
            Ok(response) => {
                debug!(
                    plugin_id = %plugin_id,
                    isbn = ?params.isbn,
                    query = ?params.query,
                    result_count = response.results.len(),
                    duration_ms = duration_ms,
                    "Book search completed"
                );

                // Update health status on success
                if self.config.auto_sync_health {
                    let _ = PluginsRepository::record_success(&self.db, plugin_id).await;
                }

                // Record success in metrics
                if let Some(ref metrics) = self.metrics_service {
                    metrics
                        .record_success(plugin_id, &plugin_name, "book_search", duration_ms)
                        .await;
                }
            }
            Err(e) => {
                if e.rpc_retry_after_seconds().is_some() {
                    warn!(
                        plugin_id = %plugin_id,
                        isbn = ?params.isbn,
                        query = ?params.query,
                        duration_ms = duration_ms,
                        error = %e,
                        "Book search rate limited by external API"
                    );
                } else {
                    warn!(
                        plugin_id = %plugin_id,
                        isbn = ?params.isbn,
                        query = ?params.query,
                        error = %e,
                        duration_ms = duration_ms,
                        "Book search failed"
                    );
                }

                // Don't record RPC rate limits as failures — the plugin is healthy
                if e.rpc_retry_after_seconds().is_none()
                    && let Some(ref metrics) = self.metrics_service
                {
                    let error_code = self.error_to_code(e);
                    metrics
                        .record_failure(
                            plugin_id,
                            &plugin_name,
                            "book_search",
                            duration_ms,
                            Some(error_code),
                        )
                        .await;
                }
            }
        }

        Ok(result?)
    }

    /// Get full book metadata using a specific plugin
    pub async fn get_book_metadata(
        &self,
        plugin_id: Uuid,
        params: MetadataGetParams,
    ) -> Result<PluginBookMetadata, PluginManagerError> {
        // Check rate limit before making the request
        let plugin_name = self.check_rate_limit(plugin_id).await?;

        let start = Instant::now();
        let handle = self.get_or_spawn(plugin_id).await?;
        let result = handle.get_book_metadata(params).await;
        let duration_ms = start.elapsed().as_millis() as u64;

        match &result {
            Ok(_) => {
                // Update health status on success
                if self.config.auto_sync_health {
                    let _ = PluginsRepository::record_success(&self.db, plugin_id).await;
                }

                // Record success in metrics
                if let Some(ref metrics) = self.metrics_service {
                    metrics
                        .record_success(plugin_id, &plugin_name, "book_get", duration_ms)
                        .await;
                }
            }
            Err(e) => {
                // Don't record RPC rate limits as failures — the plugin is healthy
                if e.rpc_retry_after_seconds().is_none()
                    && let Some(ref metrics) = self.metrics_service
                {
                    let error_code = self.error_to_code(e);
                    metrics
                        .record_failure(
                            plugin_id,
                            &plugin_name,
                            "book_get",
                            duration_ms,
                            Some(error_code),
                        )
                        .await;
                }
            }
        }

        Ok(result?)
    }

    /// Find best book match using a specific plugin
    pub async fn match_book(
        &self,
        plugin_id: Uuid,
        params: BookMatchParams,
    ) -> Result<Option<SearchResult>, PluginManagerError> {
        // Check rate limit before making the request
        let plugin_name = self.check_rate_limit(plugin_id).await?;

        let start = Instant::now();
        let handle = self.get_or_spawn(plugin_id).await?;
        let result = handle.match_book(params).await;
        let duration_ms = start.elapsed().as_millis() as u64;

        match &result {
            Ok(_) => {
                // Update health status on success
                if self.config.auto_sync_health {
                    let _ = PluginsRepository::record_success(&self.db, plugin_id).await;
                }

                // Record success in metrics
                if let Some(ref metrics) = self.metrics_service {
                    metrics
                        .record_success(plugin_id, &plugin_name, "book_match", duration_ms)
                        .await;
                }
            }
            Err(e) => {
                // Don't record RPC rate limits as failures — the plugin is healthy
                if e.rpc_retry_after_seconds().is_none()
                    && let Some(ref metrics) = self.metrics_service
                {
                    let error_code = self.error_to_code(e);
                    metrics
                        .record_failure(
                            plugin_id,
                            &plugin_name,
                            "book_match",
                            duration_ms,
                            Some(error_code),
                        )
                        .await;
                }
            }
        }

        Ok(result?)
    }

    // =========================================================================
    // Health Check Methods
    // =========================================================================

    /// Ping a plugin to check health
    pub async fn ping(&self, plugin_id: Uuid) -> Result<(), PluginManagerError> {
        let handle = self.get_or_spawn(plugin_id).await?;
        handle.ping().await?;

        if self.config.auto_sync_health {
            let _ = PluginsRepository::record_success(&self.db, plugin_id).await;
        }

        Ok(())
    }

    /// Test a plugin connection by spawning it and getting its manifest
    ///
    /// This is useful for admin testing of plugin configuration without
    /// affecting the managed plugin state.
    pub async fn test_plugin(
        &self,
        _db: &DatabaseConnection,
        plugin: &plugins::Model,
    ) -> Result<super::protocol::PluginManifest, PluginManagerError> {
        // Create config for the test
        let handle_config = self.create_plugin_config(plugin).await?;
        let handle = PluginHandle::new(handle_config);

        // Start the plugin and get manifest
        let manifest = handle.start().await?;

        // Stop the test instance
        let _ = handle.stop().await;

        Ok(manifest)
    }

    /// Shutdown a specific plugin
    pub async fn stop_plugin(&self, plugin_id: Uuid) -> Result<(), PluginManagerError> {
        let mut plugins = self.plugins.write().await;

        if let Some(entry) = plugins.get_mut(&plugin_id)
            && let Some(handle) = entry.handle.take()
        {
            handle.stop().await?;
        }

        Ok(())
    }

    /// Shutdown all plugins gracefully
    pub async fn shutdown_all(&self) {
        // Stop health checks first
        self.stop_health_checks().await;

        let mut plugins = self.plugins.write().await;

        for (id, entry) in plugins.iter_mut() {
            if let Some(handle) = entry.handle.take() {
                debug!("Shutting down plugin {}", id);
                if let Err(e) = handle.stop().await {
                    warn!("Failed to stop plugin {}: {}", id, e);
                }
            }
        }

        plugins.clear();
        info!("All plugins shut down");
    }

    /// Start periodic health checks for all running plugins
    pub async fn start_health_checks(self: &Arc<Self>) {
        // Don't start if health checks are disabled
        if self.config.health_check_interval.is_zero() {
            debug!("Health checks disabled (interval is 0)");
            return;
        }

        // Stop any existing health check task
        self.stop_health_checks().await;

        let interval = self.config.health_check_interval;
        let manager = Arc::clone(self);

        let handle = tokio::spawn(async move {
            info!("Starting plugin health checks every {:?}", interval);

            loop {
                tokio::time::sleep(interval).await;

                // Get list of plugin IDs that have active handles
                let plugin_ids: Vec<Uuid> = {
                    let plugins = manager.plugins.read().await;
                    plugins
                        .iter()
                        .filter(|(_, entry)| entry.handle.is_some() && entry.db_config.enabled)
                        .map(|(id, _)| *id)
                        .collect()
                };

                if plugin_ids.is_empty() {
                    debug!("No active plugins to health check");
                    continue;
                }

                debug!("Running health checks for {} plugins", plugin_ids.len());

                for plugin_id in plugin_ids {
                    match manager.ping(plugin_id).await {
                        Ok(()) => {
                            debug!("Plugin {} health check passed", plugin_id);
                        }
                        Err(e) => {
                            warn!("Plugin {} health check failed: {}", plugin_id, e);
                            // Failure is already recorded by ping()
                        }
                    }
                }
            }
        });

        *self.health_check_handle.write().await = Some(handle);
    }

    /// Stop periodic health checks
    pub async fn stop_health_checks(&self) {
        let mut handle = self.health_check_handle.write().await;
        if let Some(h) = handle.take() {
            h.abort();
            info!("Stopped plugin health checks");
        }
    }

    /// Record a plugin failure and check if it should be auto-disabled
    ///
    /// This uses time-windowed failure tracking instead of consecutive failure counts.
    /// A plugin is auto-disabled if it has >= threshold failures within the time window.
    ///
    /// Returns true if the plugin was auto-disabled.
    async fn record_failure_and_check_disable(
        &self,
        plugin_id: Uuid,
        error_message: &str,
        error_code: Option<&str>,
        method: Option<&str>,
    ) -> bool {
        // Record the failure in the plugin_failures table
        let failure_context = FailureContext {
            error_code: error_code.map(|s| s.to_string()),
            method: method.map(|s| s.to_string()),
            request_id: None,
            context: None,
            request_summary: None,
        };

        if let Err(e) = PluginFailuresRepository::record_failure(
            &self.db,
            plugin_id,
            error_message,
            failure_context,
            Some(self.config.failure_retention_days),
        )
        .await
        {
            warn!("Failed to record plugin failure: {}", e);
        }

        // Also update the plugins table for quick status display
        let _ = PluginsRepository::record_failure(&self.db, plugin_id, Some(error_message)).await;

        // Check if we should auto-disable using time-windowed counting
        match PluginFailuresRepository::count_failures_in_window(
            &self.db,
            plugin_id,
            self.config.failure_window_seconds,
        )
        .await
        {
            Ok(count) => {
                if count >= self.config.failure_threshold as u64 {
                    let reason = format!(
                        "Disabled after {} failures in {} seconds",
                        count, self.config.failure_window_seconds
                    );
                    let _ = PluginsRepository::auto_disable(&self.db, plugin_id, &reason).await;
                    warn!(
                        "Plugin {} auto-disabled: {} failures in window",
                        plugin_id, count
                    );
                    return true;
                }
            }
            Err(e) => {
                warn!("Failed to count plugin failures: {}", e);
            }
        }

        false
    }

    /// Create a PluginConfig from database model
    /// Resolve the scoped data directory for a plugin.
    ///
    /// Uses `PluginFileStorage` to create and return `{plugins_dir}/{plugin_name}/`.
    /// Returns `None` if plugin file storage is not configured or resolution fails.
    async fn resolve_plugin_data_dir(&self, plugin_name: &str) -> Option<String> {
        let storage = self.plugin_file_storage.as_ref()?;
        match storage.get_plugin_dir(plugin_name).await {
            Ok(path) => Some(path.to_string_lossy().to_string()),
            Err(e) => {
                warn!(
                    plugin = plugin_name,
                    error = %e,
                    "Failed to resolve plugin data directory"
                );
                None
            }
        }
    }

    async fn create_plugin_config(
        &self,
        plugin: &plugins::Model,
    ) -> Result<PluginConfig, PluginManagerError> {
        // Build process config with plugin name for logging context
        let mut process_config = PluginProcessConfig::new(&plugin.command);
        process_config = process_config
            .plugin_name(&plugin.name)
            .args(plugin.args_vec());

        // Add environment variables from config
        for (key, value) in plugin.env_vec() {
            process_config = process_config.env(&key, &value);
        }

        if let Some(wd) = &plugin.working_directory {
            process_config = process_config.working_directory(wd);
        }

        // Handle credentials based on delivery method
        // We use SecretValue to prevent credential exposure in logs
        let mut credentials: Option<SecretValue> = None;

        if plugin.has_credentials() {
            let decrypted = PluginsRepository::get_credentials(&self.db, plugin.id)
                .await?
                .ok_or_else(|| {
                    PluginManagerError::Encryption("Failed to decrypt credentials".to_string())
                })?;

            match plugin.credential_delivery.as_str() {
                "env" | "both" => {
                    // Add credentials as environment variables
                    if let Some(obj) = decrypted.as_object() {
                        for (key, value) in obj {
                            if let Some(v) = value.as_str() {
                                process_config = process_config.env(key.to_uppercase(), v);
                            }
                        }
                    }
                }
                _ => {}
            }

            match plugin.credential_delivery.as_str() {
                "init_message" | "both" => {
                    // Wrap in SecretValue to prevent logging
                    credentials = Some(SecretValue::new(decrypted));
                }
                _ => {}
            }
        }

        let data_dir = self.resolve_plugin_data_dir(&plugin.name).await;

        Ok(PluginConfig {
            process: process_config,
            request_timeout: self.config.default_request_timeout,
            shutdown_timeout: self.config.default_shutdown_timeout,
            max_failures: self.config.failure_threshold,
            admin_config: Some(plugin.config.clone()),
            user_config: None,
            credentials,
            data_dir,
        })
    }

    /// Convert a PluginError to an error code for metrics
    fn error_to_code(&self, error: &PluginError) -> &'static str {
        match error {
            PluginError::Process(_) => "PROCESS_ERROR",
            PluginError::Rpc(_) => "RPC_ERROR",
            PluginError::NotInitialized => "NOT_INITIALIZED",
            PluginError::Disabled { .. } => "DISABLED",
            PluginError::SpawnFailed(_) => "SPAWN_FAILED",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_manager_config_default() {
        let config = PluginManagerConfig::default();
        assert_eq!(config.default_request_timeout, Duration::from_secs(30));
        assert_eq!(config.default_shutdown_timeout, Duration::from_secs(5));
        assert_eq!(config.failure_window_seconds, 3600); // 1 hour
        assert_eq!(config.failure_threshold, 3);
        assert_eq!(config.failure_retention_days, 30);
        assert!(config.auto_sync_health);
        assert_eq!(config.health_check_interval, Duration::from_secs(60));
        assert_eq!(config.cache_ttl, Duration::from_secs(30));
    }

    #[test]
    fn test_token_bucket_rate_limiter_basic() {
        // 60 requests per minute = 1 per second
        let limiter = TokenBucketRateLimiter::new(60);

        // Should start with full capacity
        assert_eq!(limiter.available_tokens(), 60);
        assert_eq!(limiter.capacity(), 60);

        // Should be able to acquire tokens
        assert!(limiter.try_acquire());
        assert_eq!(limiter.available_tokens(), 59);

        // Consume more tokens
        for _ in 0..59 {
            assert!(limiter.try_acquire());
        }

        // Should now be rate limited
        assert_eq!(limiter.available_tokens(), 0);
        assert!(!limiter.try_acquire());
    }

    #[test]
    fn test_token_bucket_rate_limiter_refill() {
        // 600 requests per minute = 10 per second for faster testing
        let limiter = TokenBucketRateLimiter::new(600);

        // Consume all tokens
        for _ in 0..600 {
            assert!(limiter.try_acquire());
        }

        // Should be rate limited
        assert!(!limiter.try_acquire());

        // Wait 100ms - should refill 1 token (600/60 = 10 per second, so ~1 in 100ms)
        std::thread::sleep(std::time::Duration::from_millis(150));

        // Should have at least 1 token now
        assert!(limiter.try_acquire());
    }

    #[test]
    fn test_token_bucket_rate_limiter_max_capacity() {
        let limiter = TokenBucketRateLimiter::new(10);

        // Full capacity
        assert_eq!(limiter.available_tokens(), 10);

        // Use 5 tokens
        for _ in 0..5 {
            limiter.try_acquire();
        }
        assert_eq!(limiter.available_tokens(), 5);

        // Wait for refill (longer than needed to fully refill)
        std::thread::sleep(std::time::Duration::from_millis(700));

        // Tokens should be capped at capacity
        assert!(limiter.available_tokens() <= 10);
    }

    #[test]
    fn test_token_bucket_concurrent_access() {
        use std::sync::Arc;
        use std::thread;

        let limiter = Arc::new(TokenBucketRateLimiter::new(100));
        let mut handles = vec![];

        // Spawn 10 threads, each trying to acquire 15 tokens
        for _ in 0..10 {
            let limiter = Arc::clone(&limiter);
            handles.push(thread::spawn(move || {
                let mut acquired = 0;
                for _ in 0..15 {
                    if limiter.try_acquire() {
                        acquired += 1;
                    }
                }
                acquired
            }));
        }

        let total_acquired: usize = handles.into_iter().map(|h| h.join().unwrap()).sum();

        // Total acquired should be exactly 100 (the capacity)
        assert_eq!(total_acquired, 100);
    }

    #[test]
    fn test_is_cache_stale() {
        use std::sync::Arc;

        // Create a manager with a short TTL for testing
        let db = Arc::new(sea_orm::DatabaseConnection::Disconnected);
        let config = PluginManagerConfig {
            cache_ttl: Duration::from_millis(100),
            ..Default::default()
        };
        let manager = PluginManager::new(db, config);

        // No loaded_at means stale
        assert!(manager.is_cache_stale(None));

        // Just loaded means fresh
        assert!(!manager.is_cache_stale(Some(Instant::now())));

        // Old timestamp means stale
        let old = Instant::now() - Duration::from_millis(200);
        assert!(manager.is_cache_stale(Some(old)));
    }

    #[test]
    fn test_rate_limiter_disabled_with_zero() {
        use chrono::Utc;

        // Create a plugin model with rate_limit = 0 (disabled)
        let plugin = plugins::Model {
            id: Uuid::new_v4(),
            name: "test".to_string(),
            display_name: "Test".to_string(),
            description: None,
            plugin_type: "system".to_string(),
            command: "node".to_string(),
            args: serde_json::json!([]),
            env: serde_json::json!({}),
            working_directory: None,
            permissions: serde_json::json!([]),
            scopes: serde_json::json!([]),
            library_ids: serde_json::json!([]),
            credentials: None,
            credential_delivery: "env".to_string(),
            config: serde_json::json!({}),
            manifest: None,
            enabled: true,
            health_status: "healthy".to_string(),
            failure_count: 0,
            last_failure_at: None,
            last_success_at: None,
            disabled_reason: None,
            rate_limit_requests_per_minute: Some(0), // 0 = disabled
            search_query_template: None,
            search_preprocessing_rules: None,
            auto_match_conditions: None,
            use_existing_external_id: true,
            metadata_targets: None,
            internal_config: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            created_by: None,
            updated_by: None,
        };

        let entry = PluginEntry::new(plugin);
        assert!(
            entry.rate_limiter.is_none(),
            "Rate limiter should be None when rate_limit is 0"
        );
    }

    #[test]
    fn test_rate_limiter_disabled_with_none() {
        use chrono::Utc;

        // Create a plugin model with rate_limit = None (disabled)
        let plugin = plugins::Model {
            id: Uuid::new_v4(),
            name: "test".to_string(),
            display_name: "Test".to_string(),
            description: None,
            plugin_type: "system".to_string(),
            command: "node".to_string(),
            args: serde_json::json!([]),
            env: serde_json::json!({}),
            working_directory: None,
            permissions: serde_json::json!([]),
            scopes: serde_json::json!([]),
            library_ids: serde_json::json!([]),
            credentials: None,
            credential_delivery: "env".to_string(),
            config: serde_json::json!({}),
            manifest: None,
            enabled: true,
            health_status: "healthy".to_string(),
            failure_count: 0,
            last_failure_at: None,
            last_success_at: None,
            disabled_reason: None,
            rate_limit_requests_per_minute: None, // None = disabled
            search_query_template: None,
            search_preprocessing_rules: None,
            auto_match_conditions: None,
            use_existing_external_id: true,
            metadata_targets: None,
            internal_config: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            created_by: None,
            updated_by: None,
        };

        let entry = PluginEntry::new(plugin);
        assert!(
            entry.rate_limiter.is_none(),
            "Rate limiter should be None when rate_limit is None"
        );
    }

    #[test]
    fn test_rate_limiter_enabled_with_positive_value() {
        use chrono::Utc;

        // Create a plugin model with rate_limit = 60 (enabled)
        let plugin = plugins::Model {
            id: Uuid::new_v4(),
            name: "test".to_string(),
            display_name: "Test".to_string(),
            description: None,
            plugin_type: "system".to_string(),
            command: "node".to_string(),
            args: serde_json::json!([]),
            env: serde_json::json!({}),
            working_directory: None,
            permissions: serde_json::json!([]),
            scopes: serde_json::json!([]),
            library_ids: serde_json::json!([]),
            credentials: None,
            credential_delivery: "env".to_string(),
            config: serde_json::json!({}),
            manifest: None,
            enabled: true,
            health_status: "healthy".to_string(),
            failure_count: 0,
            last_failure_at: None,
            last_success_at: None,
            disabled_reason: None,
            rate_limit_requests_per_minute: Some(60), // 60 = enabled
            search_query_template: None,
            search_preprocessing_rules: None,
            auto_match_conditions: None,
            use_existing_external_id: true,
            metadata_targets: None,
            internal_config: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            created_by: None,
            updated_by: None,
        };

        let entry = PluginEntry::new(plugin);
        assert!(
            entry.rate_limiter.is_some(),
            "Rate limiter should be Some when rate_limit is positive"
        );
        assert_eq!(entry.rate_limiter.as_ref().unwrap().capacity(), 60);
    }

    fn create_test_plugin(
        config: serde_json::Value,
        manifest: Option<serde_json::Value>,
    ) -> plugins::Model {
        use chrono::Utc;

        plugins::Model {
            id: Uuid::new_v4(),
            name: "test-plugin".to_string(),
            display_name: "Test Plugin".to_string(),
            description: None,
            plugin_type: "user".to_string(),
            command: "node".to_string(),
            args: serde_json::json!([]),
            env: serde_json::json!({}),
            working_directory: None,
            permissions: serde_json::json!([]),
            scopes: serde_json::json!([]),
            library_ids: serde_json::json!([]),
            credentials: None,
            credential_delivery: "init_message".to_string(),
            config,
            manifest,
            enabled: true,
            health_status: "healthy".to_string(),
            failure_count: 0,
            last_failure_at: None,
            last_success_at: None,
            disabled_reason: None,
            rate_limit_requests_per_minute: None,
            search_query_template: None,
            search_preprocessing_rules: None,
            auto_match_conditions: None,
            use_existing_external_id: true,
            metadata_targets: None,
            internal_config: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            created_by: None,
            updated_by: None,
        }
    }

    fn create_manifest_with_oauth(client_id: Option<&str>) -> serde_json::Value {
        serde_json::json!({
            "name": "test-plugin",
            "displayName": "Test Plugin",
            "version": "1.0.0",
            "description": "Test",
            "protocolVersion": "1.0",
            "capabilities": {},
            "oauth": {
                "authorizationUrl": "https://example.com/auth",
                "tokenUrl": "https://example.com/token",
                "clientId": client_id,
                "scopes": ["read"]
            }
        })
    }

    #[test]
    fn test_get_oauth_config_from_manifest() {
        let manifest = create_manifest_with_oauth(Some("manifest-client"));
        let plugin = create_test_plugin(serde_json::json!({}), Some(manifest));

        let oauth = PluginManager::get_oauth_config(&plugin);
        assert!(oauth.is_some());

        let oauth = oauth.unwrap();
        assert_eq!(oauth.token_url, "https://example.com/token");
        assert_eq!(oauth.authorization_url, "https://example.com/auth");
        assert_eq!(oauth.client_id.as_deref(), Some("manifest-client"));
    }

    #[test]
    fn test_get_oauth_config_no_manifest() {
        let plugin = create_test_plugin(serde_json::json!({}), None);
        assert!(PluginManager::get_oauth_config(&plugin).is_none());
    }

    #[test]
    fn test_get_oauth_config_manifest_without_oauth() {
        let manifest = serde_json::json!({
            "name": "test-plugin",
            "displayName": "Test Plugin",
            "version": "1.0.0",
            "description": "Test",
            "protocolVersion": "1.0",
            "capabilities": {}
        });
        let plugin = create_test_plugin(serde_json::json!({}), Some(manifest));
        assert!(PluginManager::get_oauth_config(&plugin).is_none());
    }

    #[test]
    fn test_get_oauth_client_id_from_config_override() {
        let manifest = create_manifest_with_oauth(Some("manifest-client"));
        let plugin = create_test_plugin(
            serde_json::json!({"oauth_client_id": "config-override"}),
            Some(manifest),
        );

        let client_id = PluginManager::get_oauth_client_id(&plugin);
        assert_eq!(client_id.as_deref(), Some("config-override"));
    }

    #[test]
    fn test_get_oauth_client_id_falls_back_to_manifest() {
        let manifest = create_manifest_with_oauth(Some("manifest-client"));
        let plugin = create_test_plugin(serde_json::json!({}), Some(manifest));

        let client_id = PluginManager::get_oauth_client_id(&plugin);
        assert_eq!(client_id.as_deref(), Some("manifest-client"));
    }

    #[test]
    fn test_get_oauth_client_id_none_when_no_config_or_manifest() {
        let plugin = create_test_plugin(serde_json::json!({}), None);
        assert!(PluginManager::get_oauth_client_id(&plugin).is_none());
    }

    #[test]
    fn test_get_oauth_client_secret_from_config() {
        let plugin = create_test_plugin(
            serde_json::json!({"oauth_client_secret": "my-secret"}),
            None,
        );

        let secret = PluginManager::get_oauth_client_secret(&plugin);
        assert_eq!(secret.as_deref(), Some("my-secret"));
    }

    #[test]
    fn test_get_oauth_client_secret_none_when_missing() {
        let plugin = create_test_plugin(serde_json::json!({}), None);
        assert!(PluginManager::get_oauth_client_secret(&plugin).is_none());
    }

    // Integration tests require a database connection
    // See tests/integration/plugin_manager.rs for full tests
}
