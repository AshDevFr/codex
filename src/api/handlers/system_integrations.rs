//! System Integrations API handlers (Admin only)

use crate::api::{
    dto::{
        CreateSystemIntegrationRequest, IntegrationStatusResponse, IntegrationTestResult,
        SystemIntegrationDto, SystemIntegrationsListResponse, UpdateSystemIntegrationRequest,
    },
    error::ApiError,
    extractors::AuthContext,
    permissions::Permission,
    AppState,
};
use crate::db::repositories::SystemIntegrationsRepository;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use std::sync::Arc;
use utoipa::OpenApi;
use uuid::Uuid;

#[derive(OpenApi)]
#[openapi(
    paths(
        list_system_integrations,
        create_system_integration,
        get_system_integration,
        update_system_integration,
        delete_system_integration,
        enable_system_integration,
        disable_system_integration,
        test_system_integration,
    ),
    components(schemas(
        SystemIntegrationDto,
        SystemIntegrationsListResponse,
        CreateSystemIntegrationRequest,
        UpdateSystemIntegrationRequest,
        IntegrationTestResult,
        IntegrationStatusResponse,
    )),
    tags(
        (name = "System Integrations", description = "Admin-managed external service integrations")
    )
)]
pub struct SystemIntegrationsApi;

/// List all system integrations
#[utoipa::path(
    get,
    path = "/api/v1/admin/integrations",
    responses(
        (status = 200, description = "System integrations retrieved", body = SystemIntegrationsListResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Admin permission required"),
    ),
    security(
        ("bearer_auth" = []),
        ("api_key" = [])
    ),
    tag = "System Integrations"
)]
pub async fn list_system_integrations(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
) -> Result<Json<SystemIntegrationsListResponse>, ApiError> {
    auth.require_permission(&Permission::SystemAdmin)?;

    let integrations = SystemIntegrationsRepository::get_all(&state.db)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to get integrations: {}", e)))?;

    let total = integrations.len();
    let dtos: Vec<SystemIntegrationDto> = integrations.into_iter().map(Into::into).collect();

    Ok(Json(SystemIntegrationsListResponse {
        integrations: dtos,
        total,
    }))
}

/// Create a new system integration
#[utoipa::path(
    post,
    path = "/api/v1/admin/integrations",
    request_body = CreateSystemIntegrationRequest,
    responses(
        (status = 201, description = "Integration created", body = SystemIntegrationDto),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Admin permission required"),
        (status = 409, description = "Integration with this name already exists"),
    ),
    security(
        ("bearer_auth" = []),
        ("api_key" = [])
    ),
    tag = "System Integrations"
)]
pub async fn create_system_integration(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    Json(request): Json<CreateSystemIntegrationRequest>,
) -> Result<(StatusCode, Json<SystemIntegrationDto>), ApiError> {
    auth.require_permission(&Permission::SystemAdmin)?;

    // Validate name
    if !is_valid_integration_name(&request.name) {
        return Err(ApiError::BadRequest(
            "Invalid integration name. Use lowercase alphanumeric characters and underscores only"
                .to_string(),
        ));
    }

    // Validate integration type
    let valid_types = ["metadata_provider", "notification", "storage", "sync"];
    if !valid_types.contains(&request.integration_type.as_str()) {
        return Err(ApiError::BadRequest(format!(
            "Invalid integration type '{}'. Valid types: {:?}",
            request.integration_type, valid_types
        )));
    }

    // Check if name already exists
    if let Some(_) = SystemIntegrationsRepository::get_by_name(&state.db, &request.name)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to check existing: {}", e)))?
    {
        return Err(ApiError::Conflict(format!(
            "Integration with name '{}' already exists",
            request.name
        )));
    }

    let integration = SystemIntegrationsRepository::create(
        &state.db,
        &request.name,
        &request.display_name,
        &request.integration_type,
        request.credentials.as_ref(),
        request.config,
        request.enabled,
        Some(auth.user_id),
    )
    .await
    .map_err(|e| ApiError::Internal(format!("Failed to create integration: {}", e)))?;

    Ok((StatusCode::CREATED, Json(integration.into())))
}

/// Get a system integration by ID
#[utoipa::path(
    get,
    path = "/api/v1/admin/integrations/{id}",
    params(
        ("id" = Uuid, Path, description = "Integration ID")
    ),
    responses(
        (status = 200, description = "Integration retrieved", body = SystemIntegrationDto),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Admin permission required"),
        (status = 404, description = "Integration not found"),
    ),
    security(
        ("bearer_auth" = []),
        ("api_key" = [])
    ),
    tag = "System Integrations"
)]
pub async fn get_system_integration(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    Path(id): Path<Uuid>,
) -> Result<Json<SystemIntegrationDto>, ApiError> {
    auth.require_permission(&Permission::SystemAdmin)?;

    let integration = SystemIntegrationsRepository::get_by_id(&state.db, id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to get integration: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Integration not found".to_string()))?;

    Ok(Json(integration.into()))
}

/// Update a system integration
#[utoipa::path(
    patch,
    path = "/api/v1/admin/integrations/{id}",
    params(
        ("id" = Uuid, Path, description = "Integration ID")
    ),
    request_body = UpdateSystemIntegrationRequest,
    responses(
        (status = 200, description = "Integration updated", body = SystemIntegrationDto),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Admin permission required"),
        (status = 404, description = "Integration not found"),
    ),
    security(
        ("bearer_auth" = []),
        ("api_key" = [])
    ),
    tag = "System Integrations"
)]
pub async fn update_system_integration(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    Path(id): Path<Uuid>,
    Json(request): Json<UpdateSystemIntegrationRequest>,
) -> Result<Json<SystemIntegrationDto>, ApiError> {
    auth.require_permission(&Permission::SystemAdmin)?;

    // Determine credentials action:
    // - Some(value) -> update credentials
    // - None -> no change to credentials
    // For clearing credentials, send explicit null in JSON
    let credentials = if request.credentials.is_some() {
        Some(request.credentials.as_ref())
    } else {
        None
    };

    let integration = SystemIntegrationsRepository::update(
        &state.db,
        id,
        request.display_name,
        credentials,
        request.config,
        Some(auth.user_id),
    )
    .await
    .map_err(|e| {
        if e.to_string().contains("not found") {
            ApiError::NotFound("Integration not found".to_string())
        } else {
            ApiError::Internal(format!("Failed to update integration: {}", e))
        }
    })?;

    Ok(Json(integration.into()))
}

/// Delete a system integration
#[utoipa::path(
    delete,
    path = "/api/v1/admin/integrations/{id}",
    params(
        ("id" = Uuid, Path, description = "Integration ID")
    ),
    responses(
        (status = 204, description = "Integration deleted"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Admin permission required"),
        (status = 404, description = "Integration not found"),
    ),
    security(
        ("bearer_auth" = []),
        ("api_key" = [])
    ),
    tag = "System Integrations"
)]
pub async fn delete_system_integration(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    auth.require_permission(&Permission::SystemAdmin)?;

    let deleted = SystemIntegrationsRepository::delete(&state.db, id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to delete integration: {}", e)))?;

    if deleted {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::NotFound("Integration not found".to_string()))
    }
}

/// Enable a system integration
#[utoipa::path(
    post,
    path = "/api/v1/admin/integrations/{id}/enable",
    params(
        ("id" = Uuid, Path, description = "Integration ID")
    ),
    responses(
        (status = 200, description = "Integration enabled", body = IntegrationStatusResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Admin permission required"),
        (status = 404, description = "Integration not found"),
    ),
    security(
        ("bearer_auth" = []),
        ("api_key" = [])
    ),
    tag = "System Integrations"
)]
pub async fn enable_system_integration(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    Path(id): Path<Uuid>,
) -> Result<Json<IntegrationStatusResponse>, ApiError> {
    auth.require_permission(&Permission::SystemAdmin)?;

    let integration = SystemIntegrationsRepository::enable(&state.db, id, Some(auth.user_id))
        .await
        .map_err(|e| {
            if e.to_string().contains("not found") {
                ApiError::NotFound("Integration not found".to_string())
            } else {
                ApiError::Internal(format!("Failed to enable integration: {}", e))
            }
        })?;

    Ok(Json(IntegrationStatusResponse {
        integration: integration.into(),
        message: "Integration enabled successfully".to_string(),
    }))
}

/// Disable a system integration
#[utoipa::path(
    post,
    path = "/api/v1/admin/integrations/{id}/disable",
    params(
        ("id" = Uuid, Path, description = "Integration ID")
    ),
    responses(
        (status = 200, description = "Integration disabled", body = IntegrationStatusResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Admin permission required"),
        (status = 404, description = "Integration not found"),
    ),
    security(
        ("bearer_auth" = []),
        ("api_key" = [])
    ),
    tag = "System Integrations"
)]
pub async fn disable_system_integration(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    Path(id): Path<Uuid>,
) -> Result<Json<IntegrationStatusResponse>, ApiError> {
    auth.require_permission(&Permission::SystemAdmin)?;

    let integration = SystemIntegrationsRepository::disable(&state.db, id, Some(auth.user_id))
        .await
        .map_err(|e| {
            if e.to_string().contains("not found") {
                ApiError::NotFound("Integration not found".to_string())
            } else {
                ApiError::Internal(format!("Failed to disable integration: {}", e))
            }
        })?;

    Ok(Json(IntegrationStatusResponse {
        integration: integration.into(),
        message: "Integration disabled successfully".to_string(),
    }))
}

/// Test a system integration connection
#[utoipa::path(
    post,
    path = "/api/v1/admin/integrations/{id}/test",
    params(
        ("id" = Uuid, Path, description = "Integration ID")
    ),
    responses(
        (status = 200, description = "Test completed", body = IntegrationTestResult),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Admin permission required"),
        (status = 404, description = "Integration not found"),
    ),
    security(
        ("bearer_auth" = []),
        ("api_key" = [])
    ),
    tag = "System Integrations"
)]
pub async fn test_system_integration(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    Path(id): Path<Uuid>,
) -> Result<Json<IntegrationTestResult>, ApiError> {
    auth.require_permission(&Permission::SystemAdmin)?;

    let _integration = SystemIntegrationsRepository::get_by_id(&state.db, id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to get integration: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Integration not found".to_string()))?;

    // TODO: Implement actual provider testing based on integration type
    // For now, return a placeholder response
    Ok(Json(IntegrationTestResult {
        success: true,
        message: "Integration test not yet implemented".to_string(),
        latency_ms: None,
    }))
}

/// Validate an integration name (lowercase alphanumeric with underscores)
fn is_valid_integration_name(name: &str) -> bool {
    if name.is_empty() || name.len() > 100 {
        return false;
    }

    name.chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        && !name.starts_with('_')
        && !name.ends_with('_')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_integration_names() {
        assert!(is_valid_integration_name("mangaupdates"));
        assert!(is_valid_integration_name("anilist_api"));
        assert!(is_valid_integration_name("provider123"));
        assert!(is_valid_integration_name("my_custom_provider"));
    }

    #[test]
    fn test_invalid_integration_names() {
        assert!(!is_valid_integration_name(""));
        assert!(!is_valid_integration_name("MangaUpdates")); // uppercase
        assert!(!is_valid_integration_name("my-provider")); // dash
        assert!(!is_valid_integration_name("my provider")); // space
        assert!(!is_valid_integration_name("_provider")); // starts with underscore
        assert!(!is_valid_integration_name("provider_")); // ends with underscore
                                                          // Too long
        let long_name = "a".repeat(101);
        assert!(!is_valid_integration_name(&long_name));
    }
}
