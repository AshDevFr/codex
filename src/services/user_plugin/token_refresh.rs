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
        Err(e) => {
            warn!(
                user_plugin_id = %user_plugin.id,
                error = %e,
                "OAuth token refresh failed"
            );

            // Record failure
            let _ = UserPluginsRepository::record_failure(db, user_plugin.id).await;

            // Check if the error indicates invalid refresh token
            let err_str = e.to_string();
            if err_str.contains("invalid_grant") || err_str.contains("401") {
                return Ok(RefreshResult::ReauthRequired);
            }

            Ok(RefreshResult::Failed(err_str))
        }
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

/// Perform the actual token refresh HTTP request
async fn refresh_oauth_token(
    oauth_config: &OAuthConfig,
    refresh_token: &str,
    client_id: &str,
    client_secret: Option<&str>,
) -> Result<OAuthTokenResponse> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| anyhow!("Failed to create HTTP client: {}", e))?;

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
        .map_err(|e| anyhow!("Token refresh HTTP request failed: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response
            .text()
            .await
            .unwrap_or_else(|_| "Unable to read response body".to_string());
        return Err(anyhow!(
            "Token refresh failed with status {}: {}",
            status,
            body
        ));
    }

    let token_response: OAuthTokenResponse = response
        .json()
        .await
        .map_err(|e| anyhow!("Failed to parse token refresh response: {}", e))?;

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
}
