use axum::{
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug)]
pub enum ApiError {
    Unauthorized(String),
    Forbidden(String),
    NotFound(String),
    BadRequest(String),
    Conflict(String),
    Internal(String),
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ErrorResponse {
    pub error: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, error, message) = match self {
            ApiError::Unauthorized(msg) => (StatusCode::UNAUTHORIZED, "Unauthorized", msg),
            ApiError::Forbidden(msg) => (StatusCode::FORBIDDEN, "Forbidden", msg),
            ApiError::NotFound(msg) => (StatusCode::NOT_FOUND, "NotFound", msg),
            ApiError::BadRequest(msg) => (StatusCode::BAD_REQUEST, "BadRequest", msg),
            ApiError::Conflict(msg) => (StatusCode::CONFLICT, "Conflict", msg),
            ApiError::Internal(msg) => {
                tracing::error!("Internal server error: {}", msg);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "InternalServerError",
                    "An internal error occurred".to_string(),
                )
            }
        };

        let body = Json(ErrorResponse {
            error: error.to_string(),
            message,
            details: None,
        });

        // Don't add WWW-Authenticate header for 401 responses
        // This would trigger browser's basic auth dialog, which we don't want for JWT-based API
        (status, body).into_response()
    }
}

// Implement From for common error types
impl From<anyhow::Error> for ApiError {
    fn from(err: anyhow::Error) -> Self {
        ApiError::Internal(err.to_string())
    }
}

impl From<sea_orm::DbErr> for ApiError {
    fn from(err: sea_orm::DbErr) -> Self {
        ApiError::Internal(err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_response_serialization() {
        let response = ErrorResponse {
            error: "Unauthorized".to_string(),
            message: "Invalid token".to_string(),
            details: None,
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("Unauthorized"));
        assert!(json.contains("Invalid token"));
    }

    #[test]
    fn test_error_response_with_details() {
        use serde_json::json;

        let response = ErrorResponse {
            error: "BadRequest".to_string(),
            message: "Validation failed".to_string(),
            details: Some(json!({"field": "email", "reason": "invalid format"})),
        };

        let json_str = serde_json::to_string(&response).unwrap();
        assert!(json_str.contains("BadRequest"));
        assert!(json_str.contains("email"));
    }
}
