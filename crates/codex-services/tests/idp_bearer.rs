//! IdP bearer validation integration tests: full validate() runs against a
//! local axum "IdP" serving a discovery document and a JWKS built from
//! static RSA test keys under `tests/fixtures/idp_bearer/` (test-only key
//! material, never used outside the suite).

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};
use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
use tokio::sync::Mutex;

use codex_config::{OidcConfig, OidcDefaultRole, OidcProviderConfig};
use codex_services::idp_bearer::{IdpBearerError, IdpBearerValidator};

fn fixture_path(name: &str) -> String {
    format!(
        "{}/tests/fixtures/idp_bearer/{name}",
        env!("CARGO_MANIFEST_DIR")
    )
}

fn key_pem(name: &str) -> Vec<u8> {
    std::fs::read(fixture_path(&format!("{name}.pem"))).expect("test key fixture")
}

fn jwks(name: &str) -> serde_json::Value {
    let raw = std::fs::read_to_string(fixture_path(&format!("{name}.json"))).expect("jwks fixture");
    serde_json::from_str(&raw).expect("jwks fixture parses")
}

/// In-process IdP stub: discovery + JWKS endpoints with a swappable key
/// set and a fetch counter for cache-behavior assertions.
struct IdpStub {
    url: String,
    jwks_body: Arc<Mutex<serde_json::Value>>,
    jwks_hits: Arc<AtomicUsize>,
}

impl IdpStub {
    async fn start(initial_jwks: serde_json::Value) -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind ephemeral port");
        let url = format!("http://{}", listener.local_addr().unwrap());

        let jwks_body = Arc::new(Mutex::new(initial_jwks));
        let jwks_hits = Arc::new(AtomicUsize::new(0));

        #[derive(Clone)]
        struct StubState {
            issuer: String,
            jwks_body: Arc<Mutex<serde_json::Value>>,
            jwks_hits: Arc<AtomicUsize>,
        }

        let state = StubState {
            issuer: url.clone(),
            jwks_body: Arc::clone(&jwks_body),
            jwks_hits: Arc::clone(&jwks_hits),
        };

        let app = Router::new()
            .route(
                "/.well-known/openid-configuration",
                get(|State(s): State<StubState>| async move {
                    Json(serde_json::json!({
                        "issuer": s.issuer,
                        "jwks_uri": format!("{}/jwks", s.issuer),
                    }))
                }),
            )
            .route(
                "/jwks",
                get(|State(s): State<StubState>| async move {
                    s.jwks_hits.fetch_add(1, Ordering::SeqCst);
                    Json(s.jwks_body.lock().await.clone())
                }),
            )
            .with_state(state);

        tokio::spawn(async move {
            axum::serve(listener, app).await.expect("stub IdP serves");
        });

        Self {
            url,
            jwks_body,
            jwks_hits,
        }
    }

    async fn set_jwks(&self, body: serde_json::Value) {
        *self.jwks_body.lock().await = body;
    }

    fn jwks_fetches(&self) -> usize {
        self.jwks_hits.load(Ordering::SeqCst)
    }
}

fn provider_config(issuer_url: &str, accepted_audiences: Vec<String>) -> OidcProviderConfig {
    OidcProviderConfig {
        display_name: "Test IdP".to_string(),
        issuer_url: issuer_url.to_string(),
        client_id: "codex-client".to_string(),
        client_secret: None,
        client_secret_env: None,
        scopes: vec![],
        role_mapping: HashMap::new(),
        groups_claim: "groups".to_string(),
        username_claim: "preferred_username".to_string(),
        email_claim: "email".to_string(),
        accepted_audiences,
    }
}

fn validator(provider_name: &str, provider: OidcProviderConfig) -> IdpBearerValidator {
    let config = OidcConfig {
        enabled: true,
        auto_create_users: false,
        default_role: OidcDefaultRole::Reader,
        redirect_uri_base: None,
        providers: HashMap::from([(provider_name.to_string(), provider)]),
    };
    IdpBearerValidator::new(&config)
}

fn sign(kid: &str, key: &str, claims: &serde_json::Value) -> String {
    let mut header = Header::new(Algorithm::RS256);
    header.kid = Some(kid.to_owned());
    encode(
        &header,
        claims,
        &EncodingKey::from_rsa_pem(&key_pem(key)).expect("test key parses"),
    )
    .expect("token signs")
}

fn claims(iss: &str, aud: &str, exp_offset_secs: i64) -> serde_json::Value {
    serde_json::json!({
        "iss": iss,
        "aud": aud,
        "exp": chrono::Utc::now().timestamp() + exp_offset_secs,
        "sub": "idp-user-42",
    })
}

#[tokio::test]
async fn valid_token_resolves_to_provider_and_subject() {
    let idp = IdpStub::start(jwks("jwks_key_a")).await;
    let v = validator("authentik", provider_config(&idp.url, vec![]));

    let token = sign("key-a", "key_a", &claims(&idp.url, "codex-client", 3600));
    let validated = v.validate(&token).await.expect("valid token validates");

    assert_eq!(validated.provider_name, "authentik");
    assert_eq!(validated.subject, "idp-user-42");
}

#[tokio::test]
async fn second_validation_is_served_from_the_jwks_cache() {
    let idp = IdpStub::start(jwks("jwks_key_a")).await;
    let v = validator("authentik", provider_config(&idp.url, vec![]));

    let token = sign("key-a", "key_a", &claims(&idp.url, "codex-client", 3600));
    v.validate(&token).await.expect("first validation");
    v.validate(&token).await.expect("second validation");

    assert_eq!(idp.jwks_fetches(), 1, "second call must hit the cache");
}

#[tokio::test]
async fn each_tampered_dimension_fails_with_its_specific_error() {
    let idp = IdpStub::start(jwks("jwks_key_a")).await;
    let v = validator("authentik", provider_config(&idp.url, vec![]));

    // Wrong issuer: no configured provider matches.
    let token = sign(
        "key-a",
        "key_a",
        &claims("https://evil.example.com", "codex-client", 3600),
    );
    assert!(matches!(
        v.validate(&token).await,
        Err(IdpBearerError::UnknownIssuer)
    ));

    // Wrong audience.
    let token = sign("key-a", "key_a", &claims(&idp.url, "not-codex", 3600));
    assert!(matches!(
        v.validate(&token).await,
        Err(IdpBearerError::WrongAudience)
    ));

    // Missing audience entirely: must fail closed, not skip the check.
    let token = sign(
        "key-a",
        "key_a",
        &serde_json::json!({
            "iss": idp.url,
            "exp": chrono::Utc::now().timestamp() + 3600,
            "sub": "idp-user-42",
        }),
    );
    match v.validate(&token).await {
        Err(IdpBearerError::MissingClaim(claim)) => assert_eq!(claim, "aud"),
        other => panic!("expected MissingClaim(aud), got {other:?}"),
    }

    // Expired (well past the 30s leeway).
    let token = sign("key-a", "key_a", &claims(&idp.url, "codex-client", -3600));
    assert!(matches!(
        v.validate(&token).await,
        Err(IdpBearerError::Expired)
    ));

    // Not yet valid (nbf in the future, past the leeway).
    let mut not_yet = claims(&idp.url, "codex-client", 3600);
    not_yet["nbf"] = serde_json::json!(chrono::Utc::now().timestamp() + 3600);
    let token = sign("key-a", "key_a", &not_yet);
    assert!(matches!(
        v.validate(&token).await,
        Err(IdpBearerError::Immature)
    ));

    // Signed by key B but claiming key A's kid: signature mismatch.
    let token = sign("key-a", "key_b", &claims(&idp.url, "codex-client", 3600));
    assert!(matches!(
        v.validate(&token).await,
        Err(IdpBearerError::BadSignature)
    ));

    // No subject claim.
    let token = sign(
        "key-a",
        "key_a",
        &serde_json::json!({
            "iss": idp.url,
            "aud": "codex-client",
            "exp": chrono::Utc::now().timestamp() + 3600,
        }),
    );
    assert!(matches!(
        v.validate(&token).await,
        Err(IdpBearerError::MissingSubject)
    ));
}

#[tokio::test]
async fn expiry_within_leeway_is_tolerated() {
    let idp = IdpStub::start(jwks("jwks_key_a")).await;
    let v = validator("authentik", provider_config(&idp.url, vec![]));

    // Expired 10s ago: inside the 30s clock-skew leeway.
    let token = sign("key-a", "key_a", &claims(&idp.url, "codex-client", -10));
    v.validate(&token)
        .await
        .expect("clock skew within leeway tolerated");
}

#[tokio::test]
async fn accepted_audiences_list_admits_trusted_foreign_clients() {
    let idp = IdpStub::start(jwks("jwks_key_a")).await;
    let v = validator(
        "authentik",
        provider_config(
            &idp.url,
            vec!["codex-client".to_string(), "shisho-shared".to_string()],
        ),
    );

    // A token minted for the shared client is accepted...
    let token = sign("key-a", "key_a", &claims(&idp.url, "shisho-shared", 3600));
    v.validate(&token).await.expect("shared audience accepted");

    // ...while an unlisted audience still fails.
    let token = sign("key-a", "key_a", &claims(&idp.url, "other-app", 3600));
    assert!(matches!(
        v.validate(&token).await,
        Err(IdpBearerError::WrongAudience)
    ));
}

#[tokio::test]
async fn issuer_trailing_slash_mismatch_between_config_and_token_is_tolerated() {
    let idp = IdpStub::start(jwks("jwks_key_a")).await;
    // Configured WITH a trailing slash, token issuer without (Authentik
    // configs are routinely pasted with the slash).
    let v = validator(
        "authentik",
        provider_config(&format!("{}/", idp.url), vec![]),
    );

    let token = sign("key-a", "key_a", &claims(&idp.url, "codex-client", 3600));
    v.validate(&token).await.expect("slash variant accepted");

    // And the other spelling in the token also validates.
    let token = sign(
        "key-a",
        "key_a",
        &claims(&format!("{}/", idp.url), "codex-client", 3600),
    );
    v.validate(&token).await.expect("slashed issuer accepted");
}

#[tokio::test]
async fn unknown_kid_triggers_one_refetch_then_fails_closed() {
    let idp = IdpStub::start(jwks("jwks_key_a")).await;
    let v = validator("authentik", provider_config(&idp.url, vec![]));

    let token = sign("key-c", "key_b", &claims(&idp.url, "codex-client", 3600));
    assert!(matches!(
        v.validate(&token).await,
        Err(IdpBearerError::UnknownKey)
    ));
    // Initial load + the one miss-triggered refetch.
    assert_eq!(idp.jwks_fetches(), 2);

    // Immediately retrying the same unknown kid stays failed without
    // another fetch (miss refetches are rate-limited).
    assert!(matches!(
        v.validate(&token).await,
        Err(IdpBearerError::UnknownKey)
    ));
    assert_eq!(idp.jwks_fetches(), 2, "rate limit must hold");
}

#[tokio::test]
async fn rotated_key_is_picked_up_via_the_kid_miss_refetch() {
    let idp = IdpStub::start(jwks("jwks_key_a")).await;
    let v = validator("authentik", provider_config(&idp.url, vec![]));

    // Warm the cache on key A.
    let token_a = sign("key-a", "key_a", &claims(&idp.url, "codex-client", 3600));
    v.validate(&token_a).await.expect("key A validates");

    // Rotate: the IdP now also serves key B; a key-B token must validate
    // transparently via the miss refetch, no restart.
    idp.set_jwks(jwks("jwks_keys_a_b")).await;
    let token_b = sign("key-b", "key_b", &claims(&idp.url, "codex-client", 3600));
    v.validate(&token_b)
        .await
        .expect("rotation must be transparent");
}

#[tokio::test]
async fn unreachable_idp_is_a_backend_error_not_a_token_rejection() {
    // Nothing listens on this port.
    let v = validator("authentik", provider_config("http://127.0.0.1:1", vec![]));

    let token = sign(
        "key-a",
        "key_a",
        &claims("http://127.0.0.1:1", "codex-client", 3600),
    );
    let err = v.validate(&token).await.expect_err("IdP is down");
    assert!(
        err.is_backend(),
        "IdP outage must be a backend error, got {err:?}"
    );
}
