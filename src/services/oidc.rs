//! OIDC Authentication Service
//!
//! Implements OpenID Connect (OIDC) authentication flows for integrating with
//! external identity providers like Authentik, Keycloak, or any OIDC-compliant IdP.
//!
//! ## Features
//!
//! - Provider discovery document fetching and caching
//! - Authorization URL generation with PKCE support
//! - Token exchange (authorization code for tokens)
//! - ID token validation
//! - UserInfo endpoint calls
//! - Group-to-role mapping
//!
//! ## Security
//!
//! - PKCE (Proof Key for Code Exchange) is always enabled
//! - CSRF protection via state parameter
//! - Nonce validation for ID token replay protection
//! - HTTP client configured to reject redirects (SSRF prevention)

use anyhow::{Context, Result, anyhow};
use chrono::{Duration, Utc};
use dashmap::DashMap;
use openidconnect::{
    AccessToken, AuthorizationCode, ClientId, ClientSecret, CsrfToken, IssuerUrl, Nonce,
    OAuth2TokenResponse, PkceCodeChallenge, PkceCodeVerifier, RedirectUrl, Scope, TokenResponse,
    core::{
        CoreAuthenticationFlow, CoreClient, CoreGenderClaim, CoreProviderMetadata,
        CoreTokenResponse, CoreUserInfoClaims,
    },
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, info, warn};

use crate::config::{OidcConfig, OidcDefaultRole, OidcProviderConfig};

/// Duration for discovery document cache (1 hour)
const DISCOVERY_CACHE_TTL_SECS: i64 = 3600;

/// Duration for pending auth state (5 minutes)
/// This is how long we wait for a user to complete authentication at the IdP
const AUTH_STATE_TTL_SECS: i64 = 300;

/// Result of an OIDC authentication flow
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct OidcAuthResult {
    /// The subject (sub) claim - unique identifier at the IdP
    pub subject: String,
    /// Email address from the IdP
    pub email: Option<String>,
    /// Username (preferred_username claim)
    pub username: Option<String>,
    /// Display name (name claim)
    pub display_name: Option<String>,
    /// Groups from the IdP
    pub groups: Vec<String>,
    /// Mapped Codex role based on group membership
    pub mapped_role: String,
    /// Access token (for potential UserInfo calls)
    pub access_token: String,
    /// When the access token expires
    pub token_expires_at: Option<chrono::DateTime<Utc>>,
}

/// Pending authentication state
#[derive(Debug, Clone)]
pub struct PendingAuth {
    /// PKCE code verifier (needed for token exchange)
    pub pkce_verifier: String,
    /// Nonce (for ID token validation)
    pub nonce: String,
    /// When this state was created
    pub created_at: chrono::DateTime<Utc>,
    /// Provider name this state is for
    pub provider_name: String,
}

/// Cached provider metadata (instead of caching the client directly,
/// we cache the discovery metadata and reconstruct clients as needed)
struct CachedDiscovery {
    metadata: CoreProviderMetadata,
    expires_at: chrono::DateTime<Utc>,
}

/// OIDC Service for handling authentication flows with external identity providers
///
/// This service manages:
/// - OIDC client configuration per provider
/// - Discovery document caching
/// - Authentication state management
/// - Token validation and claims extraction
/// - Group-to-role mapping
#[derive(Clone)]
pub struct OidcService {
    /// Configuration
    config: OidcConfig,
    /// Cached OIDC clients per provider (keyed by provider name)
    clients: Arc<DashMap<String, CachedDiscovery>>,
    /// Pending authentication states (keyed by CSRF state token)
    pending_states: Arc<DashMap<String, PendingAuth>>,
    /// Redirect URI for callbacks
    redirect_uri_base: String,
    /// Shared HTTP client for OIDC requests (reused across all providers)
    /// Uses the reqwest version from openidconnect/oauth2, not the top-level reqwest crate
    http_client: openidconnect::reqwest::Client,
}

impl OidcService {
    /// Create a new OIDC service
    ///
    /// # Arguments
    ///
    /// * `config` - OIDC configuration with provider settings
    /// * `redirect_uri_base` - Base URL for OAuth callbacks (e.g., "http://localhost:8080")
    pub fn new(config: OidcConfig, redirect_uri_base: String) -> Self {
        // Build HTTP client with redirect disabled (SSRF prevention)
        // Uses the reqwest version re-exported by openidconnect/oauth2
        let http_client = openidconnect::reqwest::Client::builder()
            .redirect(openidconnect::reqwest::redirect::Policy::none())
            .build()
            .expect("Failed to build OIDC HTTP client");

        Self {
            config,
            clients: Arc::new(DashMap::new()),
            pending_states: Arc::new(DashMap::new()),
            redirect_uri_base,
            http_client,
        }
    }

    /// Check if OIDC is enabled
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Check if auto-creation of users is enabled
    pub fn auto_create_users(&self) -> bool {
        self.config.auto_create_users
    }

    /// Get the default role for new OIDC users
    #[allow(dead_code)]
    pub fn default_role(&self) -> &OidcDefaultRole {
        &self.config.default_role
    }

    /// Get list of configured providers
    pub fn get_providers(&self) -> Vec<ProviderInfo> {
        self.config
            .providers
            .iter()
            .map(
                |(name, config): (&String, &OidcProviderConfig)| ProviderInfo {
                    name: name.clone(),
                    display_name: config.display_name.clone(),
                },
            )
            .collect()
    }

    /// Get provider configuration by name
    pub fn get_provider_config(&self, provider_name: &str) -> Option<&OidcProviderConfig> {
        self.config.providers.get(provider_name)
    }

    /// Resolve the client secret for a provider
    ///
    /// Supports both direct secrets and environment variable references.
    fn resolve_client_secret(&self, provider: &OidcProviderConfig) -> Option<String> {
        // First check direct client_secret
        if let Some(ref secret) = provider.client_secret {
            return Some(secret.clone());
        }

        // Then check client_secret_env
        if let Some(ref env_var) = provider.client_secret_env {
            return std::env::var(env_var).ok();
        }

        None
    }

    /// Build redirect URI for a provider
    fn build_redirect_uri(&self, provider_name: &str) -> String {
        format!(
            "{}/api/v1/auth/oidc/{}/callback",
            self.redirect_uri_base.trim_end_matches('/'),
            provider_name
        )
    }

    /// Get cached provider metadata, fetching if needed
    async fn get_provider_metadata(&self, provider_name: &str) -> Result<CoreProviderMetadata> {
        // Check cache first
        if let Some(cached) = self.clients.get(provider_name) {
            if cached.expires_at > Utc::now() {
                debug!(provider = %provider_name, "Using cached OIDC provider metadata");
                return Ok(cached.metadata.clone());
            }
            // Cache expired, remove it
            drop(cached);
            self.clients.remove(provider_name);
        }

        // Get provider configuration
        let provider_config = self
            .config
            .providers
            .get(provider_name)
            .ok_or_else(|| anyhow!("Unknown OIDC provider: {}", provider_name))?;

        info!(
            provider = %provider_name,
            issuer = %provider_config.issuer_url,
            "Fetching OIDC discovery document"
        );

        // Parse issuer URL
        let issuer_url =
            IssuerUrl::new(provider_config.issuer_url.clone()).context("Invalid issuer URL")?;

        // Fetch discovery document
        let provider_metadata = CoreProviderMetadata::discover_async(issuer_url, &self.http_client)
            .await
            .context("Failed to fetch OIDC discovery document")?;

        // Cache the metadata
        let cached = CachedDiscovery {
            metadata: provider_metadata.clone(),
            expires_at: Utc::now() + Duration::seconds(DISCOVERY_CACHE_TTL_SECS),
        };
        self.clients.insert(provider_name.to_string(), cached);

        debug!(provider = %provider_name, "OIDC provider metadata fetched and cached");
        Ok(provider_metadata)
    }

    /// Build an OIDC client for a provider from cached metadata
    ///
    /// This constructs the client with redirect URI set, using cached provider metadata.
    async fn build_client(
        &self,
        provider_name: &str,
    ) -> Result<
        CoreClient<
            openidconnect::EndpointSet,
            openidconnect::EndpointNotSet,
            openidconnect::EndpointNotSet,
            openidconnect::EndpointNotSet,
            openidconnect::EndpointMaybeSet,
            openidconnect::EndpointMaybeSet,
        >,
    > {
        let provider_metadata = self.get_provider_metadata(provider_name).await?;

        let provider_config = self
            .config
            .providers
            .get(provider_name)
            .ok_or_else(|| anyhow!("Unknown OIDC provider: {}", provider_name))?;

        // Resolve client secret
        let client_secret = self
            .resolve_client_secret(provider_config)
            .map(ClientSecret::new);

        // Build redirect URI
        let redirect_uri = RedirectUrl::new(self.build_redirect_uri(provider_name))
            .context("Invalid redirect URI")?;

        // Create client with redirect URI set
        let client = CoreClient::from_provider_metadata(
            provider_metadata,
            ClientId::new(provider_config.client_id.clone()),
            client_secret,
        )
        .set_redirect_uri(redirect_uri);

        Ok(client)
    }

    /// Generate an authorization URL to redirect the user to the IdP
    ///
    /// Returns the authorization URL and stores the state for later validation.
    ///
    /// # Arguments
    ///
    /// * `provider_name` - Name of the configured OIDC provider
    ///
    /// # Returns
    ///
    /// * `(url, state)` - The authorization URL to redirect to and the CSRF state token
    pub async fn generate_auth_url(&self, provider_name: &str) -> Result<(String, String)> {
        let provider_config = self
            .config
            .providers
            .get(provider_name)
            .ok_or_else(|| anyhow!("Unknown OIDC provider: {}", provider_name))?;

        let client = self.build_client(provider_name).await?;

        // Generate PKCE challenge
        let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();

        // Build authorization request
        let mut auth_request = client.authorize_url(
            CoreAuthenticationFlow::AuthorizationCode,
            CsrfToken::new_random,
            Nonce::new_random,
        );

        // Add configured scopes
        for scope in &provider_config.scopes {
            auth_request = auth_request.add_scope(Scope::new(scope.to_string()));
        }

        // Set PKCE challenge
        auth_request = auth_request.set_pkce_challenge(pkce_challenge);

        // Generate the URL
        let (auth_url, csrf_token, nonce) = auth_request.url();

        // Store pending auth state
        let pending = PendingAuth {
            pkce_verifier: pkce_verifier.secret().clone(),
            nonce: nonce.secret().clone(),
            created_at: Utc::now(),
            provider_name: provider_name.to_string(),
        };
        self.pending_states
            .insert(csrf_token.secret().clone(), pending);

        debug!(
            provider = %provider_name,
            state = %csrf_token.secret(),
            "Generated OIDC authorization URL"
        );

        Ok((auth_url.to_string(), csrf_token.secret().clone()))
    }

    /// Exchange an authorization code for tokens and extract user information
    ///
    /// This method:
    /// 1. Validates the CSRF state
    /// 2. Exchanges the authorization code for tokens using PKCE
    /// 3. Validates the ID token
    /// 4. Extracts claims from the ID token
    /// 5. Optionally calls the UserInfo endpoint for additional claims
    /// 6. Maps groups to a Codex role
    ///
    /// # Arguments
    ///
    /// * `provider_name` - Name of the OIDC provider
    /// * `code` - Authorization code from the callback
    /// * `state` - CSRF state token from the callback
    ///
    /// # Returns
    ///
    /// The authentication result with user information and mapped role.
    pub async fn exchange_code(
        &self,
        provider_name: &str,
        code: &str,
        state: &str,
    ) -> Result<OidcAuthResult> {
        // Validate and consume state
        let pending = self
            .pending_states
            .remove(state)
            .map(|(_, v)| v)
            .ok_or_else(|| anyhow!("Invalid or expired OIDC state"))?;

        // Verify state hasn't expired
        let state_age = Utc::now() - pending.created_at;
        if state_age.num_seconds() > AUTH_STATE_TTL_SECS {
            return Err(anyhow!("OIDC state has expired"));
        }

        // Verify provider matches
        if pending.provider_name != provider_name {
            warn!(
                expected = %pending.provider_name,
                actual = %provider_name,
                "OIDC provider mismatch"
            );
            return Err(anyhow!("Provider mismatch in callback"));
        }

        let provider_config = self
            .config
            .providers
            .get(provider_name)
            .ok_or_else(|| anyhow!("Unknown OIDC provider: {}", provider_name))?;

        let client = self.build_client(provider_name).await?;

        // Reconstruct PKCE verifier
        let pkce_verifier = PkceCodeVerifier::new(pending.pkce_verifier);

        // Exchange code for tokens
        let token_response: CoreTokenResponse = client
            .exchange_code(AuthorizationCode::new(code.to_string()))
            .context("Token endpoint not configured")?
            .set_pkce_verifier(pkce_verifier)
            .request_async(&self.http_client)
            .await
            .context("Failed to exchange authorization code")?;

        // Get ID token
        let id_token = token_response
            .id_token()
            .ok_or_else(|| anyhow!("No ID token in response"))?;

        // Validate ID token
        let id_token_verifier = client.id_token_verifier();
        let nonce = Nonce::new(pending.nonce);
        let claims = id_token
            .claims(&id_token_verifier, &nonce)
            .context("Failed to verify ID token")?;

        // Extract standard claims
        let subject = claims.subject().to_string();
        let email = claims.email().map(|e| e.as_str().to_string());
        let username = claims.preferred_username().map(|u| u.as_str().to_string());
        let display_name = claims
            .name()
            .and_then(|n| n.get(None))
            .map(|n| n.as_str().to_string());

        // Extract groups from the raw ID token (additional claims not in standard)
        // We need to decode the ID token to get custom claims like groups
        let groups = self.extract_groups_from_id_token(id_token, &provider_config.groups_claim);

        // Map groups to role
        let mapped_role = self.map_groups_to_role(&groups, provider_config);

        // Calculate token expiration
        let token_expires_at = token_response
            .expires_in()
            .map(|d| Utc::now() + Duration::seconds(d.as_secs() as i64));

        info!(
            provider = %provider_name,
            subject = %subject,
            email = ?email,
            mapped_role = %mapped_role,
            "OIDC authentication successful"
        );
        debug!(
            provider = %provider_name,
            groups = ?groups,
            groups_claim = %provider_config.groups_claim,
            "OIDC groups received from IdP"
        );

        Ok(OidcAuthResult {
            subject,
            email,
            username,
            display_name,
            groups,
            mapped_role,
            access_token: token_response.access_token().secret().clone(),
            token_expires_at,
        })
    }

    /// Extract groups from an ID token's raw claims
    ///
    /// The openidconnect crate's standard claims don't include groups, so we
    /// extract them by parsing the token's payload directly.
    fn extract_groups_from_id_token(
        &self,
        id_token: &openidconnect::IdToken<
            openidconnect::EmptyAdditionalClaims,
            CoreGenderClaim,
            openidconnect::core::CoreJweContentEncryptionAlgorithm,
            openidconnect::core::CoreJwsSigningAlgorithm,
        >,
        groups_claim: &str,
    ) -> Vec<String> {
        // The ID token is a JWT - we can get the raw token and decode the payload
        // to access non-standard claims
        let token_str = id_token.to_string();

        // JWT format: header.payload.signature
        let parts: Vec<&str> = token_str.split('.').collect();
        if parts.len() != 3 {
            return Vec::new();
        }

        // Decode the payload (base64url)
        use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
        let payload = match URL_SAFE_NO_PAD.decode(parts[1]) {
            Ok(p) => p,
            Err(_) => return Vec::new(),
        };

        // Parse as JSON
        let claims: HashMap<String, serde_json::Value> = match serde_json::from_slice(&payload) {
            Ok(c) => c,
            Err(_) => return Vec::new(),
        };

        // Extract groups from the specified claim
        self.extract_groups_from_claims(&claims, groups_claim)
    }

    /// Call the UserInfo endpoint to get additional claims
    ///
    /// This is optional but can provide additional user information that
    /// may not be included in the ID token.
    ///
    /// # Arguments
    ///
    /// * `provider_name` - Name of the OIDC provider
    /// * `access_token` - Access token from the token exchange
    #[allow(dead_code)]
    pub async fn get_user_info(
        &self,
        provider_name: &str,
        access_token: &str,
    ) -> Result<UserInfoResult> {
        // Validate provider exists (future: use for groups extraction from userinfo)
        let _provider_config = self
            .config
            .providers
            .get(provider_name)
            .ok_or_else(|| anyhow!("Unknown OIDC provider: {}", provider_name))?;

        let client = self.build_client(provider_name).await?;

        // Call UserInfo endpoint
        let userinfo: CoreUserInfoClaims = client
            .user_info(AccessToken::new(access_token.to_string()), None)?
            .request_async(&self.http_client)
            .await
            .context("Failed to fetch UserInfo")?;

        // Extract standard claims
        let subject = userinfo.subject().to_string();
        let email = userinfo.email().map(|e| e.as_str().to_string());
        let username = userinfo
            .preferred_username()
            .map(|u| u.as_str().to_string());
        let display_name = userinfo
            .name()
            .and_then(|n| n.get(None))
            .map(|n| n.as_str().to_string());

        // For groups, we need to make a raw HTTP request to the userinfo endpoint
        // to get non-standard claims. For now, we'll return empty groups since
        // groups should typically be in the ID token.
        // In a production implementation, you might want to call the userinfo
        // endpoint directly with reqwest to get the raw JSON response.
        let groups = Vec::new();

        Ok(UserInfoResult {
            subject,
            email,
            username,
            display_name,
            groups,
        })
    }

    /// Extract groups from claims (works with both ID token and UserInfo claims)
    fn extract_groups_from_claims(
        &self,
        claims: &HashMap<String, serde_json::Value>,
        groups_claim: &str,
    ) -> Vec<String> {
        claims
            .get(groups_claim)
            .and_then(|v| self.parse_groups_value(v))
            .unwrap_or_default()
    }

    /// Parse a JSON value as a list of groups
    fn parse_groups_value(&self, value: &serde_json::Value) -> Option<Vec<String>> {
        if let Some(arr) = value.as_array() {
            Some(
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect(),
            )
        } else {
            // Some IdPs return groups as a comma-separated string
            value
                .as_str()
                .map(|s| s.split(',').map(|s| s.trim().to_string()).collect())
        }
    }

    /// Map IdP groups to a Codex role
    ///
    /// Uses the provider's role_mapping configuration to map groups to roles.
    /// Returns the highest privilege role that matches, or the default role
    /// if no groups match.
    ///
    /// Role priority: admin > maintainer > reader
    pub fn map_groups_to_role(&self, groups: &[String], provider: &OidcProviderConfig) -> String {
        debug!(
            user_groups = ?groups,
            role_mapping = ?provider.role_mapping,
            "Mapping IdP groups to Codex role"
        );

        // Check for admin first (highest privilege)
        if let Some(admin_groups) = provider.role_mapping.get("admin")
            && groups.iter().any(|g| admin_groups.contains(g))
        {
            return "admin".to_string();
        }

        // Check for maintainer
        if let Some(maintainer_groups) = provider.role_mapping.get("maintainer")
            && groups.iter().any(|g| maintainer_groups.contains(g))
        {
            return "maintainer".to_string();
        }

        // Check for reader
        if let Some(reader_groups) = provider.role_mapping.get("reader")
            && groups.iter().any(|g| reader_groups.contains(g))
        {
            return "reader".to_string();
        }

        // Default role from config
        self.config.default_role.as_str().to_string()
    }

    /// Clean up expired pending authentication states
    ///
    /// Should be called periodically to prevent memory leaks.
    #[allow(dead_code)]
    pub fn cleanup_expired_states(&self) {
        let cutoff = Utc::now() - Duration::seconds(AUTH_STATE_TTL_SECS);
        let mut removed = 0;

        self.pending_states.retain(|_, pending| {
            let keep = pending.created_at > cutoff;
            if !keep {
                removed += 1;
            }
            keep
        });

        if removed > 0 {
            debug!(count = removed, "Cleaned up expired OIDC auth states");
        }
    }

    /// Invalidate the cached client for a provider
    ///
    /// This forces a re-fetch of the discovery document on the next request.
    #[allow(dead_code)]
    pub fn invalidate_client_cache(&self, provider_name: &str) {
        self.clients.remove(provider_name);
        debug!(provider = %provider_name, "Invalidated OIDC client cache");
    }

    /// Invalidate all cached clients
    #[allow(dead_code)]
    pub fn invalidate_all_caches(&self) {
        self.clients.clear();
        debug!("Invalidated all OIDC client caches");
    }

    /// Get the number of pending authentication states
    ///
    /// Useful for monitoring and debugging.
    #[cfg(test)]
    pub fn pending_state_count(&self) -> usize {
        self.pending_states.len()
    }

    /// Get the number of cached clients
    ///
    /// Useful for monitoring and debugging.
    #[cfg(test)]
    pub fn cached_client_count(&self) -> usize {
        self.clients.len()
    }
}

/// Information about an OIDC provider
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderInfo {
    /// Internal name of the provider (used in URLs)
    pub name: String,
    /// Display name shown to users
    pub display_name: String,
}

/// Result from the UserInfo endpoint
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct UserInfoResult {
    /// Subject (unique identifier at the IdP)
    pub subject: String,
    /// Email address
    pub email: Option<String>,
    /// Username (preferred_username)
    pub username: Option<String>,
    /// Display name
    pub display_name: Option<String>,
    /// Groups from the IdP
    pub groups: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_config() -> OidcConfig {
        let mut providers = HashMap::new();

        let mut role_mapping = HashMap::new();
        role_mapping.insert(
            "admin".to_string(),
            vec!["codex-admins".to_string(), "administrators".to_string()],
        );
        role_mapping.insert("maintainer".to_string(), vec!["codex-editors".to_string()]);
        role_mapping.insert(
            "reader".to_string(),
            vec!["codex-users".to_string(), "users".to_string()],
        );

        providers.insert(
            "test-provider".to_string(),
            OidcProviderConfig {
                display_name: "Test Provider".to_string(),
                issuer_url: "https://auth.example.com".to_string(),
                client_id: "test-client-id".to_string(),
                client_secret: Some("test-client-secret".to_string()),
                client_secret_env: None,
                scopes: vec!["email".to_string(), "profile".to_string()],
                role_mapping,
                groups_claim: "groups".to_string(),
                username_claim: "preferred_username".to_string(),
                email_claim: "email".to_string(),
            },
        );

        OidcConfig {
            enabled: true,
            auto_create_users: true,
            default_role: OidcDefaultRole::Reader,
            redirect_uri_base: None,
            providers,
        }
    }

    #[test]
    fn test_service_creation() {
        let config = create_test_config();
        let service = OidcService::new(config, "http://localhost:8080".to_string());

        assert!(service.is_enabled());
        assert!(service.auto_create_users());
        assert_eq!(service.default_role().as_str(), "reader");
    }

    #[test]
    fn test_get_providers() {
        let config = create_test_config();
        let service = OidcService::new(config, "http://localhost:8080".to_string());

        let providers = service.get_providers();
        assert_eq!(providers.len(), 1);
        assert_eq!(providers[0].name, "test-provider");
        assert_eq!(providers[0].display_name, "Test Provider");
    }

    #[test]
    fn test_get_provider_config() {
        let config = create_test_config();
        let service = OidcService::new(config, "http://localhost:8080".to_string());

        let provider = service.get_provider_config("test-provider");
        assert!(provider.is_some());
        assert_eq!(provider.unwrap().client_id, "test-client-id");

        let unknown = service.get_provider_config("unknown");
        assert!(unknown.is_none());
    }

    #[test]
    fn test_resolve_client_secret_direct() {
        let config = create_test_config();
        let service = OidcService::new(config.clone(), "http://localhost:8080".to_string());

        let provider = config.providers.get("test-provider").unwrap();
        let secret = service.resolve_client_secret(provider);
        assert_eq!(secret, Some("test-client-secret".to_string()));
    }

    #[test]
    fn test_resolve_client_secret_from_env() {
        // Create provider with env var reference
        let provider = OidcProviderConfig {
            display_name: "Env Test".to_string(),
            issuer_url: "https://auth.example.com".to_string(),
            client_id: "test".to_string(),
            client_secret: None,
            client_secret_env: Some("TEST_OIDC_SECRET_12345".to_string()),
            scopes: vec![],
            role_mapping: HashMap::new(),
            groups_claim: "groups".to_string(),
            username_claim: "preferred_username".to_string(),
            email_claim: "email".to_string(),
        };

        let config = create_test_config();
        let service = OidcService::new(config, "http://localhost:8080".to_string());

        // Without env var set, should return None
        let secret = service.resolve_client_secret(&provider);
        assert!(secret.is_none());
    }

    #[test]
    fn test_build_redirect_uri() {
        let config = create_test_config();
        let service = OidcService::new(config, "http://localhost:8080".to_string());

        let uri = service.build_redirect_uri("authentik");
        assert_eq!(
            uri,
            "http://localhost:8080/api/v1/auth/oidc/authentik/callback"
        );

        // Test with trailing slash
        let service2 = OidcService::new(create_test_config(), "http://localhost:8080/".to_string());
        let uri2 = service2.build_redirect_uri("keycloak");
        assert_eq!(
            uri2,
            "http://localhost:8080/api/v1/auth/oidc/keycloak/callback"
        );
    }

    #[test]
    fn test_map_groups_to_role_admin() {
        let config = create_test_config();
        let service = OidcService::new(config.clone(), "http://localhost:8080".to_string());
        let provider = config.providers.get("test-provider").unwrap();

        let groups = vec!["codex-admins".to_string(), "some-other-group".to_string()];
        let role = service.map_groups_to_role(&groups, provider);
        assert_eq!(role, "admin");

        let groups2 = vec!["administrators".to_string()];
        let role2 = service.map_groups_to_role(&groups2, provider);
        assert_eq!(role2, "admin");
    }

    #[test]
    fn test_map_groups_to_role_maintainer() {
        let config = create_test_config();
        let service = OidcService::new(config.clone(), "http://localhost:8080".to_string());
        let provider = config.providers.get("test-provider").unwrap();

        let groups = vec!["codex-editors".to_string(), "some-group".to_string()];
        let role = service.map_groups_to_role(&groups, provider);
        assert_eq!(role, "maintainer");
    }

    #[test]
    fn test_map_groups_to_role_reader() {
        let config = create_test_config();
        let service = OidcService::new(config.clone(), "http://localhost:8080".to_string());
        let provider = config.providers.get("test-provider").unwrap();

        let groups = vec!["codex-users".to_string()];
        let role = service.map_groups_to_role(&groups, provider);
        assert_eq!(role, "reader");

        let groups2 = vec!["users".to_string()];
        let role2 = service.map_groups_to_role(&groups2, provider);
        assert_eq!(role2, "reader");
    }

    #[test]
    fn test_map_groups_to_role_default() {
        let config = create_test_config();
        let service = OidcService::new(config.clone(), "http://localhost:8080".to_string());
        let provider = config.providers.get("test-provider").unwrap();

        // No matching groups
        let groups = vec!["unknown-group".to_string()];
        let role = service.map_groups_to_role(&groups, provider);
        assert_eq!(role, "reader"); // Default role

        // Empty groups
        let empty: Vec<String> = vec![];
        let role2 = service.map_groups_to_role(&empty, provider);
        assert_eq!(role2, "reader");
    }

    #[test]
    fn test_map_groups_to_role_priority() {
        let config = create_test_config();
        let service = OidcService::new(config.clone(), "http://localhost:8080".to_string());
        let provider = config.providers.get("test-provider").unwrap();

        // User has both admin and reader groups - should get admin
        let groups = vec!["codex-users".to_string(), "codex-admins".to_string()];
        let role = service.map_groups_to_role(&groups, provider);
        assert_eq!(role, "admin");

        // User has both maintainer and reader - should get maintainer
        let groups2 = vec!["codex-users".to_string(), "codex-editors".to_string()];
        let role2 = service.map_groups_to_role(&groups2, provider);
        assert_eq!(role2, "maintainer");
    }

    #[test]
    fn test_extract_groups_from_claims_array() {
        let config = create_test_config();
        let service = OidcService::new(config, "http://localhost:8080".to_string());

        let mut claims = HashMap::new();
        claims.insert(
            "groups".to_string(),
            serde_json::json!(["group1", "group2", "group3"]),
        );

        let groups = service.extract_groups_from_claims(&claims, "groups");
        assert_eq!(groups, vec!["group1", "group2", "group3"]);
    }

    #[test]
    fn test_extract_groups_from_claims_string() {
        let config = create_test_config();
        let service = OidcService::new(config, "http://localhost:8080".to_string());

        let mut claims = HashMap::new();
        claims.insert(
            "groups".to_string(),
            serde_json::json!("group1, group2, group3"),
        );

        let groups = service.extract_groups_from_claims(&claims, "groups");
        assert_eq!(groups, vec!["group1", "group2", "group3"]);
    }

    #[test]
    fn test_extract_groups_from_claims_missing() {
        let config = create_test_config();
        let service = OidcService::new(config, "http://localhost:8080".to_string());

        let claims = HashMap::new();
        let groups = service.extract_groups_from_claims(&claims, "groups");
        assert!(groups.is_empty());
    }

    #[test]
    fn test_extract_groups_custom_claim_name() {
        let config = create_test_config();
        let service = OidcService::new(config, "http://localhost:8080".to_string());

        let mut claims = HashMap::new();
        claims.insert(
            "custom_groups".to_string(),
            serde_json::json!(["custom1", "custom2"]),
        );

        let groups = service.extract_groups_from_claims(&claims, "custom_groups");
        assert_eq!(groups, vec!["custom1", "custom2"]);
    }

    #[test]
    fn test_cleanup_expired_states() {
        let config = create_test_config();
        let service = OidcService::new(config, "http://localhost:8080".to_string());

        // Add an expired state
        let expired_state = PendingAuth {
            pkce_verifier: "verifier".to_string(),
            nonce: "nonce".to_string(),
            created_at: Utc::now() - Duration::seconds(AUTH_STATE_TTL_SECS + 100),
            provider_name: "test".to_string(),
        };
        service
            .pending_states
            .insert("expired".to_string(), expired_state);

        // Add a valid state
        let valid_state = PendingAuth {
            pkce_verifier: "verifier2".to_string(),
            nonce: "nonce2".to_string(),
            created_at: Utc::now(),
            provider_name: "test".to_string(),
        };
        service
            .pending_states
            .insert("valid".to_string(), valid_state);

        assert_eq!(service.pending_state_count(), 2);

        // Cleanup
        service.cleanup_expired_states();

        assert_eq!(service.pending_state_count(), 1);
        assert!(service.pending_states.contains_key("valid"));
        assert!(!service.pending_states.contains_key("expired"));
    }

    #[test]
    fn test_invalidate_caches() {
        let config = create_test_config();
        let service = OidcService::new(config, "http://localhost:8080".to_string());

        assert_eq!(service.cached_client_count(), 0);

        service.invalidate_client_cache("test-provider");
        service.invalidate_all_caches();

        assert_eq!(service.cached_client_count(), 0);
    }

    #[test]
    fn test_disabled_oidc() {
        let mut config = create_test_config();
        config.enabled = false;

        let service = OidcService::new(config, "http://localhost:8080".to_string());

        assert!(!service.is_enabled());
    }

    #[test]
    fn test_different_default_roles() {
        // Test with admin default
        let mut config = create_test_config();
        config.default_role = OidcDefaultRole::Admin;
        let service = OidcService::new(config.clone(), "http://localhost:8080".to_string());
        let provider = config.providers.get("test-provider").unwrap();

        let groups: Vec<String> = vec![];
        let role = service.map_groups_to_role(&groups, provider);
        assert_eq!(role, "admin");

        // Test with maintainer default
        let mut config2 = create_test_config();
        config2.default_role = OidcDefaultRole::Maintainer;
        let service2 = OidcService::new(config2.clone(), "http://localhost:8080".to_string());
        let provider2 = config2.providers.get("test-provider").unwrap();

        let role2 = service2.map_groups_to_role(&groups, provider2);
        assert_eq!(role2, "maintainer");
    }

    #[test]
    fn test_parse_groups_value_array() {
        let config = create_test_config();
        let service = OidcService::new(config, "http://localhost:8080".to_string());

        let value = serde_json::json!(["a", "b", "c"]);
        let groups = service.parse_groups_value(&value);
        assert_eq!(
            groups,
            Some(vec!["a".to_string(), "b".to_string(), "c".to_string()])
        );
    }

    #[test]
    fn test_parse_groups_value_string() {
        let config = create_test_config();
        let service = OidcService::new(config, "http://localhost:8080".to_string());

        let value = serde_json::json!("a, b, c");
        let groups = service.parse_groups_value(&value);
        assert_eq!(
            groups,
            Some(vec!["a".to_string(), "b".to_string(), "c".to_string()])
        );
    }

    #[test]
    fn test_parse_groups_value_invalid() {
        let config = create_test_config();
        let service = OidcService::new(config, "http://localhost:8080".to_string());

        let value = serde_json::json!(123);
        let groups = service.parse_groups_value(&value);
        assert!(groups.is_none());

        let value2 = serde_json::json!({"nested": "object"});
        let groups2 = service.parse_groups_value(&value2);
        assert!(groups2.is_none());
    }

    #[test]
    fn test_provider_info_serialization() {
        let info = ProviderInfo {
            name: "authentik".to_string(),
            display_name: "Authentik SSO".to_string(),
        };

        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("authentik"));
        assert!(json.contains("Authentik SSO"));

        let deserialized: ProviderInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.name, "authentik");
        assert_eq!(deserialized.display_name, "Authentik SSO");
    }

    #[test]
    fn test_extract_groups_from_claims_nested_objects_ignored() {
        let config = create_test_config();
        let service = OidcService::new(config, "http://localhost:8080".to_string());

        let mut claims = HashMap::new();
        // Groups as nested objects (invalid format)
        claims.insert(
            "groups".to_string(),
            serde_json::json!([{"name": "admin"}, {"name": "users"}]),
        );

        // Non-string array items should be filtered out
        let groups = service.extract_groups_from_claims(&claims, "groups");
        assert!(groups.is_empty());
    }

    #[test]
    fn test_extract_groups_from_claims_mixed_array() {
        let config = create_test_config();
        let service = OidcService::new(config, "http://localhost:8080".to_string());

        let mut claims = HashMap::new();
        // Mix of strings and non-strings
        claims.insert(
            "groups".to_string(),
            serde_json::json!(["admin", 42, "users", null, true]),
        );

        let groups = service.extract_groups_from_claims(&claims, "groups");
        assert_eq!(groups, vec!["admin", "users"]);
    }

    #[test]
    fn test_parse_groups_value_empty_string() {
        let config = create_test_config();
        let service = OidcService::new(config, "http://localhost:8080".to_string());

        let value = serde_json::json!("");
        let groups = service.parse_groups_value(&value);
        // Empty string split by comma gives one empty element
        assert_eq!(groups, Some(vec!["".to_string()]));
    }

    #[test]
    fn test_parse_groups_value_single_group_string() {
        let config = create_test_config();
        let service = OidcService::new(config, "http://localhost:8080".to_string());

        let value = serde_json::json!("single-group");
        let groups = service.parse_groups_value(&value);
        assert_eq!(groups, Some(vec!["single-group".to_string()]));
    }

    #[test]
    fn test_parse_groups_value_empty_array() {
        let config = create_test_config();
        let service = OidcService::new(config, "http://localhost:8080".to_string());

        let value = serde_json::json!([]);
        let groups = service.parse_groups_value(&value);
        assert_eq!(groups, Some(vec![]));
    }

    #[test]
    fn test_parse_groups_value_boolean() {
        let config = create_test_config();
        let service = OidcService::new(config, "http://localhost:8080".to_string());

        let value = serde_json::json!(true);
        let groups = service.parse_groups_value(&value);
        assert!(groups.is_none());
    }

    #[test]
    fn test_parse_groups_value_null() {
        let config = create_test_config();
        let service = OidcService::new(config, "http://localhost:8080".to_string());

        let value = serde_json::json!(null);
        let groups = service.parse_groups_value(&value);
        assert!(groups.is_none());
    }

    #[test]
    fn test_multiple_providers_listing() {
        let mut providers = HashMap::new();
        providers.insert(
            "provider-a".to_string(),
            OidcProviderConfig {
                display_name: "Provider A".to_string(),
                issuer_url: "https://a.example.com".to_string(),
                client_id: "a".to_string(),
                client_secret: None,
                client_secret_env: None,
                scopes: vec![],
                role_mapping: HashMap::new(),
                groups_claim: "groups".to_string(),
                username_claim: "preferred_username".to_string(),
                email_claim: "email".to_string(),
            },
        );
        providers.insert(
            "provider-b".to_string(),
            OidcProviderConfig {
                display_name: "Provider B".to_string(),
                issuer_url: "https://b.example.com".to_string(),
                client_id: "b".to_string(),
                client_secret: None,
                client_secret_env: None,
                scopes: vec![],
                role_mapping: HashMap::new(),
                groups_claim: "groups".to_string(),
                username_claim: "preferred_username".to_string(),
                email_claim: "email".to_string(),
            },
        );

        let config = OidcConfig {
            enabled: true,
            auto_create_users: true,
            default_role: OidcDefaultRole::Reader,
            redirect_uri_base: None,
            providers,
        };

        let service = OidcService::new(config, "http://localhost:8080".to_string());
        let providers = service.get_providers();
        assert_eq!(providers.len(), 2);

        let names: Vec<&str> = providers.iter().map(|p| p.name.as_str()).collect();
        assert!(names.contains(&"provider-a"));
        assert!(names.contains(&"provider-b"));
    }

    #[test]
    fn test_cleanup_only_removes_expired_states() {
        let config = create_test_config();
        let service = OidcService::new(config, "http://localhost:8080".to_string());

        // Add multiple states with different ages
        for i in 0..5 {
            let state = PendingAuth {
                pkce_verifier: format!("verifier_{}", i),
                nonce: format!("nonce_{}", i),
                created_at: Utc::now() - Duration::seconds(AUTH_STATE_TTL_SECS + (i * 10) as i64),
                provider_name: "test".to_string(),
            };
            service
                .pending_states
                .insert(format!("expired_{}", i), state);
        }

        for i in 0..3 {
            let state = PendingAuth {
                pkce_verifier: format!("verifier_valid_{}", i),
                nonce: format!("nonce_valid_{}", i),
                created_at: Utc::now(),
                provider_name: "test".to_string(),
            };
            service.pending_states.insert(format!("valid_{}", i), state);
        }

        assert_eq!(service.pending_state_count(), 8);

        service.cleanup_expired_states();

        // Only the 3 valid states should remain
        assert_eq!(service.pending_state_count(), 3);
        for i in 0..3 {
            assert!(service.pending_states.contains_key(&format!("valid_{}", i)));
        }
    }

    #[test]
    fn test_resolve_client_secret_prefers_direct_over_env() {
        let config = create_test_config();
        let service = OidcService::new(config, "http://localhost:8080".to_string());

        // Provider with both direct secret and env var
        let provider = OidcProviderConfig {
            display_name: "Both".to_string(),
            issuer_url: "https://auth.example.com".to_string(),
            client_id: "test".to_string(),
            client_secret: Some("direct-secret".to_string()),
            client_secret_env: Some("SOME_ENV_VAR".to_string()),
            scopes: vec![],
            role_mapping: HashMap::new(),
            groups_claim: "groups".to_string(),
            username_claim: "preferred_username".to_string(),
            email_claim: "email".to_string(),
        };

        let secret = service.resolve_client_secret(&provider);
        assert_eq!(secret, Some("direct-secret".to_string()));
    }

    #[test]
    fn test_resolve_client_secret_no_secret() {
        let config = create_test_config();
        let service = OidcService::new(config, "http://localhost:8080".to_string());

        // Provider with no secret at all (public client)
        let provider = OidcProviderConfig {
            display_name: "Public".to_string(),
            issuer_url: "https://auth.example.com".to_string(),
            client_id: "test".to_string(),
            client_secret: None,
            client_secret_env: None,
            scopes: vec![],
            role_mapping: HashMap::new(),
            groups_claim: "groups".to_string(),
            username_claim: "preferred_username".to_string(),
            email_claim: "email".to_string(),
        };

        let secret = service.resolve_client_secret(&provider);
        assert!(secret.is_none());
    }

    #[test]
    fn test_build_redirect_uri_various_bases() {
        let config = create_test_config();

        // Standard base
        let service = OidcService::new(config.clone(), "http://localhost:8080".to_string());
        assert_eq!(
            service.build_redirect_uri("my-provider"),
            "http://localhost:8080/api/v1/auth/oidc/my-provider/callback"
        );

        // HTTPS with path prefix
        let service = OidcService::new(config.clone(), "https://codex.example.com".to_string());
        assert_eq!(
            service.build_redirect_uri("auth0"),
            "https://codex.example.com/api/v1/auth/oidc/auth0/callback"
        );

        // With trailing slash (should be stripped)
        let service = OidcService::new(config.clone(), "https://codex.example.com/".to_string());
        assert_eq!(
            service.build_redirect_uri("test"),
            "https://codex.example.com/api/v1/auth/oidc/test/callback"
        );

        // With port
        let service = OidcService::new(config, "http://192.168.1.100:3000".to_string());
        assert_eq!(
            service.build_redirect_uri("local"),
            "http://192.168.1.100:3000/api/v1/auth/oidc/local/callback"
        );
    }
}
