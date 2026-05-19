//! Integration tests for the refresh-token endpoints (Phase 2).
//!
//! Covers:
//! - Login responses include / omit `refresh_token` based on the config flag.
//! - `/auth/refresh` happy path and 401 paths (unknown / revoked / expired).
//! - Theft detection: replaying a rotated refresh token revokes the family.
//! - Logout revokes the supplied refresh token.

#[path = "../common/mod.rs"]
mod common;

use chrono::{Duration, Utc};
use codex::api::extractors::AppState;
use codex::api::extractors::auth::UserAuthCache;
use codex::api::routes::create_router;
use codex::api::routes::v1::dto::auth::{
    LoginRequest, LoginResponse, LogoutRequest, RefreshRequest, TokenPair,
};
use codex::config::{AuthConfig, DatabaseConfig, EmailConfig, FilesConfig, PdfConfig};
use codex::db::repositories::{NewRefreshToken, RefreshTokenRepository, UserRepository};
use codex::events::EventBroadcaster;
use codex::services::email::EmailService;
use codex::services::{
    AuthTrackingService, FileCleanupService, InflightThumbnailTracker, PdfHandleCache,
    PdfPageCache, PluginMetricsService, ReadProgressService, RefreshTokenService, SettingsService,
    ThumbnailService, plugin::PluginManager,
};
use codex::utils::jwt::JwtService;
use codex::utils::password;
use common::*;
use hyper::StatusCode;
use sea_orm::DatabaseConnection;
use std::sync::Arc;
use uuid::Uuid;

/// Build an `AppState` whose `AuthConfig.refresh_token_enabled` honors the
/// passed flag. The default factory bakes in `refresh_token_enabled: false`
/// so the refresh endpoint would always 401.
async fn build_state(db: DatabaseConnection, refresh_enabled: bool) -> Arc<AppState> {
    let jwt_service = Arc::new(JwtService::new(
        "test_secret_key_for_integration_tests".to_string(),
        24,
    ));

    let refresh_token_service = Arc::new(RefreshTokenService::new(db.clone(), 30));
    let auth_config = Arc::new(AuthConfig {
        refresh_token_enabled: refresh_enabled,
        ..AuthConfig::default()
    });
    let database_config = Arc::new(DatabaseConfig::default());
    let pdf_config = Arc::new(PdfConfig::default());
    let email_service = Arc::new(EmailService::new(EmailConfig::default()));
    let event_broadcaster = Arc::new(EventBroadcaster::new(1000));
    let settings_service = Arc::new(
        SettingsService::new(db.clone())
            .await
            .expect("settings service"),
    );
    let files_config = FilesConfig::default();
    let thumbnail_service = Arc::new(ThumbnailService::new(files_config.clone()));
    let file_cleanup_service = Arc::new(FileCleanupService::new(files_config));
    let read_progress_service = Arc::new(ReadProgressService::new(db.clone()));
    let auth_tracking_service = Arc::new(AuthTrackingService::new(db.clone()));
    let pdf_page_cache = Arc::new(PdfPageCache::new(&pdf_config.cache_dir, false));
    let pdf_handle_cache = Arc::new(PdfHandleCache::new(
        8,
        std::time::Duration::from_secs(60),
        false,
    ));
    let plugin_manager = Arc::new(PluginManager::with_defaults(Arc::new(db.clone())));
    let plugin_metrics_service = Arc::new(PluginMetricsService::new());

    Arc::new(AppState {
        db,
        jwt_service,
        refresh_token_service,
        auth_config,
        database_config,
        pdf_config,
        email_service,
        event_broadcaster,
        settings_service,
        thumbnail_service,
        file_cleanup_service,
        task_metrics_service: None,
        scheduler: None,
        read_progress_service,
        auth_tracking_service,
        pdf_page_cache,
        pdf_handle_cache,
        inflight_thumbnails: Arc::new(InflightThumbnailTracker::new()),
        user_auth_cache: Arc::new(UserAuthCache::new()),
        rate_limiter_service: None,
        plugin_manager,
        plugin_metrics_service,
        oidc_service: None,
        oauth_state_manager: Arc::new(codex::services::user_plugin::OAuthStateManager::new()),
        export_storage: None,
        plugin_file_storage: None,
        scheduler_timezone: "UTC".to_string(),
    })
}

async fn seed_user(db: &DatabaseConnection, username: &str, password_plain: &str) {
    let hash = password::hash_password(password_plain).unwrap();
    let user = create_test_user(username, &format!("{}@example.com", username), &hash, false);
    UserRepository::create(db, &user).await.unwrap();
}

async fn login_and_get_pair(
    state: Arc<AppState>,
    username: &str,
    password_plain: &str,
) -> LoginResponse {
    let app = create_router(state, &create_test_config());
    let body = LoginRequest {
        username: username.to_string(),
        password: password_plain.to_string(),
    };
    let request = post_json_request("/api/v1/auth/login", &body);
    let (status, parsed): (StatusCode, Option<LoginResponse>) =
        make_json_request(app, request).await;
    assert_eq!(status, StatusCode::OK);
    parsed.expect("login response")
}

#[tokio::test]
async fn login_response_includes_refresh_token_when_enabled() {
    let (db, _tmp) = setup_test_db().await;
    seed_user(&db, "alice", "secret123").await;
    let state = build_state(db, true).await;

    let resp = login_and_get_pair(state, "alice", "secret123").await;
    assert!(
        resp.refresh_token.is_some(),
        "refresh_token must be present when feature is enabled"
    );
    assert!(!resp.refresh_token.unwrap().is_empty());
}

#[tokio::test]
async fn login_response_omits_refresh_token_when_disabled() {
    let (db, _tmp) = setup_test_db().await;
    seed_user(&db, "bob", "secret123").await;
    let state = build_state(db, false).await;

    let resp = login_and_get_pair(state, "bob", "secret123").await;
    assert!(
        resp.refresh_token.is_none(),
        "refresh_token must be absent when feature is disabled"
    );
}

#[tokio::test]
async fn refresh_happy_path_rotates_pair_and_revokes_old() {
    let (db, _tmp) = setup_test_db().await;
    seed_user(&db, "carol", "secret123").await;
    let state = build_state(db.clone(), true).await;
    let app = create_router(state.clone(), &create_test_config());

    let login = login_and_get_pair(state, "carol", "secret123").await;
    let first_refresh = login.refresh_token.expect("first refresh token");

    // Look up old row to verify it is later revoked.
    let old_hash = RefreshTokenService::hash_token(&first_refresh);
    let old_row = RefreshTokenRepository::get_by_hash(&db, &old_hash)
        .await
        .unwrap()
        .unwrap();

    let body = RefreshRequest {
        refresh_token: first_refresh.clone(),
    };
    let request = post_json_request("/api/v1/auth/refresh", &body);
    let (status, parsed): (StatusCode, Option<TokenPair>) = make_json_request(app, request).await;
    assert_eq!(status, StatusCode::OK);
    let pair = parsed.unwrap();

    assert!(!pair.access_token.is_empty());
    assert_ne!(pair.refresh_token, first_refresh, "must rotate");
    assert_eq!(pair.token_type, "Bearer");
    assert_eq!(pair.expires_in, 24 * 3600);

    // Old row revoked, new row exists with same family_id.
    let old_row_now = RefreshTokenRepository::get_by_id(&db, old_row.id)
        .await
        .unwrap()
        .unwrap();
    assert!(old_row_now.revoked_at.is_some(), "old must be revoked");

    let new_hash = RefreshTokenService::hash_token(&pair.refresh_token);
    let new_row = RefreshTokenRepository::get_by_hash(&db, &new_hash)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(new_row.family_id, old_row.family_id);
    assert!(new_row.revoked_at.is_none());
}

#[tokio::test]
async fn refresh_with_unknown_token_returns_401() {
    let (db, _tmp) = setup_test_db().await;
    let state = build_state(db, true).await;
    let app = create_router(state, &create_test_config());

    let bogus = RefreshTokenService::generate_token();
    let body = RefreshRequest {
        refresh_token: bogus,
    };
    let request = post_json_request("/api/v1/auth/refresh", &body);
    let (status, _body) = make_request(app, request).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn refresh_with_expired_token_returns_401() {
    let (db, _tmp) = setup_test_db().await;
    let hash = password::hash_password("pw").unwrap();
    let user = create_test_user("dave", "dave@example.com", &hash, false);
    let saved = UserRepository::create(&db, &user).await.unwrap();
    let state = build_state(db.clone(), true).await;
    let app = create_router(state, &create_test_config());

    // Manually insert an already-expired refresh-token row.
    let plain = RefreshTokenService::generate_token();
    let token_hash = RefreshTokenService::hash_token(&plain);
    let now = Utc::now();
    RefreshTokenRepository::create(
        &db,
        NewRefreshToken {
            user_id: saved.id,
            family_id: Uuid::new_v4(),
            token_hash,
            issued_at: now - Duration::days(2),
            expires_at: now - Duration::days(1),
            user_agent: None,
            ip_address: None,
        },
    )
    .await
    .unwrap();

    let body = RefreshRequest {
        refresh_token: plain,
    };
    let request = post_json_request("/api/v1/auth/refresh", &body);
    let (status, _body) = make_request(app, request).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn reusing_rotated_refresh_token_revokes_family() {
    let (db, _tmp) = setup_test_db().await;
    seed_user(&db, "eve", "secret123").await;
    let state = build_state(db.clone(), true).await;
    let app = create_router(state.clone(), &create_test_config());

    let login = login_and_get_pair(state, "eve", "secret123").await;
    let first = login.refresh_token.expect("first");

    // Rotate once - get the sibling token in the same family.
    let body = RefreshRequest {
        refresh_token: first.clone(),
    };
    let request = post_json_request("/api/v1/auth/refresh", &body);
    let (status, parsed): (StatusCode, Option<TokenPair>) =
        make_json_request(app.clone(), request).await;
    assert_eq!(status, StatusCode::OK);
    let sibling = parsed.unwrap().refresh_token;

    // Replay the original (now-revoked) token. Theft signal -> 401 + family
    // wiped, which we verify by trying the sibling next and getting 401.
    let body = RefreshRequest {
        refresh_token: first,
    };
    let request = post_json_request("/api/v1/auth/refresh", &body);
    let (status, _body) = make_request(app.clone(), request).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let body = RefreshRequest {
        refresh_token: sibling.clone(),
    };
    let request = post_json_request("/api/v1/auth/refresh", &body);
    let (status, _body) = make_request(app, request).await;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "the legitimate sibling must also fail after family revocation"
    );

    // Verify in the DB that the sibling row is revoked too.
    let sibling_hash = RefreshTokenService::hash_token(&sibling);
    let row = RefreshTokenRepository::get_by_hash(&db, &sibling_hash)
        .await
        .unwrap()
        .unwrap();
    assert!(row.revoked_at.is_some());
}

#[tokio::test]
async fn logout_revokes_supplied_refresh_token() {
    let (db, _tmp) = setup_test_db().await;
    seed_user(&db, "frank", "secret123").await;
    let state = build_state(db.clone(), true).await;
    let app = create_router(state.clone(), &create_test_config());

    let login = login_and_get_pair(state.clone(), "frank", "secret123").await;
    let refresh = login.refresh_token.expect("refresh");

    let logout_body = LogoutRequest {
        refresh_token: Some(refresh.clone()),
    };
    let request =
        post_json_request_with_auth("/api/v1/auth/logout", &logout_body, &login.access_token);
    let (status, _body) = make_request(app.clone(), request).await;
    assert_eq!(status, StatusCode::OK);

    // Subsequent refresh with the revoked token must fail.
    let body = RefreshRequest {
        refresh_token: refresh.clone(),
    };
    let request = post_json_request("/api/v1/auth/refresh", &body);
    let (status, _body) = make_request(app, request).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    // And the row is revoked in the DB.
    let row = RefreshTokenRepository::get_by_hash(&db, &RefreshTokenService::hash_token(&refresh))
        .await
        .unwrap()
        .unwrap();
    assert!(row.revoked_at.is_some());
}

#[tokio::test]
async fn refresh_endpoint_401_when_feature_disabled() {
    let (db, _tmp) = setup_test_db().await;
    let state = build_state(db, false).await;
    let app = create_router(state, &create_test_config());

    let body = RefreshRequest {
        refresh_token: RefreshTokenService::generate_token(),
    };
    let request = post_json_request("/api/v1/auth/refresh", &body);
    let (status, _body) = make_request(app, request).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}
