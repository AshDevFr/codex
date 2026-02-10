//! User Plugin Handlers
//!
//! Handlers for user plugin management and OAuth authentication flows.
//! These endpoints allow users to enable/disable plugins, connect via OAuth,
//! and manage their plugin integrations.

use super::super::dto::plugins::ConfigSchemaDto;
use super::super::dto::user_plugins::{
    AvailablePluginDto, OAuthCallbackQuery, OAuthStartResponse, SetUserCredentialsRequest,
    SyncStatusDto, SyncStatusQuery, SyncTriggerResponse, UpdateUserPluginConfigRequest,
    UserPluginCapabilitiesDto, UserPluginDto, UserPluginsListResponse,
};
use crate::api::extractors::auth::AuthContext;
use crate::api::{error::ApiError, extractors::AppState};
use crate::db::repositories::{
    PluginsRepository, TaskRepository, UserPluginDataRepository, UserPluginsRepository,
};
use crate::services::plugin::protocol::{OAuthConfig, PluginManifest, methods};
use crate::services::plugin::sync::SyncStatusResponse;
use crate::tasks::handlers::user_plugin_sync::LAST_SYNC_RESULT_KEY;
use crate::tasks::types::TaskType;
use axum::{
    Json,
    extract::{Path, Query, State},
    http::HeaderMap,
};
use std::sync::Arc;
use tracing::{debug, info, warn};
use uuid::Uuid;

/// Parse a plugin's manifest JSON into a typed PluginManifest.
/// Deserializes once and caches the result for callers that need multiple fields.
fn parse_manifest(plugin: &crate::db::entities::plugins::Model) -> Option<PluginManifest> {
    plugin
        .manifest
        .as_ref()
        .and_then(|m| serde_json::from_value(m.clone()).ok())
}

/// Helper to extract OAuth config from a plugin's stored manifest
fn get_oauth_config_from_plugin(
    plugin: &crate::db::entities::plugins::Model,
) -> Option<OAuthConfig> {
    parse_manifest(plugin).and_then(|m| m.oauth)
}

/// Helper to get the OAuth client_id for a plugin.
///
/// Priority: plugin config > manifest default
fn get_oauth_client_id(plugin: &crate::db::entities::plugins::Model) -> Option<String> {
    // Check plugin config for client_id override
    if let Some(client_id) = plugin
        .config
        .get("oauth_client_id")
        .and_then(|v| v.as_str())
    {
        return Some(client_id.to_string());
    }

    // Fall back to manifest's default client_id
    let oauth_config = get_oauth_config_from_plugin(plugin)?;
    oauth_config.client_id
}

/// Helper to get OAuth client_secret from plugin config
fn get_oauth_client_secret(plugin: &crate::db::entities::plugins::Model) -> Option<String> {
    plugin
        .config
        .get("oauth_client_secret")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

/// Resolve the external base URL for OAuth redirect URIs.
///
/// Priority:
/// 1. `redirect_uri_base` from OIDC config (explicit config)
/// 2. `Origin` header from the request (reflects the user's browser URL)
/// 3. Fallback to `http://localhost:3000`
fn resolve_oauth_redirect_base(state: &AppState, headers: &HeaderMap) -> String {
    // 1. Explicit config takes priority
    if let Some(ref base) = state.auth_config.oidc.redirect_uri_base {
        return base.clone();
    }

    // 2. Use Origin header from the request (browser's URL)
    if let Some(origin) = headers
        .get("origin")
        .and_then(|v| v.to_str().ok())
        .filter(|s| !s.is_empty())
    {
        return origin.trim_end_matches('/').to_string();
    }

    // 3. Fallback
    "http://localhost:3000".to_string()
}

/// Build a UserPluginDto from a user plugin instance and its parent plugin definition.
///
/// If `prefetched_sync_result` is `Some`, uses that value directly.
/// If `None`, fetches the last sync result from the database (1 query).
async fn build_user_plugin_dto(
    db: &sea_orm::DatabaseConnection,
    instance: &crate::db::entities::user_plugins::Model,
    plugin: &crate::db::entities::plugins::Model,
    prefetched_sync_result: Option<Option<serde_json::Value>>,
) -> UserPluginDto {
    let manifest = parse_manifest(plugin);
    let oauth_config = manifest.as_ref().and_then(|m| m.oauth.clone());

    let capabilities = UserPluginCapabilitiesDto {
        read_sync: manifest
            .as_ref()
            .map(|m| m.capabilities.user_read_sync)
            .unwrap_or(false),
        user_recommendation_provider: manifest
            .as_ref()
            .map(|m| m.capabilities.user_recommendation_provider)
            .unwrap_or(false),
    };

    let user_config_schema = manifest
        .as_ref()
        .and_then(|m| m.user_config_schema.clone())
        .and_then(|v| serde_json::from_value::<ConfigSchemaDto>(v).ok());

    // Use pre-fetched value or fetch from DB
    let last_sync_result = match prefetched_sync_result {
        Some(value) => value,
        None => UserPluginDataRepository::get(db, instance.id, LAST_SYNC_RESULT_KEY)
            .await
            .ok()
            .flatten()
            .map(|entry| entry.data),
    };

    UserPluginDto {
        id: instance.id,
        plugin_id: plugin.id,
        plugin_name: plugin.name.clone(),
        plugin_display_name: plugin.display_name.clone(),
        plugin_type: plugin.plugin_type.clone(),
        enabled: instance.enabled,
        connected: instance.is_authenticated(),
        health_status: instance.health_status.clone(),
        external_username: instance.external_username.clone(),
        external_avatar_url: instance.external_avatar_url.clone(),
        last_sync_at: instance.last_sync_at,
        last_success_at: instance.last_success_at,
        requires_oauth: oauth_config.is_some(),
        oauth_configured: get_oauth_client_id(plugin).is_some(),
        description: manifest.as_ref().and_then(|m| m.user_description.clone()),
        user_setup_instructions: manifest.and_then(|m| m.user_setup_instructions),
        config: instance.config.clone(),
        capabilities,
        user_config_schema,
        last_sync_result,
        created_at: instance.created_at,
    }
}

/// List user's plugins (enabled and available)
///
/// Returns both plugins the user has enabled and plugins available for them to enable.
#[utoipa::path(
    get,
    path = "/api/v1/user/plugins",
    responses(
        (status = 200, description = "User plugins list", body = UserPluginsListResponse),
        (status = 401, description = "Not authenticated"),
    ),
    tag = "User Plugins"
)]
pub async fn list_user_plugins(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
) -> Result<Json<UserPluginsListResponse>, ApiError> {
    // Get user's plugin instances
    let user_instances = UserPluginsRepository::get_all_for_user(&state.db, auth.user_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to get user plugins: {}", e)))?;

    // Get all user-type plugins that are enabled by admin
    let all_plugins = PluginsRepository::get_all(&state.db)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to get plugins: {}", e)))?;

    let user_plugins: Vec<_> = all_plugins
        .iter()
        .filter(|p| p.plugin_type == "user" && p.enabled)
        .collect();

    // Batch-fetch all last_sync_result entries (1 query instead of N)
    let user_plugin_ids: Vec<Uuid> = user_instances.iter().map(|i| i.id).collect();
    let sync_results_map = UserPluginDataRepository::get_by_key_for_user_plugin_ids(
        &state.db,
        &user_plugin_ids,
        LAST_SYNC_RESULT_KEY,
    )
    .await
    .unwrap_or_default();

    // Build enabled plugins list using pre-fetched sync results
    let mut enabled: Vec<UserPluginDto> = Vec::new();
    for instance in &user_instances {
        if let Some(plugin) = user_plugins.iter().find(|p| p.id == instance.plugin_id) {
            let prefetched = sync_results_map
                .get(&instance.id)
                .map(|entry| entry.data.clone());
            enabled
                .push(build_user_plugin_dto(&state.db, instance, plugin, Some(prefetched)).await);
        }
    }

    // Build available plugins (not yet enabled by user)
    let enabled_plugin_ids: std::collections::HashSet<_> =
        user_instances.iter().map(|i| i.plugin_id).collect();

    let available: Vec<AvailablePluginDto> = user_plugins
        .iter()
        .filter(|p| !enabled_plugin_ids.contains(&p.id))
        .map(|plugin| {
            let manifest = parse_manifest(plugin);
            let oauth_config = manifest.as_ref().and_then(|m| m.oauth.clone());

            AvailablePluginDto {
                plugin_id: plugin.id,
                name: plugin.name.clone(),
                display_name: plugin.display_name.clone(),
                description: manifest
                    .as_ref()
                    .and_then(|m| m.user_description.clone())
                    .or_else(|| manifest.as_ref().and_then(|m| m.description.clone())),
                user_setup_instructions: manifest
                    .as_ref()
                    .and_then(|m| m.user_setup_instructions.clone()),
                requires_oauth: oauth_config.is_some(),
                oauth_configured: get_oauth_client_id(plugin).is_some(),
                capabilities: UserPluginCapabilitiesDto {
                    read_sync: manifest
                        .as_ref()
                        .map(|m| m.capabilities.user_read_sync)
                        .unwrap_or(false),
                    user_recommendation_provider: manifest
                        .as_ref()
                        .map(|m| m.capabilities.user_recommendation_provider)
                        .unwrap_or(false),
                },
            }
        })
        .collect();

    Ok(Json(UserPluginsListResponse { enabled, available }))
}

/// Enable a plugin for the current user
#[utoipa::path(
    post,
    path = "/api/v1/user/plugins/{plugin_id}/enable",
    operation_id = "enable_user_plugin",
    params(
        ("plugin_id" = Uuid, Path, description = "Plugin ID to enable")
    ),
    responses(
        (status = 200, description = "Plugin enabled", body = UserPluginDto),
        (status = 400, description = "Plugin is not a user plugin or not available"),
        (status = 401, description = "Not authenticated"),
        (status = 409, description = "Plugin already enabled for this user"),
    ),
    tag = "User Plugins"
)]
pub async fn enable_plugin(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    Path(plugin_id): Path<Uuid>,
) -> Result<Json<UserPluginDto>, ApiError> {
    // Verify the plugin exists and is a user plugin
    let plugin = PluginsRepository::get_by_id(&state.db, plugin_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to get plugin: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Plugin not found".to_string()))?;

    if plugin.plugin_type != "user" {
        return Err(ApiError::BadRequest(
            "Only user plugins can be enabled by users".to_string(),
        ));
    }

    if !plugin.enabled {
        return Err(ApiError::BadRequest(
            "Plugin is not available (disabled by admin)".to_string(),
        ));
    }

    // Check if already enabled
    if let Some(_existing) =
        UserPluginsRepository::get_by_user_and_plugin(&state.db, auth.user_id, plugin_id)
            .await
            .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?
    {
        return Err(ApiError::Conflict(
            "Plugin is already enabled for this user".to_string(),
        ));
    }

    // Create user plugin instance
    let instance = UserPluginsRepository::create(&state.db, plugin_id, auth.user_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to enable plugin: {}", e)))?;

    info!(
        user_id = %auth.user_id,
        plugin_id = %plugin_id,
        plugin_name = %plugin.name,
        "User enabled plugin"
    );

    Ok(Json(
        build_user_plugin_dto(&state.db, &instance, &plugin, None).await,
    ))
}

/// Disable a plugin for the current user
#[utoipa::path(
    post,
    path = "/api/v1/user/plugins/{plugin_id}/disable",
    operation_id = "disable_user_plugin",
    params(
        ("plugin_id" = Uuid, Path, description = "Plugin ID to disable")
    ),
    responses(
        (status = 200, description = "Plugin disabled"),
        (status = 401, description = "Not authenticated"),
        (status = 404, description = "Plugin not enabled for this user"),
    ),
    tag = "User Plugins"
)]
pub async fn disable_plugin(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    Path(plugin_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let instance =
        UserPluginsRepository::get_by_user_and_plugin(&state.db, auth.user_id, plugin_id)
            .await
            .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?
            .ok_or_else(|| ApiError::NotFound("Plugin not enabled for this user".to_string()))?;

    UserPluginsRepository::set_enabled(&state.db, instance.id, false)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to disable plugin: {}", e)))?;

    info!(
        user_id = %auth.user_id,
        plugin_id = %plugin_id,
        "User disabled plugin"
    );

    Ok(Json(serde_json::json!({ "success": true })))
}

/// Disconnect a plugin (remove data and credentials)
#[utoipa::path(
    delete,
    path = "/api/v1/user/plugins/{plugin_id}",
    params(
        ("plugin_id" = Uuid, Path, description = "Plugin ID to disconnect")
    ),
    responses(
        (status = 200, description = "Plugin disconnected and data removed"),
        (status = 401, description = "Not authenticated"),
        (status = 404, description = "Plugin not enabled for this user"),
    ),
    tag = "User Plugins"
)]
pub async fn disconnect_plugin(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    Path(plugin_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let instance =
        UserPluginsRepository::get_by_user_and_plugin(&state.db, auth.user_id, plugin_id)
            .await
            .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?
            .ok_or_else(|| ApiError::NotFound("Plugin not enabled for this user".to_string()))?;

    UserPluginsRepository::delete(&state.db, instance.id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to disconnect plugin: {}", e)))?;

    info!(
        user_id = %auth.user_id,
        plugin_id = %plugin_id,
        "User disconnected plugin"
    );

    Ok(Json(serde_json::json!({ "success": true })))
}

/// Start OAuth flow for a user plugin
///
/// Generates an authorization URL and returns it to the client.
/// The client should open this URL in a popup or redirect the user.
#[utoipa::path(
    post,
    path = "/api/v1/user/plugins/{plugin_id}/oauth/start",
    params(
        ("plugin_id" = Uuid, Path, description = "Plugin ID to start OAuth for")
    ),
    responses(
        (status = 200, description = "OAuth authorization URL generated", body = OAuthStartResponse),
        (status = 400, description = "Plugin does not support OAuth or not configured"),
        (status = 401, description = "Not authenticated"),
        (status = 404, description = "Plugin not found or not enabled"),
    ),
    tag = "User Plugins"
)]
pub async fn oauth_start(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    headers: HeaderMap,
    Path(plugin_id): Path<Uuid>,
) -> Result<Json<OAuthStartResponse>, ApiError> {
    // Get the plugin definition
    let plugin = PluginsRepository::get_by_id(&state.db, plugin_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to get plugin: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Plugin not found".to_string()))?;

    if plugin.plugin_type != "user" {
        return Err(ApiError::BadRequest(
            "Only user plugins support OAuth".to_string(),
        ));
    }

    // Rate-limit OAuth flow initiation: max 3 pending flows per user
    const MAX_PENDING_OAUTH_FLOWS_PER_USER: usize = 3;
    let pending = state
        .oauth_state_manager
        .pending_count_for_user(auth.user_id);
    if pending >= MAX_PENDING_OAUTH_FLOWS_PER_USER {
        return Err(ApiError::TooManyRequests(format!(
            "Too many pending OAuth flows (max {}). Please complete or wait for existing flows to expire.",
            MAX_PENDING_OAUTH_FLOWS_PER_USER
        )));
    }

    // Get OAuth config from manifest
    let oauth_config = get_oauth_config_from_plugin(&plugin).ok_or_else(|| {
        ApiError::BadRequest("Plugin does not have OAuth configuration".to_string())
    })?;

    // Get client_id (required)
    let client_id = get_oauth_client_id(&plugin).ok_or_else(|| {
        ApiError::BadRequest(
            "OAuth client_id not configured. Admin must set oauth_client_id in plugin config."
                .to_string(),
        )
    })?;

    // Ensure user has this plugin enabled (or create the instance)
    let _user_plugin =
        match UserPluginsRepository::get_by_user_and_plugin(&state.db, auth.user_id, plugin_id)
            .await
            .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?
        {
            Some(instance) => instance,
            None => {
                // Auto-enable the plugin when starting OAuth
                UserPluginsRepository::create(&state.db, plugin_id, auth.user_id)
                    .await
                    .map_err(|e| ApiError::Internal(format!("Failed to enable plugin: {}", e)))?
            }
        };

    // Build redirect URI using config, request origin, or fallback
    let base_url = resolve_oauth_redirect_base(&state, &headers);
    let redirect_uri = format!("{}/api/v1/user/plugins/oauth/callback", base_url);

    // Start the OAuth flow
    let (auth_url, _state_token) = state
        .oauth_state_manager
        .start_oauth_flow(
            plugin_id,
            auth.user_id,
            &oauth_config,
            &client_id,
            &redirect_uri,
        )
        .map_err(|e| ApiError::Internal(format!("Failed to start OAuth flow: {}", e)))?;

    debug!(
        user_id = %auth.user_id,
        plugin_id = %plugin_id,
        "Started OAuth flow for user plugin"
    );

    Ok(Json(OAuthStartResponse {
        redirect_url: auth_url,
    }))
}

/// Handle OAuth callback from external provider
///
/// This endpoint receives the callback after the user authenticates with the
/// external service. It exchanges the authorization code for tokens and stores
/// them encrypted in the database.
#[utoipa::path(
    get,
    path = "/api/v1/user/plugins/oauth/callback",
    params(
        ("code" = String, Query, description = "Authorization code from OAuth provider"),
        ("state" = String, Query, description = "State parameter for CSRF protection"),
    ),
    responses(
        (status = 200, description = "HTML page that auto-closes the popup"),
        (status = 400, description = "Invalid callback parameters"),
    ),
    tag = "User Plugins"
)]
pub async fn oauth_callback(
    State(state): State<Arc<AppState>>,
    Query(query): Query<OAuthCallbackQuery>,
) -> Result<axum::response::Html<String>, ApiError> {
    // Validate state and get pending flow info
    let pending = state
        .oauth_state_manager
        .validate_state(&query.state)
        .map_err(|e| {
            warn!(error = %e, "OAuth callback state validation failed");
            ApiError::BadRequest(format!("Invalid or expired OAuth state: {}", e))
        })?;

    let plugin_id = pending.plugin_id;
    let user_id = pending.user_id;

    debug!(
        plugin_id = %plugin_id,
        user_id = %user_id,
        "Processing OAuth callback"
    );

    // Get plugin and OAuth config
    let plugin = PluginsRepository::get_by_id(&state.db, plugin_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to get plugin: {}", e)))?
        .ok_or_else(|| ApiError::Internal("Plugin not found during callback".to_string()))?;

    let oauth_config = get_oauth_config_from_plugin(&plugin).ok_or_else(|| {
        ApiError::Internal("Plugin OAuth config missing during callback".to_string())
    })?;

    let client_id = get_oauth_client_id(&plugin)
        .ok_or_else(|| ApiError::Internal("OAuth client_id not configured".to_string()))?;

    let client_secret = get_oauth_client_secret(&plugin);

    // Use the redirect_uri from the pending flow to ensure it matches the authorization request
    let redirect_uri = pending.redirect_uri.clone();

    // Exchange code for tokens
    let oauth_result = state
        .oauth_state_manager
        .exchange_code(
            &oauth_config,
            &query.code,
            &client_id,
            client_secret.as_deref(),
            &redirect_uri,
            pending.pkce_verifier.as_deref(),
        )
        .await
        .map_err(|e| {
            warn!(error = %e, plugin_id = %plugin_id, "OAuth code exchange failed");
            ApiError::BadRequest(format!("OAuth authentication failed: {}", e))
        })?;

    // Get or create user plugin instance
    let user_plugin =
        match UserPluginsRepository::get_by_user_and_plugin(&state.db, user_id, plugin_id)
            .await
            .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?
        {
            Some(instance) => instance,
            None => UserPluginsRepository::create(&state.db, plugin_id, user_id)
                .await
                .map_err(|e| ApiError::Internal(format!("Failed to create user plugin: {}", e)))?,
        };

    // Store encrypted tokens
    UserPluginsRepository::update_oauth_tokens(
        &state.db,
        user_plugin.id,
        &oauth_result.access_token,
        oauth_result.refresh_token.as_deref(),
        oauth_result.expires_at,
        oauth_result.scope.as_deref(),
    )
    .await
    .map_err(|e| ApiError::Internal(format!("Failed to store OAuth tokens: {}", e)))?;

    // Record success
    let _ = UserPluginsRepository::record_success(&state.db, user_plugin.id).await;

    info!(
        user_id = %user_id,
        plugin_id = %plugin_id,
        plugin_name = %plugin.name,
        has_refresh_token = oauth_result.refresh_token.is_some(),
        "OAuth flow completed successfully"
    );

    // Return a minimal HTML page that closes the popup
    let html = r#"<!DOCTYPE html>
<html><head><title>Connected</title></head>
<body style="background:#1a1b1e;color:#c1c2c5;font-family:system-ui;display:flex;align-items:center;justify-content:center;height:100vh;margin:0">
<div style="text-align:center">
<p style="font-size:1.2rem">Connected successfully!</p>
<p style="color:#868e96;font-size:0.9rem">This window will close automatically...</p>
</div>
<script>window.close();</script>
</body></html>"#
        .to_string();

    Ok(axum::response::Html(html))
}

/// Get a single user plugin instance
///
/// Returns detailed status for a plugin the user has enabled.
#[utoipa::path(
    get,
    path = "/api/v1/user/plugins/{plugin_id}",
    params(
        ("plugin_id" = Uuid, Path, description = "Plugin ID")
    ),
    responses(
        (status = 200, description = "User plugin details", body = UserPluginDto),
        (status = 401, description = "Not authenticated"),
        (status = 404, description = "Plugin not enabled for this user"),
    ),
    tag = "User Plugins"
)]
pub async fn get_user_plugin(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    Path(plugin_id): Path<Uuid>,
) -> Result<Json<UserPluginDto>, ApiError> {
    let instance =
        UserPluginsRepository::get_by_user_and_plugin(&state.db, auth.user_id, plugin_id)
            .await
            .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?
            .ok_or_else(|| ApiError::NotFound("Plugin not enabled for this user".to_string()))?;

    let plugin = PluginsRepository::get_by_id(&state.db, plugin_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to get plugin: {}", e)))?
        .ok_or_else(|| ApiError::Internal("Plugin definition not found".to_string()))?;

    Ok(Json(
        build_user_plugin_dto(&state.db, &instance, &plugin, None).await,
    ))
}

/// Update user plugin configuration
///
/// Allows the user to set per-user configuration overrides for their plugin instance.
#[utoipa::path(
    patch,
    path = "/api/v1/user/plugins/{plugin_id}/config",
    params(
        ("plugin_id" = Uuid, Path, description = "Plugin ID to update config for")
    ),
    request_body = UpdateUserPluginConfigRequest,
    responses(
        (status = 200, description = "Configuration updated", body = UserPluginDto),
        (status = 400, description = "Invalid configuration"),
        (status = 401, description = "Not authenticated"),
        (status = 404, description = "Plugin not enabled for this user"),
    ),
    tag = "User Plugins"
)]
pub async fn update_user_plugin_config(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    Path(plugin_id): Path<Uuid>,
    Json(request): Json<UpdateUserPluginConfigRequest>,
) -> Result<Json<UserPluginDto>, ApiError> {
    // Validate config is a JSON object
    if !request.config.is_object() {
        return Err(ApiError::BadRequest(
            "Config must be a JSON object".to_string(),
        ));
    }

    let instance =
        UserPluginsRepository::get_by_user_and_plugin(&state.db, auth.user_id, plugin_id)
            .await
            .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?
            .ok_or_else(|| ApiError::NotFound("Plugin not enabled for this user".to_string()))?;

    let updated = UserPluginsRepository::update_config(&state.db, instance.id, request.config)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to update config: {}", e)))?;

    let plugin = PluginsRepository::get_by_id(&state.db, plugin_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to get plugin: {}", e)))?
        .ok_or_else(|| ApiError::Internal("Plugin definition not found".to_string()))?;

    debug!(
        user_id = %auth.user_id,
        plugin_id = %plugin_id,
        "User updated plugin config"
    );

    Ok(Json(
        build_user_plugin_dto(&state.db, &updated, &plugin, None).await,
    ))
}

/// Trigger a sync operation for a user plugin
///
/// Enqueues a background sync task that will push/pull reading progress
/// between Codex and the external service.
#[utoipa::path(
    post,
    path = "/api/v1/user/plugins/{plugin_id}/sync",
    params(
        ("plugin_id" = Uuid, Path, description = "Plugin ID to sync")
    ),
    responses(
        (status = 200, description = "Sync task enqueued", body = SyncTriggerResponse),
        (status = 400, description = "Plugin is not a sync provider or not connected"),
        (status = 401, description = "Not authenticated"),
        (status = 404, description = "Plugin not enabled for this user"),
        (status = 409, description = "Sync already in progress"),
    ),
    tag = "User Plugins"
)]
pub async fn trigger_sync(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    Path(plugin_id): Path<Uuid>,
) -> Result<Json<SyncTriggerResponse>, ApiError> {
    // Verify user has this plugin enabled
    let instance =
        UserPluginsRepository::get_by_user_and_plugin(&state.db, auth.user_id, plugin_id)
            .await
            .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?
            .ok_or_else(|| ApiError::NotFound("Plugin not enabled for this user".to_string()))?;

    if !instance.enabled {
        return Err(ApiError::BadRequest(
            "Plugin is disabled. Enable it before syncing.".to_string(),
        ));
    }

    // Verify the plugin is a sync provider
    let plugin = PluginsRepository::get_by_id(&state.db, plugin_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to get plugin: {}", e)))?
        .ok_or_else(|| ApiError::Internal("Plugin definition not found".to_string()))?;

    let manifest = parse_manifest(&plugin);
    let is_read_sync = manifest
        .as_ref()
        .map(|m| m.capabilities.user_read_sync)
        .unwrap_or(false);

    if !is_read_sync {
        return Err(ApiError::BadRequest(
            "Plugin does not support reading sync".to_string(),
        ));
    }

    // Verify the plugin is connected (has credentials)
    if !instance.is_authenticated() {
        return Err(ApiError::BadRequest(
            "Plugin is not connected. Complete authentication before syncing.".to_string(),
        ));
    }

    // Check for duplicate pending/processing sync task
    let has_existing = TaskRepository::has_pending_or_processing(
        &state.db,
        "user_plugin_sync",
        plugin_id,
        auth.user_id,
    )
    .await
    .map_err(|e| ApiError::Internal(format!("Failed to check existing tasks: {}", e)))?;

    if has_existing {
        return Err(ApiError::Conflict("Sync already in progress".to_string()));
    }

    // Enqueue sync task
    let task_type = TaskType::UserPluginSync {
        plugin_id,
        user_id: auth.user_id,
    };

    let task_id = TaskRepository::enqueue(&state.db, task_type, 0, None)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to enqueue sync task: {}", e)))?;

    info!(
        user_id = %auth.user_id,
        plugin_id = %plugin_id,
        task_id = %task_id,
        plugin_name = %plugin.name,
        "Enqueued user plugin sync task"
    );

    Ok(Json(SyncTriggerResponse {
        task_id,
        message: format!("Sync task enqueued for {}", plugin.display_name),
    }))
}

/// Get sync status for a user plugin
///
/// Returns the current sync status including last sync time, health, and failure count.
/// Pass `?live=true` to also query the plugin process for live sync state (pending push/pull,
/// conflicts, external entry count). This spawns the plugin process and is more expensive.
#[utoipa::path(
    get,
    path = "/api/v1/user/plugins/{plugin_id}/sync/status",
    params(
        ("plugin_id" = Uuid, Path, description = "Plugin ID to check sync status"),
        SyncStatusQuery,
    ),
    responses(
        (status = 200, description = "Sync status", body = SyncStatusDto),
        (status = 401, description = "Not authenticated"),
        (status = 404, description = "Plugin not enabled for this user"),
    ),
    tag = "User Plugins"
)]
pub async fn get_sync_status(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    Path(plugin_id): Path<Uuid>,
    Query(query): Query<SyncStatusQuery>,
) -> Result<Json<SyncStatusDto>, ApiError> {
    let instance =
        UserPluginsRepository::get_by_user_and_plugin(&state.db, auth.user_id, plugin_id)
            .await
            .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?
            .ok_or_else(|| ApiError::NotFound("Plugin not enabled for this user".to_string()))?;

    let plugin = PluginsRepository::get_by_id(&state.db, plugin_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to get plugin: {}", e)))?
        .ok_or_else(|| ApiError::Internal("Plugin definition not found".to_string()))?;

    // Optionally query live sync state from the plugin process
    let (external_count, pending_push, pending_pull, conflicts, live_error) = if query.live {
        debug!(
            user_id = %auth.user_id,
            plugin_id = %plugin_id,
            "Querying live sync status from plugin process"
        );
        match state
            .plugin_manager
            .get_user_plugin_handle(plugin_id, auth.user_id)
            .await
        {
            Ok((handle, _context)) => {
                match handle
                    .call_method::<serde_json::Value, SyncStatusResponse>(
                        methods::SYNC_STATUS,
                        serde_json::json!({}),
                    )
                    .await
                {
                    Ok(resp) => (
                        resp.external_count,
                        Some(resp.pending_push),
                        Some(resp.pending_pull),
                        Some(resp.conflicts),
                        None,
                    ),
                    Err(e) => {
                        warn!(
                            plugin_id = %plugin_id,
                            error = %e,
                            "Failed to get live sync status from plugin"
                        );
                        (
                            None,
                            None,
                            None,
                            None,
                            Some(format!("sync/status call failed: {}", e)),
                        )
                    }
                }
            }
            Err(e) => {
                warn!(
                    plugin_id = %plugin_id,
                    error = %e,
                    "Failed to spawn plugin for live sync status"
                );
                (
                    None,
                    None,
                    None,
                    None,
                    Some(format!("Plugin unavailable: {}", e)),
                )
            }
        }
    } else {
        (None, None, None, None, None)
    };

    Ok(Json(SyncStatusDto {
        plugin_id: plugin.id,
        plugin_name: plugin.display_name.clone(),
        connected: instance.is_authenticated(),
        last_sync_at: instance.last_sync_at,
        last_success_at: instance.last_success_at,
        last_failure_at: instance.last_failure_at,
        health_status: instance.health_status.clone(),
        failure_count: instance.failure_count,
        enabled: instance.enabled,
        external_count,
        pending_push,
        pending_pull,
        conflicts,
        live_error,
    }))
}

/// Set user credentials (personal access token) for a plugin
///
/// Allows users to authenticate by pasting a personal access token
/// instead of going through the OAuth flow.
#[utoipa::path(
    post,
    path = "/api/v1/user/plugins/{plugin_id}/credentials",
    params(
        ("plugin_id" = Uuid, Path, description = "Plugin ID to set credentials for")
    ),
    request_body = SetUserCredentialsRequest,
    responses(
        (status = 200, description = "Credentials stored", body = UserPluginDto),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Not authenticated"),
        (status = 404, description = "Plugin not enabled for this user"),
    ),
    tag = "User Plugins"
)]
pub async fn set_user_credentials(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    Path(plugin_id): Path<Uuid>,
    Json(request): Json<SetUserCredentialsRequest>,
) -> Result<Json<UserPluginDto>, ApiError> {
    if request.access_token.trim().is_empty() {
        return Err(ApiError::BadRequest(
            "Access token cannot be empty".to_string(),
        ));
    }

    let instance =
        UserPluginsRepository::get_by_user_and_plugin(&state.db, auth.user_id, plugin_id)
            .await
            .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?
            .ok_or_else(|| ApiError::NotFound("Plugin not enabled for this user".to_string()))?;

    // Store as credentials JSON (same format as required_credentials keys)
    let credentials = serde_json::json!({
        "access_token": request.access_token.trim()
    });

    let updated = UserPluginsRepository::update_credentials(&state.db, instance.id, &credentials)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to store credentials: {}", e)))?;

    // Record success for health tracking
    let _ = UserPluginsRepository::record_success(&state.db, updated.id).await;

    let plugin = PluginsRepository::get_by_id(&state.db, plugin_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to get plugin: {}", e)))?
        .ok_or_else(|| ApiError::Internal("Plugin definition not found".to_string()))?;

    info!(
        user_id = %auth.user_id,
        plugin_id = %plugin_id,
        plugin_name = %plugin.name,
        "User set personal access token"
    );

    // Re-fetch to get updated state after record_success
    let updated = UserPluginsRepository::get_by_id(&state.db, instance.id)
        .await
        .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?
        .ok_or_else(|| ApiError::Internal("User plugin not found after update".to_string()))?;

    Ok(Json(
        build_user_plugin_dto(&state.db, &updated, &plugin, None).await,
    ))
}
