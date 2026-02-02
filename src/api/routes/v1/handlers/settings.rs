use super::super::dto::{
    BrandingSettingsDto, BulkUpdateSettingsRequest, HistoryQuery, ListSettingsQuery,
    PublicSettingDto, SettingDto, SettingHistoryDto, UpdateSettingRequest,
};
use crate::api::{
    error::ApiError,
    extractors::{AuthContext, AuthState},
    permissions::Permission,
};
use crate::db::repositories::SettingsRepository;
use crate::require_permission;
use axum::{
    extract::{Path, Query, State},
    Json,
};
use std::collections::HashMap;
use std::sync::Arc;

/// Public settings that can be accessed by any authenticated user
/// These are non-sensitive settings that affect UI/display behavior
const PUBLIC_SETTING_KEYS: &[&str] = &["display.custom_metadata_template", "application.name"];

/// List all settings (admin only)
#[utoipa::path(
    get,
    path = "/api/v1/admin/settings",
    params(
        ("category" = Option<String>, Query, description = "Filter by category")
    ),
    responses(
        (status = 200, description = "List of settings", body = Vec<SettingDto>),
        (status = 403, description = "Forbidden - Admin only"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Settings"
)]
pub async fn list_settings(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Query(query): Query<ListSettingsQuery>,
) -> Result<Json<Vec<SettingDto>>, ApiError> {
    require_permission!(auth, Permission::SystemAdmin)?;

    let settings = if let Some(category) = query.category {
        SettingsRepository::get_by_category(&state.db, &category)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to fetch settings: {}", e)))?
    } else {
        SettingsRepository::get_all(&state.db)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to fetch settings: {}", e)))?
    };

    let dtos: Vec<SettingDto> = settings
        .into_iter()
        .map(|setting| SettingDto {
            id: setting.id,
            key: setting.key,
            value: setting.value,
            value_type: setting.value_type,
            category: setting.category,
            description: setting.description,
            is_sensitive: setting.is_sensitive,
            default_value: setting.default_value,
            min_value: setting.min_value,
            max_value: setting.max_value,
            updated_at: setting.updated_at,
            updated_by: setting.updated_by,
            version: setting.version,
        })
        .collect();

    Ok(Json(dtos))
}

/// Get single setting by key (admin only)
#[utoipa::path(
    get,
    path = "/api/v1/admin/settings/{setting_key}",
    params(
        ("setting_key" = String, Path, description = "Setting key (e.g., scanner.max_concurrent_scans)")
    ),
    responses(
        (status = 200, description = "Setting details", body = SettingDto),
        (status = 404, description = "Setting not found"),
        (status = 403, description = "Forbidden - Admin only"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Settings"
)]
pub async fn get_setting(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(setting_key): Path<String>,
) -> Result<Json<SettingDto>, ApiError> {
    require_permission!(auth, Permission::SystemAdmin)?;

    let setting = SettingsRepository::get(&state.db, &setting_key)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch setting: {}", e)))?
        .ok_or_else(|| ApiError::NotFound(format!("Setting '{}' not found", setting_key)))?;

    let dto = SettingDto {
        id: setting.id,
        key: setting.key,
        value: setting.value,
        value_type: setting.value_type,
        category: setting.category,
        description: setting.description,
        is_sensitive: setting.is_sensitive,
        default_value: setting.default_value,
        min_value: setting.min_value,
        max_value: setting.max_value,
        updated_at: setting.updated_at,
        updated_by: setting.updated_by,
        version: setting.version,
    };

    Ok(Json(dto))
}

/// Update setting (admin only)
#[utoipa::path(
    put,
    path = "/api/v1/admin/settings/{setting_key}",
    params(
        ("setting_key" = String, Path, description = "Setting key (e.g., scanner.max_concurrent_scans)")
    ),
    request_body = UpdateSettingRequest,
    responses(
        (status = 200, description = "Setting updated", body = SettingDto),
        (status = 400, description = "Invalid value"),
        (status = 404, description = "Setting not found"),
        (status = 403, description = "Forbidden - Admin only"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Settings"
)]
pub async fn update_setting(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    client_info: crate::api::extractors::ClientInfo,
    Path(setting_key): Path<String>,
    Json(request): Json<UpdateSettingRequest>,
) -> Result<Json<SettingDto>, ApiError> {
    require_permission!(auth, Permission::SystemAdmin)?;

    let ip_address = client_info.ip_address;

    let setting = SettingsRepository::set(
        &state.db,
        &setting_key,
        request.value,
        auth.user_id,
        request.change_reason,
        ip_address,
    )
    .await
    .map_err(|e| {
        let err_msg = e.to_string();
        // Check if it's a validation error (contains keywords like "Invalid", "above maximum", "below minimum")
        if err_msg.contains("Invalid")
            || err_msg.contains("above maximum")
            || err_msg.contains("below minimum")
        {
            ApiError::BadRequest(err_msg)
        } else {
            ApiError::Internal(format!("Failed to update setting: {}", e))
        }
    })?;

    // Reload scheduler when deduplication settings are updated
    if setting_key.starts_with("deduplication.") {
        if let Some(scheduler) = &state.scheduler {
            if let Err(e) = scheduler.lock().await.reload_schedules().await {
                tracing::warn!(
                    "Failed to reload scheduler after deduplication settings update: {}",
                    e
                );
                // Don't fail the request - setting was updated successfully
            }
        }
    }

    let dto = SettingDto {
        id: setting.id,
        key: setting.key,
        value: setting.value,
        value_type: setting.value_type,
        category: setting.category,
        description: setting.description,
        is_sensitive: setting.is_sensitive,
        default_value: setting.default_value,
        min_value: setting.min_value,
        max_value: setting.max_value,
        updated_at: setting.updated_at,
        updated_by: setting.updated_by,
        version: setting.version,
    };

    Ok(Json(dto))
}

/// Bulk update settings (admin only)
#[utoipa::path(
    post,
    path = "/api/v1/admin/settings/bulk",
    request_body = BulkUpdateSettingsRequest,
    responses(
        (status = 200, description = "Settings updated", body = Vec<SettingDto>),
        (status = 400, description = "Invalid value"),
        (status = 403, description = "Forbidden - Admin only"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Settings"
)]
pub async fn bulk_update_settings(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    client_info: crate::api::extractors::ClientInfo,
    Json(request): Json<BulkUpdateSettingsRequest>,
) -> Result<Json<Vec<SettingDto>>, ApiError> {
    require_permission!(auth, Permission::SystemAdmin)?;

    let updates: Vec<(String, String)> = request
        .updates
        .into_iter()
        .map(|u| (u.key, u.value))
        .collect();

    let ip_address = client_info.ip_address;

    let settings = SettingsRepository::bulk_update(
        &state.db,
        updates,
        auth.user_id,
        request.change_reason,
        ip_address,
    )
    .await
    .map_err(|e| {
        let err_msg = e.to_string();
        // Check if it's a validation error
        if err_msg.contains("Invalid")
            || err_msg.contains("above maximum")
            || err_msg.contains("below minimum")
        {
            ApiError::BadRequest(err_msg)
        } else {
            ApiError::Internal(format!("Failed to bulk update settings: {}", e))
        }
    })?;

    let dtos: Vec<SettingDto> = settings
        .into_iter()
        .map(|setting| SettingDto {
            id: setting.id,
            key: setting.key,
            value: setting.value,
            value_type: setting.value_type,
            category: setting.category,
            description: setting.description,
            is_sensitive: setting.is_sensitive,
            default_value: setting.default_value,
            min_value: setting.min_value,
            max_value: setting.max_value,
            updated_at: setting.updated_at,
            updated_by: setting.updated_by,
            version: setting.version,
        })
        .collect();

    Ok(Json(dtos))
}

/// Reset setting to default value (admin only)
#[utoipa::path(
    post,
    path = "/api/v1/admin/settings/{setting_key}/reset",
    params(
        ("setting_key" = String, Path, description = "Setting key (e.g., scanner.max_concurrent_scans)")
    ),
    responses(
        (status = 200, description = "Setting reset to default", body = SettingDto),
        (status = 404, description = "Setting not found"),
        (status = 403, description = "Forbidden - Admin only"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Settings"
)]
pub async fn reset_setting(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    client_info: crate::api::extractors::ClientInfo,
    Path(setting_key): Path<String>,
) -> Result<Json<SettingDto>, ApiError> {
    require_permission!(auth, Permission::SystemAdmin)?;

    let ip_address = client_info.ip_address;

    let setting = SettingsRepository::reset_to_default(
        &state.db,
        &setting_key,
        auth.user_id,
        Some("Reset to default via admin API".to_string()),
        ip_address,
    )
    .await
    .map_err(|e| ApiError::Internal(format!("Failed to reset setting: {}", e)))?;

    let dto = SettingDto {
        id: setting.id,
        key: setting.key,
        value: setting.value,
        value_type: setting.value_type,
        category: setting.category,
        description: setting.description,
        is_sensitive: setting.is_sensitive,
        default_value: setting.default_value,
        min_value: setting.min_value,
        max_value: setting.max_value,
        updated_at: setting.updated_at,
        updated_by: setting.updated_by,
        version: setting.version,
    };

    Ok(Json(dto))
}

/// Get setting history (admin only)
#[utoipa::path(
    get,
    path = "/api/v1/admin/settings/{setting_key}/history",
    params(
        ("setting_key" = String, Path, description = "Setting key (e.g., scanner.max_concurrent_scans)"),
        ("limit" = Option<i64>, Query, description = "Maximum number of history entries to return")
    ),
    responses(
        (status = 200, description = "Setting history", body = Vec<SettingHistoryDto>),
        (status = 404, description = "Setting not found"),
        (status = 403, description = "Forbidden - Admin only"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Settings"
)]
pub async fn get_setting_history(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(setting_key): Path<String>,
    Query(query): Query<HistoryQuery>,
) -> Result<Json<Vec<SettingHistoryDto>>, ApiError> {
    require_permission!(auth, Permission::SystemAdmin)?;

    let history = SettingsRepository::get_history(&state.db, &setting_key, query.limit)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch setting history: {}", e)))?;

    let dtos: Vec<SettingHistoryDto> = history
        .into_iter()
        .map(|entry| SettingHistoryDto {
            id: entry.id,
            setting_id: entry.setting_id,
            key: entry.key,
            old_value: entry.old_value.unwrap_or_default(),
            new_value: entry.new_value,
            changed_by: entry.changed_by,
            changed_at: entry.changed_at,
            change_reason: entry.change_reason,
            ip_address: entry.ip_address,
        })
        .collect();

    Ok(Json(dtos))
}

/// Get public display settings (authenticated users)
///
/// Returns non-sensitive settings that affect UI/display behavior.
/// This endpoint is available to all authenticated users, not just admins.
#[utoipa::path(
    get,
    path = "/api/v1/settings/public",
    responses(
        (status = 200, description = "Public settings", body = HashMap<String, PublicSettingDto>,
         example = json!({
             "display.custom_metadata_template": {
                 "key": "display.custom_metadata_template",
                 "value": "{{#if custom_metadata}}## Additional Information\n{{#each custom_metadata}}- **{{@key}}**: {{this}}\n{{/each}}{{/if}}"
             }
         })
        ),
        (status = 401, description = "Unauthorized"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Settings"
)]
pub async fn get_public_settings(
    State(state): State<Arc<AuthState>>,
    _auth: AuthContext,
) -> Result<Json<HashMap<String, PublicSettingDto>>, ApiError> {
    let mut result = HashMap::new();

    for key in PUBLIC_SETTING_KEYS {
        if let Ok(Some(setting)) = SettingsRepository::get(&state.db, key).await {
            // Only include non-sensitive settings
            if !setting.is_sensitive {
                result.insert(
                    setting.key.clone(),
                    PublicSettingDto {
                        key: setting.key,
                        value: setting.value,
                    },
                );
            }
        }
    }

    Ok(Json(result))
}

/// Default application name when setting is not found
const DEFAULT_APP_NAME: &str = "Codex";

/// Get branding settings (unauthenticated)
///
/// Returns branding-related settings that are needed on unauthenticated pages
/// like the login screen. This endpoint does not require authentication.
#[utoipa::path(
    get,
    path = "/api/v1/settings/branding",
    responses(
        (status = 200, description = "Branding settings", body = BrandingSettingsDto,
         example = json!({
             "applicationName": "Codex"
         })
        ),
    ),
    tag = "Settings"
)]
pub async fn get_branding_settings(
    State(state): State<Arc<AuthState>>,
) -> Result<Json<BrandingSettingsDto>, ApiError> {
    let application_name = SettingsRepository::get(&state.db, "application.name")
        .await
        .ok()
        .flatten()
        .map(|s| s.value)
        .unwrap_or_else(|| DEFAULT_APP_NAME.to_string());

    Ok(Json(BrandingSettingsDto { application_name }))
}
