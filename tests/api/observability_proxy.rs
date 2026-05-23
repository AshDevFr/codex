//! Integration tests for the browser RUM bootstrap + OTLP forwarding proxy
//! (Phase 4 of the observability plan).
//!
//! Scope: the handler layer is reqwest + axum — no OTel SDK is required to
//! exercise it. These tests cover:
//!  - `/api/v1/observability/config` requires auth and reflects the
//!    server-side flag state without leaking secrets.
//!  - `/api/v1/observability/otlp/v1/traces` rejects when the browser
//!    feature is off (503).
//!  - The same path forwards the body verbatim, stamps the configured
//!    auth headers, and ignores browser-supplied headers when the
//!    feature is on.
//!
//! Upstream collector is faked with a local axum listener so we can
//! observe what reached it.

#[path = "../common/mod.rs"]
mod common;

use std::sync::Arc;

use axum::{
    Router,
    body::Bytes,
    extract::State,
    http::{HeaderMap, StatusCode},
    routing::post,
};
use codex::api::extractors::AppState;
use codex::api::routes::create_router;
use codex::config::{Config, ObservabilityBrowserConfig, ObservabilityConfig};
use codex::db::repositories::UserRepository;
use codex::utils::password;
use common::*;
use hyper::Request;
use tokio::sync::Mutex;

/// One captured upstream POST.
#[derive(Clone, Debug)]
struct CapturedRequest {
    path: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

#[derive(Default, Clone)]
struct CaptureState {
    captures: Arc<Mutex<Vec<CapturedRequest>>>,
}

async fn capture_handler(
    State(state): State<CaptureState>,
    headers: HeaderMap,
    axum::extract::OriginalUri(uri): axum::extract::OriginalUri,
    body: Bytes,
) -> StatusCode {
    let header_pairs = headers
        .iter()
        .filter_map(|(k, v)| v.to_str().ok().map(|s| (k.to_string(), s.to_string())))
        .collect();
    state.captures.lock().await.push(CapturedRequest {
        path: uri.path().to_string(),
        headers: header_pairs,
        body: body.to_vec(),
    });
    StatusCode::OK
}

/// Spawn a one-listener axum collector. Returns the base URL (e.g.
/// `http://127.0.0.1:PORT`) and the capture state so the test can assert
/// what arrived.
async fn spawn_capture_upstream() -> (String, CaptureState) {
    let state = CaptureState::default();
    let app = Router::new()
        .route("/v1/traces", post(capture_handler))
        .route("/v1/metrics", post(capture_handler))
        .with_state(state.clone());

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    (format!("http://{}", addr), state)
}

/// Build an observability config that points at the given upstream and has
/// the browser proxy enabled (or not).
fn observability_config(
    upstream: &str,
    browser_enabled: bool,
    extra_headers: Vec<(String, String)>,
) -> ObservabilityConfig {
    let mut cfg = ObservabilityConfig {
        browser: ObservabilityBrowserConfig {
            enabled: browser_enabled,
            proxy_path: "/api/v1/observability/otlp".to_string(),
            sample_ratio: 0.25,
        },
        ..ObservabilityConfig::default()
    };
    cfg.otlp.endpoint = upstream.to_string();
    cfg.otlp.timeout_ms = 2000;
    for (k, v) in extra_headers {
        cfg.otlp.headers.insert(k, v);
    }
    cfg.service_name = "codex-test".to_string();
    cfg
}

/// Build an AppState that uses the supplied observability config.
async fn app_state_with_observability(
    db: sea_orm::DatabaseConnection,
    obs: ObservabilityConfig,
) -> Arc<AppState> {
    let mut state = (*create_test_app_state(db).await).clone();
    state.observability_config = Arc::new(obs);
    Arc::new(state)
}

async fn bootstrap_user(
    db: &sea_orm::DatabaseConnection,
    username: &str,
) -> (codex::db::entities::users::Model, String) {
    let pwd_hash = password::hash_password("hunter2-for-the-tests").unwrap();
    let user = create_test_user(
        username,
        &format!("{username}@example.com"),
        &pwd_hash,
        false,
    );
    UserRepository::create(db, &user).await.unwrap();
    (user, "hunter2-for-the-tests".to_string())
}

fn router_for(state: Arc<AppState>) -> Router {
    let config = Config::default();
    create_router(state, &config)
}

#[tokio::test]
async fn observability_config_requires_auth() {
    let (db, _temp) = setup_test_db().await;
    let state = create_test_app_state(db).await;
    let app = router_for(state);

    let request = Request::builder()
        .method("GET")
        .uri("/api/v1/observability/config")
        .body(String::new())
        .unwrap();
    let (status, _body) = make_request(app, request).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn observability_config_returns_disabled_payload_by_default() {
    let (db, _temp) = setup_test_db().await;
    let state = create_test_app_state(db).await;
    let (user, _) = bootstrap_user(&state.db, "obs_default").await;
    let token = generate_test_token(&state, &user);
    let app = router_for(state);

    let request = get_request_with_auth("/api/v1/observability/config", &token);
    let (status, body) = make_json_request::<serde_json::Value>(app, request).await;
    assert_eq!(status, StatusCode::OK);
    let payload = body.expect("config payload");
    assert_eq!(payload["enabled"], serde_json::Value::Bool(false));
    assert_eq!(payload["proxyPath"], "/api/v1/observability/otlp");
    assert_eq!(payload["serviceName"], "codex");
}

#[tokio::test]
async fn observability_config_advertises_enabled_when_browser_on() {
    let (db, _temp) = setup_test_db().await;
    let obs = observability_config("http://example.invalid:4318", true, vec![]);
    let state = app_state_with_observability(db, obs).await;
    let (user, _) = bootstrap_user(&state.db, "obs_enabled").await;
    let token = generate_test_token(&state, &user);
    let app = router_for(state);

    let request = get_request_with_auth("/api/v1/observability/config", &token);
    let (status, body) = make_json_request::<serde_json::Value>(app, request).await;
    assert_eq!(status, StatusCode::OK);
    let payload = body.expect("config payload");
    assert_eq!(payload["enabled"], serde_json::Value::Bool(true));
    assert_eq!(payload["sampleRatio"], 0.25);
    assert_eq!(payload["serviceName"], "codex-test");
}

#[tokio::test]
async fn otlp_proxy_rejects_when_browser_disabled() {
    let (db, _temp) = setup_test_db().await;
    let state = create_test_app_state(db).await; // browser_enabled=false by default
    let (user, _) = bootstrap_user(&state.db, "obs_disabled_proxy").await;
    let token = generate_test_token(&state, &user);
    let app = router_for(state);

    let request = Request::builder()
        .method("POST")
        .uri("/api/v1/observability/otlp/v1/traces")
        .header("authorization", format!("Bearer {token}"))
        .header("content-type", "application/x-protobuf")
        .body(String::from("anything"))
        .unwrap();
    let (status, _body) = make_request(app, request).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
async fn otlp_proxy_forwards_body_and_headers() {
    let (upstream_url, capture) = spawn_capture_upstream().await;
    let (db, _temp) = setup_test_db().await;
    let obs = observability_config(
        &upstream_url,
        true,
        vec![("x-tenant".to_string(), "test-tenant".to_string())],
    );
    let state = app_state_with_observability(db, obs).await;
    let (user, _) = bootstrap_user(&state.db, "obs_forward").await;
    let token = generate_test_token(&state, &user);
    let app = router_for(state);

    let payload = b"\x0aFAKE-OTLP-PROTO-BYTES".to_vec();
    let request = Request::builder()
        .method("POST")
        .uri("/api/v1/observability/otlp/v1/traces")
        .header("authorization", format!("Bearer {token}"))
        .header("content-type", "application/x-protobuf")
        // Browser-supplied header that should NOT be forwarded.
        .header("x-tenant", "evil-spoof")
        .body(String::from_utf8_lossy(&payload).to_string())
        .unwrap();
    let (status, _body) = make_request(app, request).await;
    assert_eq!(status, StatusCode::OK, "proxy should pass through 200");

    let captured = capture.captures.lock().await.clone();
    assert_eq!(captured.len(), 1, "exactly one upstream POST should arrive");
    let c = &captured[0];
    assert_eq!(c.path, "/v1/traces");
    assert_eq!(c.body, payload, "body should reach upstream unmodified");
    let content_type = c
        .headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("content-type"))
        .map(|(_, v)| v.as_str());
    assert_eq!(content_type, Some("application/x-protobuf"));
    let tenant = c
        .headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("x-tenant"))
        .map(|(_, v)| v.as_str());
    assert_eq!(
        tenant,
        Some("test-tenant"),
        "operator-configured header must win; browser-supplied value is dropped"
    );
}
