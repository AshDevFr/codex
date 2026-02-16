//! API handlers for plugin file storage operations
//!
//! These endpoints allow administrators to view storage metrics and clean up
//! plugin file storage directories.

use axum::{
    Json,
    extract::{Path, State},
};
use std::sync::Arc;

use super::super::dto::{AllPluginStorageStatsDto, PluginCleanupResultDto, PluginStorageStatsDto};
use crate::api::{
    error::ApiError,
    extractors::{AppState, AuthContext},
    permissions::Permission,
};
use crate::require_permission;

/// Get storage statistics for all plugins
///
/// Scans the plugins directory and returns file count and size per plugin.
///
/// # Permission Required
/// - Admin access required
#[utoipa::path(
    get,
    path = "/api/v1/admin/plugin-storage",
    responses(
        (status = 200, description = "Plugin storage statistics retrieved", body = AllPluginStorageStatsDto),
        (status = 403, description = "Admin access required"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Admin"
)]
pub async fn get_all_plugin_storage_stats(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
) -> Result<Json<AllPluginStorageStatsDto>, ApiError> {
    require_permission!(auth, Permission::SystemAdmin)?;

    let plugin_storage = state
        .plugin_file_storage
        .as_ref()
        .ok_or_else(|| ApiError::Internal("Plugin file storage not configured".to_string()))?;

    let stats = plugin_storage
        .scan_storage()
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to scan plugin storage: {}", e)))?;

    let total_file_count: u64 = stats.iter().map(|s| s.file_count).sum();
    let total_bytes: u64 = stats.iter().map(|s| s.total_bytes).sum();

    Ok(Json(AllPluginStorageStatsDto {
        plugins: stats.into_iter().map(Into::into).collect(),
        total_file_count,
        total_bytes,
    }))
}

/// Get storage statistics for a specific plugin
///
/// # Permission Required
/// - Admin access required
#[utoipa::path(
    get,
    path = "/api/v1/admin/plugin-storage/{name}",
    params(
        ("name" = String, Path, description = "Plugin name"),
    ),
    responses(
        (status = 200, description = "Plugin storage statistics retrieved", body = PluginStorageStatsDto),
        (status = 403, description = "Admin access required"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Admin"
)]
pub async fn get_plugin_storage_stats(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    Path(name): Path<String>,
) -> Result<Json<PluginStorageStatsDto>, ApiError> {
    require_permission!(auth, Permission::SystemAdmin)?;

    let plugin_storage = state
        .plugin_file_storage
        .as_ref()
        .ok_or_else(|| ApiError::Internal("Plugin file storage not configured".to_string()))?;

    let stats = plugin_storage
        .get_plugin_storage_stats(&name)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to get plugin storage stats: {}", e)))?;

    Ok(Json(stats.into()))
}

/// Delete all storage files for a specific plugin
///
/// Removes the plugin's entire data directory. This is irreversible.
///
/// # Permission Required
/// - Admin access required
#[utoipa::path(
    delete,
    path = "/api/v1/admin/plugin-storage/{name}",
    params(
        ("name" = String, Path, description = "Plugin name"),
    ),
    responses(
        (status = 200, description = "Plugin storage cleaned up", body = PluginCleanupResultDto),
        (status = 403, description = "Admin access required"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Admin"
)]
pub async fn cleanup_plugin_storage(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    Path(name): Path<String>,
) -> Result<Json<PluginCleanupResultDto>, ApiError> {
    require_permission!(auth, Permission::SystemAdmin)?;

    let plugin_storage = state
        .plugin_file_storage
        .as_ref()
        .ok_or_else(|| ApiError::Internal("Plugin file storage not configured".to_string()))?;

    let stats = plugin_storage
        .cleanup_plugin(&name)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to cleanup plugin storage: {}", e)))?;

    Ok(Json(stats.into()))
}
