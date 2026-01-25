use axum::{
    http::StatusCode,
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
    /// Resource exists but cannot be processed (e.g., PDF without PDFium)
    UnprocessableEntity(String),
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
            ApiError::Unauthorized(msg) => {
                // Log auth failures at debug level - common during normal operation
                // but useful for debugging auth issues
                tracing::debug!(error = "Unauthorized", message = %msg, "Authentication failed");
                (StatusCode::UNAUTHORIZED, "Unauthorized", msg)
            }
            ApiError::Forbidden(msg) => {
                // Log permission denials at warn level - may indicate misconfigured
                // permissions or unauthorized access attempts
                tracing::warn!(error = "Forbidden", message = %msg, "Permission denied");
                (StatusCode::FORBIDDEN, "Forbidden", msg)
            }
            ApiError::NotFound(msg) => {
                // Log at debug level - 404s are very common and usually expected
                tracing::debug!(error = "NotFound", message = %msg, "Resource not found");
                (StatusCode::NOT_FOUND, "NotFound", msg)
            }
            ApiError::BadRequest(msg) => {
                // Log at debug level - client-side validation errors
                tracing::debug!(error = "BadRequest", message = %msg, "Bad request");
                (StatusCode::BAD_REQUEST, "BadRequest", msg)
            }
            ApiError::Conflict(msg) => {
                // Log at debug level - duplicate resource creation attempts
                tracing::debug!(error = "Conflict", message = %msg, "Resource conflict");
                (StatusCode::CONFLICT, "Conflict", msg)
            }
            ApiError::UnprocessableEntity(msg) => {
                // Log at debug level - validation failures or unsupported operations
                tracing::debug!(error = "UnprocessableEntity", message = %msg, "Unprocessable entity");
                (StatusCode::UNPROCESSABLE_ENTITY, "UnprocessableEntity", msg)
            }
            ApiError::Internal(msg) => {
                tracing::error!(error = "InternalServerError", message = %msg, "Internal server error");
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
        let err_msg = err.to_string();

        // Check for PDFium not available error - this is a known limitation, not a server error
        if err_msg.contains("PDFium renderer is not available") {
            return ApiError::UnprocessableEntity(
                "This PDF contains text or vector graphics that require PDFium to render. \
                 PDFium is not installed or configured on this server."
                    .to_string(),
            );
        }

        ApiError::Internal(err_msg)
    }
}

impl From<sea_orm::DbErr> for ApiError {
    fn from(err: sea_orm::DbErr) -> Self {
        ApiError::Internal(err.to_string())
    }
}

impl ApiError {
    /// Convert an anyhow error to ApiError with additional context message.
    /// Preserves special error handling (e.g., PDFium errors become UnprocessableEntity).
    pub fn from_anyhow_with_context(err: anyhow::Error, context: &str) -> Self {
        let err_msg = err.to_string();

        // Check for PDFium not available error - this is a known limitation, not a server error
        if err_msg.contains("PDFium renderer is not available") {
            return ApiError::UnprocessableEntity(
                "This PDF contains text or vector graphics that require PDFium to render. \
                 PDFium is not installed or configured on this server."
                    .to_string(),
            );
        }

        ApiError::Internal(format!("{}: {}", context, err_msg))
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
