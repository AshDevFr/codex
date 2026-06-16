#[path = "../common/mod.rs"]
mod common;

use codex::api::error::ErrorResponse;
use codex::api::routes::v1::dto::{BookDto, ReadListDto, ReadListListResponse};
use codex::db::ScanningStrategy;
use codex::db::repositories::{
    BookRepository, LibraryRepository, SeriesRepository, UserRepository,
};
use codex::utils::password;
use common::*;
use hyper::StatusCode;

async fn user_and_token(
    db: &sea_orm::DatabaseConnection,
    state: &codex::api::extractors::AuthState,
    username: &str,
    is_admin: bool,
) -> (uuid::Uuid, String) {
    let password_hash = password::hash_password("pw123456").unwrap();
    let user = create_test_user(
        username,
        &format!("{username}@example.com"),
        &password_hash,
        is_admin,
    );
    let created = UserRepository::create(db, &user).await.unwrap();
    let token = state
        .jwt_service
        .generate_token(created.id, created.username.clone(), created.get_role())
        .unwrap();
    (created.id, token)
}

/// Create a library, a series, and N books under it.
async fn make_books(
    db: &sea_orm::DatabaseConnection,
    count: usize,
) -> Vec<codex::db::entities::books::Model> {
    use chrono::Utc;
    let library = LibraryRepository::create(db, "Lib", "/test", ScanningStrategy::Default)
        .await
        .unwrap();
    let series = SeriesRepository::create(db, library.id, "Series", None)
        .await
        .unwrap();
    let mut out = Vec::new();
    for _ in 0..count {
        let book = codex::db::entities::books::Model {
            id: uuid::Uuid::new_v4(),
            series_id: series.id,
            library_id: library.id,
            path: format!("/test/{}.cbz", uuid::Uuid::new_v4()),
            file_name: "book.cbz".to_string(),
            file_size: 1024,
            file_hash: format!("hash_{}", uuid::Uuid::new_v4()),
            partial_hash: String::new(),
            format: "cbz".to_string(),
            page_count: 10,
            deleted: false,
            analyzed: false,
            analysis_error: None,
            analysis_errors: None,
            modified_at: Utc::now(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            thumbnail_path: None,
            thumbnail_generated_at: None,
            koreader_hash: None,
            epub_positions: None,
            epub_spine_items: None,
        };
        out.push(BookRepository::create(db, &book, None).await.unwrap());
    }
    out
}

#[tokio::test]
async fn test_create_with_summary_get_and_list() {
    let (db, _t) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_uid, token) = user_and_token(&db, &state, "admin", true).await;
    let app = create_test_router(state).await;

    let req = post_json_request_with_auth(
        "/api/v1/readlists",
        &serde_json::json!({ "name": "Civil War", "summary": "Crossover event" }),
        &token,
    );
    let (status, created): (StatusCode, Option<ReadListDto>) =
        make_json_request(app.clone(), req).await;
    assert_eq!(status, StatusCode::CREATED);
    let created = created.unwrap();
    assert_eq!(created.name, "Civil War");
    assert_eq!(created.summary.as_deref(), Some("Crossover event"));
    // Read lists default to ordered = true.
    assert!(created.ordered);
    assert_eq!(created.book_count, 0);

    let req = get_request_with_auth(&format!("/api/v1/readlists/{}", created.id), &token);
    let (status, fetched): (StatusCode, Option<ReadListDto>) =
        make_json_request(app.clone(), req).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(fetched.unwrap().id, created.id);

    let req = get_request_with_auth("/api/v1/readlists", &token);
    let (status, list): (StatusCode, Option<ReadListListResponse>) =
        make_json_request(app, req).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(list.unwrap().total, 1);
}

#[tokio::test]
async fn test_update_clears_summary() {
    let (db, _t) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_uid, token) = user_and_token(&db, &state, "admin", true).await;
    let app = create_test_router(state).await;

    let req = post_json_request_with_auth(
        "/api/v1/readlists",
        &serde_json::json!({ "name": "List", "summary": "to be cleared" }),
        &token,
    );
    let (_s, created): (StatusCode, Option<ReadListDto>) =
        make_json_request(app.clone(), req).await;
    let id = created.unwrap().id;

    // Explicit null clears the summary; ordered toggled off.
    let req = patch_json_request_with_auth(
        &format!("/api/v1/readlists/{id}"),
        &serde_json::json!({ "summary": null, "ordered": false }),
        &token,
    );
    let (status, updated): (StatusCode, Option<ReadListDto>) = make_json_request(app, req).await;
    assert_eq!(status, StatusCode::OK);
    let updated = updated.unwrap();
    assert_eq!(updated.summary, None);
    assert!(!updated.ordered);
}

#[tokio::test]
async fn test_permission_matrix() {
    let (db, _t) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_admin, admin_token) = user_and_token(&db, &state, "admin", true).await;
    let (_reader, reader_token) = user_and_token(&db, &state, "reader", false).await;
    let app = create_test_router(state).await;

    // Reader can list.
    let req = get_request_with_auth("/api/v1/readlists", &reader_token);
    let (status, _): (StatusCode, Option<ReadListListResponse>) =
        make_json_request(app.clone(), req).await;
    assert_eq!(status, StatusCode::OK);

    // Reader cannot create.
    let req = post_json_request_with_auth(
        "/api/v1/readlists",
        &serde_json::json!({ "name": "Nope" }),
        &reader_token,
    );
    let (status, _): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app.clone(), req).await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    // Admin can create and delete.
    let req = post_json_request_with_auth(
        "/api/v1/readlists",
        &serde_json::json!({ "name": "Admin List" }),
        &admin_token,
    );
    let (status, created): (StatusCode, Option<ReadListDto>) =
        make_json_request(app.clone(), req).await;
    assert_eq!(status, StatusCode::CREATED);
    let id = created.unwrap().id;

    let req = delete_request_with_auth(&format!("/api/v1/readlists/{id}"), &reader_token);
    let (status, _): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app.clone(), req).await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let req = delete_request_with_auth(&format!("/api/v1/readlists/{id}"), &admin_token);
    let (status, _): (StatusCode, Option<String>) = make_json_request(app, req).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn test_member_management_order_and_count() {
    let (db, _t) = setup_test_db().await;
    let books = make_books(&db, 3).await;

    let state = create_test_auth_state(db.clone()).await;
    let (_uid, token) = user_and_token(&db, &state, "admin", true).await;
    let app = create_test_router(state).await;

    let req = post_json_request_with_auth(
        "/api/v1/readlists",
        &serde_json::json!({ "name": "List", "ordered": true }),
        &token,
    );
    let (_s, rl): (StatusCode, Option<ReadListDto>) = make_json_request(app.clone(), req).await;
    let rl_id = rl.unwrap().id;

    // Add all three books.
    let req = post_json_request_with_auth(
        &format!("/api/v1/readlists/{rl_id}/books"),
        &serde_json::json!({ "bookIds": books.iter().map(|b| b.id).collect::<Vec<_>>() }),
        &token,
    );
    let (status, updated): (StatusCode, Option<ReadListDto>) =
        make_json_request(app.clone(), req).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(updated.unwrap().book_count, 3);

    // Members in insertion order.
    let req = get_request_with_auth(&format!("/api/v1/readlists/{rl_id}/books"), &token);
    let (status, members): (StatusCode, Option<Vec<BookDto>>) =
        make_json_request(app.clone(), req).await;
    assert_eq!(status, StatusCode::OK);
    let members = members.unwrap();
    assert_eq!(members.len(), 3);
    assert_eq!(members[0].id, books[0].id);

    // Reorder reversed.
    let reversed: Vec<_> = books.iter().rev().map(|b| b.id).collect();
    let req = put_json_request_with_auth(
        &format!("/api/v1/readlists/{rl_id}/books"),
        &serde_json::json!({ "bookIds": reversed }),
        &token,
    );
    let (status, _): (StatusCode, Option<String>) = make_json_request(app.clone(), req).await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let req = get_request_with_auth(&format!("/api/v1/readlists/{rl_id}/books"), &token);
    let (_s, members): (StatusCode, Option<Vec<BookDto>>) =
        make_json_request(app.clone(), req).await;
    assert_eq!(members.unwrap()[0].id, books[2].id);

    // Remove the middle book.
    let req = delete_request_with_auth(
        &format!("/api/v1/readlists/{rl_id}/books/{}", books[1].id),
        &token,
    );
    let (status, _): (StatusCode, Option<String>) = make_json_request(app.clone(), req).await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    // books/{id}/readlists reverse lookup.
    let req = get_request_with_auth(&format!("/api/v1/books/{}/readlists", books[0].id), &token);
    let (status, containers): (StatusCode, Option<ReadListListResponse>) =
        make_json_request(app, req).await;
    assert_eq!(status, StatusCode::OK);
    let containers = containers.unwrap();
    assert_eq!(containers.total, 1);
    assert_eq!(containers.items[0].id, rl_id);
}

#[tokio::test]
async fn test_add_nonexistent_book_and_duplicate_name() {
    let (db, _t) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_uid, token) = user_and_token(&db, &state, "admin", true).await;
    let app = create_test_router(state).await;

    let body = serde_json::json!({ "name": "Dupe" });
    let req = post_json_request_with_auth("/api/v1/readlists", &body, &token);
    let (_s, rl): (StatusCode, Option<ReadListDto>) = make_json_request(app.clone(), req).await;
    let rl_id = rl.unwrap().id;

    // Duplicate name -> 409.
    let req = post_json_request_with_auth("/api/v1/readlists", &body, &token);
    let (status, _): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app.clone(), req).await;
    assert_eq!(status, StatusCode::CONFLICT);

    // Adding an unknown book -> 404.
    let req = post_json_request_with_auth(
        &format!("/api/v1/readlists/{rl_id}/books"),
        &serde_json::json!({ "bookIds": [uuid::Uuid::new_v4()] }),
        &token,
    );
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, req).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}
