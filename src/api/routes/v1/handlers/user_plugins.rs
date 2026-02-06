//! User Plugin Handlers
//!
//! Handlers for user plugin management and OAuth authentication flows.
//! These endpoints allow users to enable/disable plugins, connect via OAuth,
//! and manage their plugin integrations.

use super::super::dto::user_plugins::{
    AvailablePluginDto, OAuthCallbackQuery, OAuthStartResponse, UserPluginCapabilitiesDto,
    UserPluginDto, UserPluginsListResponse,
};
use crate::api::extractors::auth::AuthContext;
use crate::api::{error::ApiError, extractors::AppState};
use crate::db::repositories::{PluginsRepository, UserPluginsRepository};
use crate::services::plugin::protocol::{OAuthConfig, PluginManifest};
use axum::{
    Json,
    extract::{Path, Query, State},
};
use std::sync::Arc;
use tracing::{debug, info, warn};
use uuid::Uuid;

/// Helper to extract OAuth config from a plugin's stored manifest
fn get_oauth_config_from_plugin(
    plugin: &crate::db::entities::plugins::Model,
) -> Option<OAuthConfig> {
    let manifest_json = plugin.manifest.as_ref()?;
    let manifest: PluginManifest = serde_json::from_value(manifest_json.clone()).ok()?;
    manifest.oauth
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

    // Build enabled plugins list
    let enabled: Vec<UserPluginDto> = user_instances
        .iter()
        .filter_map(|instance| {
            let plugin = user_plugins.iter().find(|p| p.id == instance.plugin_id)?;
            let oauth_config = get_oauth_config_from_plugin(plugin);
            let manifest: Option<PluginManifest> = plugin
                .manifest
                .as_ref()
                .and_then(|m| serde_json::from_value(m.clone()).ok());

            Some(UserPluginDto {
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
                description: manifest.and_then(|m| m.user_description),
                config: instance.config.clone(),
                created_at: instance.created_at,
            })
        })
        .collect();

    // Build available plugins (not yet enabled by user)
    let enabled_plugin_ids: std::collections::HashSet<_> =
        user_instances.iter().map(|i| i.plugin_id).collect();

    let available: Vec<AvailablePluginDto> = user_plugins
        .iter()
        .filter(|p| !enabled_plugin_ids.contains(&p.id))
        .map(|plugin| {
            let manifest: Option<PluginManifest> = plugin
                .manifest
                .as_ref()
                .and_then(|m| serde_json::from_value(m.clone()).ok());
            let oauth_config = get_oauth_config_from_plugin(plugin);

            AvailablePluginDto {
                plugin_id: plugin.id,
                name: plugin.name.clone(),
                display_name: plugin.display_name.clone(),
                description: manifest
                    .as_ref()
                    .and_then(|m| m.user_description.clone())
                    .or_else(|| manifest.as_ref().and_then(|m| m.description.clone())),
                requires_oauth: oauth_config.is_some(),
                capabilities: UserPluginCapabilitiesDto {
                    sync_provider: manifest
                        .as_ref()
                        .map(|m| m.capabilities.user_sync_provider)
                        .unwrap_or(false),
                    recommendation_provider: manifest
                        .as_ref()
                        .map(|m| m.capabilities.recommendation_provider)
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

    let oauth_config = get_oauth_config_from_plugin(&plugin);
    let manifest: Option<PluginManifest> = plugin
        .manifest
        .as_ref()
        .and_then(|m| serde_json::from_value(m.clone()).ok());

    info!(
        user_id = %auth.user_id,
        plugin_id = %plugin_id,
        plugin_name = %plugin.name,
        "User enabled plugin"
    );

    Ok(Json(UserPluginDto {
        id: instance.id,
        plugin_id: plugin.id,
        plugin_name: plugin.name,
        plugin_display_name: plugin.display_name,
        plugin_type: plugin.plugin_type,
        enabled: instance.enabled,
        connected: false,
        health_status: instance.health_status,
        external_username: None,
        external_avatar_url: None,
        last_sync_at: None,
        last_success_at: None,
        requires_oauth: oauth_config.is_some(),
        description: manifest.and_then(|m| m.user_description),
        config: instance.config,
        created_at: instance.created_at,
    }))
}

/// Disable a plugin for the current user
#[utoipa::path(
    post,
    path = "/api/v1/user/plugins/{plugin_id}/disable",
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

    // Build redirect URI for the callback
    // Uses the OIDC redirect_uri_base as the server's external URL
    let base_url = state
        .auth_config
        .oidc
        .redirect_uri_base
        .as_deref()
        .unwrap_or("http://localhost:3000");
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
        (status = 302, description = "Redirect to frontend with result"),
        (status = 400, description = "Invalid callback parameters"),
    ),
    tag = "User Plugins"
)]
pub async fn oauth_callback(
    State(state): State<Arc<AppState>>,
    Query(query): Query<OAuthCallbackQuery>,
) -> Result<axum::response::Redirect, ApiError> {
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

    let base_url = state
        .auth_config
        .oidc
        .redirect_uri_base
        .as_deref()
        .unwrap_or("http://localhost:3000");
    let redirect_uri = format!("{}/api/v1/user/plugins/oauth/callback", base_url);

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

    // Redirect to frontend with success indicator
    // The frontend handles the popup close/state update
    let redirect_url = format!("/settings/integrations?oauth=success&plugin={}", plugin_id);

    Ok(axum::response::Redirect::to(&redirect_url))
}
