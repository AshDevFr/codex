//! IdP-issued bearer token validation: the OAuth2 resource-server half of
//! Codex's OIDC support. Turns a raw `Authorization: Bearer` JWT from a
//! configured identity provider into a verified `(provider_name, subject)`
//! pair, or a precise rejection reason.
//!
//! ## Security model
//!
//! - **Asymmetric algorithms only** ([`ALLOWED_ALGS`]). Codex's own session
//!   JWTs are HS256 and verified elsewhere with the local secret; HS* is
//!   never valid on this path, closing the classic key-confusion attack
//!   where an attacker signs a token with public key material.
//! - **Provider selection by unverified `iss` peek.** The issuer claim is
//!   read without verification only to pick which provider's JWKS and
//!   audience rules apply; nothing is trusted until the signature verifies
//!   and the issuer is re-checked as part of validation.
//! - **Audience enforcement.** The token's `aud` must match the provider's
//!   accepted list (`accepted_audiences`, defaulting to the provider's own
//!   `client_id`). Tokens without an `aud` claim are rejected.
//! - **No auto-provisioning.** This module only proves token authenticity.
//!   Mapping the validated identity to a Codex user happens upstream, and
//!   only against existing `oidc_connections` rows: a valid IdP token for
//!   an unlinked identity must not create an account.
//!
//! JWKS material is fetched lazily from the provider's discovery document
//! and cached; see [`JwksCache`] for the refresh and fail-closed rules.
//! Errors never include token material.

use std::time::Duration;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use jsonwebtoken::jwk::{Jwk, JwkSet};
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode, decode_header};
use serde::Deserialize;
use tokio::sync::Mutex;
use tokio::time::Instant;

use codex_config::OidcConfig;

/// Algorithms accepted on IdP-issued bearer tokens. Symmetric algorithms
/// are deliberately absent: there is no shared secret in this trust model.
const ALLOWED_ALGS: &[Algorithm] = &[Algorithm::RS256, Algorithm::ES256];

/// Age at which a cached JWKS is refreshed on next use. Matches the 1h
/// discovery cache TTL used by the web-login flow.
const JWKS_REFRESH_AFTER: Duration = Duration::from_secs(3600);

/// Minimum spacing between refetches triggered by unknown-`kid` misses, so
/// a stream of garbage kids cannot turn into IdP load.
const MISS_REFETCH_MIN_INTERVAL: Duration = Duration::from_secs(60);

/// Leeway applied to `exp`/`nbf`, absorbing small clock skew between the
/// IdP and Codex.
const CLOCK_SKEW_LEEWAY_SECS: u64 = 30;

/// Validation failure reasons. Variants are deliberately fine-grained so
/// the auth extractor can log a precise cause; none carry token material.
#[derive(Debug, thiserror::Error)]
pub enum IdpBearerError {
    // -- IdP-side failures (the caller's token may be fine). --
    #[error("could not fetch the IdP discovery document")]
    Discovery(#[source] reqwest::Error),
    #[error("could not fetch the IdP JWKS")]
    Jwks(#[source] reqwest::Error),
    #[error(
        "discovery document issuer `{found}` does not match the configured issuer `{expected}`"
    )]
    IssuerMismatch { expected: String, found: String },

    // -- Token failures (distinct traced reasons). --
    #[error("malformed token header")]
    BadHeader(#[source] jsonwebtoken::errors::Error),
    #[error("malformed token payload")]
    BadPayload,
    #[error("unsupported signing algorithm")]
    UnsupportedAlgorithm,
    #[error("token issuer does not match any configured provider")]
    UnknownIssuer,
    #[error("no JWKS key matches the token's key id")]
    UnknownKey,
    #[error("the matched JWKS key is unusable")]
    BadKey(#[source] jsonwebtoken::errors::Error),
    #[error("wrong issuer")]
    WrongIssuer,
    #[error("wrong audience")]
    WrongAudience,
    #[error("token expired")]
    Expired,
    #[error("token not yet valid")]
    Immature,
    #[error("signature verification failed")]
    BadSignature,
    #[error("token missing required claim `{0}`")]
    MissingClaim(String),
    #[error("token has no usable subject claim")]
    MissingSubject,
    #[error("token rejected")]
    Invalid(#[source] jsonwebtoken::errors::Error),
}

impl IdpBearerError {
    /// IdP-side failures the caller cannot fix by changing their token;
    /// everything else is a rejection of the presented credential.
    pub fn is_backend(&self) -> bool {
        matches!(
            self,
            IdpBearerError::Discovery(_)
                | IdpBearerError::Jwks(_)
                | IdpBearerError::IssuerMismatch { .. }
        )
    }
}

/// A successfully validated IdP bearer token, reduced to the identity pair
/// the caller resolves against `oidc_connections`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedIdpToken {
    /// Configured provider name (the `oidc_connections.provider` value).
    pub provider_name: String,
    /// The token's `sub` claim: the user's unique identifier at the IdP.
    pub subject: String,
}

/// The slice of the OIDC discovery document this module consumes.
#[derive(Debug, Clone, Deserialize)]
struct Discovery {
    issuer: String,
    jwks_uri: String,
}

#[derive(Default)]
struct KeyState {
    jwks: Option<JwkSet>,
    fetched_at: Option<Instant>,
    last_miss_refetch: Option<Instant>,
}

/// Lazy, per-provider JWKS cache.
///
/// Keys are fetched on first use (an IdP outage degrades bearer auth
/// instead of failing startup) and refreshed two ways: a periodic refresh
/// once the set is older than [`JWKS_REFRESH_AFTER`], and a single
/// miss-triggered refetch when a token names an unknown `kid` (key
/// rotation). Miss refetches are rate-limited by
/// [`MISS_REFETCH_MIN_INTERVAL`]; after the one refetch the lookup fails
/// closed. A refresh failure serves the previous (stale) set rather than
/// taking down auth.
struct JwksCache {
    /// Configured issuer, without the trailing slash.
    issuer: String,
    discovery: Mutex<Option<Discovery>>,
    keys: Mutex<KeyState>,
}

impl JwksCache {
    fn new(issuer_url: &str) -> Self {
        Self {
            issuer: issuer_url.trim_end_matches('/').to_owned(),
            discovery: Mutex::new(None),
            keys: Mutex::new(KeyState::default()),
        }
    }

    /// The discovery document, fetched once per process and then served
    /// from memory (endpoints do not move within an IdP deployment).
    async fn discovery(&self, http: &reqwest::Client) -> Result<Discovery, IdpBearerError> {
        let mut cached = self.discovery.lock().await;
        if let Some(doc) = cached.as_ref() {
            return Ok(doc.clone());
        }

        let url = format!("{}/.well-known/openid-configuration", self.issuer);
        let doc: Discovery = http
            .get(&url)
            .send()
            .await
            .and_then(reqwest::Response::error_for_status)
            .map_err(IdpBearerError::Discovery)?
            .json()
            .await
            .map_err(IdpBearerError::Discovery)?;

        // The discovery issuer authenticates the document (OIDC Discovery
        // §4.3); a mismatch means the configured URL points at something
        // other than the issuer it claims to serve.
        if doc.issuer.trim_end_matches('/') != self.issuer {
            return Err(IdpBearerError::IssuerMismatch {
                expected: self.issuer.clone(),
                found: doc.issuer.clone(),
            });
        }

        *cached = Some(doc.clone());
        Ok(doc)
    }

    /// The decoding key for `kid`, refetching the JWKS once on an unknown
    /// `kid` (rate-limited), then failing closed.
    async fn decoding_key(
        &self,
        http: &reqwest::Client,
        kid: Option<&str>,
    ) -> Result<DecodingKey, IdpBearerError> {
        let mut state = self.keys.lock().await;

        let stale = state
            .fetched_at
            .is_none_or(|at| at.elapsed() >= JWKS_REFRESH_AFTER);
        if stale {
            match self.refetch(http, &mut state).await {
                Ok(()) => {}
                // Serve from the stale set if we have one; only a cold
                // cache propagates the fetch failure.
                Err(err) if state.jwks.is_none() => return Err(err),
                Err(err) => {
                    tracing::warn!(
                        issuer = %self.issuer,
                        error = %err,
                        "JWKS refresh failed; serving stale key set"
                    );
                }
            }
        }

        if let Some(jwk) = Self::find(state.jwks.as_ref(), kid) {
            return DecodingKey::from_jwk(jwk).map_err(IdpBearerError::BadKey);
        }

        // Unknown kid: one refetch covers key rotation; the rate limit
        // keeps garbage kids from hammering the IdP.
        let may_refetch = state
            .last_miss_refetch
            .is_none_or(|at| at.elapsed() >= MISS_REFETCH_MIN_INTERVAL);
        if may_refetch {
            state.last_miss_refetch = Some(Instant::now());
            self.refetch(http, &mut state).await?;
            if let Some(jwk) = Self::find(state.jwks.as_ref(), kid) {
                return DecodingKey::from_jwk(jwk).map_err(IdpBearerError::BadKey);
            }
        }

        Err(IdpBearerError::UnknownKey)
    }

    /// A `kid` match, or the sole key of a single-key set when the token
    /// header carries no `kid` at all.
    fn find<'a>(jwks: Option<&'a JwkSet>, kid: Option<&str>) -> Option<&'a Jwk> {
        let jwks = jwks?;
        match kid {
            Some(kid) => jwks.find(kid),
            None if jwks.keys.len() == 1 => jwks.keys.first(),
            None => None,
        }
    }

    async fn refetch(
        &self,
        http: &reqwest::Client,
        state: &mut KeyState,
    ) -> Result<(), IdpBearerError> {
        let discovery = self.discovery(http).await?;
        let jwks: JwkSet = http
            .get(&discovery.jwks_uri)
            .send()
            .await
            .and_then(reqwest::Response::error_for_status)
            .map_err(IdpBearerError::Jwks)?
            .json()
            .await
            .map_err(IdpBearerError::Jwks)?;

        tracing::debug!(issuer = %self.issuer, keys = jwks.keys.len(), "JWKS refreshed");
        state.jwks = Some(jwks);
        state.fetched_at = Some(Instant::now());
        Ok(())
    }
}

/// One configured provider, with its audience rules resolved and its JWKS
/// cache.
struct ProviderState {
    name: String,
    /// Audiences accepted on tokens from this provider. Resolved at
    /// construction: an empty `accepted_audiences` config means
    /// `[client_id]`.
    accepted_audiences: Vec<String>,
    jwks: JwksCache,
}

/// Validates IdP-issued bearer tokens against the configured OIDC
/// providers. See the module docs for the security model.
pub struct IdpBearerValidator {
    providers: Vec<ProviderState>,
    http: reqwest::Client,
}

impl IdpBearerValidator {
    /// Build a validator for every provider in `config`.
    ///
    /// Whether bearer validation is enabled at all (`oidc.enabled`, at
    /// least one provider) is the caller's decision; this constructor only
    /// shapes provider state.
    pub fn new(config: &OidcConfig) -> Self {
        // Redirects disabled to match the SSRF posture of the login flow;
        // the timeout keeps a hung IdP from hanging API auth requests.
        let http = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(10))
            .build()
            .expect("failed to build IdP bearer HTTP client");

        let providers = config
            .providers
            .iter()
            .map(|(name, provider)| ProviderState {
                name: name.clone(),
                accepted_audiences: if provider.accepted_audiences.is_empty() {
                    vec![provider.client_id.clone()]
                } else {
                    provider.accepted_audiences.clone()
                },
                jwks: JwksCache::new(&provider.issuer_url),
            })
            .collect();

        Self { providers, http }
    }

    /// Number of configured providers (zero means nothing can validate).
    pub fn provider_count(&self) -> usize {
        self.providers.len()
    }

    /// Validate an IdP-issued bearer token: algorithm allowlist, provider
    /// selection by `iss`, then signature, issuer, audience, and
    /// `exp`/`nbf` (30s leeway) against the provider's JWKS.
    pub async fn validate(&self, token: &str) -> Result<ValidatedIdpToken, IdpBearerError> {
        let header = decode_header(token).map_err(IdpBearerError::BadHeader)?;
        if !ALLOWED_ALGS.contains(&header.alg) {
            return Err(IdpBearerError::UnsupportedAlgorithm);
        }

        let issuer = peek_issuer(token)?;
        let provider = self
            .provider_for_issuer(&issuer)
            .ok_or(IdpBearerError::UnknownIssuer)?;

        let key = provider
            .jwks
            .decoding_key(&self.http, header.kid.as_deref())
            .await?;

        // Validate against exactly the header's (allowlisted) algorithm;
        // jsonwebtoken rejects the token if the key family differs.
        let mut validation = Validation::new(header.alg);
        validation.leeway = CLOCK_SKEW_LEEWAY_SECS;
        validation.validate_nbf = true;
        // `iss`/`aud` are only checked when present unless required:
        // a token without them must fail closed, not skip the check.
        validation.set_required_spec_claims(&["exp", "iss", "aud"]);
        // Issuers in the wild differ on the trailing slash; accept both
        // spellings of the configured issuer, nothing else.
        let trimmed = provider.jwks.issuer.as_str();
        validation.set_issuer(&[trimmed.to_owned(), format!("{trimmed}/")]);
        validation.set_audience(&provider.accepted_audiences);

        #[derive(Deserialize)]
        struct BearerClaims {
            sub: Option<String>,
        }

        match decode::<BearerClaims>(token, &key, &validation) {
            Ok(data) => {
                let subject = data
                    .claims
                    .sub
                    .filter(|sub| !sub.is_empty())
                    .ok_or(IdpBearerError::MissingSubject)?;
                tracing::debug!(
                    provider = %provider.name,
                    "IdP bearer token validated"
                );
                Ok(ValidatedIdpToken {
                    provider_name: provider.name.clone(),
                    subject,
                })
            }
            Err(err) => Err(match err.kind() {
                jsonwebtoken::errors::ErrorKind::InvalidIssuer => IdpBearerError::WrongIssuer,
                jsonwebtoken::errors::ErrorKind::InvalidAudience => IdpBearerError::WrongAudience,
                jsonwebtoken::errors::ErrorKind::ExpiredSignature => IdpBearerError::Expired,
                jsonwebtoken::errors::ErrorKind::ImmatureSignature => IdpBearerError::Immature,
                jsonwebtoken::errors::ErrorKind::InvalidSignature => IdpBearerError::BadSignature,
                jsonwebtoken::errors::ErrorKind::MissingRequiredClaim(claim) => {
                    IdpBearerError::MissingClaim(claim.clone())
                }
                _ => IdpBearerError::Invalid(err),
            }),
        }
    }

    /// The provider whose configured issuer matches `iss`, trailing-slash
    /// tolerant.
    fn provider_for_issuer(&self, iss: &str) -> Option<&ProviderState> {
        let iss = iss.trim_end_matches('/');
        self.providers.iter().find(|p| p.jwks.issuer == iss)
    }
}

/// Read the token's `iss` claim without verifying anything. Used solely to
/// select which provider's keys and rules apply; the claim is re-validated
/// against that provider during signature verification.
fn peek_issuer(token: &str) -> Result<String, IdpBearerError> {
    #[derive(Deserialize)]
    struct IssClaim {
        iss: Option<String>,
    }

    let payload = token.split('.').nth(1).ok_or(IdpBearerError::BadPayload)?;
    let bytes = URL_SAFE_NO_PAD
        .decode(payload)
        .map_err(|_| IdpBearerError::BadPayload)?;
    let claims: IssClaim =
        serde_json::from_slice(&bytes).map_err(|_| IdpBearerError::BadPayload)?;
    claims.iss.ok_or(IdpBearerError::UnknownIssuer)
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_config::{OidcDefaultRole, OidcProviderConfig};
    use std::collections::HashMap;

    fn provider(issuer_url: &str, client_id: &str, accepted: Vec<String>) -> OidcProviderConfig {
        OidcProviderConfig {
            display_name: "Test".to_string(),
            issuer_url: issuer_url.to_string(),
            client_id: client_id.to_string(),
            client_secret: None,
            client_secret_env: None,
            scopes: vec![],
            role_mapping: HashMap::new(),
            groups_claim: "groups".to_string(),
            username_claim: "preferred_username".to_string(),
            email_claim: "email".to_string(),
            accepted_audiences: accepted,
        }
    }

    fn config(providers: Vec<(&str, OidcProviderConfig)>) -> OidcConfig {
        OidcConfig {
            enabled: true,
            auto_create_users: false,
            default_role: OidcDefaultRole::Reader,
            redirect_uri_base: None,
            providers: providers
                .into_iter()
                .map(|(name, p)| (name.to_string(), p))
                .collect(),
        }
    }

    /// Hand-assemble a `header.payload.signature` token. The signature is
    /// garbage: these tests only exercise the steps before verification.
    fn raw_token(header: serde_json::Value, payload: serde_json::Value) -> String {
        let encode = |v: &serde_json::Value| URL_SAFE_NO_PAD.encode(v.to_string());
        format!("{}.{}.c2ln", encode(&header), encode(&payload))
    }

    #[test]
    fn accepted_audiences_default_to_client_id() {
        let cfg = config(vec![(
            "authentik",
            provider("https://idp.example.com/app/", "codex-client", vec![]),
        )]);
        let validator = IdpBearerValidator::new(&cfg);
        assert_eq!(
            validator.providers[0].accepted_audiences,
            vec!["codex-client".to_string()]
        );
    }

    #[test]
    fn explicit_accepted_audiences_are_kept_verbatim() {
        let cfg = config(vec![(
            "authentik",
            provider(
                "https://idp.example.com/app/",
                "codex-client",
                vec!["codex-client".to_string(), "shared-client".to_string()],
            ),
        )]);
        let validator = IdpBearerValidator::new(&cfg);
        assert_eq!(
            validator.providers[0].accepted_audiences,
            vec!["codex-client".to_string(), "shared-client".to_string()]
        );
    }

    #[test]
    fn provider_lookup_tolerates_trailing_slash_in_both_directions() {
        let cfg = config(vec![
            (
                "with-slash",
                provider("https://a.example.com/app/", "a", vec![]),
            ),
            ("no-slash", provider("https://b.example.com", "b", vec![])),
        ]);
        let validator = IdpBearerValidator::new(&cfg);

        for iss in ["https://a.example.com/app", "https://a.example.com/app/"] {
            let found = validator.provider_for_issuer(iss).expect(iss);
            assert_eq!(found.name, "with-slash");
        }
        for iss in ["https://b.example.com", "https://b.example.com/"] {
            let found = validator.provider_for_issuer(iss).expect(iss);
            assert_eq!(found.name, "no-slash");
        }
        assert!(
            validator
                .provider_for_issuer("https://evil.example.com")
                .is_none()
        );
    }

    #[test]
    fn peek_issuer_reads_the_unverified_claim() {
        let token = raw_token(
            serde_json::json!({"alg": "RS256", "typ": "JWT"}),
            serde_json::json!({"iss": "https://idp.example.com", "sub": "u1"}),
        );
        assert_eq!(peek_issuer(&token).unwrap(), "https://idp.example.com");
    }

    #[test]
    fn peek_issuer_rejects_garbage_and_missing_iss() {
        assert!(matches!(
            peek_issuer("not-a-jwt"),
            Err(IdpBearerError::BadPayload)
        ));
        assert!(matches!(
            peek_issuer("a.!!!.c"),
            Err(IdpBearerError::BadPayload)
        ));

        let token = raw_token(
            serde_json::json!({"alg": "RS256", "typ": "JWT"}),
            serde_json::json!({"sub": "u1"}),
        );
        assert!(matches!(
            peek_issuer(&token),
            Err(IdpBearerError::UnknownIssuer)
        ));
    }

    #[tokio::test]
    async fn hs256_tokens_are_rejected_before_any_network_io() {
        let cfg = config(vec![(
            "authentik",
            provider("https://idp.example.com", "codex", vec![]),
        )]);
        let validator = IdpBearerValidator::new(&cfg);

        // A real HS256 token signed with some secret: the IdP path must
        // refuse it on algorithm alone, never trying key material.
        let token = jsonwebtoken::encode(
            &jsonwebtoken::Header::new(Algorithm::HS256),
            &serde_json::json!({"iss": "https://idp.example.com", "sub": "u1", "exp": 4102444800u64}),
            &jsonwebtoken::EncodingKey::from_secret(b"local-secret"),
        )
        .unwrap();

        assert!(matches!(
            validator.validate(&token).await,
            Err(IdpBearerError::UnsupportedAlgorithm)
        ));
    }

    #[tokio::test]
    async fn alg_none_tokens_are_rejected_as_malformed() {
        let cfg = config(vec![(
            "authentik",
            provider("https://idp.example.com", "codex", vec![]),
        )]);
        let validator = IdpBearerValidator::new(&cfg);

        let header = URL_SAFE_NO_PAD.encode(r#"{"alg":"none","typ":"JWT"}"#);
        let payload = URL_SAFE_NO_PAD.encode(r#"{"iss":"https://idp.example.com","sub":"u1"}"#);
        let token = format!("{header}.{payload}.");

        assert!(matches!(
            validator.validate(&token).await,
            Err(IdpBearerError::BadHeader(_))
        ));
    }

    #[tokio::test]
    async fn unknown_issuer_is_rejected_before_any_network_io() {
        let cfg = config(vec![(
            "authentik",
            provider("https://idp.example.com", "codex", vec![]),
        )]);
        let validator = IdpBearerValidator::new(&cfg);

        let token = raw_token(
            serde_json::json!({"alg": "RS256", "typ": "JWT"}),
            serde_json::json!({"iss": "https://evil.example.com", "sub": "u1"}),
        );
        assert!(matches!(
            validator.validate(&token).await,
            Err(IdpBearerError::UnknownIssuer)
        ));
    }

    #[test]
    fn backend_errors_are_distinguished_from_token_rejections() {
        assert!(
            IdpBearerError::IssuerMismatch {
                expected: "a".into(),
                found: "b".into()
            }
            .is_backend()
        );
        assert!(!IdpBearerError::WrongAudience.is_backend());
        assert!(!IdpBearerError::Expired.is_backend());
        assert!(!IdpBearerError::UnknownKey.is_backend());
    }
}
