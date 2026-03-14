//! KOReader authentication handlers

use crate::api::error::ApiError;
use crate::api::extractors::AuthContext;
use crate::api::routes::koreader::dto::progress::AuthorizedDto;
use axum::Json;
use axum::http::StatusCode;

/// POST /koreader/users/create
///
/// Always returns 403 Forbidden. User registration is handled by Codex itself,
/// not through the KOReader sync protocol.
pub async fn create_user() -> StatusCode {
    StatusCode::FORBIDDEN
}

/// GET /koreader/users/auth
///
/// Returns 200 with `{"authorized": "OK"}` if the user is authenticated.
/// KOReader uses x-auth-user/x-auth-key headers, which map to Basic Auth in Codex.
pub async fn authorize(_auth: AuthContext) -> Result<Json<AuthorizedDto>, ApiError> {
    Ok(Json(AuthorizedDto {
        authorized: "OK".to_string(),
    }))
}
