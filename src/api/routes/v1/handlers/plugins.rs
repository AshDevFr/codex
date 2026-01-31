//! Plugins API handlers
//!
//! Provides CRUD operations for plugins that communicate with Codex via JSON-RPC over stdio.
//! Requires the `PluginsManage` permission (granted to Admins by default).

use super::super::dto::{
    available_credential_delivery_methods, available_permissions, available_scopes,
    parse_permission, parse_scope, CreatePluginRequest, EnvVarDto, PluginDto, PluginFailureDto,
    PluginFailuresResponse, PluginHealthDto, PluginHealthResponse, PluginManifestDto,
    PluginStatusResponse, PluginTestResult, PluginsListResponse, UpdatePluginRequest,
};
use crate::api::{error::ApiError, extractors::AuthContext, permissions::Permission, AppState};
use crate::db::entities::plugins::PluginPermission;
use crate::db::repositories::{PluginFailuresRepository, PluginsRepository};
use crate::services::plugin::process::{allowed_commands_description, is_command_allowed};
use crate::services::plugin::protocol::PluginScope;
use crate::services::PluginHealthStatus;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use std::sync::Arc;
use std::time::Instant;
use utoipa::OpenApi;
use uuid::Uuid;

#[derive(OpenApi)]
#[openapi(
    paths(
        list_plugins,
        create_plugin,
        get_plugin,
        update_plugin,
        delete_plugin,
        enable_plugin,
        disable_plugin,
        test_plugin,
        get_plugin_health,
        reset_plugin_failures,
        get_plugin_failures,
    ),
    components(schemas(
        PluginDto,
        PluginsListResponse,
        CreatePluginRequest,
        UpdatePluginRequest,
        PluginTestResult,
        PluginStatusResponse,
        PluginHealthDto,
        PluginHealthResponse,
        PluginManifestDto,
        PluginFailureDto,
        PluginFailuresResponse,
        EnvVarDto,
    )),
    tags(
        (name = "Plugins", description = "Admin-managed external plugin processes")
    )
)]
#[allow(dead_code)] // OpenAPI documentation struct - referenced by utoipa derive macros
pub struct PluginsApi;

/// List all plugins
#[utoipa::path(
    get,
    path = "/api/v1/admin/plugins",
    responses(
        (status = 200, description = "Plugins retrieved", body = PluginsListResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "PluginsManage permission required"),
    ),
    security(
        ("bearer_auth" = []),
        ("api_key" = [])
    ),
    tag = "Plugins"
)]
pub async fn list_plugins(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
) -> Result<Json<PluginsListResponse>, ApiError> {
    auth.require_permission(&Permission::PluginsManage)?;

    let plugins = PluginsRepository::get_all(&state.db)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to get plugins: {}", e)))?;

    let total = plugins.len();
    let dtos: Vec<PluginDto> = plugins.into_iter().map(Into::into).collect();

    Ok(Json(PluginsListResponse {
        plugins: dtos,
        total,
    }))
}

/// Create a new plugin
///
/// Creates a new plugin configuration. If the plugin is created with `enabled: true`,
/// an automatic health check is performed to verify connectivity.
#[utoipa::path(
    post,
    path = "/api/v1/admin/plugins",
    request_body = CreatePluginRequest,
    responses(
        (status = 201, description = "Plugin created", body = PluginStatusResponse),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "PluginsManage permission required"),
        (status = 409, description = "Plugin with this name already exists"),
    ),
    security(
        ("bearer_auth" = []),
        ("api_key" = [])
    ),
    tag = "Plugins"
)]
pub async fn create_plugin(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    Json(request): Json<CreatePluginRequest>,
) -> Result<(StatusCode, Json<PluginStatusResponse>), ApiError> {
    auth.require_permission(&Permission::PluginsManage)?;

    // Validate name
    if !is_valid_plugin_name(&request.name) {
        return Err(ApiError::BadRequest(
            "Invalid plugin name. Use lowercase alphanumeric characters and hyphens only. Cannot start or end with a hyphen."
                .to_string(),
        ));
    }

    // Validate plugin type
    let valid_types = ["system", "user"];
    if !valid_types.contains(&request.plugin_type.as_str()) {
        return Err(ApiError::BadRequest(format!(
            "Invalid plugin type '{}'. Valid types: {:?}",
            request.plugin_type, valid_types
        )));
    }

    // Validate command against allowlist (security)
    if !is_command_allowed(&request.command) {
        return Err(ApiError::BadRequest(format!(
            "Command '{}' is not in the plugin allowlist. Allowed commands: {}. \
             To add custom commands, set the CODEX_PLUGIN_ALLOWED_COMMANDS environment variable.",
            request.command,
            allowed_commands_description()
        )));
    }

    // Validate credential delivery
    let valid_delivery = available_credential_delivery_methods();
    if !valid_delivery.contains(&request.credential_delivery.as_str()) {
        return Err(ApiError::BadRequest(format!(
            "Invalid credential delivery '{}'. Valid options: {:?}",
            request.credential_delivery, valid_delivery
        )));
    }

    // Validate permissions
    let valid_perms = available_permissions();
    let mut permissions: Vec<PluginPermission> = Vec::new();
    for perm_str in &request.permissions {
        if !valid_perms.contains(&perm_str.as_str()) {
            return Err(ApiError::BadRequest(format!(
                "Invalid permission '{}'. Valid permissions: {:?}",
                perm_str, valid_perms
            )));
        }
        if let Some(perm) = parse_permission(perm_str) {
            permissions.push(perm);
        }
    }

    // Validate scopes
    let valid_scopes = available_scopes();
    let mut scopes: Vec<PluginScope> = Vec::new();
    for scope_str in &request.scopes {
        if !valid_scopes.contains(&scope_str.as_str()) {
            return Err(ApiError::BadRequest(format!(
                "Invalid scope '{}'. Valid scopes: {:?}",
                scope_str, valid_scopes
            )));
        }
        if let Some(scope) = parse_scope(scope_str) {
            scopes.push(scope);
        }
    }

    // Check if name already exists
    if PluginsRepository::get_by_name(&state.db, &request.name)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to check existing: {}", e)))?
        .is_some()
    {
        return Err(ApiError::Conflict(format!(
            "Plugin with name '{}' already exists",
            request.name
        )));
    }

    // Convert env vars
    let is_enabled = request.enabled;
    let env: Vec<(String, String)> = request.env.into_iter().map(|e| (e.key, e.value)).collect();

    let plugin = PluginsRepository::create(
        &state.db,
        &request.name,
        &request.display_name,
        request.description.as_deref(),
        &request.plugin_type,
        &request.command,
        request.args,
        env,
        request.working_directory.as_deref(),
        permissions,
        scopes,
        request.library_ids, // Library filtering: empty = all libraries
        request.credentials.as_ref(),
        &request.credential_delivery,
        request.config,
        is_enabled,
        Some(auth.user_id),
        request.rate_limit_requests_per_minute,
    )
    .await
    .map_err(|e| ApiError::Internal(format!("Failed to create plugin: {}", e)))?;

    let plugin_id = plugin.id;

    // Reload the plugin manager to pick up the new plugin
    if let Err(e) = state.plugin_manager.reload(plugin_id).await {
        tracing::warn!("Failed to reload plugin manager after create: {}", e);
    }

    // Perform automatic health check if plugin is enabled
    if is_enabled {
        let start = Instant::now();
        let health_result = state.plugin_manager.ping(plugin_id).await;
        let latency = start.elapsed().as_millis() as u64;

        // Re-fetch the plugin to get updated health status after ping
        let updated_plugin = PluginsRepository::get_by_id(&state.db, plugin_id)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to get updated plugin: {}", e)))?
            .ok_or_else(|| ApiError::NotFound("Plugin not found".to_string()))?;

        let (health_check_passed, health_check_error, message) = match health_result {
            Ok(()) => (
                Some(true),
                None,
                "Plugin created and health check passed".to_string(),
            ),
            Err(e) => (
                Some(false),
                Some(e.to_string()),
                format!("Plugin created but health check failed: {}", e),
            ),
        };

        return Ok((
            StatusCode::CREATED,
            Json(PluginStatusResponse {
                plugin: updated_plugin.into(),
                message,
                health_check_performed: true,
                health_check_passed,
                health_check_latency_ms: Some(latency),
                health_check_error,
            }),
        ));
    }

    // Plugin created disabled - no health check
    Ok((
        StatusCode::CREATED,
        Json(PluginStatusResponse {
            plugin: plugin.into(),
            message: "Plugin created (disabled)".to_string(),
            health_check_performed: false,
            health_check_passed: None,
            health_check_latency_ms: None,
            health_check_error: None,
        }),
    ))
}

/// Get a plugin by ID
#[utoipa::path(
    get,
    path = "/api/v1/admin/plugins/{id}",
    params(
        ("id" = Uuid, Path, description = "Plugin ID")
    ),
    responses(
        (status = 200, description = "Plugin retrieved", body = PluginDto),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "PluginsManage permission required"),
        (status = 404, description = "Plugin not found"),
    ),
    security(
        ("bearer_auth" = []),
        ("api_key" = [])
    ),
    tag = "Plugins"
)]
pub async fn get_plugin(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    Path(id): Path<Uuid>,
) -> Result<Json<PluginDto>, ApiError> {
    auth.require_permission(&Permission::PluginsManage)?;

    let plugin = PluginsRepository::get_by_id(&state.db, id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to get plugin: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Plugin not found".to_string()))?;

    Ok(Json(plugin.into()))
}

/// Update a plugin
#[utoipa::path(
    patch,
    path = "/api/v1/admin/plugins/{id}",
    params(
        ("id" = Uuid, Path, description = "Plugin ID")
    ),
    request_body = UpdatePluginRequest,
    responses(
        (status = 200, description = "Plugin updated", body = PluginDto),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "PluginsManage permission required"),
        (status = 404, description = "Plugin not found"),
    ),
    security(
        ("bearer_auth" = []),
        ("api_key" = [])
    ),
    tag = "Plugins"
)]
pub async fn update_plugin(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    Path(id): Path<Uuid>,
    Json(request): Json<UpdatePluginRequest>,
) -> Result<Json<PluginDto>, ApiError> {
    auth.require_permission(&Permission::PluginsManage)?;

    // Validate command against allowlist if provided (security)
    if let Some(ref command) = request.command {
        if !is_command_allowed(command) {
            return Err(ApiError::BadRequest(format!(
                "Command '{}' is not in the plugin allowlist. Allowed commands: {}. \
                 To add custom commands, set the CODEX_PLUGIN_ALLOWED_COMMANDS environment variable.",
                command,
                allowed_commands_description()
            )));
        }
    }

    // Validate credential delivery if provided
    if let Some(ref delivery) = request.credential_delivery {
        let valid_delivery = available_credential_delivery_methods();
        if !valid_delivery.contains(&delivery.as_str()) {
            return Err(ApiError::BadRequest(format!(
                "Invalid credential delivery '{}'. Valid options: {:?}",
                delivery, valid_delivery
            )));
        }
    }

    // Validate permissions if provided
    let permissions: Option<Vec<PluginPermission>> =
        if let Some(ref perm_strs) = request.permissions {
            let valid_perms = available_permissions();
            let mut perms = Vec::new();
            for perm_str in perm_strs {
                if !valid_perms.contains(&perm_str.as_str()) {
                    return Err(ApiError::BadRequest(format!(
                        "Invalid permission '{}'. Valid permissions: {:?}",
                        perm_str, valid_perms
                    )));
                }
                if let Some(perm) = parse_permission(perm_str) {
                    perms.push(perm);
                }
            }
            Some(perms)
        } else {
            None
        };

    // Validate scopes if provided
    let scopes: Option<Vec<PluginScope>> = if let Some(ref scope_strs) = request.scopes {
        let valid_scopes = available_scopes();
        let mut parsed_scopes = Vec::new();
        for scope_str in scope_strs {
            if !valid_scopes.contains(&scope_str.as_str()) {
                return Err(ApiError::BadRequest(format!(
                    "Invalid scope '{}'. Valid scopes: {:?}",
                    scope_str, valid_scopes
                )));
            }
            if let Some(scope) = parse_scope(scope_str) {
                parsed_scopes.push(scope);
            }
        }
        Some(parsed_scopes)
    } else {
        None
    };

    // Convert env vars if provided
    let env: Option<Vec<(String, String)>> = request
        .env
        .map(|e| e.into_iter().map(|ev| (ev.key, ev.value)).collect());

    // Update the plugin
    let plugin = PluginsRepository::update(
        &state.db,
        id,
        request.display_name,
        request.description,
        request.command,
        request.args,
        env,
        request.working_directory,
        permissions,
        scopes,
        request.library_ids, // Library filtering: None = no change, Some([]) = all, Some([ids]) = specific
        request.credential_delivery,
        request.config,
        Some(auth.user_id),
        request.rate_limit_requests_per_minute,
    )
    .await
    .map_err(|e| {
        if e.to_string().contains("not found") {
            ApiError::NotFound("Plugin not found".to_string())
        } else {
            ApiError::Internal(format!("Failed to update plugin: {}", e))
        }
    })?;

    // Update credentials separately if provided
    if request.credentials.is_some() {
        PluginsRepository::update_credentials(
            &state.db,
            id,
            request.credentials.as_ref(),
            Some(auth.user_id),
        )
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to update credentials: {}", e)))?;

        // Reload the plugin manager to pick up the updated plugin
        if let Err(e) = state.plugin_manager.reload(id).await {
            tracing::warn!("Failed to reload plugin manager after update: {}", e);
        }

        // Re-fetch to get updated plugin
        let updated = PluginsRepository::get_by_id(&state.db, id)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to get updated plugin: {}", e)))?
            .ok_or_else(|| ApiError::NotFound("Plugin not found".to_string()))?;
        return Ok(Json(updated.into()));
    }

    // Reload the plugin manager to pick up the updated plugin
    if let Err(e) = state.plugin_manager.reload(id).await {
        tracing::warn!("Failed to reload plugin manager after update: {}", e);
    }

    Ok(Json(plugin.into()))
}

/// Delete a plugin
#[utoipa::path(
    delete,
    path = "/api/v1/admin/plugins/{id}",
    params(
        ("id" = Uuid, Path, description = "Plugin ID")
    ),
    responses(
        (status = 204, description = "Plugin deleted"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "PluginsManage permission required"),
        (status = 404, description = "Plugin not found"),
    ),
    security(
        ("bearer_auth" = []),
        ("api_key" = [])
    ),
    tag = "Plugins"
)]
pub async fn delete_plugin(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    auth.require_permission(&Permission::PluginsManage)?;

    // Stop the plugin process if running (via PluginManager)
    if let Err(e) = state.plugin_manager.stop_plugin(id).await {
        tracing::warn!("Failed to stop plugin before delete: {}", e);
    }

    let deleted = PluginsRepository::delete(&state.db, id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to delete plugin: {}", e)))?;

    if deleted {
        // Remove the plugin from the manager's memory
        state.plugin_manager.remove(id).await;
        // Remove the plugin's metrics
        state.plugin_metrics_service.remove_plugin(id).await;
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::NotFound("Plugin not found".to_string()))
    }
}

/// Enable a plugin
///
/// Enables the plugin and automatically performs a health check to verify connectivity.
#[utoipa::path(
    post,
    path = "/api/v1/admin/plugins/{id}/enable",
    params(
        ("id" = Uuid, Path, description = "Plugin ID")
    ),
    responses(
        (status = 200, description = "Plugin enabled", body = PluginStatusResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "PluginsManage permission required"),
        (status = 404, description = "Plugin not found"),
    ),
    security(
        ("bearer_auth" = []),
        ("api_key" = [])
    ),
    tag = "Plugins"
)]
pub async fn enable_plugin(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    Path(id): Path<Uuid>,
) -> Result<Json<PluginStatusResponse>, ApiError> {
    auth.require_permission(&Permission::PluginsManage)?;

    let _plugin = PluginsRepository::enable(&state.db, id, Some(auth.user_id))
        .await
        .map_err(|e| {
            if e.to_string().contains("not found") {
                ApiError::NotFound("Plugin not found".to_string())
            } else {
                ApiError::Internal(format!("Failed to enable plugin: {}", e))
            }
        })?;

    // Reload the plugin manager to pick up the enabled plugin
    if let Err(e) = state.plugin_manager.reload(id).await {
        tracing::warn!("Failed to reload plugin manager after enable: {}", e);
    }

    // Perform automatic health check
    let start = Instant::now();
    let health_result = state.plugin_manager.ping(id).await;
    let latency = start.elapsed().as_millis() as u64;

    // Re-fetch the plugin to get updated health status after ping
    let updated_plugin = PluginsRepository::get_by_id(&state.db, id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to get updated plugin: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Plugin not found".to_string()))?;

    let (health_check_passed, health_check_error, message) = match health_result {
        Ok(()) => (
            Some(true),
            None,
            "Plugin enabled and health check passed".to_string(),
        ),
        Err(e) => (
            Some(false),
            Some(e.to_string()),
            format!("Plugin enabled but health check failed: {}", e),
        ),
    };

    Ok(Json(PluginStatusResponse {
        plugin: updated_plugin.into(),
        message,
        health_check_performed: true,
        health_check_passed,
        health_check_latency_ms: Some(latency),
        health_check_error,
    }))
}

/// Disable a plugin
#[utoipa::path(
    post,
    path = "/api/v1/admin/plugins/{id}/disable",
    params(
        ("id" = Uuid, Path, description = "Plugin ID")
    ),
    responses(
        (status = 200, description = "Plugin disabled", body = PluginStatusResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "PluginsManage permission required"),
        (status = 404, description = "Plugin not found"),
    ),
    security(
        ("bearer_auth" = []),
        ("api_key" = [])
    ),
    tag = "Plugins"
)]
pub async fn disable_plugin(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    Path(id): Path<Uuid>,
) -> Result<Json<PluginStatusResponse>, ApiError> {
    auth.require_permission(&Permission::PluginsManage)?;

    // Stop the plugin process if running (via PluginManager)
    if let Err(e) = state.plugin_manager.stop_plugin(id).await {
        tracing::warn!("Failed to stop plugin before disable: {}", e);
    }

    let plugin = PluginsRepository::disable(&state.db, id, Some(auth.user_id))
        .await
        .map_err(|e| {
            if e.to_string().contains("not found") {
                ApiError::NotFound("Plugin not found".to_string())
            } else {
                ApiError::Internal(format!("Failed to disable plugin: {}", e))
            }
        })?;

    // Reload the plugin manager to remove the disabled plugin from memory
    if let Err(e) = state.plugin_manager.reload(id).await {
        tracing::warn!("Failed to reload plugin manager after disable: {}", e);
    }

    // Mark plugin as unhealthy in metrics
    state
        .plugin_metrics_service
        .set_health_status(id, PluginHealthStatus::Unhealthy)
        .await;

    Ok(Json(PluginStatusResponse {
        plugin: plugin.into(),
        message: "Plugin disabled successfully".to_string(),
        health_check_performed: false,
        health_check_passed: None,
        health_check_latency_ms: None,
        health_check_error: None,
    }))
}

/// Test a plugin connection
///
/// Spawns the plugin process, sends an initialize request, and returns the manifest.
#[utoipa::path(
    post,
    path = "/api/v1/admin/plugins/{id}/test",
    params(
        ("id" = Uuid, Path, description = "Plugin ID")
    ),
    responses(
        (status = 200, description = "Test completed", body = PluginTestResult),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "PluginsManage permission required"),
        (status = 404, description = "Plugin not found"),
    ),
    security(
        ("bearer_auth" = []),
        ("api_key" = [])
    ),
    tag = "Plugins"
)]
pub async fn test_plugin(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    Path(id): Path<Uuid>,
) -> Result<Json<PluginTestResult>, ApiError> {
    auth.require_permission(&Permission::PluginsManage)?;

    let plugin = PluginsRepository::get_by_id(&state.db, id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to get plugin: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Plugin not found".to_string()))?;

    // Get the plugin manager and test the connection
    let start = Instant::now();

    // Try to spawn and initialize the plugin
    match state.plugin_manager.test_plugin(&state.db, &plugin).await {
        Ok(manifest) => {
            let latency = start.elapsed().as_millis() as u64;

            // Update the cached manifest
            if let Err(e) = PluginsRepository::update_manifest(
                &state.db,
                id,
                Some(serde_json::to_value(&manifest).unwrap_or_default()),
            )
            .await
            {
                tracing::warn!("Failed to cache plugin manifest: {}", e);
            }

            // Record success
            if let Err(e) = PluginsRepository::record_success(&state.db, id).await {
                tracing::warn!("Failed to record plugin success: {}", e);
            }

            Ok(Json(PluginTestResult {
                success: true,
                message: format!(
                    "Successfully connected to {} v{}",
                    manifest.display_name, manifest.version
                ),
                latency_ms: Some(latency),
                manifest: Some(PluginManifestDto::from(manifest)),
            }))
        }
        Err(e) => {
            let latency = start.elapsed().as_millis() as u64;

            // Record failure
            if let Err(record_err) =
                PluginsRepository::record_failure(&state.db, id, Some(&e.to_string())).await
            {
                tracing::warn!("Failed to record plugin failure: {}", record_err);
            }

            Ok(Json(PluginTestResult {
                success: false,
                message: format!("Failed to connect: {}", e),
                latency_ms: Some(latency),
                manifest: None,
            }))
        }
    }
}

/// Get plugin health information
#[utoipa::path(
    get,
    path = "/api/v1/admin/plugins/{id}/health",
    params(
        ("id" = Uuid, Path, description = "Plugin ID")
    ),
    responses(
        (status = 200, description = "Health information retrieved", body = PluginHealthResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "PluginsManage permission required"),
        (status = 404, description = "Plugin not found"),
    ),
    security(
        ("bearer_auth" = []),
        ("api_key" = [])
    ),
    tag = "Plugins"
)]
pub async fn get_plugin_health(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    Path(id): Path<Uuid>,
) -> Result<Json<PluginHealthResponse>, ApiError> {
    auth.require_permission(&Permission::PluginsManage)?;

    let plugin = PluginsRepository::get_by_id(&state.db, id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to get plugin: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Plugin not found".to_string()))?;

    Ok(Json(PluginHealthResponse {
        health: plugin.into(),
    }))
}

/// Reset plugin failure count
///
/// Clears the failure count and disabled reason, allowing the plugin to be used again.
#[utoipa::path(
    post,
    path = "/api/v1/admin/plugins/{id}/reset",
    params(
        ("id" = Uuid, Path, description = "Plugin ID")
    ),
    responses(
        (status = 200, description = "Failure count reset", body = PluginStatusResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "PluginsManage permission required"),
        (status = 404, description = "Plugin not found"),
    ),
    security(
        ("bearer_auth" = []),
        ("api_key" = [])
    ),
    tag = "Plugins"
)]
pub async fn reset_plugin_failures(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    Path(id): Path<Uuid>,
) -> Result<Json<PluginStatusResponse>, ApiError> {
    auth.require_permission(&Permission::PluginsManage)?;

    let plugin = PluginsRepository::reset_failure_count(&state.db, id, Some(auth.user_id))
        .await
        .map_err(|e| {
            if e.to_string().contains("not found") {
                ApiError::NotFound("Plugin not found".to_string())
            } else {
                ApiError::Internal(format!("Failed to reset failure count: {}", e))
            }
        })?;

    Ok(Json(PluginStatusResponse {
        plugin: plugin.into(),
        message: "Failure count reset successfully".to_string(),
        health_check_performed: false,
        health_check_passed: None,
        health_check_latency_ms: None,
        health_check_error: None,
    }))
}

/// Query parameters for plugin failures endpoint
#[derive(Debug, Clone, serde::Deserialize, utoipa::IntoParams)]
pub struct PluginFailuresQuery {
    /// Maximum number of failures to return
    #[serde(default = "default_failures_limit")]
    pub limit: u64,

    /// Number of failures to skip
    #[serde(default)]
    pub offset: u64,
}

fn default_failures_limit() -> u64 {
    20
}

/// Get plugin failure history
///
/// Returns failure events for a plugin, including time-window statistics.
#[utoipa::path(
    get,
    path = "/api/v1/admin/plugins/{id}/failures",
    params(
        ("id" = Uuid, Path, description = "Plugin ID"),
        PluginFailuresQuery,
    ),
    responses(
        (status = 200, description = "Failures retrieved", body = PluginFailuresResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "PluginsManage permission required"),
        (status = 404, description = "Plugin not found"),
    ),
    security(
        ("bearer_auth" = []),
        ("api_key" = [])
    ),
    tag = "Plugins"
)]
pub async fn get_plugin_failures(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    Path(id): Path<Uuid>,
    axum::extract::Query(query): axum::extract::Query<PluginFailuresQuery>,
) -> Result<Json<PluginFailuresResponse>, ApiError> {
    auth.require_permission(&Permission::PluginsManage)?;

    // Verify plugin exists
    PluginsRepository::get_by_id(&state.db, id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to get plugin: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Plugin not found".to_string()))?;

    // Get paginated failures
    let (failures, total) =
        PluginFailuresRepository::get_failures_paginated(&state.db, id, query.limit, query.offset)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to get failures: {}", e)))?;

    // Get count within time window
    // Use the default settings (can be made configurable via settings later)
    let window_seconds = 3600_i64; // 1 hour
    let threshold = 3_u32;

    let window_failures =
        PluginFailuresRepository::count_failures_in_window(&state.db, id, window_seconds)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to count failures: {}", e)))?;

    let failure_dtos: Vec<PluginFailureDto> = failures.into_iter().map(Into::into).collect();

    Ok(Json(PluginFailuresResponse {
        failures: failure_dtos,
        total,
        window_failures,
        window_seconds,
        threshold,
    }))
}

/// Validate a plugin name (lowercase alphanumeric with hyphens)
///
/// A valid plugin name:
/// - Is 1-100 characters long
/// - Contains only lowercase letters, digits, or hyphens
/// - Does not start or end with a hyphen
fn is_valid_plugin_name(name: &str) -> bool {
    if name.is_empty() || name.len() > 100 {
        return false;
    }

    name.chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        && !name.starts_with('-')
        && !name.ends_with('-')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_plugin_names() {
        // Basic names
        assert!(is_valid_plugin_name("mangabaka"));
        assert!(is_valid_plugin_name("provider123"));

        // With hyphens
        assert!(is_valid_plugin_name("my-plugin"));
        assert!(is_valid_plugin_name("anilist-api"));
        assert!(is_valid_plugin_name("my-custom-plugin"));

        // Numbers and hyphens
        assert!(is_valid_plugin_name("plugin-v2"));
        assert!(is_valid_plugin_name("api123-test"));
    }

    #[test]
    fn test_invalid_plugin_names() {
        // Empty
        assert!(!is_valid_plugin_name(""));

        // Uppercase
        assert!(!is_valid_plugin_name("MangaBaka"));
        assert!(!is_valid_plugin_name("My-Plugin"));

        // Spaces
        assert!(!is_valid_plugin_name("my plugin"));

        // Underscores (not allowed)
        assert!(!is_valid_plugin_name("my_plugin"));
        assert!(!is_valid_plugin_name("anilist_api"));

        // Starts with hyphen
        assert!(!is_valid_plugin_name("-plugin"));

        // Ends with hyphen
        assert!(!is_valid_plugin_name("plugin-"));

        // Too long
        let long_name = "a".repeat(101);
        assert!(!is_valid_plugin_name(&long_name));

        // Special characters
        assert!(!is_valid_plugin_name("plugin@name"));
        assert!(!is_valid_plugin_name("plugin.name"));
    }
}
