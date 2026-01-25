//! Komga-compatible user handlers
//!
//! Handlers for user endpoints in the Komga-compatible API.
//! These endpoints allow Komic and other Komga-compatible apps to get
//! information about the currently authenticated user.

use super::super::dto::user::KomgaUserDto;
use crate::api::{
    error::ApiError,
    extractors::{AuthState, FlexibleAuthContext},
    permissions::Permission,
};
use crate::require_permission;
use axum::{extract::State, Json};
use std::sync::Arc;

/// Get current user information
///
/// Returns information about the currently authenticated user in Komga format.
/// This endpoint is used by Komic and other apps to verify authentication
/// and determine user capabilities.
///
/// ## Endpoint
/// `GET /{prefix}/api/v1/users/me`
///
/// ## Response
/// Returns a `KomgaUserDto` containing:
/// - User ID (UUID as string)
/// - Email address
/// - Roles (ADMIN, USER, FILE_DOWNLOAD)
/// - Library access settings
/// - Content restrictions
///
/// ## Authentication
/// - Bearer token (JWT)
/// - Basic Auth
/// - API Key
#[utoipa::path(
    get,
    path = "/{prefix}/api/v1/users/me",
    responses(
        (status = 200, description = "Current user information", body = KomgaUserDto),
        (status = 401, description = "Unauthorized"),
    ),
    params(
        ("prefix" = String, Path, description = "Komga API prefix (default: komga)")
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Komga"
)]
pub async fn get_current_user(
    State(_state): State<Arc<AuthState>>,
    FlexibleAuthContext(auth): FlexibleAuthContext,
) -> Result<Json<KomgaUserDto>, ApiError> {
    require_permission!(auth, Permission::BooksRead)?;

    // Convert Codex auth context to Komga user DTO
    // Note: auth.role is a UserRole enum, so we convert it to string
    let user_dto = KomgaUserDto::from_codex(auth.user_id, &auth.username, &auth.role.to_string());

    Ok(Json(user_dto))
}

#[cfg(test)]
mod tests {
    use crate::api::routes::komga::dto::user::KomgaUserDto;

    #[test]
    fn test_user_dto_admin_mapping() {
        let id = uuid::Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let dto = KomgaUserDto::from_codex(id, "admin@test.com", "admin");

        assert_eq!(dto.id, "550e8400-e29b-41d4-a716-446655440000");
        assert_eq!(dto.email, "admin@test.com");
        assert!(dto.roles.contains(&"ADMIN".to_string()));
        assert!(dto.shared_all_libraries);
    }

    #[test]
    fn test_user_dto_reader_mapping() {
        let id = uuid::Uuid::new_v4();
        let dto = KomgaUserDto::from_codex(id, "reader@test.com", "reader");

        assert_eq!(dto.roles, vec!["USER".to_string()]);
        assert!(dto.shared_all_libraries);
    }

    #[test]
    fn test_user_dto_serialization() {
        let id = uuid::Uuid::new_v4();
        let dto = KomgaUserDto::from_codex(id, "test@test.com", "admin");
        let json = serde_json::to_string(&dto).unwrap();

        // Verify camelCase field names
        assert!(json.contains("\"sharedAllLibraries\":true"));
        assert!(json.contains("\"roles\":[\"ADMIN\"]"));
        assert!(json.contains("\"email\":\"test@test.com\""));
    }
}
