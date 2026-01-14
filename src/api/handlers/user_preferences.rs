//! User Preferences API handlers

use crate::api::{
    dto::{
        BulkSetPreferencesRequest, DeletePreferenceResponse, SetPreferenceRequest,
        SetPreferencesResponse, UserPreferenceDto, UserPreferencesResponse,
    },
    error::ApiError,
    extractors::AuthContext,
    AppState,
};
use crate::db::repositories::UserPreferencesRepository;
use axum::{
    extract::{Path, State},
    Json,
};
use std::sync::Arc;
use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    paths(
        get_all_preferences,
        get_preference,
        set_preference,
        set_bulk_preferences,
        delete_preference,
    ),
    components(schemas(
        UserPreferenceDto,
        UserPreferencesResponse,
        SetPreferenceRequest,
        BulkSetPreferencesRequest,
        SetPreferencesResponse,
        DeletePreferenceResponse,
    )),
    tags(
        (name = "User Preferences", description = "Per-user settings and preferences")
    )
)]
#[allow(dead_code)] // OpenAPI documentation struct - referenced by utoipa derive macros
pub struct UserPreferencesApi;

/// Get all preferences for the authenticated user
#[utoipa::path(
    get,
    path = "/api/v1/user/preferences",
    responses(
        (status = 200, description = "User preferences retrieved", body = UserPreferencesResponse),
        (status = 401, description = "Unauthorized"),
    ),
    security(
        ("bearer_auth" = []),
        ("api_key" = [])
    ),
    tag = "User Preferences"
)]
pub async fn get_all_preferences(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
) -> Result<Json<UserPreferencesResponse>, ApiError> {
    let prefs = UserPreferencesRepository::get_all_by_user(&state.db, auth.user_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to get preferences: {}", e)))?;

    let preferences: Result<Vec<UserPreferenceDto>, _> =
        prefs.into_iter().map(UserPreferenceDto::try_from).collect();

    let preferences = preferences
        .map_err(|e| ApiError::Internal(format!("Failed to convert preferences: {}", e)))?;

    Ok(Json(UserPreferencesResponse { preferences }))
}

/// Get a single preference by key
#[utoipa::path(
    get,
    path = "/api/v1/user/preferences/{key}",
    params(
        ("key" = String, Path, description = "Preference key (e.g., 'ui.theme')")
    ),
    responses(
        (status = 200, description = "Preference retrieved", body = UserPreferenceDto),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Preference not found"),
    ),
    security(
        ("bearer_auth" = []),
        ("api_key" = [])
    ),
    tag = "User Preferences"
)]
pub async fn get_preference(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    Path(key): Path<String>,
) -> Result<Json<UserPreferenceDto>, ApiError> {
    let pref = UserPreferencesRepository::get_by_key(&state.db, auth.user_id, &key)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to get preference: {}", e)))?
        .ok_or_else(|| ApiError::NotFound(format!("Preference '{}' not found", key)))?;

    let dto = UserPreferenceDto::try_from(pref)
        .map_err(|e| ApiError::Internal(format!("Failed to convert preference: {}", e)))?;

    Ok(Json(dto))
}

/// Set a single preference value
#[utoipa::path(
    put,
    path = "/api/v1/user/preferences/{key}",
    params(
        ("key" = String, Path, description = "Preference key (e.g., 'ui.theme')")
    ),
    request_body = SetPreferenceRequest,
    responses(
        (status = 200, description = "Preference set successfully", body = UserPreferenceDto),
        (status = 401, description = "Unauthorized"),
        (status = 400, description = "Invalid preference value"),
    ),
    security(
        ("bearer_auth" = []),
        ("api_key" = [])
    ),
    tag = "User Preferences"
)]
pub async fn set_preference(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    Path(key): Path<String>,
    Json(request): Json<SetPreferenceRequest>,
) -> Result<Json<UserPreferenceDto>, ApiError> {
    // Validate key format (dot-separated, alphanumeric with underscores)
    if !is_valid_preference_key(&key) {
        return Err(ApiError::BadRequest(format!(
            "Invalid preference key '{}'. Keys must be dot-separated alphanumeric segments (e.g., 'ui.theme')",
            key
        )));
    }

    let pref = UserPreferencesRepository::set(&state.db, auth.user_id, &key, &request.value)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to set preference: {}", e)))?;

    let dto = UserPreferenceDto::try_from(pref)
        .map_err(|e| ApiError::Internal(format!("Failed to convert preference: {}", e)))?;

    Ok(Json(dto))
}

/// Set multiple preferences at once
#[utoipa::path(
    put,
    path = "/api/v1/user/preferences",
    request_body = BulkSetPreferencesRequest,
    responses(
        (status = 200, description = "Preferences updated successfully", body = SetPreferencesResponse),
        (status = 401, description = "Unauthorized"),
        (status = 400, description = "Invalid preference key or value"),
    ),
    security(
        ("bearer_auth" = []),
        ("api_key" = [])
    ),
    tag = "User Preferences"
)]
pub async fn set_bulk_preferences(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    Json(request): Json<BulkSetPreferencesRequest>,
) -> Result<Json<SetPreferencesResponse>, ApiError> {
    // Validate all keys first
    for key in request.preferences.keys() {
        if !is_valid_preference_key(key) {
            return Err(ApiError::BadRequest(format!(
                "Invalid preference key '{}'. Keys must be dot-separated alphanumeric segments",
                key
            )));
        }
    }

    let preferences_vec: Vec<(String, serde_json::Value)> =
        request.preferences.into_iter().collect();

    let updated_prefs =
        UserPreferencesRepository::set_bulk(&state.db, auth.user_id, preferences_vec)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to set preferences: {}", e)))?;

    let preferences: Result<Vec<UserPreferenceDto>, _> = updated_prefs
        .into_iter()
        .map(UserPreferenceDto::try_from)
        .collect();

    let preferences = preferences
        .map_err(|e| ApiError::Internal(format!("Failed to convert preferences: {}", e)))?;

    let updated = preferences.len();

    Ok(Json(SetPreferencesResponse {
        updated,
        preferences,
    }))
}

/// Delete (reset) a preference to its default
#[utoipa::path(
    delete,
    path = "/api/v1/user/preferences/{key}",
    params(
        ("key" = String, Path, description = "Preference key to delete")
    ),
    responses(
        (status = 200, description = "Preference deleted", body = DeletePreferenceResponse),
        (status = 401, description = "Unauthorized"),
    ),
    security(
        ("bearer_auth" = []),
        ("api_key" = [])
    ),
    tag = "User Preferences"
)]
pub async fn delete_preference(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    Path(key): Path<String>,
) -> Result<Json<DeletePreferenceResponse>, ApiError> {
    let deleted = UserPreferencesRepository::delete(&state.db, auth.user_id, &key)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to delete preference: {}", e)))?;

    let message = if deleted {
        format!("Preference '{}' was reset to default", key)
    } else {
        format!("Preference '{}' was not set", key)
    };

    Ok(Json(DeletePreferenceResponse { deleted, message }))
}

/// Validate a preference key format
/// Valid: "ui.theme", "reader.default_zoom", "library.view_mode"
/// Invalid: ".theme", "ui.", "ui..theme", "ui/theme", "ui theme"
fn is_valid_preference_key(key: &str) -> bool {
    if key.is_empty() || key.len() > 255 {
        return false;
    }

    let parts: Vec<&str> = key.split('.').collect();

    // Must have at least one segment
    if parts.is_empty() {
        return false;
    }

    // Each segment must be non-empty and alphanumeric (with underscores)
    for part in parts {
        if part.is_empty() {
            return false;
        }
        if !part.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            return false;
        }
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_preference_keys() {
        assert!(is_valid_preference_key("ui.theme"));
        assert!(is_valid_preference_key("reader.default_zoom"));
        assert!(is_valid_preference_key("library.view_mode"));
        assert!(is_valid_preference_key("single_key"));
        assert!(is_valid_preference_key("deep.nested.key.value"));
        assert!(is_valid_preference_key("with_underscore.another_one"));
        assert!(is_valid_preference_key("key123.value456"));
    }

    #[test]
    fn test_invalid_preference_keys() {
        assert!(!is_valid_preference_key(""));
        assert!(!is_valid_preference_key(".theme"));
        assert!(!is_valid_preference_key("ui."));
        assert!(!is_valid_preference_key("ui..theme"));
        assert!(!is_valid_preference_key("ui/theme"));
        assert!(!is_valid_preference_key("ui theme"));
        assert!(!is_valid_preference_key("ui-theme"));
        assert!(!is_valid_preference_key("ui:theme"));
        // Test max length
        let long_key = "a".repeat(256);
        assert!(!is_valid_preference_key(&long_key));
    }
}
