#[path = "../common/mod.rs"]
mod common;

use codex::api::error::ErrorResponse;
use codex::api::routes::v1::dto::{CollectionDto, CollectionListResponse, SeriesDto};
use codex::db::ScanningStrategy;
use codex::db::repositories::{LibraryRepository, SeriesRepository, UserRepository};
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

/// A reader carrying an explicit `collections-write` custom permission.
async fn reader_with_write_token(
    db: &sea_orm::DatabaseConnection,
    state: &codex::api::extractors::AuthState,
) -> String {
    let password_hash = password::hash_password("pw123456").unwrap();
    let user = create_test_user_with_permissions(
        "editor",
        "editor@example.com",
        &password_hash,
        false,
        vec![
            "collections-read".to_string(),
            "collections-write".to_string(),
        ],
    );
    let created = UserRepository::create(db, &user).await.unwrap();
    state
        .jwt_service
        .generate_token(created.id, created.username.clone(), created.get_role())
        .unwrap()
}

async fn make_series(
    db: &sea_orm::DatabaseConnection,
    name: &str,
) -> codex::db::entities::series::Model {
    let library = LibraryRepository::create(db, "Lib", "/test", ScanningStrategy::Default)
        .await
        .unwrap();
    SeriesRepository::create(db, library.id, name, None)
        .await
        .unwrap()
}

/// Create N series under a single shared library.
async fn make_series_in_library(
    db: &sea_orm::DatabaseConnection,
    names: &[&str],
) -> Vec<codex::db::entities::series::Model> {
    let library = LibraryRepository::create(db, "Lib", "/test", ScanningStrategy::Default)
        .await
        .unwrap();
    let mut out = Vec::new();
    for name in names {
        out.push(
            SeriesRepository::create(db, library.id, name, None)
                .await
                .unwrap(),
        );
    }
    out
}

#[tokio::test]
async fn test_create_get_and_list_collection() {
    let (db, _t) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_uid, token) = user_and_token(&db, &state, "admin", true).await;
    let app = create_test_router(state).await;

    let req = post_json_request_with_auth(
        "/api/v1/collections",
        &serde_json::json!({ "name": "Batman", "ordered": true }),
        &token,
    );
    let (status, created): (StatusCode, Option<CollectionDto>) =
        make_json_request(app.clone(), req).await;
    assert_eq!(status, StatusCode::CREATED);
    let created = created.unwrap();
    assert_eq!(created.name, "Batman");
    assert!(created.ordered);
    assert_eq!(created.series_count, 0);

    let req = get_request_with_auth(&format!("/api/v1/collections/{}", created.id), &token);
    let (status, fetched): (StatusCode, Option<CollectionDto>) =
        make_json_request(app.clone(), req).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(fetched.unwrap().id, created.id);

    let req = get_request_with_auth("/api/v1/collections", &token);
    let (status, list): (StatusCode, Option<CollectionListResponse>) =
        make_json_request(app, req).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(list.unwrap().total, 1);
}

#[tokio::test]
async fn test_create_rejects_empty_and_duplicate_name() {
    let (db, _t) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_uid, token) = user_and_token(&db, &state, "admin", true).await;
    let app = create_test_router(state).await;

    let req = post_json_request_with_auth(
        "/api/v1/collections",
        &serde_json::json!({ "name": "   " }),
        &token,
    );
    let (status, _): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app.clone(), req).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let body = serde_json::json!({ "name": "Marvel" });
    let req = post_json_request_with_auth("/api/v1/collections", &body, &token);
    let (status, _): (StatusCode, Option<CollectionDto>) =
        make_json_request(app.clone(), req).await;
    assert_eq!(status, StatusCode::CREATED);

    let req = post_json_request_with_auth("/api/v1/collections", &body, &token);
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, req).await;
    assert_eq!(status, StatusCode::CONFLICT);
}

#[tokio::test]
async fn test_permission_matrix() {
    let (db, _t) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_admin, admin_token) = user_and_token(&db, &state, "admin", true).await;
    let (_reader, reader_token) = user_and_token(&db, &state, "reader", false).await;
    let editor_token = reader_with_write_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Reader can list (CollectionsRead is in the reader bundle).
    let req = get_request_with_auth("/api/v1/collections", &reader_token);
    let (status, _): (StatusCode, Option<CollectionListResponse>) =
        make_json_request(app.clone(), req).await;
    assert_eq!(status, StatusCode::OK);

    // Reader cannot create (no CollectionsWrite).
    let body = serde_json::json!({ "name": "Nope" });
    let req = post_json_request_with_auth("/api/v1/collections", &body, &reader_token);
    let (status, _): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app.clone(), req).await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    // A reader with an explicit collections-write permission can create.
    let req = post_json_request_with_auth(
        "/api/v1/collections",
        &serde_json::json!({ "name": "Editor's Pick" }),
        &editor_token,
    );
    let (status, _): (StatusCode, Option<CollectionDto>) =
        make_json_request(app.clone(), req).await;
    assert_eq!(status, StatusCode::CREATED);

    // Admin can create.
    let req = post_json_request_with_auth("/api/v1/collections", &body, &admin_token);
    let (status, created): (StatusCode, Option<CollectionDto>) =
        make_json_request(app.clone(), req).await;
    assert_eq!(status, StatusCode::CREATED);
    let created = created.unwrap();

    // Reader cannot delete (no CollectionsDelete; editor lacks it too).
    let req = delete_request_with_auth(
        &format!("/api/v1/collections/{}", created.id),
        &reader_token,
    );
    let (status, _): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app.clone(), req).await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    // Admin can delete.
    let req =
        delete_request_with_auth(&format!("/api/v1/collections/{}", created.id), &admin_token);
    let (status, _): (StatusCode, Option<String>) = make_json_request(app, req).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn test_member_management_order_and_count() {
    let (db, _t) = setup_test_db().await;
    let series = make_series_in_library(&db, &["Alpha", "Bravo", "Charlie"]).await;

    let state = create_test_auth_state(db.clone()).await;
    let (_uid, token) = user_and_token(&db, &state, "admin", true).await;
    let app = create_test_router(state).await;

    // Ordered collection so manual order is honored.
    let req = post_json_request_with_auth(
        "/api/v1/collections",
        &serde_json::json!({ "name": "Coll", "ordered": true }),
        &token,
    );
    let (_s, coll): (StatusCode, Option<CollectionDto>) = make_json_request(app.clone(), req).await;
    let coll_id = coll.unwrap().id;

    // Add all three.
    let req = post_json_request_with_auth(
        &format!("/api/v1/collections/{coll_id}/series"),
        &serde_json::json!({ "seriesIds": series.iter().map(|s| s.id).collect::<Vec<_>>() }),
        &token,
    );
    let (status, updated): (StatusCode, Option<CollectionDto>) =
        make_json_request(app.clone(), req).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(updated.unwrap().series_count, 3);

    // Members come back in insertion order.
    let req = get_request_with_auth(&format!("/api/v1/collections/{coll_id}/series"), &token);
    let (status, members): (StatusCode, Option<Vec<SeriesDto>>) =
        make_json_request(app.clone(), req).await;
    assert_eq!(status, StatusCode::OK);
    let members = members.unwrap();
    assert_eq!(members.len(), 3);
    assert_eq!(members[0].id, series[0].id);
    assert_eq!(members[2].id, series[2].id);

    // Reorder reversed.
    let reversed: Vec<_> = series.iter().rev().map(|s| s.id).collect();
    let req = put_json_request_with_auth(
        &format!("/api/v1/collections/{coll_id}/series"),
        &serde_json::json!({ "seriesIds": reversed }),
        &token,
    );
    let (status, _): (StatusCode, Option<String>) = make_json_request(app.clone(), req).await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let req = get_request_with_auth(&format!("/api/v1/collections/{coll_id}/series"), &token);
    let (_s, members): (StatusCode, Option<Vec<SeriesDto>>) =
        make_json_request(app.clone(), req).await;
    let members = members.unwrap();
    assert_eq!(members[0].id, series[2].id);
    assert_eq!(members[2].id, series[0].id);

    // Remove the middle series.
    let req = delete_request_with_auth(
        &format!("/api/v1/collections/{coll_id}/series/{}", series[1].id),
        &token,
    );
    let (status, _): (StatusCode, Option<String>) = make_json_request(app.clone(), req).await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    // series/{id}/collections reverse lookup includes this collection.
    let req = get_request_with_auth(
        &format!("/api/v1/series/{}/collections", series[0].id),
        &token,
    );
    let (status, containers): (StatusCode, Option<CollectionListResponse>) =
        make_json_request(app, req).await;
    assert_eq!(status, StatusCode::OK);
    let containers = containers.unwrap();
    assert_eq!(containers.total, 1);
    assert_eq!(containers.items[0].id, coll_id);
}

#[tokio::test]
async fn test_update_and_not_found() {
    let (db, _t) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_uid, token) = user_and_token(&db, &state, "admin", true).await;
    let app = create_test_router(state).await;

    let req = post_json_request_with_auth(
        "/api/v1/collections",
        &serde_json::json!({ "name": "Old" }),
        &token,
    );
    let (_s, coll): (StatusCode, Option<CollectionDto>) = make_json_request(app.clone(), req).await;
    let coll_id = coll.unwrap().id;

    let req = patch_json_request_with_auth(
        &format!("/api/v1/collections/{coll_id}"),
        &serde_json::json!({ "name": "New", "ordered": true }),
        &token,
    );
    let (status, updated): (StatusCode, Option<CollectionDto>) =
        make_json_request(app.clone(), req).await;
    assert_eq!(status, StatusCode::OK);
    let updated = updated.unwrap();
    assert_eq!(updated.name, "New");
    assert!(updated.ordered);

    // Unknown collection -> 404.
    let req = get_request_with_auth(
        &format!("/api/v1/collections/{}", uuid::Uuid::new_v4()),
        &token,
    );
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, req).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_add_nonexistent_series_returns_404() {
    let (db, _t) = setup_test_db().await;
    let _series = make_series(&db, "Solo").await;
    let state = create_test_auth_state(db.clone()).await;
    let (_uid, token) = user_and_token(&db, &state, "admin", true).await;
    let app = create_test_router(state).await;

    let req = post_json_request_with_auth(
        "/api/v1/collections",
        &serde_json::json!({ "name": "Coll" }),
        &token,
    );
    let (_s, coll): (StatusCode, Option<CollectionDto>) = make_json_request(app.clone(), req).await;
    let coll_id = coll.unwrap().id;

    let req = post_json_request_with_auth(
        &format!("/api/v1/collections/{coll_id}/series"),
        &serde_json::json!({ "seriesIds": [uuid::Uuid::new_v4()] }),
        &token,
    );
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, req).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}
