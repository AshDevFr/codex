use crate::api::{
    dto::{
        BulkUpdateSettingsRequest, HistoryQuery, ListSettingsQuery, SettingDto, SettingHistoryDto,
        UpdateSettingRequest,
    },
    error::ApiError,
    extractors::{AuthContext, AuthState},
};
use crate::db::repositories::SettingsRepository;
use crate::require_admin;
use axum::{
    extract::{Path, Query, State},
    Json,
};
use std::sync::Arc;
use utoipa::IntoParams;

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
    tag = "settings"
)]
pub async fn list_settings(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Query(query): Query<ListSettingsQuery>,
) -> Result<Json<Vec<SettingDto>>, ApiError> {
    require_admin!(auth)?;

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
    path = "/api/v1/admin/settings/{key}",
    params(
        ("key" = String, Path, description = "Setting key (e.g., scanner.max_concurrent_scans)")
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
    tag = "settings"
)]
pub async fn get_setting(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(key): Path<String>,
) -> Result<Json<SettingDto>, ApiError> {
    require_admin!(auth)?;

    let setting = SettingsRepository::get(&state.db, &key)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch setting: {}", e)))?
        .ok_or_else(|| ApiError::NotFound(format!("Setting '{}' not found", key)))?;

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
    path = "/api/v1/admin/settings/{key}",
    params(
        ("key" = String, Path, description = "Setting key (e.g., scanner.max_concurrent_scans)")
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
    tag = "settings"
)]
pub async fn update_setting(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(key): Path<String>,
    Json(request): Json<UpdateSettingRequest>,
) -> Result<Json<SettingDto>, ApiError> {
    require_admin!(auth)?;

    // TODO: Extract IP address from request headers
    let ip_address = None;

    let setting = SettingsRepository::set(
        &state.db,
        &key,
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

    // TODO: Reload scheduler when deduplication settings are updated
    // If the setting key starts with "deduplication." or library scanning settings change,
    // trigger a scheduler.reload_schedules() call to pick up the new cron schedule.
    // This requires adding the scheduler to AppState as Arc<Mutex<Scheduler>>.
    // Example implementation:
    //   if key.starts_with("deduplication.") {
    //       if let Some(scheduler) = &state.scheduler {
    //           scheduler.lock().await.reload_schedules().await?;
    //       }
    //   }

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
    tag = "settings"
)]
pub async fn bulk_update_settings(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Json(request): Json<BulkUpdateSettingsRequest>,
) -> Result<Json<Vec<SettingDto>>, ApiError> {
    require_admin!(auth)?;

    let updates: Vec<(String, String)> = request
        .updates
        .into_iter()
        .map(|u| (u.key, u.value))
        .collect();

    // TODO: Extract IP address from request headers
    let ip_address = None;

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
    path = "/api/v1/admin/settings/{key}/reset",
    params(
        ("key" = String, Path, description = "Setting key (e.g., scanner.max_concurrent_scans)")
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
    tag = "settings"
)]
pub async fn reset_setting(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(key): Path<String>,
) -> Result<Json<SettingDto>, ApiError> {
    require_admin!(auth)?;

    // TODO: Extract IP address from request headers
    let ip_address = None;

    let setting = SettingsRepository::reset_to_default(
        &state.db,
        &key,
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
    path = "/api/v1/admin/settings/{key}/history",
    params(
        ("key" = String, Path, description = "Setting key (e.g., scanner.max_concurrent_scans)"),
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
    tag = "settings"
)]
pub async fn get_setting_history(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(key): Path<String>,
    Query(query): Query<HistoryQuery>,
) -> Result<Json<Vec<SettingHistoryDto>>, ApiError> {
    require_admin!(auth)?;

    let history = SettingsRepository::get_history(&state.db, &key, query.limit)
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
