//! User Integrations API Handlers
//!
//! Provides endpoints for managing per-user external service connections.
//! Users can connect their personal accounts to services like AniList, MyAnimeList, etc.

use crate::api::{
    dto::{
        AvailableIntegrationDto, ConnectIntegrationRequest, ConnectIntegrationResponse,
        OAuthCallbackRequest, SyncTriggerResponse, UpdateIntegrationSettingsRequest,
        UserIntegrationDto, UserIntegrationsListResponse,
    },
    error::ApiError,
    extractors::AuthContext,
    AppState,
};
use crate::db::entities::user_integrations::IntegrationProvider;
use crate::db::repositories::UserIntegrationsRepository;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use std::sync::Arc;

/// List all integrations for the current user
#[utoipa::path(
    get,
    path = "/api/v1/user/integrations",
    tag = "User Integrations",
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "List of user integrations", body = UserIntegrationsListResponse),
        (status = 401, description = "Not authenticated"),
    )
)]
pub async fn list_user_integrations(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
) -> Result<Json<UserIntegrationsListResponse>, ApiError> {
    let user_id = auth.user_id;

    // Get connected integrations
    let integrations = UserIntegrationsRepository::get_all_for_user(&state.db, user_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch integrations: {}", e)))?;

    let connected_names: Vec<_> = integrations
        .iter()
        .map(|i| i.integration_name.clone())
        .collect();

    let integration_dtos: Vec<UserIntegrationDto> =
        integrations.into_iter().map(Into::into).collect();

    // Build available integrations list
    let available: Vec<AvailableIntegrationDto> = IntegrationProvider::all()
        .iter()
        .map(|provider| {
            let connected = connected_names.contains(&provider.to_string());
            AvailableIntegrationDto {
                name: provider.as_str().to_string(),
                display_name: provider.display_name().to_string(),
                description: get_provider_description(provider),
                auth_type: provider.auth_type().to_string(),
                features: provider.features().iter().map(|s| s.to_string()).collect(),
                connected,
            }
        })
        .collect();

    Ok(Json(UserIntegrationsListResponse {
        integrations: integration_dtos,
        available,
    }))
}

/// Get a specific integration by name
#[utoipa::path(
    get,
    path = "/api/v1/user/integrations/{name}",
    tag = "User Integrations",
    security(("bearer_auth" = [])),
    params(
        ("name" = String, Path, description = "Integration name (e.g., anilist)")
    ),
    responses(
        (status = 200, description = "Integration details", body = UserIntegrationDto),
        (status = 401, description = "Not authenticated"),
        (status = 404, description = "Integration not found"),
    )
)]
pub async fn get_user_integration(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    Path(name): Path<String>,
) -> Result<Json<UserIntegrationDto>, ApiError> {
    let user_id = auth.user_id;

    let integration = UserIntegrationsRepository::get_by_user_and_name(&state.db, user_id, &name)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch integration: {}", e)))?
        .ok_or_else(|| ApiError::NotFound(format!("Integration '{}' not connected", name)))?;

    Ok(Json(integration.into()))
}

/// Initiate connection to an integration
#[utoipa::path(
    post,
    path = "/api/v1/user/integrations",
    tag = "User Integrations",
    security(("bearer_auth" = [])),
    request_body = ConnectIntegrationRequest,
    responses(
        (status = 200, description = "Connection initiated", body = ConnectIntegrationResponse),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Not authenticated"),
        (status = 409, description = "Integration already connected"),
    )
)]
pub async fn connect_integration(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    Json(request): Json<ConnectIntegrationRequest>,
) -> Result<Json<ConnectIntegrationResponse>, ApiError> {
    let user_id = auth.user_id;

    // Validate integration name
    let provider = IntegrationProvider::from_str(&request.integration_name).ok_or_else(|| {
        ApiError::BadRequest(format!("Unknown integration: {}", request.integration_name))
    })?;

    // Check if already connected
    let is_connected =
        UserIntegrationsRepository::is_connected(&state.db, user_id, &request.integration_name)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to check connection: {}", e)))?;

    if is_connected {
        return Err(ApiError::Conflict(format!(
            "Integration '{}' is already connected",
            request.integration_name
        )));
    }

    // Handle based on auth type
    match provider.auth_type() {
        "oauth2" => {
            // For OAuth2, we need to redirect the user to the provider's authorization URL
            // For now, return a placeholder - actual OAuth implementation will be in Phase 4
            let redirect_uri = request.redirect_uri.ok_or_else(|| {
                ApiError::BadRequest("redirect_uri is required for OAuth integrations".to_string())
            })?;

            // Generate state parameter for CSRF protection
            let state_param = uuid::Uuid::new_v4().to_string();

            // TODO: Store state in session/cache for validation on callback
            // TODO: Build actual OAuth URL based on provider

            // TODO: Build actual OAuth URL based on provider with proper URL encoding
            // For now, return a placeholder URL - actual OAuth implementation in Phase 4
            let auth_url = format!(
                "https://{}.example.com/oauth/authorize?state={}",
                provider.as_str(),
                state_param
            );

            Ok(Json(ConnectIntegrationResponse {
                auth_url: Some(auth_url),
                connected: false,
                integration: None,
            }))
        }
        "api_key" => {
            // For API key auth, create the integration immediately
            let api_key = request.api_key.ok_or_else(|| {
                ApiError::BadRequest("api_key is required for API key integrations".to_string())
            })?;

            let credentials = serde_json::json!({
                "api_key": api_key
            });

            let integration = UserIntegrationsRepository::create(
                &state.db,
                user_id,
                &request.integration_name,
                None,
                &credentials,
                None,
                None,
                None,
                None,
            )
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to create integration: {}", e)))?;

            Ok(Json(ConnectIntegrationResponse {
                auth_url: None,
                connected: true,
                integration: Some(integration.into()),
            }))
        }
        _ => Err(ApiError::Internal("Unknown auth type".to_string())),
    }
}

/// Handle OAuth callback
#[utoipa::path(
    post,
    path = "/api/v1/user/integrations/{name}/callback",
    tag = "User Integrations",
    security(("bearer_auth" = [])),
    params(
        ("name" = String, Path, description = "Integration name (e.g., anilist)")
    ),
    request_body = OAuthCallbackRequest,
    responses(
        (status = 200, description = "Integration connected", body = UserIntegrationDto),
        (status = 400, description = "Invalid callback"),
        (status = 401, description = "Not authenticated"),
    )
)]
pub async fn oauth_callback(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    Path(name): Path<String>,
    Json(request): Json<OAuthCallbackRequest>,
) -> Result<Json<UserIntegrationDto>, ApiError> {
    let user_id = auth.user_id;

    // Validate integration name
    let provider = IntegrationProvider::from_str(&name)
        .ok_or_else(|| ApiError::BadRequest(format!("Unknown integration: {}", name)))?;

    // Verify this is an OAuth provider
    if provider.auth_type() != "oauth2" {
        return Err(ApiError::BadRequest(format!(
            "Integration '{}' does not use OAuth",
            name
        )));
    }

    // TODO: Validate state parameter against stored value (CSRF protection)
    // TODO: Exchange authorization code for tokens using provider-specific implementation
    // For now, create a placeholder integration

    // Placeholder credentials - actual implementation will exchange the code for tokens
    let credentials = serde_json::json!({
        "access_token": format!("placeholder-token-{}", request.code),
        "refresh_token": "placeholder-refresh",
        "code": request.code,
    });

    let integration = UserIntegrationsRepository::create(
        &state.db,
        user_id,
        &name,
        None,
        &credentials,
        Some(serde_json::json!({
            "sync_progress": true,
            "sync_ratings": true,
        })),
        None,
        None,
        None,
    )
    .await
    .map_err(|e| ApiError::Internal(format!("Failed to create integration: {}", e)))?;

    Ok(Json(integration.into()))
}

/// Update integration settings
#[utoipa::path(
    patch,
    path = "/api/v1/user/integrations/{name}",
    tag = "User Integrations",
    security(("bearer_auth" = [])),
    params(
        ("name" = String, Path, description = "Integration name (e.g., anilist)")
    ),
    request_body = UpdateIntegrationSettingsRequest,
    responses(
        (status = 200, description = "Integration updated", body = UserIntegrationDto),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Not authenticated"),
        (status = 404, description = "Integration not found"),
    )
)]
pub async fn update_integration_settings(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    Path(name): Path<String>,
    Json(request): Json<UpdateIntegrationSettingsRequest>,
) -> Result<Json<UserIntegrationDto>, ApiError> {
    let user_id = auth.user_id;

    let integration = UserIntegrationsRepository::get_by_user_and_name(&state.db, user_id, &name)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch integration: {}", e)))?
        .ok_or_else(|| ApiError::NotFound(format!("Integration '{}' not connected", name)))?;

    // Update enabled state if provided
    let mut current = if let Some(enabled) = request.enabled {
        if enabled {
            UserIntegrationsRepository::enable(&state.db, integration.id)
                .await
                .map_err(|e| ApiError::Internal(format!("Failed to enable integration: {}", e)))?
        } else {
            UserIntegrationsRepository::disable(&state.db, integration.id)
                .await
                .map_err(|e| ApiError::Internal(format!("Failed to disable integration: {}", e)))?
        }
    } else {
        integration
    };

    // Update display name if provided
    if let Some(display_name) = request.display_name {
        current = UserIntegrationsRepository::update_display_name(
            &state.db,
            current.id,
            Some(display_name),
        )
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to update display name: {}", e)))?;
    }

    // Update settings if provided
    let final_integration = if let Some(settings) = request.settings {
        UserIntegrationsRepository::update_settings(&state.db, current.id, settings)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to update settings: {}", e)))?
    } else {
        current
    };

    Ok(Json(final_integration.into()))
}

/// Disconnect an integration
#[utoipa::path(
    delete,
    path = "/api/v1/user/integrations/{name}",
    tag = "User Integrations",
    security(("bearer_auth" = [])),
    params(
        ("name" = String, Path, description = "Integration name (e.g., anilist)")
    ),
    responses(
        (status = 204, description = "Integration disconnected"),
        (status = 401, description = "Not authenticated"),
        (status = 404, description = "Integration not found"),
    )
)]
pub async fn disconnect_integration(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    Path(name): Path<String>,
) -> Result<StatusCode, ApiError> {
    let user_id = auth.user_id;

    let deleted = UserIntegrationsRepository::delete_by_user_and_name(&state.db, user_id, &name)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to disconnect integration: {}", e)))?;

    if !deleted {
        return Err(ApiError::NotFound(format!(
            "Integration '{}' not connected",
            name
        )));
    }

    Ok(StatusCode::NO_CONTENT)
}

/// Trigger a sync for an integration
#[utoipa::path(
    post,
    path = "/api/v1/user/integrations/{name}/sync",
    tag = "User Integrations",
    security(("bearer_auth" = [])),
    params(
        ("name" = String, Path, description = "Integration name (e.g., anilist)")
    ),
    responses(
        (status = 200, description = "Sync triggered", body = SyncTriggerResponse),
        (status = 401, description = "Not authenticated"),
        (status = 404, description = "Integration not found"),
        (status = 409, description = "Sync already in progress"),
    )
)]
pub async fn trigger_sync(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    Path(name): Path<String>,
) -> Result<Json<SyncTriggerResponse>, ApiError> {
    let user_id = auth.user_id;

    let integration = UserIntegrationsRepository::get_by_user_and_name(&state.db, user_id, &name)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch integration: {}", e)))?
        .ok_or_else(|| ApiError::NotFound(format!("Integration '{}' not connected", name)))?;

    // Check if already syncing
    if integration.sync_status == "syncing" {
        return Err(ApiError::Conflict("Sync already in progress".to_string()));
    }

    // Check if enabled
    if !integration.enabled {
        return Err(ApiError::BadRequest("Integration is disabled".to_string()));
    }

    // Update status to syncing
    let updated =
        UserIntegrationsRepository::update_sync_status(&state.db, integration.id, "syncing", None)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to update sync status: {}", e)))?;

    // TODO: Trigger actual sync in background task
    // For now, immediately mark as complete
    let final_integration =
        UserIntegrationsRepository::update_sync_status(&state.db, updated.id, "idle", None)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to complete sync: {}", e)))?;

    Ok(Json(SyncTriggerResponse {
        started: true,
        message: "Sync completed".to_string(),
        integration: final_integration.into(),
    }))
}

/// Get description for a provider
fn get_provider_description(provider: &IntegrationProvider) -> String {
    match provider {
        IntegrationProvider::Anilist => {
            "Sync your reading progress, ratings, and lists with AniList".to_string()
        }
        IntegrationProvider::MyAnimeList => {
            "Sync your reading progress, ratings, and lists with MyAnimeList".to_string()
        }
        IntegrationProvider::Kitsu => {
            "Sync your reading progress and ratings with Kitsu".to_string()
        }
        IntegrationProvider::MangaDex => "Sync your reading progress with MangaDex".to_string(),
        IntegrationProvider::Kavita => {
            "Sync your reading progress and ratings with Kavita".to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::entities::user_integrations::IntegrationProvider;

    #[test]
    fn test_all_providers_have_descriptions() {
        for provider in IntegrationProvider::all() {
            let description = get_provider_description(&provider);
            assert!(!description.is_empty());
        }
    }

    #[test]
    fn test_provider_auth_types() {
        assert_eq!(IntegrationProvider::Anilist.auth_type(), "oauth2");
        assert_eq!(IntegrationProvider::MyAnimeList.auth_type(), "oauth2");
        assert_eq!(IntegrationProvider::MangaDex.auth_type(), "api_key");
        assert_eq!(IntegrationProvider::Kavita.auth_type(), "api_key");
    }
}
