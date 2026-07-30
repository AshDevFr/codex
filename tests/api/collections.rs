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
    assert_eq!(created.series_count, Some(0));

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
    assert_eq!(updated.unwrap().series_count, Some(3));

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
async fn test_unordered_collection_series_sorting() {
    use codex::db::repositories::SeriesMetadataRepository;

    let (db, _t) = setup_test_db().await;
    // Insertion order deliberately differs from title order.
    let series = make_series_in_library(&db, &["Bravo", "Charlie", "Alpha"]).await;

    let state = create_test_auth_state(db.clone()).await;
    let (_uid, token) = user_and_token(&db, &state, "admin", true).await;
    let app = create_test_router(state).await;

    let req = post_json_request_with_auth(
        "/api/v1/collections",
        &serde_json::json!({ "name": "Coll", "ordered": false }),
        &token,
    );
    let (_s, coll): (StatusCode, Option<CollectionDto>) = make_json_request(app.clone(), req).await;
    let coll_id = coll.unwrap().id;

    let req = post_json_request_with_auth(
        &format!("/api/v1/collections/{coll_id}/series"),
        &serde_json::json!({ "seriesIds": series.iter().map(|s| s.id).collect::<Vec<_>>() }),
        &token,
    );
    let (status, _): (StatusCode, Option<CollectionDto>) =
        make_json_request(app.clone(), req).await;
    assert_eq!(status, StatusCode::OK);

    // Default: sorted by title, not insertion order.
    let req = get_request_with_auth(&format!("/api/v1/collections/{coll_id}/series"), &token);
    let (status, members): (StatusCode, Option<Vec<SeriesDto>>) =
        make_json_request(app.clone(), req).await;
    assert_eq!(status, StatusCode::OK);
    let ids: Vec<_> = members.unwrap().iter().map(|s| s.id).collect();
    assert_eq!(ids, [series[2].id, series[0].id, series[1].id]);

    // sort=added: insertion order.
    let req = get_request_with_auth(
        &format!("/api/v1/collections/{coll_id}/series?sort=added"),
        &token,
    );
    let (status, members): (StatusCode, Option<Vec<SeriesDto>>) =
        make_json_request(app.clone(), req).await;
    assert_eq!(status, StatusCode::OK);
    let ids: Vec<_> = members.unwrap().iter().map(|s| s.id).collect();
    assert_eq!(ids, [series[0].id, series[1].id, series[2].id]);

    // direction=desc reverses the title order.
    let req = get_request_with_auth(
        &format!("/api/v1/collections/{coll_id}/series?sort=title&direction=desc"),
        &token,
    );
    let (status, members): (StatusCode, Option<Vec<SeriesDto>>) =
        make_json_request(app.clone(), req).await;
    assert_eq!(status, StatusCode::OK);
    let ids: Vec<_> = members.unwrap().iter().map(|s| s.id).collect();
    assert_eq!(ids, [series[1].id, series[0].id, series[2].id]);

    // sort=year: release year ascending, unknown years last.
    SeriesMetadataRepository::update_year(&db, series[0].id, Some(2020))
        .await
        .unwrap();
    SeriesMetadataRepository::update_year(&db, series[1].id, Some(1999))
        .await
        .unwrap();
    let req = get_request_with_auth(
        &format!("/api/v1/collections/{coll_id}/series?sort=year"),
        &token,
    );
    let (status, members): (StatusCode, Option<Vec<SeriesDto>>) = make_json_request(app, req).await;
    assert_eq!(status, StatusCode::OK);
    let ids: Vec<_> = members.unwrap().iter().map(|s| s.id).collect();
    assert_eq!(ids, [series[1].id, series[0].id, series[2].id]);
}

#[tokio::test]
async fn test_ordered_collection_defaults_to_manual_and_honors_explicit_sort() {
    let (db, _t) = setup_test_db().await;
    let series = make_series_in_library(&db, &["Bravo", "Charlie", "Alpha"]).await;

    let state = create_test_auth_state(db.clone()).await;
    let (_uid, token) = user_and_token(&db, &state, "admin", true).await;
    let app = create_test_router(state).await;

    let req = post_json_request_with_auth(
        "/api/v1/collections",
        &serde_json::json!({ "name": "Coll", "ordered": true }),
        &token,
    );
    let (_s, coll): (StatusCode, Option<CollectionDto>) = make_json_request(app.clone(), req).await;
    let coll_id = coll.unwrap().id;

    let req = post_json_request_with_auth(
        &format!("/api/v1/collections/{coll_id}/series"),
        &serde_json::json!({ "seriesIds": series.iter().map(|s| s.id).collect::<Vec<_>>() }),
        &token,
    );
    let (status, _): (StatusCode, Option<CollectionDto>) =
        make_json_request(app.clone(), req).await;
    assert_eq!(status, StatusCode::OK);

    // No sort param: the ordered flag picks manual (insertion) order.
    let req = get_request_with_auth(&format!("/api/v1/collections/{coll_id}/series"), &token);
    let (status, members): (StatusCode, Option<Vec<SeriesDto>>) =
        make_json_request(app.clone(), req).await;
    assert_eq!(status, StatusCode::OK);
    let ids: Vec<_> = members.unwrap().iter().map(|s| s.id).collect();
    assert_eq!(ids, [series[0].id, series[1].id, series[2].id]);

    // An explicit sort overrides the flag's default (names: Bravo/Charlie/Alpha).
    let req = get_request_with_auth(
        &format!("/api/v1/collections/{coll_id}/series?sort=title"),
        &token,
    );
    let (status, members): (StatusCode, Option<Vec<SeriesDto>>) =
        make_json_request(app.clone(), req).await;
    assert_eq!(status, StatusCode::OK);
    let ids: Vec<_> = members.unwrap().iter().map(|s| s.id).collect();
    assert_eq!(ids, [series[2].id, series[0].id, series[1].id]);

    // And manual order can be requested explicitly.
    let req = get_request_with_auth(
        &format!("/api/v1/collections/{coll_id}/series?sort=manual"),
        &token,
    );
    let (status, members): (StatusCode, Option<Vec<SeriesDto>>) = make_json_request(app, req).await;
    assert_eq!(status, StatusCode::OK);
    let ids: Vec<_> = members.unwrap().iter().map(|s| s.id).collect();
    assert_eq!(ids, [series[0].id, series[1].id, series[2].id]);
}

#[tokio::test]
async fn test_summary_create_update_and_clear() {
    let (db, _t) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_uid, token) = user_and_token(&db, &state, "admin", true).await;
    let app = create_test_router(state).await;

    let req = post_json_request_with_auth(
        "/api/v1/collections",
        &serde_json::json!({ "name": "Batman", "summary": "Essential arcs" }),
        &token,
    );
    let (status, created): (StatusCode, Option<CollectionDto>) =
        make_json_request(app.clone(), req).await;
    assert_eq!(status, StatusCode::CREATED);
    let created = created.unwrap();
    assert_eq!(created.summary.as_deref(), Some("Essential arcs"));

    // Absent field leaves the summary unchanged.
    let req = patch_json_request_with_auth(
        &format!("/api/v1/collections/{}", created.id),
        &serde_json::json!({ "name": "Bat" }),
        &token,
    );
    let (_s, updated): (StatusCode, Option<CollectionDto>) =
        make_json_request(app.clone(), req).await;
    assert_eq!(updated.unwrap().summary.as_deref(), Some("Essential arcs"));

    // Explicit null clears it.
    let req = patch_json_request_with_auth(
        &format!("/api/v1/collections/{}", created.id),
        &serde_json::json!({ "summary": null }),
        &token,
    );
    let (_s, updated): (StatusCode, Option<CollectionDto>) = make_json_request(app, req).await;
    assert_eq!(updated.unwrap().summary, None);
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

// ============================================================================
// Automatic (rule-backed) collections
// ============================================================================

/// A tag rule as request JSON. Built as raw `serde_json` rather than typed
/// conditions so the tests exercise the same wire shape a client sends.
fn tag_rule(tag: &str) -> serde_json::Value {
    serde_json::json!({ "tag": { "operator": "is", "value": tag } })
}

async fn set_tag(db: &sea_orm::DatabaseConnection, series_id: uuid::Uuid, tag: &str) {
    codex::db::repositories::TagRepository::set_tags_for_series(
        db,
        series_id,
        vec![tag.to_string()],
    )
    .await
    .unwrap();
}

/// Create an automatic collection over the API and return its DTO.
async fn create_auto_collection(
    app: axum::Router,
    token: &str,
    name: &str,
    rule: serde_json::Value,
) -> CollectionDto {
    let body = serde_json::json!({ "name": name, "condition": rule });
    let req = post_json_request_with_auth("/api/v1/collections", &body, token);
    let (status, response): (StatusCode, Option<CollectionDto>) = make_json_request(app, req).await;
    assert_eq!(status, StatusCode::CREATED, "creating '{name}'");
    response.unwrap()
}

#[tokio::test]
async fn test_create_automatic_collection_and_browse_it() {
    let (db, _tmp) = setup_test_db().await;
    let series = make_series_in_library(&db, &["Isekai One", "Isekai Two", "Mecha"]).await;
    set_tag(&db, series[0].id, "isekai").await;
    set_tag(&db, series[1].id, "isekai").await;
    set_tag(&db, series[2].id, "mecha").await;

    let state = create_test_auth_state(db.clone()).await;
    let (_id, token) = user_and_token(&db, &state, "admin", true).await;

    let dto = create_auto_collection(
        create_test_router(state.clone()).await,
        &token,
        "Isekai",
        tag_rule("isekai"),
    )
    .await;

    assert!(dto.automatic, "a collection with a rule is automatic");
    assert!(dto.condition.is_some());
    assert_eq!(
        dto.series_count, None,
        "seriesCount is null for automatic collections"
    );
    assert!(!dto.ordered, "ordered is forced off");

    // Browsable immediately, with no population step.
    let app = create_test_router(state).await;
    let req = get_request_with_auth(&format!("/api/v1/collections/{}/series", dto.id), &token);
    let (status, members): (StatusCode, Option<Vec<SeriesDto>>) = make_json_request(app, req).await;
    assert_eq!(status, StatusCode::OK);
    let titles: Vec<String> = members.unwrap().into_iter().map(|s| s.title).collect();
    assert_eq!(titles, vec!["Isekai One", "Isekai Two"]);
}

/// A reader (no write permission) can browse an automatic collection.
#[tokio::test]
async fn test_reader_can_browse_an_automatic_collection() {
    let (db, _tmp) = setup_test_db().await;
    let series = make_series_in_library(&db, &["Alpha"]).await;
    set_tag(&db, series[0].id, "pick").await;

    let state = create_test_auth_state(db.clone()).await;
    let (_id, admin_token) = user_and_token(&db, &state, "admin", true).await;
    let (_rid, reader_token) = user_and_token(&db, &state, "reader", false).await;

    let dto = create_auto_collection(
        create_test_router(state.clone()).await,
        &admin_token,
        "Picks",
        tag_rule("pick"),
    )
    .await;

    let app = create_test_router(state).await;
    let req = get_request_with_auth(
        &format!("/api/v1/collections/{}/series", dto.id),
        &reader_token,
    );
    let (status, members): (StatusCode, Option<Vec<SeriesDto>>) = make_json_request(app, req).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(members.unwrap().len(), 1);
}

#[tokio::test]
async fn test_editing_the_rule_changes_members_on_the_next_read() {
    let (db, _tmp) = setup_test_db().await;
    let series = make_series_in_library(&db, &["Alpha", "Beta"]).await;
    set_tag(&db, series[0].id, "isekai").await;
    set_tag(&db, series[1].id, "mecha").await;

    let state = create_test_auth_state(db.clone()).await;
    let (_id, token) = user_and_token(&db, &state, "admin", true).await;

    let dto = create_auto_collection(
        create_test_router(state.clone()).await,
        &token,
        "Themed",
        tag_rule("isekai"),
    )
    .await;

    let app = create_test_router(state.clone()).await;
    let req = patch_json_request_with_auth(
        &format!("/api/v1/collections/{}", dto.id),
        &serde_json::json!({ "condition": tag_rule("mecha") }),
        &token,
    );
    let (status, updated): (StatusCode, Option<CollectionDto>) = make_json_request(app, req).await;
    assert_eq!(status, StatusCode::OK);
    assert!(updated.unwrap().automatic);

    let app = create_test_router(state).await;
    let req = get_request_with_auth(&format!("/api/v1/collections/{}/series", dto.id), &token);
    let (status, members): (StatusCode, Option<Vec<SeriesDto>>) = make_json_request(app, req).await;
    assert_eq!(status, StatusCode::OK);
    let titles: Vec<String> = members.unwrap().into_iter().map(|s| s.title).collect();
    assert_eq!(titles, vec!["Beta"]);
}

/// Clearing the rule converts the collection to manual and leaves it empty: it
/// never had hand-picked members.
#[tokio::test]
async fn test_clearing_the_rule_converts_to_an_empty_manual_collection() {
    let (db, _tmp) = setup_test_db().await;
    let series = make_series_in_library(&db, &["Alpha"]).await;
    set_tag(&db, series[0].id, "pick").await;

    let state = create_test_auth_state(db.clone()).await;
    let (_id, token) = user_and_token(&db, &state, "admin", true).await;

    let dto = create_auto_collection(
        create_test_router(state.clone()).await,
        &token,
        "Picks",
        tag_rule("pick"),
    )
    .await;

    let app = create_test_router(state.clone()).await;
    let req = patch_json_request_with_auth(
        &format!("/api/v1/collections/{}", dto.id),
        &serde_json::json!({ "condition": null }),
        &token,
    );
    let (status, updated): (StatusCode, Option<CollectionDto>) = make_json_request(app, req).await;
    assert_eq!(status, StatusCode::OK);
    let updated = updated.unwrap();
    assert!(!updated.automatic);
    assert!(updated.condition.is_none());
    assert_eq!(
        updated.series_count,
        Some(0),
        "now manual, so it reports a real count again"
    );

    let app = create_test_router(state.clone()).await;
    let req = get_request_with_auth(&format!("/api/v1/collections/{}/series", dto.id), &token);
    let (status, members): (StatusCode, Option<Vec<SeriesDto>>) = make_json_request(app, req).await;
    assert_eq!(status, StatusCode::OK);
    assert!(members.unwrap().is_empty());

    // And it accepts hand-picked members again.
    let app = create_test_router(state).await;
    let req = post_json_request_with_auth(
        &format!("/api/v1/collections/{}/series", dto.id),
        &serde_json::json!({ "seriesIds": [series[0].id] }),
        &token,
    );
    let (status, _): (StatusCode, Option<CollectionDto>) = make_json_request(app, req).await;
    assert_eq!(status, StatusCode::OK);
}

/// An absent `condition` on PATCH leaves the rule alone; only an explicit null
/// clears it.
#[tokio::test]
async fn test_omitting_condition_on_update_leaves_the_rule_intact() {
    let (db, _tmp) = setup_test_db().await;
    let series = make_series_in_library(&db, &["Alpha"]).await;
    set_tag(&db, series[0].id, "pick").await;

    let state = create_test_auth_state(db.clone()).await;
    let (_id, token) = user_and_token(&db, &state, "admin", true).await;

    let dto = create_auto_collection(
        create_test_router(state.clone()).await,
        &token,
        "Picks",
        tag_rule("pick"),
    )
    .await;

    let app = create_test_router(state).await;
    let req = patch_json_request_with_auth(
        &format!("/api/v1/collections/{}", dto.id),
        &serde_json::json!({ "name": "Renamed" }),
        &token,
    );
    let (status, updated): (StatusCode, Option<CollectionDto>) = make_json_request(app, req).await;
    assert_eq!(status, StatusCode::OK);
    let updated = updated.unwrap();
    assert_eq!(updated.name, "Renamed");
    assert!(updated.automatic, "the rule survived a rename");
}

// ---- Validation ------------------------------------------------------------

#[tokio::test]
async fn test_rule_with_in_collection_is_rejected() {
    let (db, _tmp) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_id, token) = user_and_token(&db, &state, "admin", true).await;
    let app = create_test_router(state).await;

    let body = serde_json::json!({
        "name": "Recursive",
        "condition": { "inCollection": { "operator": "isTrue" } },
    });
    let req = post_json_request_with_auth("/api/v1/collections", &body, &token);
    let (status, error): (StatusCode, Option<ErrorResponse>) = make_json_request(app, req).await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    let message = format!("{:?}", error.unwrap());
    assert!(
        message.contains("inCollection"),
        "the error must name the offending field, got: {message}"
    );
}

/// Nesting it deeper does not get it past validation.
#[tokio::test]
async fn test_nested_in_collection_is_rejected() {
    let (db, _tmp) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_id, token) = user_and_token(&db, &state, "admin", true).await;
    let app = create_test_router(state).await;

    let body = serde_json::json!({
        "name": "Recursive",
        "condition": {
            "allOf": [
                tag_rule("isekai"),
                { "anyOf": [ { "inCollection": { "operator": "isTrue" } } ] },
            ]
        },
    });
    let req = post_json_request_with_auth("/api/v1/collections", &body, &token);
    let (status, error): (StatusCode, Option<ErrorResponse>) = make_json_request(app, req).await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(format!("{:?}", error.unwrap()).contains("inCollection"));
}

#[tokio::test]
async fn test_empty_rule_is_rejected() {
    let (db, _tmp) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_id, token) = user_and_token(&db, &state, "admin", true).await;

    for empty in [
        serde_json::json!({ "allOf": [] }),
        serde_json::json!({ "anyOf": [] }),
    ] {
        let app = create_test_router(state.clone()).await;
        let body = serde_json::json!({ "name": format!("Empty {empty}"), "condition": empty });
        let req = post_json_request_with_auth("/api/v1/collections", &body, &token);
        let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, req).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "empty rule {empty}");
    }
}

/// Validation also runs on update, so a valid collection cannot be edited into
/// an invalid one.
#[tokio::test]
async fn test_update_rejects_an_invalid_rule() {
    let (db, _tmp) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_id, token) = user_and_token(&db, &state, "admin", true).await;

    let dto = create_auto_collection(
        create_test_router(state.clone()).await,
        &token,
        "Picks",
        tag_rule("pick"),
    )
    .await;

    let app = create_test_router(state).await;
    let req = patch_json_request_with_auth(
        &format!("/api/v1/collections/{}", dto.id),
        &serde_json::json!({ "condition": { "inCollection": { "operator": "isTrue" } } }),
        &token,
    );
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, req).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_ordered_is_rejected_alongside_a_rule() {
    let (db, _tmp) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_id, token) = user_and_token(&db, &state, "admin", true).await;

    let app = create_test_router(state.clone()).await;
    let body = serde_json::json!({
        "name": "Ordered auto",
        "ordered": true,
        "condition": tag_rule("pick"),
    });
    let req = post_json_request_with_auth("/api/v1/collections", &body, &token);
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, req).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // Also on update of an existing automatic collection.
    let dto = create_auto_collection(
        create_test_router(state.clone()).await,
        &token,
        "Picks",
        tag_rule("pick"),
    )
    .await;
    let app = create_test_router(state).await;
    let req = patch_json_request_with_auth(
        &format!("/api/v1/collections/{}", dto.id),
        &serde_json::json!({ "ordered": true }),
        &token,
    );
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, req).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

// ---- Write protection ------------------------------------------------------

#[tokio::test]
async fn test_add_series_to_automatic_collection_returns_409() {
    let (db, _tmp) = setup_test_db().await;
    let series = make_series_in_library(&db, &["Alpha"]).await;
    let state = create_test_auth_state(db.clone()).await;
    let (_id, token) = user_and_token(&db, &state, "admin", true).await;

    let dto = create_auto_collection(
        create_test_router(state.clone()).await,
        &token,
        "Picks",
        tag_rule("pick"),
    )
    .await;

    let app = create_test_router(state).await;
    let req = post_json_request_with_auth(
        &format!("/api/v1/collections/{}/series", dto.id),
        &serde_json::json!({ "seriesIds": [series[0].id] }),
        &token,
    );
    let (status, error): (StatusCode, Option<ErrorResponse>) = make_json_request(app, req).await;

    assert_eq!(status, StatusCode::CONFLICT);
    let message = format!("{:?}", error.unwrap());
    assert!(
        message.contains("automatic") && message.contains("rule"),
        "the 409 must explain why, got: {message}"
    );
}

#[tokio::test]
async fn test_remove_series_from_automatic_collection_returns_409() {
    let (db, _tmp) = setup_test_db().await;
    let series = make_series_in_library(&db, &["Alpha"]).await;
    let state = create_test_auth_state(db.clone()).await;
    let (_id, token) = user_and_token(&db, &state, "admin", true).await;

    let dto = create_auto_collection(
        create_test_router(state.clone()).await,
        &token,
        "Picks",
        tag_rule("pick"),
    )
    .await;

    let app = create_test_router(state).await;
    let req = delete_request_with_auth(
        &format!("/api/v1/collections/{}/series/{}", dto.id, series[0].id),
        &token,
    );
    let (status, error): (StatusCode, Option<ErrorResponse>) = make_json_request(app, req).await;

    assert_eq!(status, StatusCode::CONFLICT);
    assert!(format!("{:?}", error.unwrap()).contains("automatic"));
}

#[tokio::test]
async fn test_reorder_automatic_collection_returns_409() {
    let (db, _tmp) = setup_test_db().await;
    let series = make_series_in_library(&db, &["Alpha"]).await;
    let state = create_test_auth_state(db.clone()).await;
    let (_id, token) = user_and_token(&db, &state, "admin", true).await;

    let dto = create_auto_collection(
        create_test_router(state.clone()).await,
        &token,
        "Picks",
        tag_rule("pick"),
    )
    .await;

    let app = create_test_router(state).await;
    let req = put_json_request_with_auth(
        &format!("/api/v1/collections/{}/series", dto.id),
        &serde_json::json!({ "seriesIds": [series[0].id] }),
        &token,
    );
    let (status, error): (StatusCode, Option<ErrorResponse>) = make_json_request(app, req).await;

    assert_eq!(status, StatusCode::CONFLICT);
    assert!(format!("{:?}", error.unwrap()).contains("automatic"));
}

/// Manual collections keep working through every mutation endpoint: the guards
/// must not have caught the common case.
#[tokio::test]
async fn test_manual_collection_mutation_still_works() {
    let (db, _tmp) = setup_test_db().await;
    let series = make_series_in_library(&db, &["Alpha", "Beta"]).await;
    let state = create_test_auth_state(db.clone()).await;
    let (_id, token) = user_and_token(&db, &state, "admin", true).await;

    let app = create_test_router(state.clone()).await;
    let req = post_json_request_with_auth(
        "/api/v1/collections",
        &serde_json::json!({ "name": "Manual", "ordered": true }),
        &token,
    );
    let (status, dto): (StatusCode, Option<CollectionDto>) = make_json_request(app, req).await;
    assert_eq!(status, StatusCode::CREATED);
    let dto = dto.unwrap();
    assert!(!dto.automatic);
    assert!(dto.ordered, "a manual collection keeps ordered");
    assert_eq!(dto.series_count, Some(0));

    let app = create_test_router(state.clone()).await;
    let req = post_json_request_with_auth(
        &format!("/api/v1/collections/{}/series", dto.id),
        &serde_json::json!({ "seriesIds": [series[0].id, series[1].id] }),
        &token,
    );
    let (status, updated): (StatusCode, Option<CollectionDto>) = make_json_request(app, req).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(updated.unwrap().series_count, Some(2));

    let app = create_test_router(state.clone()).await;
    let req = put_json_request_with_auth(
        &format!("/api/v1/collections/{}/series", dto.id),
        &serde_json::json!({ "seriesIds": [series[1].id, series[0].id] }),
        &token,
    );
    let (status, _): (StatusCode, Option<serde_json::Value>) = make_json_request(app, req).await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let app = create_test_router(state).await;
    let req = delete_request_with_auth(
        &format!("/api/v1/collections/{}/series/{}", dto.id, series[0].id),
        &token,
    );
    let (status, _): (StatusCode, Option<serde_json::Value>) = make_json_request(app, req).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
}

/// The list endpoint must not report a count for automatic collections, and must
/// keep reporting one for manual collections in the same response.
#[tokio::test]
async fn test_list_reports_null_count_only_for_automatic_collections() {
    let (db, _tmp) = setup_test_db().await;
    let series = make_series_in_library(&db, &["Alpha"]).await;
    set_tag(&db, series[0].id, "pick").await;

    let state = create_test_auth_state(db.clone()).await;
    let (_id, token) = user_and_token(&db, &state, "admin", true).await;

    let app = create_test_router(state.clone()).await;
    let req = post_json_request_with_auth(
        "/api/v1/collections",
        &serde_json::json!({ "name": "Manual" }),
        &token,
    );
    let (status, manual): (StatusCode, Option<CollectionDto>) = make_json_request(app, req).await;
    assert_eq!(status, StatusCode::CREATED);
    let manual = manual.unwrap();

    let app = create_test_router(state.clone()).await;
    let req = post_json_request_with_auth(
        &format!("/api/v1/collections/{}/series", manual.id),
        &serde_json::json!({ "seriesIds": [series[0].id] }),
        &token,
    );
    let (status, _): (StatusCode, Option<CollectionDto>) = make_json_request(app, req).await;
    assert_eq!(status, StatusCode::OK);

    create_auto_collection(
        create_test_router(state.clone()).await,
        &token,
        "Auto",
        tag_rule("pick"),
    )
    .await;

    let app = create_test_router(state).await;
    let req = get_request_with_auth("/api/v1/collections", &token);
    let (status, listed): (StatusCode, Option<CollectionListResponse>) =
        make_json_request(app, req).await;
    assert_eq!(status, StatusCode::OK);

    let listed = listed.unwrap();
    let auto = listed.items.iter().find(|c| c.automatic).unwrap();
    let hand_picked = listed.items.iter().find(|c| !c.automatic).unwrap();
    assert_eq!(auto.series_count, None);
    assert_eq!(hand_picked.series_count, Some(1));
}

/// A condition outside the grammar is rejected at deserialization, so a
/// malformed rule can never reach the database.
#[tokio::test]
async fn test_rule_outside_the_grammar_is_rejected() {
    let (db, _tmp) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_id, token) = user_and_token(&db, &state, "admin", true).await;
    let app = create_test_router(state).await;

    let body = serde_json::json!({
        "name": "Bogus",
        "condition": { "notAField": { "operator": "is", "value": "x" } },
    });
    let req = post_json_request_with_auth("/api/v1/collections", &body, &token);
    let (status, _): (StatusCode, Option<serde_json::Value>) = make_json_request(app, req).await;
    assert!(
        status == StatusCode::UNPROCESSABLE_ENTITY || status == StatusCode::BAD_REQUEST,
        "expected a 4xx for an ungrammatical rule, got {status}"
    );
}
