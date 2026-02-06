//! OAuth 2.0 State Management for User Plugins
//!
//! Handles CSRF protection via state parameter, PKCE challenge generation,
//! and authorization URL construction for plugin OAuth flows.

use anyhow::{Result, anyhow};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::{DateTime, Duration, Utc};
use dashmap::DashMap;
use rand::RngCore;
use serde::Deserialize;
use std::sync::Arc;
use tracing::{debug, warn};
use uuid::Uuid;

use crate::services::plugin::protocol::OAuthConfig;

/// Duration for pending OAuth state (5 minutes)
const OAUTH_STATE_TTL_SECS: i64 = 300;

/// Pending OAuth flow state
#[derive(Debug, Clone)]
pub struct PendingOAuthFlow {
    /// Plugin ID this OAuth flow is for
    pub plugin_id: Uuid,
    /// User ID who initiated the flow
    pub user_id: Uuid,
    /// PKCE code verifier (needed for token exchange)
    pub pkce_verifier: Option<String>,
    /// PKCE code challenge (sent in auth URL, kept for debugging/logging)
    #[allow(dead_code)]
    pub pkce_challenge: Option<String>,
    /// When this state was created
    pub created_at: DateTime<Utc>,
}

/// OAuth token response from the token endpoint
#[derive(Debug, Clone, Deserialize)]
pub struct OAuthTokenResponse {
    pub access_token: String,
    #[serde(default)]
    pub refresh_token: Option<String>,
    #[serde(default)]
    pub expires_in: Option<u64>,
    #[serde(default)]
    #[allow(dead_code)]
    pub token_type: Option<String>,
    #[serde(default)]
    pub scope: Option<String>,
}

/// Result of a completed OAuth flow
#[derive(Debug, Clone)]
pub struct OAuthResult {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_at: Option<DateTime<Utc>>,
    pub scope: Option<String>,
}

/// OAuth state manager for tracking pending OAuth flows
#[derive(Clone)]
pub struct OAuthStateManager {
    /// Map of state parameter -> pending flow
    pending_flows: Arc<DashMap<String, PendingOAuthFlow>>,
}

impl OAuthStateManager {
    pub fn new() -> Self {
        Self {
            pending_flows: Arc::new(DashMap::new()),
        }
    }

    /// Generate a cryptographically random state parameter
    fn generate_state() -> String {
        let mut bytes = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut bytes);
        URL_SAFE_NO_PAD.encode(bytes)
    }

    /// Generate a PKCE code verifier and challenge
    fn generate_pkce() -> (String, String) {
        // Generate 32 bytes of random data for code verifier
        let mut verifier_bytes = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut verifier_bytes);
        let verifier = URL_SAFE_NO_PAD.encode(verifier_bytes);

        // S256 challenge: BASE64URL(SHA256(verifier))
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(verifier.as_bytes());
        let challenge = URL_SAFE_NO_PAD.encode(hasher.finalize());

        (verifier, challenge)
    }

    /// Build the authorization URL for a plugin's OAuth flow
    ///
    /// Returns (authorization_url, state_token)
    pub fn start_oauth_flow(
        &self,
        plugin_id: Uuid,
        user_id: Uuid,
        oauth_config: &OAuthConfig,
        client_id: &str,
        redirect_uri: &str,
    ) -> Result<(String, String)> {
        // Generate state for CSRF protection
        let state = Self::generate_state();

        // Generate PKCE if enabled
        let (pkce_verifier, pkce_challenge) = if oauth_config.pkce {
            let (v, c) = Self::generate_pkce();
            (Some(v), Some(c))
        } else {
            (None, None)
        };

        // Build authorization URL
        let mut auth_url = format!(
            "{}?response_type=code&client_id={}&redirect_uri={}&state={}",
            oauth_config.authorization_url,
            urlencoding::encode(client_id),
            urlencoding::encode(redirect_uri),
            urlencoding::encode(&state),
        );

        // Add scopes if present
        if !oauth_config.scopes.is_empty() {
            auth_url.push_str(&format!(
                "&scope={}",
                urlencoding::encode(&oauth_config.scopes.join(" "))
            ));
        }

        // Add PKCE challenge if enabled
        if let Some(ref challenge) = pkce_challenge {
            auth_url.push_str(&format!(
                "&code_challenge={}&code_challenge_method=S256",
                urlencoding::encode(challenge)
            ));
        }

        // Store pending flow
        let pending = PendingOAuthFlow {
            plugin_id,
            user_id,
            pkce_verifier,
            pkce_challenge,
            created_at: Utc::now(),
        };

        self.pending_flows.insert(state.clone(), pending);

        debug!(
            plugin_id = %plugin_id,
            user_id = %user_id,
            "Started OAuth flow with state"
        );

        Ok((auth_url, state))
    }

    /// Validate and consume a state parameter, returning the pending flow
    ///
    /// This is called during the OAuth callback to verify CSRF protection
    pub fn validate_state(&self, state: &str) -> Result<PendingOAuthFlow> {
        let (_, pending) = self
            .pending_flows
            .remove(state)
            .ok_or_else(|| anyhow!("Invalid or expired OAuth state parameter"))?;

        // Check TTL
        let age = Utc::now().signed_duration_since(pending.created_at);
        if age > Duration::seconds(OAUTH_STATE_TTL_SECS) {
            warn!(
                plugin_id = %pending.plugin_id,
                user_id = %pending.user_id,
                age_secs = age.num_seconds(),
                "OAuth state expired"
            );
            return Err(anyhow!(
                "OAuth state expired ({}s > {}s)",
                age.num_seconds(),
                OAUTH_STATE_TTL_SECS
            ));
        }

        Ok(pending)
    }

    /// Exchange an authorization code for tokens
    pub async fn exchange_code(
        &self,
        oauth_config: &OAuthConfig,
        code: &str,
        client_id: &str,
        client_secret: Option<&str>,
        redirect_uri: &str,
        pkce_verifier: Option<&str>,
    ) -> Result<OAuthResult> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| anyhow!("Failed to create HTTP client: {}", e))?;

        let mut params = vec![
            ("grant_type", "authorization_code"),
            ("code", code),
            ("client_id", client_id),
            ("redirect_uri", redirect_uri),
        ];

        // Add client_secret if present
        let secret_string;
        if let Some(secret) = client_secret {
            secret_string = secret.to_string();
            params.push(("client_secret", &secret_string));
        }

        // Add PKCE verifier if present
        let verifier_string;
        if let Some(verifier) = pkce_verifier {
            verifier_string = verifier.to_string();
            params.push(("code_verifier", &verifier_string));
        }

        let response = client
            .post(&oauth_config.token_url)
            .form(&params)
            .send()
            .await
            .map_err(|e| anyhow!("Token exchange HTTP request failed: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "Unable to read response body".to_string());
            return Err(anyhow!(
                "Token exchange failed with status {}: {}",
                status,
                body
            ));
        }

        let token_response: OAuthTokenResponse = response
            .json()
            .await
            .map_err(|e| anyhow!("Failed to parse token response: {}", e))?;

        let expires_at = token_response
            .expires_in
            .map(|secs| Utc::now() + Duration::seconds(secs as i64));

        Ok(OAuthResult {
            access_token: token_response.access_token,
            refresh_token: token_response.refresh_token,
            expires_at,
            scope: token_response.scope,
        })
    }

    /// Clean up expired pending flows
    #[allow(dead_code)]
    pub fn cleanup_expired(&self) -> usize {
        let now = Utc::now();
        let ttl = Duration::seconds(OAUTH_STATE_TTL_SECS);
        let mut removed = 0;

        self.pending_flows.retain(|_, flow| {
            let expired = now.signed_duration_since(flow.created_at) > ttl;
            if expired {
                removed += 1;
            }
            !expired
        });

        if removed > 0 {
            debug!(removed, "Cleaned up expired OAuth flows");
        }

        removed
    }

    /// Get the number of pending flows (for testing/monitoring)
    #[allow(dead_code)]
    pub fn pending_count(&self) -> usize {
        self.pending_flows.len()
    }
}

impl Default for OAuthStateManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_oauth_config() -> OAuthConfig {
        OAuthConfig {
            authorization_url: "https://example.com/oauth/authorize".to_string(),
            token_url: "https://example.com/oauth/token".to_string(),
            scopes: vec!["read".to_string(), "write".to_string()],
            pkce: true,
            user_info_url: None,
            client_id: None,
        }
    }

    #[test]
    fn test_generate_state() {
        let state1 = OAuthStateManager::generate_state();
        let state2 = OAuthStateManager::generate_state();

        // States should be non-empty
        assert!(!state1.is_empty());
        assert!(!state2.is_empty());

        // States should be different
        assert_ne!(state1, state2);

        // Should be base64url encoded (43 chars for 32 bytes)
        assert_eq!(state1.len(), 43);
    }

    #[test]
    fn test_generate_pkce() {
        let (verifier, challenge) = OAuthStateManager::generate_pkce();

        // Both should be non-empty
        assert!(!verifier.is_empty());
        assert!(!challenge.is_empty());

        // Verifier should be base64url encoded (43 chars for 32 bytes)
        assert_eq!(verifier.len(), 43);

        // Challenge should be base64url encoded SHA256 (43 chars for 32 bytes)
        assert_eq!(challenge.len(), 43);

        // Challenge should be deterministic for a given verifier
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(verifier.as_bytes());
        let expected_challenge = URL_SAFE_NO_PAD.encode(hasher.finalize());
        assert_eq!(challenge, expected_challenge);
    }

    #[test]
    fn test_start_oauth_flow() {
        let manager = OAuthStateManager::new();
        let config = test_oauth_config();
        let plugin_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();

        let (auth_url, state) = manager
            .start_oauth_flow(
                plugin_id,
                user_id,
                &config,
                "my-client-id",
                "https://codex.local/api/v1/user/plugins/oauth/callback",
            )
            .unwrap();

        // Auth URL should contain required parameters
        assert!(auth_url.starts_with("https://example.com/oauth/authorize?"));
        assert!(auth_url.contains("response_type=code"));
        assert!(auth_url.contains("client_id=my-client-id"));
        assert!(auth_url.contains("redirect_uri="));
        assert!(auth_url.contains("state="));
        assert!(auth_url.contains("scope=read") && auth_url.contains("write"));
        assert!(auth_url.contains("code_challenge="));
        assert!(auth_url.contains("code_challenge_method=S256"));

        // State should be stored
        assert_eq!(manager.pending_count(), 1);

        // State should be non-empty
        assert!(!state.is_empty());
    }

    #[test]
    fn test_start_oauth_flow_without_pkce() {
        let manager = OAuthStateManager::new();
        let mut config = test_oauth_config();
        config.pkce = false;
        config.scopes = vec![];
        let plugin_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();

        let (auth_url, _) = manager
            .start_oauth_flow(
                plugin_id,
                user_id,
                &config,
                "my-client-id",
                "https://codex.local/callback",
            )
            .unwrap();

        // Should NOT contain PKCE parameters
        assert!(!auth_url.contains("code_challenge"));
        assert!(!auth_url.contains("code_challenge_method"));

        // Should NOT contain scope parameter (empty scopes)
        assert!(!auth_url.contains("scope="));
    }

    #[test]
    fn test_validate_state_success() {
        let manager = OAuthStateManager::new();
        let config = test_oauth_config();
        let plugin_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();

        let (_, state) = manager
            .start_oauth_flow(
                plugin_id,
                user_id,
                &config,
                "client-id",
                "https://codex.local/callback",
            )
            .unwrap();

        // Validate should succeed
        let pending = manager.validate_state(&state).unwrap();
        assert_eq!(pending.plugin_id, plugin_id);
        assert_eq!(pending.user_id, user_id);
        assert!(pending.pkce_verifier.is_some());

        // State should be consumed (removed)
        assert_eq!(manager.pending_count(), 0);
    }

    #[test]
    fn test_validate_state_invalid() {
        let manager = OAuthStateManager::new();

        // Should fail for unknown state
        assert!(manager.validate_state("nonexistent").is_err());
    }

    #[test]
    fn test_validate_state_consumed() {
        let manager = OAuthStateManager::new();
        let config = test_oauth_config();

        let (_, state) = manager
            .start_oauth_flow(
                Uuid::new_v4(),
                Uuid::new_v4(),
                &config,
                "client-id",
                "https://codex.local/callback",
            )
            .unwrap();

        // First validation should succeed
        assert!(manager.validate_state(&state).is_ok());

        // Second validation should fail (state consumed)
        assert!(manager.validate_state(&state).is_err());
    }

    #[test]
    fn test_cleanup_expired() {
        let manager = OAuthStateManager::new();
        let config = test_oauth_config();

        // Create a flow
        manager
            .start_oauth_flow(
                Uuid::new_v4(),
                Uuid::new_v4(),
                &config,
                "client-id",
                "https://codex.local/callback",
            )
            .unwrap();

        assert_eq!(manager.pending_count(), 1);

        // Cleanup should not remove fresh flows
        let removed = manager.cleanup_expired();
        assert_eq!(removed, 0);
        assert_eq!(manager.pending_count(), 1);
    }

    #[test]
    fn test_multiple_flows() {
        let manager = OAuthStateManager::new();
        let config = test_oauth_config();

        // Start multiple flows
        let (_, state1) = manager
            .start_oauth_flow(
                Uuid::new_v4(),
                Uuid::new_v4(),
                &config,
                "client-id",
                "https://codex.local/callback",
            )
            .unwrap();

        let (_, state2) = manager
            .start_oauth_flow(
                Uuid::new_v4(),
                Uuid::new_v4(),
                &config,
                "client-id",
                "https://codex.local/callback",
            )
            .unwrap();

        assert_eq!(manager.pending_count(), 2);

        // States should be different
        assert_ne!(state1, state2);

        // Each should validate independently
        assert!(manager.validate_state(&state1).is_ok());
        assert_eq!(manager.pending_count(), 1);
        assert!(manager.validate_state(&state2).is_ok());
        assert_eq!(manager.pending_count(), 0);
    }
}
