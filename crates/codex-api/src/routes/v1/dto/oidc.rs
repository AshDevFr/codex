//! OIDC authentication DTOs
//!
//! Data transfer objects for OpenID Connect (OIDC) authentication endpoints.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Information about an available OIDC provider
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct OidcProviderInfo {
    /// Internal name of the provider (used in URLs)
    #[schema(example = "authentik")]
    pub name: String,

    /// Display name shown to users
    #[schema(example = "Authentik SSO")]
    pub display_name: String,

    /// URL to initiate login with this provider
    #[schema(example = "/api/v1/auth/oidc/authentik/login")]
    pub login_url: String,
}

/// Response listing available OIDC providers
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct OidcProvidersResponse {
    /// Whether OIDC authentication is enabled
    #[schema(example = true)]
    pub enabled: bool,

    /// List of available OIDC providers
    pub providers: Vec<OidcProviderInfo>,
}

/// Response from initiating OIDC login
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct OidcLoginResponse {
    /// URL to redirect the user to for authentication
    #[schema(example = "https://auth.example.com/authorize?client_id=...")]
    pub redirect_url: String,
}

/// Query parameters for OIDC callback
#[derive(Debug, Deserialize)]
pub struct OidcCallbackQuery {
    /// Authorization code from the identity provider
    pub code: String,

    /// State parameter for CSRF protection
    pub state: String,

    /// Error code if authentication failed
    pub error: Option<String>,

    /// Error description if authentication failed
    pub error_description: Option<String>,
}

/// Response from OIDC callback (successful authentication)
///
/// This mirrors the standard LoginResponse format for consistency.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct OidcCallbackResponse {
    /// JWT access token
    #[schema(
        example = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6IkpvaG4gRG9lIiwiaWF0IjoxNTE2MjM5MDIyfQ"
    )]
    pub access_token: String,

    /// Token type (always "Bearer")
    #[schema(example = "Bearer")]
    pub token_type: String,

    /// Token expiry in seconds
    #[schema(example = 86400)]
    pub expires_in: u64,

    /// User information
    pub user: super::UserInfo,

    /// Whether this is a newly created account
    #[schema(example = false)]
    pub new_account: bool,

    /// OIDC provider used for authentication
    #[schema(example = "authentik")]
    pub provider: String,
}

/// Error response for OIDC authentication failures
#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct OidcErrorResponse {
    /// Error code
    #[schema(example = "invalid_state")]
    pub error: String,

    /// Human-readable error description
    #[schema(example = "The authentication request has expired. Please try again.")]
    pub error_description: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_info_serialization() {
        let info = OidcProviderInfo {
            name: "authentik".to_string(),
            display_name: "Authentik SSO".to_string(),
            login_url: "/api/v1/auth/oidc/authentik/login".to_string(),
        };

        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("\"name\":\"authentik\""));
        assert!(json.contains("\"displayName\":\"Authentik SSO\""));
        assert!(json.contains("\"loginUrl\""));
    }

    #[test]
    fn test_providers_response_serialization() {
        let response = OidcProvidersResponse {
            enabled: true,
            providers: vec![OidcProviderInfo {
                name: "keycloak".to_string(),
                display_name: "Keycloak".to_string(),
                login_url: "/api/v1/auth/oidc/keycloak/login".to_string(),
            }],
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"enabled\":true"));
        assert!(json.contains("\"providers\":["));
    }

    #[test]
    fn test_login_response_serialization() {
        let response = OidcLoginResponse {
            redirect_url: "https://auth.example.com/authorize?client_id=abc".to_string(),
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(
            json.contains("\"redirectUrl\":\"https://auth.example.com/authorize?client_id=abc\"")
        );
    }

    #[test]
    fn test_callback_query_deserialization() {
        let json = r#"{"code":"abc123","state":"xyz789"}"#;
        let query: OidcCallbackQuery = serde_json::from_str(json).unwrap();

        assert_eq!(query.code, "abc123");
        assert_eq!(query.state, "xyz789");
        assert!(query.error.is_none());
    }

    #[test]
    fn test_callback_query_with_error() {
        let json = r#"{"code":"","state":"xyz789","error":"access_denied","error_description":"User cancelled"}"#;
        let query: OidcCallbackQuery = serde_json::from_str(json).unwrap();

        assert_eq!(query.error.as_deref(), Some("access_denied"));
        assert_eq!(query.error_description.as_deref(), Some("User cancelled"));
    }

    #[test]
    fn test_error_response_serialization() {
        let response = OidcErrorResponse {
            error: "invalid_state".to_string(),
            error_description: "The authentication request has expired.".to_string(),
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"error\":\"invalid_state\""));
        assert!(json.contains("\"errorDescription\":"));
    }
}
