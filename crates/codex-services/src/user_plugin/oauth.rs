//! OAuth 2.0 State Management for User Plugins
//!
//! Handles CSRF protection via state parameter, PKCE challenge generation,
//! and authorization URL construction for plugin OAuth flows.

use anyhow::{Result, anyhow};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::{DateTime, Duration, Utc};
use codex_db::repositories::{NewUserPluginOAuthState, UserPluginOAuthStateRepository};
use rand::Rng;
use sea_orm::DatabaseConnection;
use serde::Deserialize;
use tracing::{debug, warn};
use uuid::Uuid;

use crate::plugin::protocol::OAuthConfig;

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
    /// The redirect URI used in the authorization request (must match in token exchange)
    pub redirect_uri: String,
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

/// OAuth state manager for tracking pending OAuth flows.
///
/// Pending flows live in the database rather than in this process. The
/// authorization request and the callback are separate HTTP requests, so a
/// deployment running more than one `codex serve` behind a load balancer
/// without session affinity routes them to different processes, and an
/// in-process map would fail the flow every time.
#[derive(Clone)]
pub struct OAuthStateManager {
    db: DatabaseConnection,
}

impl OAuthStateManager {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    /// Generate a cryptographically random state parameter
    fn generate_state() -> String {
        let mut bytes = [0u8; 32];
        rand::rng().fill_bytes(&mut bytes);
        URL_SAFE_NO_PAD.encode(bytes)
    }

    /// Generate a PKCE code verifier and challenge
    fn generate_pkce() -> (String, String) {
        // Generate 32 bytes of random data for code verifier
        let mut verifier_bytes = [0u8; 32];
        rand::rng().fill_bytes(&mut verifier_bytes);
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
    pub async fn start_oauth_flow(
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
        let created_at = Utc::now();
        UserPluginOAuthStateRepository::create(
            &self.db,
            NewUserPluginOAuthState {
                state: state.clone(),
                plugin_id,
                user_id,
                pkce_verifier,
                pkce_challenge,
                redirect_uri: redirect_uri.to_string(),
                created_at,
                expires_at: created_at + Duration::seconds(OAUTH_STATE_TTL_SECS),
            },
        )
        .await?;

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
    pub async fn validate_state(&self, state: &str) -> Result<PendingOAuthFlow> {
        let row = UserPluginOAuthStateRepository::consume(&self.db, state)
            .await?
            .ok_or_else(|| anyhow!("Invalid or expired OAuth state parameter"))?;

        let pending = PendingOAuthFlow {
            plugin_id: row.plugin_id,
            user_id: row.user_id,
            pkce_verifier: row.pkce_verifier,
            pkce_challenge: row.pkce_challenge,
            redirect_uri: row.redirect_uri,
            created_at: row.created_at,
        };

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

    /// Clean up expired pending flows. Returns the number removed.
    pub async fn cleanup_expired(&self) -> Result<u64> {
        let removed = UserPluginOAuthStateRepository::delete_expired(&self.db).await?;
        if removed > 0 {
            debug!(removed, "Cleaned up expired OAuth flows");
        }
        Ok(removed)
    }

    /// Get the total number of pending flows (used in tests and monitoring)
    pub async fn pending_count(&self) -> Result<u64> {
        UserPluginOAuthStateRepository::count(&self.db).await
    }

    /// Get the number of unexpired flows for a specific user (for rate-limiting).
    ///
    /// Expiry is applied in the query rather than by sweeping first, so an
    /// abandoned flow stops counting against the user the moment it expires
    /// instead of whenever the next sweep happens to run.
    pub async fn pending_count_for_user(&self, user_id: Uuid) -> Result<u64> {
        UserPluginOAuthStateRepository::count_live_for_user(&self.db, user_id).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_db::entities::{plugins, users};
    use codex_db::repositories::{PluginsRepository, UserRepository};
    use codex_db::test_helpers::setup_test_db;
    use serde_json::json;

    /// A manager plus the rows its foreign keys require.
    struct Fixture {
        db: DatabaseConnection,
        manager: OAuthStateManager,
        plugin: plugins::Model,
        user: users::Model,
    }

    impl Fixture {
        async fn new() -> Self {
            let db = setup_test_db().await;
            let plugin = create_test_plugin(&db).await;
            let user = create_test_user(&db).await;
            Self {
                manager: OAuthStateManager::new(db.clone()),
                db,
                plugin,
                user,
            }
        }

        /// A second manager over the same database, standing in for a second
        /// replica: separate process state, one shared store.
        fn other_replica(&self) -> OAuthStateManager {
            OAuthStateManager::new(self.db.clone())
        }

        async fn another_user(&self) -> users::Model {
            create_test_user(&self.db).await
        }

        async fn start(&self, user_id: Uuid, config: &OAuthConfig) -> (String, String) {
            self.manager
                .start_oauth_flow(
                    self.plugin.id,
                    user_id,
                    config,
                    "client-id",
                    "https://codex.local/callback",
                )
                .await
                .unwrap()
        }

        /// Persist a flow that is already past its TTL. Used instead of
        /// backdating a live row, which the in-memory implementation allowed
        /// but a stored `expires_at` does not.
        async fn insert_expired(&self, user_id: Uuid) -> String {
            let state = OAuthStateManager::generate_state();
            let created_at = Utc::now() - Duration::seconds(OAUTH_STATE_TTL_SECS + 60);
            UserPluginOAuthStateRepository::create(
                &self.db,
                NewUserPluginOAuthState {
                    state: state.clone(),
                    plugin_id: self.plugin.id,
                    user_id,
                    pkce_verifier: None,
                    pkce_challenge: None,
                    redirect_uri: "https://codex.local/callback".to_string(),
                    created_at,
                    expires_at: created_at + Duration::seconds(OAUTH_STATE_TTL_SECS),
                },
            )
            .await
            .unwrap();
            state
        }
    }

    async fn create_test_user(db: &DatabaseConnection) -> users::Model {
        let now = Utc::now();
        let user = users::Model {
            id: Uuid::new_v4(),
            username: format!("u-{}", Uuid::new_v4()),
            email: format!("{}@example.com", Uuid::new_v4()),
            password_hash: "h".to_string(),
            role: "reader".to_string(),
            is_active: true,
            email_verified: true,
            permissions: json!([]),
            created_at: now,
            updated_at: now,
            last_login_at: None,
        };
        UserRepository::create(db, &user).await.unwrap()
    }

    async fn create_test_plugin(db: &DatabaseConnection) -> plugins::Model {
        PluginsRepository::create(
            db,
            &format!("oauth_plugin_{}", Uuid::new_v4()),
            "OAuth Test Plugin",
            Some("A test plugin"),
            "user",
            "node",
            vec!["index.js".to_string()],
            vec![],
            None,
            vec![],
            vec![],
            vec![],
            None,
            "env",
            None,
            true,
            None,
            None,
            None,
            None, // log_level
        )
        .await
        .unwrap()
    }

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

    #[tokio::test]
    async fn test_start_oauth_flow() {
        let fx = Fixture::new().await;
        let config = test_oauth_config();

        let (auth_url, state) = fx.start(fx.user.id, &config).await;

        // Auth URL should contain required parameters
        assert!(auth_url.starts_with("https://example.com/oauth/authorize?"));
        assert!(auth_url.contains("response_type=code"));
        assert!(auth_url.contains("client_id=client-id"));
        assert!(auth_url.contains("redirect_uri="));
        assert!(auth_url.contains("state="));
        assert!(auth_url.contains("scope=read") && auth_url.contains("write"));
        assert!(auth_url.contains("code_challenge="));
        assert!(auth_url.contains("code_challenge_method=S256"));

        // State should be stored
        assert_eq!(fx.manager.pending_count().await.unwrap(), 1);

        // State should be non-empty
        assert!(!state.is_empty());
    }

    #[tokio::test]
    async fn test_start_oauth_flow_without_pkce() {
        let fx = Fixture::new().await;
        let mut config = test_oauth_config();
        config.pkce = false;
        config.scopes = vec![];

        let (auth_url, _) = fx.start(fx.user.id, &config).await;

        // Should NOT contain PKCE parameters
        assert!(!auth_url.contains("code_challenge"));
        assert!(!auth_url.contains("code_challenge_method"));

        // Should NOT contain scope parameter (empty scopes)
        assert!(!auth_url.contains("scope="));
    }

    #[tokio::test]
    async fn test_validate_state_success() {
        let fx = Fixture::new().await;
        let config = test_oauth_config();

        let (_, state) = fx.start(fx.user.id, &config).await;

        // Validate should succeed
        let pending = fx.manager.validate_state(&state).await.unwrap();
        assert_eq!(pending.plugin_id, fx.plugin.id);
        assert_eq!(pending.user_id, fx.user.id);
        assert!(pending.pkce_verifier.is_some());

        // State should be consumed (removed)
        assert_eq!(fx.manager.pending_count().await.unwrap(), 0);
    }

    /// A plugin OAuth flow begun on one process has to be completable on
    /// another. The authorization request and the callback are separate HTTP
    /// requests, so a deployment running more than one `codex serve` behind a
    /// load balancer routes them to different processes.
    #[tokio::test]
    async fn test_pending_flow_is_visible_to_another_process() {
        let fx = Fixture::new().await;
        let config = test_oauth_config();

        let (_, state) = fx.start(fx.user.id, &config).await;

        let pending = fx
            .other_replica()
            .validate_state(&state)
            .await
            .expect("a flow started by one process must be consumable by another");
        assert_eq!(pending.plugin_id, fx.plugin.id);
        assert_eq!(pending.user_id, fx.user.id);
        assert_eq!(pending.redirect_uri, "https://codex.local/callback");
    }

    /// Consuming on one replica must consume everywhere, or the CSRF token is
    /// single-use only against the process that happens to serve the callback.
    #[tokio::test]
    async fn test_state_consumed_on_one_process_is_gone_on_the_other() {
        let fx = Fixture::new().await;
        let config = test_oauth_config();

        let (_, state) = fx.start(fx.user.id, &config).await;

        assert!(fx.manager.validate_state(&state).await.is_ok());
        assert!(
            fx.other_replica().validate_state(&state).await.is_err(),
            "a consumed state must not be redeemable on another replica"
        );
    }

    #[tokio::test]
    async fn test_validate_state_invalid() {
        let fx = Fixture::new().await;

        // Should fail for unknown state
        assert!(fx.manager.validate_state("nonexistent").await.is_err());
    }

    #[tokio::test]
    async fn test_validate_state_consumed() {
        let fx = Fixture::new().await;
        let config = test_oauth_config();

        let (_, state) = fx.start(fx.user.id, &config).await;

        // First validation should succeed
        assert!(fx.manager.validate_state(&state).await.is_ok());

        // Second validation should fail (state consumed)
        assert!(fx.manager.validate_state(&state).await.is_err());
    }

    /// An expired flow is rejected even when the sweep has not run yet.
    #[tokio::test]
    async fn test_validate_state_rejects_expired_flow() {
        let fx = Fixture::new().await;
        let expired = fx.insert_expired(fx.user.id).await;

        let err = fx
            .manager
            .validate_state(&expired)
            .await
            .expect_err("an expired flow must not validate");
        assert!(
            err.to_string().contains("expired"),
            "the error should say the flow expired, got: {err}"
        );
    }

    #[tokio::test]
    async fn test_cleanup_expired() {
        let fx = Fixture::new().await;
        let config = test_oauth_config();

        // Create a flow
        fx.start(fx.user.id, &config).await;

        assert_eq!(fx.manager.pending_count().await.unwrap(), 1);

        // Cleanup should not remove fresh flows
        let removed = fx.manager.cleanup_expired().await.unwrap();
        assert_eq!(removed, 0);
        assert_eq!(fx.manager.pending_count().await.unwrap(), 1);
    }

    /// The sweep must reach flows this process never created, which is the
    /// whole reason it moved off an in-process map: in production the sweep
    /// runs in the worker while the flows are started in serve.
    #[tokio::test]
    async fn test_cleanup_reaches_flows_started_by_another_process() {
        let fx = Fixture::new().await;
        let _ = fx.insert_expired(fx.user.id).await;

        let removed = fx.other_replica().cleanup_expired().await.unwrap();

        assert_eq!(removed, 1);
        assert_eq!(fx.manager.pending_count().await.unwrap(), 0);
    }

    #[tokio::test]
    async fn test_multiple_flows() {
        let fx = Fixture::new().await;
        let config = test_oauth_config();

        // Start multiple flows
        let (_, state1) = fx.start(fx.user.id, &config).await;
        let (_, state2) = fx.start(fx.user.id, &config).await;

        assert_eq!(fx.manager.pending_count().await.unwrap(), 2);

        // States should be different
        assert_ne!(state1, state2);

        // Each should validate independently
        assert!(fx.manager.validate_state(&state1).await.is_ok());
        assert_eq!(fx.manager.pending_count().await.unwrap(), 1);
        assert!(fx.manager.validate_state(&state2).await.is_ok());
        assert_eq!(fx.manager.pending_count().await.unwrap(), 0);
    }

    #[tokio::test]
    async fn test_pending_count_for_user() {
        let fx = Fixture::new().await;
        let config = test_oauth_config();
        let user_b = fx.another_user().await;

        // Start flows for the fixture user
        fx.start(fx.user.id, &config).await;
        fx.start(fx.user.id, &config).await;

        // Start a flow for user_b
        fx.start(user_b.id, &config).await;

        assert_eq!(fx.manager.pending_count().await.unwrap(), 3);
        assert_eq!(
            fx.manager.pending_count_for_user(fx.user.id).await.unwrap(),
            2
        );
        assert_eq!(
            fx.manager.pending_count_for_user(user_b.id).await.unwrap(),
            1
        );
        assert_eq!(
            fx.manager
                .pending_count_for_user(Uuid::new_v4())
                .await
                .unwrap(),
            0
        );
    }

    #[tokio::test]
    async fn test_pending_count_for_user_excludes_expired_flows() {
        let fx = Fixture::new().await;

        for _ in 0..3 {
            let _ = fx.insert_expired(fx.user.id).await;
        }

        // The rows are still present, but none of them is live, so none of
        // them counts against the user's concurrent-flow limit.
        assert_eq!(fx.manager.pending_count().await.unwrap(), 3);
        assert_eq!(
            fx.manager.pending_count_for_user(fx.user.id).await.unwrap(),
            0
        );
    }

    #[tokio::test]
    async fn test_expired_flows_do_not_block_new_flows() {
        let fx = Fixture::new().await;
        let config = test_oauth_config();

        // Three expired flows, which would hit the typical max-3 limit if they
        // still counted.
        for _ in 0..3 {
            let _ = fx.insert_expired(fx.user.id).await;
        }

        assert_eq!(
            fx.manager.pending_count_for_user(fx.user.id).await.unwrap(),
            0
        );

        // Should be able to start a new flow (not blocked by expired ones)
        fx.start(fx.user.id, &config).await;
        assert_eq!(
            fx.manager.pending_count_for_user(fx.user.id).await.unwrap(),
            1
        );
    }
}
