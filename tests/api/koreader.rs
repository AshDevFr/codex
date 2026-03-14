#[path = "../common/mod.rs"]
mod common;

use codex::api::routes::koreader::dto::progress::{AuthorizedDto, DocumentProgressDto};
use codex::db::ScanningStrategy;
use codex::db::repositories::{
    BookRepository, LibraryRepository, ReadProgressRepository, SeriesRepository, UserRepository,
};
use codex::utils::password;
use common::*;
use hyper::StatusCode;

async fn setup_admin_and_token(
    db: &sea_orm::DatabaseConnection,
    state: &codex::api::extractors::AppState,
) -> (uuid::Uuid, String) {
    let password_hash = password::hash_password("admin123").unwrap();
    let user = create_test_user("admin", "admin@example.com", &password_hash, true);
    let created = UserRepository::create(db, &user).await.unwrap();
    let token = state
        .jwt_service
        .generate_token(created.id, created.username.clone(), created.get_role())
        .unwrap();
    (created.id, token)
}

/// POST /koreader/users/create always returns 403
#[tokio::test]
async fn test_koreader_create_user_returns_403() {
    let (db, _tmp) = setup_test_db().await;
    let state = create_test_app_state(db).await;
    let app = create_test_router_with_koreader(state);

    let request = post_request("/koreader/users/create");
    let (status, _body) = make_request(app, request).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

/// GET /koreader/users/auth returns 200 with valid auth
#[tokio::test]
async fn test_koreader_auth_with_bearer_token() {
    let (db, _tmp) = setup_test_db().await;
    let state = create_test_app_state(db.clone()).await;
    let (_user_id, token) = setup_admin_and_token(&db, &state).await;
    let app = create_test_router_with_koreader(state);

    let request = get_request_with_auth("/koreader/users/auth", &token);
    let (status, body) = make_json_request::<AuthorizedDto>(app, request).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body.unwrap().authorized, "OK");
}

/// GET /koreader/users/auth returns 200 with basic auth
#[tokio::test]
async fn test_koreader_auth_with_basic_auth() {
    let (db, _tmp) = setup_test_db().await;

    let password_hash = password::hash_password("testpass").unwrap();
    let user = create_test_user("testuser", "test@example.com", &password_hash, true);
    UserRepository::create(&db, &user).await.unwrap();

    let state = create_test_app_state(db).await;
    let app = create_test_router_with_koreader(state);

    let request = get_request_with_basic_auth("/koreader/users/auth", "testuser", "testpass");
    let (status, body) = make_json_request::<AuthorizedDto>(app, request).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body.unwrap().authorized, "OK");
}

/// GET /koreader/users/auth returns 401 without auth
#[tokio::test]
async fn test_koreader_auth_without_credentials() {
    let (db, _tmp) = setup_test_db().await;
    let state = create_test_app_state(db).await;
    let app = create_test_router_with_koreader(state);

    let request = get_request("/koreader/users/auth");
    let (status, _body) = make_request(app, request).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

/// PUT /koreader/syncs/progress updates and GET retrieves progress
#[tokio::test]
async fn test_koreader_sync_progress_roundtrip() {
    let (db, _tmp) = setup_test_db().await;
    let state = create_test_app_state(db.clone()).await;
    let (user_id, token) = setup_admin_and_token(&db, &state).await;

    // Create library, series, and book with a koreader_hash
    let library =
        LibraryRepository::create(&db, "Test Library", "/test", ScanningStrategy::Default)
            .await
            .unwrap();
    let series = SeriesRepository::create(&db, library.id, "Test Series", None)
        .await
        .unwrap();

    let koreader_hash = "abc123def456";
    let mut book = create_test_book(
        series.id,
        library.id,
        "/test/book1.cbz",
        "book1.cbz",
        "hash1",
        "cbz",
        100,
    );
    book.koreader_hash = Some(koreader_hash.to_string());
    let book = BookRepository::create(&db, &book, None).await.unwrap();

    // PUT progress
    let progress = serde_json::json!({
        "document": koreader_hash,
        "progress": "42",
        "percentage": 0.42,
        "device": "test-device",
        "device_id": "device-123"
    });

    let app = create_test_router_with_koreader(state.clone());
    let request = put_request_with_auth(
        "/koreader/syncs/progress",
        &serde_json::to_string(&progress).unwrap(),
        &token,
    );
    let (status, _body) = make_request(app, request).await;
    assert_eq!(status, StatusCode::OK);

    // Verify progress was stored in DB
    let stored = ReadProgressRepository::get_by_user_and_book(&db, user_id, book.id)
        .await
        .unwrap()
        .expect("Progress should exist");
    assert_eq!(stored.current_page, 42);
    assert_eq!(stored.progress_percentage, Some(0.42));

    // GET progress back
    let app = create_test_router_with_koreader(state);
    let request = get_request_with_auth(
        &format!("/koreader/syncs/progress/{}", koreader_hash),
        &token,
    );
    let (status, body) = make_json_request::<DocumentProgressDto>(app, request).await;
    assert_eq!(status, StatusCode::OK);
    let body = body.unwrap();
    assert_eq!(body.document, koreader_hash);
    assert_eq!(body.progress, "42");
    assert!((body.percentage - 0.42).abs() < 0.001);
}

/// GET /koreader/syncs/progress/{hash} returns 404 for unknown hash
#[tokio::test]
async fn test_koreader_get_progress_unknown_hash() {
    let (db, _tmp) = setup_test_db().await;
    let state = create_test_app_state(db.clone()).await;
    let (_user_id, token) = setup_admin_and_token(&db, &state).await;
    let app = create_test_router_with_koreader(state);

    let request = get_request_with_auth("/koreader/syncs/progress/nonexistent_hash", &token);
    let (status, _body) = make_request(app, request).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}
