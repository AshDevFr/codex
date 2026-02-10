//! Token Refresh Service for User Plugins
//!
//! Handles automatic refresh of OAuth tokens before they expire.
//! Called before plugin operations to ensure valid tokens are available.

use anyhow::{Result, anyhow};
use chrono::{Duration, Utc};
use sea_orm::DatabaseConnection;
use tracing::{debug, info, warn};

use crate::db::entities::user_plugins;
use crate::db::repositories::UserPluginsRepository;
use crate::services::plugin::protocol::OAuthConfig;

use super::oauth::OAuthTokenResponse;

/// Buffer time before expiry to trigger refresh (5 minutes)
const REFRESH_BUFFER_SECS: i64 = 300;

/// Maximum consecutive failures before circuit breaker trips.
/// After this many failures within the time window, refresh attempts
/// are skipped and the user is immediately asked to re-authenticate.
pub const CIRCUIT_BREAKER_FAILURE_THRESHOLD: i32 = 3;

/// Time window (in seconds) for the circuit breaker.
/// Only failures within this window count toward the threshold.
pub const CIRCUIT_BREAKER_WINDOW_SECS: i64 = 3600; // 1 hour

/// Refresh result
#[derive(Debug)]
pub enum RefreshResult {
    /// Token was refreshed successfully
    Refreshed { access_token: String },
    /// Token is still valid, no refresh needed
    StillValid,
    /// No refresh token available, user needs to re-authenticate
    ReauthRequired,
    /// Refresh failed with an error
    Failed(String),
}

/// Structured error from an OAuth token refresh attempt.
///
/// Classifies errors by their HTTP response and body content rather than
/// relying on fragile string matching.
#[derive(Debug)]
pub enum TokenRefreshError {
    /// The refresh token is invalid or revoked — user must re-authenticate.
    /// Triggered by HTTP 401, or HTTP 400 with `error` in
    /// `["invalid_grant", "invalid_client", "unauthorized_client"]`.
    ReauthRequired {
        status: u16,
        error_code: Option<String>,
        description: Option<String>,
    },
    /// Rate limited by the OAuth provider — retry later.
    RateLimited {
        status: u16,
        retry_after: Option<String>,
    },
    /// Temporary failure — may succeed on retry.
    Temporary {
        status: Option<u16>,
        message: String,
    },
    /// Network or transport error (no HTTP response).
    Network(String),
}

impl std::fmt::Display for TokenRefreshError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TokenRefreshError::ReauthRequired {
                status,
                error_code,
                description,
            } => {
                write!(f, "Re-authentication required (HTTP {})", status)?;
                if let Some(code) = error_code {
                    write!(f, ": {}", code)?;
                }
                if let Some(desc) = description {
                    write!(f, " — {}", desc)?;
                }
                Ok(())
            }
            TokenRefreshError::RateLimited {
                status,
                retry_after,
            } => {
                write!(f, "Rate limited (HTTP {})", status)?;
                if let Some(after) = retry_after {
                    write!(f, ", retry after: {}", after)?;
                }
                Ok(())
            }
            TokenRefreshError::Temporary { status, message } => {
                if let Some(s) = status {
                    write!(f, "Token refresh failed (HTTP {}): {}", s, message)
                } else {
                    write!(f, "Token refresh failed: {}", message)
                }
            }
            TokenRefreshError::Network(msg) => {
                write!(f, "Token refresh network error: {}", msg)
            }
        }
    }
}

impl std::error::Error for TokenRefreshError {}

/// OAuth error response body as defined by RFC 6749 §5.2.
#[derive(Debug, serde::Deserialize)]
struct OAuthErrorBody {
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    error_description: Option<String>,
}

/// OAuth error codes that indicate the refresh token is permanently invalid
/// and the user must re-authenticate.
const REAUTH_ERROR_CODES: &[&str] = &["invalid_grant", "invalid_client", "unauthorized_client"];

/// Check if a user plugin's OAuth token needs refreshing and perform the refresh if needed.
///
/// Returns `RefreshResult` indicating whether the token was refreshed, still valid,
/// or requires re-authentication.
pub async fn ensure_valid_token(
    db: &DatabaseConnection,
    user_plugin: &user_plugins::Model,
    oauth_config: &OAuthConfig,
    client_id: &str,
    client_secret: Option<&str>,
) -> Result<RefreshResult> {
    // Check if plugin has OAuth tokens
    if !user_plugin.has_oauth_tokens() {
        return Ok(RefreshResult::ReauthRequired);
    }

    // Check if token is still valid (with buffer)
    if !needs_refresh(user_plugin) {
        return Ok(RefreshResult::StillValid);
    }

    // Circuit breaker: skip refresh if too many recent failures
    if is_circuit_open(user_plugin) {
        warn!(
            user_plugin_id = %user_plugin.id,
            failure_count = user_plugin.failure_count,
            last_failure_at = ?user_plugin.last_failure_at,
            "Circuit breaker open: {} failures within window, requiring re-authentication",
            user_plugin.failure_count
        );
        return Ok(RefreshResult::ReauthRequired);
    }

    debug!(
        user_plugin_id = %user_plugin.id,
        expires_at = ?user_plugin.oauth_expires_at,
        "OAuth token needs refresh"
    );

    // Get the encrypted refresh token
    let refresh_token =
        match UserPluginsRepository::get_oauth_refresh_token(db, user_plugin.id).await {
            Ok(Some(token)) => token,
            Ok(None) => {
                warn!(
                    user_plugin_id = %user_plugin.id,
                    "No refresh token available, re-authentication required"
                );
                return Ok(RefreshResult::ReauthRequired);
            }
            Err(e) => {
                return Ok(RefreshResult::Failed(format!(
                    "Failed to get refresh token: {}",
                    e
                )));
            }
        };

    // Perform the token refresh
    match refresh_oauth_token(oauth_config, &refresh_token, client_id, client_secret).await {
        Ok(token_response) => {
            let expires_at = token_response
                .expires_in
                .map(|secs| Utc::now() + Duration::seconds(secs as i64));

            // Store new tokens
            UserPluginsRepository::update_oauth_tokens(
                db,
                user_plugin.id,
                &token_response.access_token,
                token_response.refresh_token.as_deref(),
                expires_at,
                token_response.scope.as_deref(),
            )
            .await
            .map_err(|e| anyhow!("Failed to store refreshed tokens: {}", e))?;

            // Record success
            let _ = UserPluginsRepository::record_success(db, user_plugin.id).await;

            info!(
                user_plugin_id = %user_plugin.id,
                expires_at = ?expires_at,
                "Successfully refreshed OAuth token"
            );

            Ok(RefreshResult::Refreshed {
                access_token: token_response.access_token,
            })
        }
        Err(refresh_err) => {
            warn!(
                user_plugin_id = %user_plugin.id,
                error = %refresh_err,
                "OAuth token refresh failed"
            );

            // Record failure
            let _ = UserPluginsRepository::record_failure(db, user_plugin.id).await;

            // Classify the error using structured matching
            match refresh_err {
                TokenRefreshError::ReauthRequired { .. } => Ok(RefreshResult::ReauthRequired),
                TokenRefreshError::RateLimited { .. } => {
                    Ok(RefreshResult::Failed(refresh_err.to_string()))
                }
                TokenRefreshError::Temporary { .. } | TokenRefreshError::Network(..) => {
                    Ok(RefreshResult::Failed(refresh_err.to_string()))
                }
            }
        }
    }
}

/// Check if the circuit breaker is open (too many recent failures).
///
/// Returns `true` if the user plugin has >= `CIRCUIT_BREAKER_FAILURE_THRESHOLD`
/// failures within the last `CIRCUIT_BREAKER_WINDOW_SECS`.
fn is_circuit_open(user_plugin: &user_plugins::Model) -> bool {
    if user_plugin.failure_count < CIRCUIT_BREAKER_FAILURE_THRESHOLD {
        return false;
    }

    // Check if the failures are within the time window
    match user_plugin.last_failure_at {
        Some(last_failure) => {
            let window = Duration::seconds(CIRCUIT_BREAKER_WINDOW_SECS);
            let cutoff = Utc::now() - window;
            last_failure >= cutoff
        }
        // No timestamp but high failure count — treat as open
        None => true,
    }
}

/// Check if the token needs refreshing based on expiry time
fn needs_refresh(user_plugin: &user_plugins::Model) -> bool {
    match user_plugin.oauth_expires_at {
        Some(expires_at) => {
            let buffer = Duration::seconds(REFRESH_BUFFER_SECS);
            Utc::now() + buffer >= expires_at
        }
        // No expiry set - assume token doesn't expire
        None => false,
    }
}

/// Classify an HTTP error response into a structured `TokenRefreshError`.
///
/// This replaces string matching with proper HTTP status code and
/// OAuth error body parsing.
fn classify_http_error(status: u16, body: &str) -> TokenRefreshError {
    // HTTP 401 always means re-auth required
    if status == 401 {
        let parsed = serde_json::from_str::<OAuthErrorBody>(body).ok();
        return TokenRefreshError::ReauthRequired {
            status,
            error_code: parsed.as_ref().and_then(|b| b.error.clone()),
            description: parsed.as_ref().and_then(|b| b.error_description.clone()),
        };
    }

    // HTTP 429 = rate limited
    if status == 429 {
        return TokenRefreshError::RateLimited {
            status,
            retry_after: None,
        };
    }

    // HTTP 400 — check for OAuth error codes that indicate permanent failure
    if status == 400
        && let Ok(error_body) = serde_json::from_str::<OAuthErrorBody>(body)
        && let Some(ref error_code) = error_body.error
        && REAUTH_ERROR_CODES.contains(&error_code.as_str())
    {
        return TokenRefreshError::ReauthRequired {
            status,
            error_code: Some(error_code.clone()),
            description: error_body.error_description,
        };
    }

    // All other errors are treated as temporary
    TokenRefreshError::Temporary {
        status: Some(status),
        message: if body.is_empty() {
            format!("HTTP {}", status)
        } else {
            format!("HTTP {}: {}", status, body)
        },
    }
}

/// Perform the actual token refresh HTTP request
async fn refresh_oauth_token(
    oauth_config: &OAuthConfig,
    refresh_token: &str,
    client_id: &str,
    client_secret: Option<&str>,
) -> std::result::Result<OAuthTokenResponse, TokenRefreshError> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| TokenRefreshError::Network(format!("Failed to create HTTP client: {}", e)))?;

    let mut params = vec![
        ("grant_type", "refresh_token"),
        ("refresh_token", refresh_token),
        ("client_id", client_id),
    ];

    let secret_string;
    if let Some(secret) = client_secret {
        secret_string = secret.to_string();
        params.push(("client_secret", &secret_string));
    }

    let response = client
        .post(&oauth_config.token_url)
        .form(&params)
        .send()
        .await
        .map_err(|e| {
            TokenRefreshError::Network(format!("Token refresh HTTP request failed: {}", e))
        })?;

    if !response.status().is_success() {
        let status = response.status().as_u16();
        let body = response
            .text()
            .await
            .unwrap_or_else(|_| "Unable to read response body".to_string());
        return Err(classify_http_error(status, &body));
    }

    let token_response: OAuthTokenResponse =
        response
            .json()
            .await
            .map_err(|e| TokenRefreshError::Temporary {
                status: None,
                message: format!("Failed to parse token refresh response: {}", e),
            })?;

    Ok(token_response)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use uuid::Uuid;

    fn create_test_user_plugin(
        expires_at: Option<chrono::DateTime<Utc>>,
        has_tokens: bool,
    ) -> user_plugins::Model {
        user_plugins::Model {
            id: Uuid::new_v4(),
            plugin_id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            credentials: None,
            config: serde_json::json!({}),
            oauth_access_token: if has_tokens {
                Some(vec![1, 2, 3])
            } else {
                None
            },
            oauth_refresh_token: if has_tokens {
                Some(vec![4, 5, 6])
            } else {
                None
            },
            oauth_expires_at: expires_at,
            oauth_scope: None,
            external_user_id: None,
            external_username: None,
            external_avatar_url: None,
            enabled: true,
            health_status: "healthy".to_string(),
            failure_count: 0,
            last_failure_at: None,
            last_success_at: None,
            last_sync_at: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    fn create_test_user_plugin_with_failures(
        expires_at: Option<chrono::DateTime<Utc>>,
        failure_count: i32,
        last_failure_at: Option<chrono::DateTime<Utc>>,
    ) -> user_plugins::Model {
        user_plugins::Model {
            failure_count,
            last_failure_at,
            ..create_test_user_plugin(expires_at, true)
        }
    }

    // =========================================================================
    // needs_refresh tests
    // =========================================================================

    #[test]
    fn test_needs_refresh_no_expiry() {
        let user_plugin = create_test_user_plugin(None, true);
        assert!(!needs_refresh(&user_plugin));
    }

    #[test]
    fn test_needs_refresh_far_future() {
        let user_plugin = create_test_user_plugin(Some(Utc::now() + Duration::hours(1)), true);
        assert!(!needs_refresh(&user_plugin));
    }

    #[test]
    fn test_needs_refresh_within_buffer() {
        // Token expires in 3 minutes, buffer is 5 minutes → needs refresh
        let user_plugin = create_test_user_plugin(Some(Utc::now() + Duration::minutes(3)), true);
        assert!(needs_refresh(&user_plugin));
    }

    #[test]
    fn test_needs_refresh_already_expired() {
        let user_plugin = create_test_user_plugin(Some(Utc::now() - Duration::minutes(5)), true);
        assert!(needs_refresh(&user_plugin));
    }

    #[test]
    fn test_needs_refresh_at_boundary() {
        // Token expires in exactly REFRESH_BUFFER_SECS → needs refresh
        let user_plugin = create_test_user_plugin(
            Some(Utc::now() + Duration::seconds(REFRESH_BUFFER_SECS)),
            true,
        );
        assert!(needs_refresh(&user_plugin));
    }

    // =========================================================================
    // classify_http_error tests
    // =========================================================================

    #[test]
    fn test_classify_http_401_reauth_required() {
        let err = classify_http_error(401, r#"{"error":"invalid_token"}"#);
        match err {
            TokenRefreshError::ReauthRequired {
                status, error_code, ..
            } => {
                assert_eq!(status, 401);
                assert_eq!(error_code.as_deref(), Some("invalid_token"));
            }
            other => panic!("Expected ReauthRequired, got: {:?}", other),
        }
    }

    #[test]
    fn test_classify_http_401_no_body() {
        let err = classify_http_error(401, "");
        match err {
            TokenRefreshError::ReauthRequired {
                status,
                error_code,
                description,
            } => {
                assert_eq!(status, 401);
                assert!(error_code.is_none());
                assert!(description.is_none());
            }
            other => panic!("Expected ReauthRequired, got: {:?}", other),
        }
    }

    #[test]
    fn test_classify_http_401_non_json_body() {
        let err = classify_http_error(401, "Unauthorized");
        match err {
            TokenRefreshError::ReauthRequired {
                status, error_code, ..
            } => {
                assert_eq!(status, 401);
                assert!(error_code.is_none());
            }
            other => panic!("Expected ReauthRequired, got: {:?}", other),
        }
    }

    #[test]
    fn test_classify_http_400_invalid_grant() {
        let err = classify_http_error(
            400,
            r#"{"error":"invalid_grant","error_description":"Token has been revoked"}"#,
        );
        match err {
            TokenRefreshError::ReauthRequired {
                status,
                error_code,
                description,
            } => {
                assert_eq!(status, 400);
                assert_eq!(error_code.as_deref(), Some("invalid_grant"));
                assert_eq!(description.as_deref(), Some("Token has been revoked"));
            }
            other => panic!("Expected ReauthRequired, got: {:?}", other),
        }
    }

    #[test]
    fn test_classify_http_400_invalid_client() {
        let err = classify_http_error(
            400,
            r#"{"error":"invalid_client","error_description":"Client not found"}"#,
        );
        match err {
            TokenRefreshError::ReauthRequired {
                status, error_code, ..
            } => {
                assert_eq!(status, 400);
                assert_eq!(error_code.as_deref(), Some("invalid_client"));
            }
            other => panic!("Expected ReauthRequired, got: {:?}", other),
        }
    }

    #[test]
    fn test_classify_http_400_unauthorized_client() {
        let err = classify_http_error(400, r#"{"error":"unauthorized_client"}"#);
        match err {
            TokenRefreshError::ReauthRequired {
                status, error_code, ..
            } => {
                assert_eq!(status, 400);
                assert_eq!(error_code.as_deref(), Some("unauthorized_client"));
            }
            other => panic!("Expected ReauthRequired, got: {:?}", other),
        }
    }

    #[test]
    fn test_classify_http_400_other_error_is_temporary() {
        let err = classify_http_error(
            400,
            r#"{"error":"invalid_request","error_description":"Missing parameter"}"#,
        );
        match err {
            TokenRefreshError::Temporary { status, message } => {
                assert_eq!(status, Some(400));
                assert!(message.contains("invalid_request"));
            }
            other => panic!("Expected Temporary, got: {:?}", other),
        }
    }

    #[test]
    fn test_classify_http_400_non_json_body() {
        let err = classify_http_error(400, "Bad Request");
        match err {
            TokenRefreshError::Temporary { status, message } => {
                assert_eq!(status, Some(400));
                assert!(message.contains("Bad Request"));
            }
            other => panic!("Expected Temporary, got: {:?}", other),
        }
    }

    #[test]
    fn test_classify_http_429_rate_limited() {
        let err = classify_http_error(429, "Too Many Requests");
        match err {
            TokenRefreshError::RateLimited { status, .. } => {
                assert_eq!(status, 429);
            }
            other => panic!("Expected RateLimited, got: {:?}", other),
        }
    }

    #[test]
    fn test_classify_http_500_temporary() {
        let err = classify_http_error(500, "Internal Server Error");
        match err {
            TokenRefreshError::Temporary { status, message } => {
                assert_eq!(status, Some(500));
                assert!(message.contains("Internal Server Error"));
            }
            other => panic!("Expected Temporary, got: {:?}", other),
        }
    }

    #[test]
    fn test_classify_http_503_temporary() {
        let err = classify_http_error(503, "");
        match err {
            TokenRefreshError::Temporary { status, message } => {
                assert_eq!(status, Some(503));
                assert_eq!(message, "HTTP 503");
            }
            other => panic!("Expected Temporary, got: {:?}", other),
        }
    }

    // =========================================================================
    // circuit breaker tests
    // =========================================================================

    #[test]
    fn test_circuit_open_below_threshold() {
        let user_plugin = create_test_user_plugin_with_failures(
            Some(Utc::now() + Duration::minutes(3)),
            2,
            Some(Utc::now()),
        );
        assert!(!is_circuit_open(&user_plugin));
    }

    #[test]
    fn test_circuit_open_at_threshold_recent_failure() {
        let user_plugin = create_test_user_plugin_with_failures(
            Some(Utc::now() + Duration::minutes(3)),
            CIRCUIT_BREAKER_FAILURE_THRESHOLD,
            Some(Utc::now()),
        );
        assert!(is_circuit_open(&user_plugin));
    }

    #[test]
    fn test_circuit_open_above_threshold_recent_failure() {
        let user_plugin = create_test_user_plugin_with_failures(
            Some(Utc::now() + Duration::minutes(3)),
            CIRCUIT_BREAKER_FAILURE_THRESHOLD + 5,
            Some(Utc::now()),
        );
        assert!(is_circuit_open(&user_plugin));
    }

    #[test]
    fn test_circuit_closed_old_failures() {
        // Failures happened 2 hours ago — outside the 1-hour window
        let user_plugin = create_test_user_plugin_with_failures(
            Some(Utc::now() + Duration::minutes(3)),
            CIRCUIT_BREAKER_FAILURE_THRESHOLD + 1,
            Some(Utc::now() - Duration::hours(2)),
        );
        assert!(!is_circuit_open(&user_plugin));
    }

    #[test]
    fn test_circuit_open_no_timestamp_high_count() {
        // High failure count but no timestamp — treat as open
        let user_plugin = create_test_user_plugin_with_failures(
            Some(Utc::now() + Duration::minutes(3)),
            CIRCUIT_BREAKER_FAILURE_THRESHOLD,
            None,
        );
        assert!(is_circuit_open(&user_plugin));
    }

    #[test]
    fn test_circuit_closed_zero_failures() {
        let user_plugin =
            create_test_user_plugin_with_failures(Some(Utc::now() + Duration::minutes(3)), 0, None);
        assert!(!is_circuit_open(&user_plugin));
    }

    // =========================================================================
    // TokenRefreshError Display tests
    // =========================================================================

    #[test]
    fn test_display_reauth_required() {
        let err = TokenRefreshError::ReauthRequired {
            status: 400,
            error_code: Some("invalid_grant".to_string()),
            description: Some("Token revoked".to_string()),
        };
        let s = err.to_string();
        assert!(s.contains("Re-authentication required"));
        assert!(s.contains("400"));
        assert!(s.contains("invalid_grant"));
        assert!(s.contains("Token revoked"));
    }

    #[test]
    fn test_display_rate_limited() {
        let err = TokenRefreshError::RateLimited {
            status: 429,
            retry_after: Some("60".to_string()),
        };
        let s = err.to_string();
        assert!(s.contains("Rate limited"));
        assert!(s.contains("429"));
        assert!(s.contains("retry after: 60"));
    }

    #[test]
    fn test_display_temporary() {
        let err = TokenRefreshError::Temporary {
            status: Some(500),
            message: "Internal error".to_string(),
        };
        let s = err.to_string();
        assert!(s.contains("500"));
        assert!(s.contains("Internal error"));
    }

    #[test]
    fn test_display_network() {
        let err = TokenRefreshError::Network("connection refused".to_string());
        let s = err.to_string();
        assert!(s.contains("network error"));
        assert!(s.contains("connection refused"));
    }
}
