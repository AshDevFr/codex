//! End-to-end smoke tests for the fuzzy-search rollout.
//!
//! These tests exercise the `search.fuzzy.enabled` flag end-to-end through the
//! HTTP handlers: seed a small library, build the in-memory index, set the
//! flag, and verify the index-backed responses match expectations.

#[path = "../common/mod.rs"]
mod common;

use codex::api::routes::v1::dto::book::{BookDto, BookListResponse};
use codex::api::routes::v1::dto::filter::{
    BookCondition, BookListRequest, SeriesCondition, SeriesListRequest, UuidOperator,
};
use codex::api::routes::v1::dto::series::{SearchSeriesRequest, SeriesDto, SeriesListResponse};
use codex::db::ScanningStrategy;
use codex::db::repositories::{
    BookMetadataRepository, BookRepository, LibraryRepository, SeriesMetadataRepository,
    SeriesRepository, SettingsRepository, UserRepository,
};
use codex::search::builder::rebuild_into;
use codex::utils::password;
use common::*;
use hyper::StatusCode;
use uuid::Uuid;

async fn seed_book(
    db: &sea_orm::DatabaseConnection,
    series_id: Uuid,
    library_id: Uuid,
    path: &str,
    name: &str,
    title: &str,
) -> codex::db::entities::books::Model {
    use chrono::Utc;
    let now = Utc::now();
    let model = codex::db::entities::books::Model {
        id: Uuid::new_v4(),
        series_id,
        library_id,
        file_path: path.to_string(),
        file_name: name.to_string(),
        file_size: 1024,
        file_hash: format!("hash_{}", Uuid::new_v4()),
        partial_hash: String::new(),
        format: "cbz".to_string(),
        page_count: 10,
        deleted: false,
        analyzed: false,
        analysis_error: None,
        analysis_errors: None,
        modified_at: now,
        created_at: now,
        updated_at: now,
        thumbnail_path: None,
        thumbnail_generated_at: None,
        koreader_hash: None,
        epub_positions: None,
        epub_spine_items: None,
    };
    let created = BookRepository::create(db, &model, None).await.unwrap();
    BookMetadataRepository::create_with_title_and_number(
        db,
        created.id,
        Some(title.to_string()),
        None,
    )
    .await
    .unwrap();
    created
}

async fn create_admin_and_token(
    db: &sea_orm::DatabaseConnection,
    state: &codex::api::extractors::AppState,
) -> String {
    let password_hash = password::hash_password("admin123").unwrap();
    let user = create_test_user("admin", "admin@example.com", &password_hash, true);
    let created = UserRepository::create(db, &user).await.unwrap();
    state
        .jwt_service
        .generate_token(created.id, created.username.clone(), created.get_role())
        .unwrap()
}

async fn enable_fuzzy(db: &sea_orm::DatabaseConnection) {
    SettingsRepository::set(
        db,
        "search.fuzzy.enabled",
        "true".to_string(),
        Uuid::new_v4(),
        Some("Phase 3 smoke test".to_string()),
        None,
    )
    .await
    .expect("Failed to enable fuzzy search setting");
}

async fn seed_series(
    db: &sea_orm::DatabaseConnection,
    library_id: Uuid,
    name: &str,
    title: &str,
) -> codex::db::entities::series::Model {
    // `SeriesRepository::create` already inserts a metadata row using `name`
    // as the title. Update the title in place so the test data has the
    // human-facing string we want to fuzzy-match.
    let series = SeriesRepository::create(db, library_id, name, None)
        .await
        .unwrap();
    SeriesMetadataRepository::update_title(db, series.id, title.to_string(), None, None)
        .await
        .unwrap();
    series
}

#[tokio::test]
async fn fuzzy_search_matches_gap_skipped_query() {
    let (db, _temp_dir) = setup_test_db().await;
    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    let target = seed_series(&db, library.id, "one-punch-man", "One-Punch Man").await;
    seed_series(&db, library.id, "berserk", "Berserk").await;
    seed_series(&db, library.id, "vagabond", "Vagabond").await;
    seed_series(&db, library.id, "20th-century-boys", "20th Century Boys").await;

    enable_fuzzy(&db).await;

    let (state, app) = setup_test_app(db.clone()).await;
    rebuild_into(&state.fuzzy_index, &db).await.unwrap();
    let token = create_admin_and_token(&db, &state).await;

    let request_body = SearchSeriesRequest {
        query: "on ch".to_string(),
        library_id: None,
        full: false,
    };
    let request = post_json_request_with_auth("/api/v1/series/search", &request_body, &token);
    let (status, response): (StatusCode, Option<Vec<SeriesDto>>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let series = response.expect("response body");
    assert!(!series.is_empty(), "expected fuzzy hits for 'on ch'");
    assert_eq!(
        series.first().map(|s| s.id),
        Some(target.id),
        "One-Punch Man should rank first for 'on ch'",
    );
}

#[tokio::test]
async fn fuzzy_search_ignores_punctuation_between_words() {
    let (db, _temp_dir) = setup_test_db().await;
    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    let target = seed_series(&db, library.id, "one-punch-man", "One-Punch Man").await;
    seed_series(&db, library.id, "one-piece", "One Piece").await;
    seed_series(&db, library.id, "punch-out", "Punch Out").await;

    enable_fuzzy(&db).await;

    let (state, app) = setup_test_app(db.clone()).await;
    rebuild_into(&state.fuzzy_index, &db).await.unwrap();
    let token = create_admin_and_token(&db, &state).await;

    let request_body = SearchSeriesRequest {
        query: "one punch".to_string(),
        library_id: None,
        full: false,
    };
    let request = post_json_request_with_auth("/api/v1/series/search", &request_body, &token);
    let (status, response): (StatusCode, Option<Vec<SeriesDto>>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let series = response.expect("response body");
    assert!(
        series.iter().any(|s| s.id == target.id),
        "One-Punch Man should match 'one punch' despite the hyphen, got {:?}",
        series.iter().map(|s| &s.title).collect::<Vec<_>>(),
    );
}

#[tokio::test]
async fn fuzzy_search_empty_query_returns_no_results() {
    let (db, _temp_dir) = setup_test_db().await;
    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();
    seed_series(&db, library.id, "one-punch-man", "One-Punch Man").await;

    enable_fuzzy(&db).await;

    let (state, app) = setup_test_app(db.clone()).await;
    rebuild_into(&state.fuzzy_index, &db).await.unwrap();
    let token = create_admin_and_token(&db, &state).await;

    let request_body = SearchSeriesRequest {
        query: "   ".to_string(),
        library_id: None,
        full: false,
    };
    let request = post_json_request_with_auth("/api/v1/series/search", &request_body, &token);
    let (status, response): (StatusCode, Option<Vec<SeriesDto>>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        response.unwrap().len(),
        0,
        "fuzzy mode returns nothing for whitespace-only queries",
    );
}

#[tokio::test]
async fn fuzzy_search_respects_library_filter() {
    let (db, _temp_dir) = setup_test_db().await;
    let library_a = LibraryRepository::create(&db, "Manga", "/manga", ScanningStrategy::Default)
        .await
        .unwrap();
    let library_b = LibraryRepository::create(&db, "Comics", "/comics", ScanningStrategy::Default)
        .await
        .unwrap();

    let manga_target = seed_series(&db, library_a.id, "one-punch-man", "One-Punch Man").await;
    let comic_target = seed_series(
        &db,
        library_b.id,
        "one-punch-american",
        "One Punch American",
    )
    .await;

    enable_fuzzy(&db).await;

    let (state, app) = setup_test_app(db.clone()).await;
    rebuild_into(&state.fuzzy_index, &db).await.unwrap();
    let token = create_admin_and_token(&db, &state).await;

    let request_body = SearchSeriesRequest {
        query: "one punch".to_string(),
        library_id: Some(library_a.id),
        full: false,
    };
    let request = post_json_request_with_auth("/api/v1/series/search", &request_body, &token);
    let (status, response): (StatusCode, Option<Vec<SeriesDto>>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let series = response.unwrap();
    assert!(
        series.iter().any(|s| s.id == manga_target.id),
        "library-A hit should appear",
    );
    assert!(
        series.iter().all(|s| s.id != comic_target.id),
        "library-B hit must not appear when library filter is set",
    );
}

#[tokio::test]
async fn fuzzy_flag_off_falls_back_to_like_path() {
    let (db, _temp_dir) = setup_test_db().await;
    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();
    seed_series(&db, library.id, "one-punch-man", "One-Punch Man").await;

    // Flag intentionally left at its default (false). Building the index is
    // harmless — the handler must not consult it.
    let (state, app) = setup_test_app(db.clone()).await;
    rebuild_into(&state.fuzzy_index, &db).await.unwrap();
    let token = create_admin_and_token(&db, &state).await;

    // "on ch" cannot match a LIKE %on ch% pattern against "one-punch-man"
    // because LIKE needs a contiguous substring.
    let request_body = SearchSeriesRequest {
        query: "on ch".to_string(),
        library_id: None,
        full: false,
    };
    let request = post_json_request_with_auth("/api/v1/series/search", &request_body, &token);
    let (status, response): (StatusCode, Option<Vec<SeriesDto>>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    assert!(
        response.unwrap().is_empty(),
        "LIKE-based search cannot match 'on ch' against 'one-punch-man'",
    );
}

#[tokio::test]
async fn fuzzy_books_search_returns_ranked_results() {
    let (db, _temp_dir) = setup_test_db().await;
    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();
    let series = seed_series(&db, library.id, "berserk", "Berserk").await;

    let target = seed_book(
        &db,
        series.id,
        library.id,
        "/berserk-chapter-12.cbz",
        "berserk-chapter-12.cbz",
        "Berserk Chapter 12",
    )
    .await;
    let _other = seed_book(
        &db,
        series.id,
        library.id,
        "/vagabond-vol-1.cbz",
        "vagabond-vol-1.cbz",
        "Vagabond Vol 1",
    )
    .await;

    enable_fuzzy(&db).await;

    let (state, app) = setup_test_app(db.clone()).await;
    rebuild_into(&state.fuzzy_index, &db).await.unwrap();
    let token = create_admin_and_token(&db, &state).await;

    let request_body = BookListRequest {
        full_text_search: Some("berserk chapter".to_string()),
        ..Default::default()
    };
    let request = post_json_request_with_auth(
        "/api/v1/books/list?page=1&pageSize=10",
        &request_body,
        &token,
    );
    let (status, response): (StatusCode, Option<BookListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let body = response.expect("response body");
    assert!(
        body.data.iter().any(|b: &BookDto| b.id == target.id),
        "fuzzy book search should return the Berserk chapter book",
    );
    assert_eq!(
        body.data.first().map(|b| b.id),
        Some(target.id),
        "Berserk chapter should rank first for 'berserk chapter'",
    );
}

#[tokio::test]
async fn fuzzy_books_search_intersects_filter_condition() {
    let (db, _temp_dir) = setup_test_db().await;
    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();
    let series_a = seed_series(&db, library.id, "berserk", "Berserk").await;
    let series_b = seed_series(&db, library.id, "berserker-decoy", "Berserker Decoy").await;

    let target = seed_book(
        &db,
        series_a.id,
        library.id,
        "/berserk-chapter-12.cbz",
        "berserk-chapter-12.cbz",
        "Berserk Chapter 12",
    )
    .await;
    let _decoy = seed_book(
        &db,
        series_b.id,
        library.id,
        "/berserk-chapter-99.cbz",
        "berserk-chapter-99.cbz",
        "Berserk Chapter 99",
    )
    .await;

    enable_fuzzy(&db).await;

    let (state, app) = setup_test_app(db.clone()).await;
    rebuild_into(&state.fuzzy_index, &db).await.unwrap();
    let token = create_admin_and_token(&db, &state).await;

    // Restrict to series_a only — the index has two strong "berserk chapter"
    // matches, but only the series_a one passes the filter intersection.
    let request_body = BookListRequest {
        condition: Some(BookCondition::SeriesId {
            series_id: UuidOperator::Is { value: series_a.id },
        }),
        full_text_search: Some("berserk chapter".to_string()),
        ..Default::default()
    };
    let request = post_json_request_with_auth(
        "/api/v1/books/list?page=1&pageSize=10",
        &request_body,
        &token,
    );
    let (status, response): (StatusCode, Option<BookListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let body = response.expect("response body");
    assert_eq!(
        body.data.len(),
        1,
        "filter condition should pin the fuzzy result set to one book",
    );
    assert_eq!(body.data[0].id, target.id);
}

#[tokio::test]
async fn fuzzy_series_list_returns_ranked_results() {
    let (db, _temp_dir) = setup_test_db().await;
    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    let target = seed_series(&db, library.id, "one-punch-man", "One-Punch Man").await;
    seed_series(&db, library.id, "berserk", "Berserk").await;
    seed_series(&db, library.id, "vagabond", "Vagabond").await;

    enable_fuzzy(&db).await;

    let (state, app) = setup_test_app(db.clone()).await;
    rebuild_into(&state.fuzzy_index, &db).await.unwrap();
    let token = create_admin_and_token(&db, &state).await;

    // Mirrors the gap-skipped query the /series/search test exercises, but
    // now routed through /series/list — exercising the fuzzy branch.
    let request_body = SeriesListRequest {
        full_text_search: Some("on ch".to_string()),
        ..Default::default()
    };
    let request = post_json_request_with_auth(
        "/api/v1/series/list?page=1&pageSize=10",
        &request_body,
        &token,
    );
    let (status, response): (StatusCode, Option<SeriesListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let body = response.expect("response body");
    assert!(
        body.data.iter().any(|s| s.id == target.id),
        "fuzzy /series/list should return One-Punch Man for 'on ch'",
    );
    assert_eq!(
        body.data.first().map(|s| s.id),
        Some(target.id),
        "One-Punch Man should rank first under implicit relevance sort",
    );
}

#[tokio::test]
async fn fuzzy_series_list_intersects_filter_condition() {
    let (db, _temp_dir) = setup_test_db().await;
    let library_a = LibraryRepository::create(&db, "Manga", "/manga", ScanningStrategy::Default)
        .await
        .unwrap();
    let library_b = LibraryRepository::create(&db, "Comics", "/comics", ScanningStrategy::Default)
        .await
        .unwrap();

    let manga_target = seed_series(&db, library_a.id, "one-punch-man", "One-Punch Man").await;
    let comic_decoy = seed_series(
        &db,
        library_b.id,
        "one-punch-american",
        "One Punch American",
    )
    .await;

    enable_fuzzy(&db).await;

    let (state, app) = setup_test_app(db.clone()).await;
    rebuild_into(&state.fuzzy_index, &db).await.unwrap();
    let token = create_admin_and_token(&db, &state).await;

    // Both series match the fuzzy query, but the LibraryId condition pins to
    // library_a only — the intersection should drop the decoy.
    let request_body = SeriesListRequest {
        condition: Some(SeriesCondition::LibraryId {
            library_id: UuidOperator::Is {
                value: library_a.id,
            },
        }),
        full_text_search: Some("one punch".to_string()),
    };
    let request = post_json_request_with_auth(
        "/api/v1/series/list?page=1&pageSize=10",
        &request_body,
        &token,
    );
    let (status, response): (StatusCode, Option<SeriesListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let body = response.expect("response body");
    assert!(
        body.data.iter().any(|s| s.id == manga_target.id),
        "library_a hit should appear in the filtered fuzzy result",
    );
    assert!(
        body.data.iter().all(|s| s.id != comic_decoy.id),
        "library_b hit must be filtered out by the LibraryId condition",
    );
}

#[tokio::test]
async fn fuzzy_series_list_explicit_sort_overrides_relevance() {
    let (db, _temp_dir) = setup_test_db().await;
    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    // Both series match "one punch" via fuzzy. Under relevance sort
    // One-Punch Man would rank first (closer to query); under name ascending
    // sort "One Punch American" beats "One-Punch Man" because the canonical
    // title_sort/title comparison ignores the hyphen tie-breaker.
    let one_punch_man = seed_series(&db, library.id, "one-punch-man", "One-Punch Man").await;
    let one_punch_american =
        seed_series(&db, library.id, "one-punch-american", "One Punch American").await;

    enable_fuzzy(&db).await;

    let (state, app) = setup_test_app(db.clone()).await;
    rebuild_into(&state.fuzzy_index, &db).await.unwrap();
    let token = create_admin_and_token(&db, &state).await;

    let request_body = SeriesListRequest {
        full_text_search: Some("one punch".to_string()),
        ..Default::default()
    };
    let request = post_json_request_with_auth(
        "/api/v1/series/list?page=1&pageSize=10&sort=name,asc",
        &request_body,
        &token,
    );
    let (status, response): (StatusCode, Option<SeriesListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let body = response.expect("response body");
    assert_eq!(body.data.len(), 2, "both fuzzy hits should survive");
    // "One Punch American" sorts before "One-Punch Man" alphabetically.
    assert_eq!(
        body.data.first().map(|s| s.id),
        Some(one_punch_american.id),
        "explicit name,asc sort should override fuzzy ranking",
    );
    assert_eq!(body.data.get(1).map(|s| s.id), Some(one_punch_man.id),);
}

#[tokio::test]
async fn fuzzy_books_list_explicit_sort_overrides_relevance() {
    let (db, _temp_dir) = setup_test_db().await;
    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();
    let series = seed_series(&db, library.id, "berserk", "Berserk").await;

    // Two books that both match "berserk chapter" via fuzzy. Under relevance,
    // "Berserk Chapter 12" lands higher (better fuzzy hit). Under title sort
    // asc, "Berserk Chapter 100" sorts first because '1' < '9' lexically.
    let chapter_100 = seed_book(
        &db,
        series.id,
        library.id,
        "/berserk-chapter-100.cbz",
        "berserk-chapter-100.cbz",
        "Berserk Chapter 100",
    )
    .await;
    let _chapter_12 = seed_book(
        &db,
        series.id,
        library.id,
        "/berserk-chapter-12.cbz",
        "berserk-chapter-12.cbz",
        "Berserk Chapter 12",
    )
    .await;

    enable_fuzzy(&db).await;

    let (state, app) = setup_test_app(db.clone()).await;
    rebuild_into(&state.fuzzy_index, &db).await.unwrap();
    let token = create_admin_and_token(&db, &state).await;

    let request_body = BookListRequest {
        full_text_search: Some("berserk chapter".to_string()),
        ..Default::default()
    };
    let request = post_json_request_with_auth(
        "/api/v1/books/list?page=1&pageSize=10&sort=title,asc",
        &request_body,
        &token,
    );
    let (status, response): (StatusCode, Option<BookListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let body = response.expect("response body");
    assert!(body.data.len() >= 2, "both books should appear");
    assert_eq!(
        body.data.first().map(|b: &BookDto| b.id),
        Some(chapter_100.id),
        "explicit title,asc should override fuzzy relevance ordering",
    );
}

#[tokio::test]
async fn relevance_sort_without_query_falls_back_to_default() {
    let (db, _temp_dir) = setup_test_db().await;
    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    // Seed in non-alphabetical order so the natural Name sort is verifiable.
    let zebra = seed_series(&db, library.id, "zebra", "Zebra Manga").await;
    let alpha = seed_series(&db, library.id, "alpha", "Alpha Manga").await;

    enable_fuzzy(&db).await;

    let (state, app) = setup_test_app(db.clone()).await;
    rebuild_into(&state.fuzzy_index, &db).await.unwrap();
    let token = create_admin_and_token(&db, &state).await;

    // sort=relevance + no query: the handler should silently fall back to
    // the natural default (name,asc) rather than try to rank against nothing.
    let request_body = SeriesListRequest::default();
    let request = post_json_request_with_auth(
        "/api/v1/series/list?page=1&pageSize=10&sort=relevance",
        &request_body,
        &token,
    );
    let (status, response): (StatusCode, Option<SeriesListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let body = response.expect("response body");
    assert_eq!(body.data.len(), 2);
    assert_eq!(
        body.data.first().map(|s| s.id),
        Some(alpha.id),
        "alpha should sort first under fallback name,asc",
    );
    assert_eq!(body.data.get(1).map(|s| s.id), Some(zebra.id));
}

#[tokio::test]
async fn series_list_falls_back_to_like_when_fuzzy_flag_off() {
    // Phase 3 sanity check: with the flag off, /series/list must NOT consult
    // the fuzzy index and must behave like the pre-Phase-3 LIKE path.
    let (db, _temp_dir) = setup_test_db().await;
    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    let target = seed_series(&db, library.id, "one-punch-man", "One-Punch Man").await;
    seed_series(&db, library.id, "berserk", "Berserk").await;

    // Flag left at its default (false).
    let (state, app) = setup_test_app(db.clone()).await;
    rebuild_into(&state.fuzzy_index, &db).await.unwrap();
    let token = create_admin_and_token(&db, &state).await;

    // "Punch" is a literal substring of "One-Punch Man" so the LIKE path
    // still matches. Picked deliberately to differ from the gap-skipped
    // query below — together they prove the fuzzy index is not consulted.
    let request_body = SeriesListRequest {
        full_text_search: Some("Punch".to_string()),
        ..Default::default()
    };
    let request = post_json_request_with_auth(
        "/api/v1/series/list?page=1&pageSize=10",
        &request_body,
        &token,
    );
    let (status, response): (StatusCode, Option<SeriesListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let body = response.expect("response body");
    assert!(
        body.data.iter().any(|s| s.id == target.id),
        "LIKE path should still find 'Punch' inside 'One-Punch Man'",
    );

    // "on ch" — only the fuzzy index can match the gap-skipped form, so a
    // flag-off /series/list must NOT return the target series here.
    let gap_request = SeriesListRequest {
        full_text_search: Some("on ch".to_string()),
        ..Default::default()
    };
    let app2 = setup_test_app(db.clone()).await.1;
    let request2 = post_json_request_with_auth(
        "/api/v1/series/list?page=1&pageSize=10",
        &gap_request,
        &token,
    );
    let (status2, response2): (StatusCode, Option<SeriesListResponse>) =
        make_json_request(app2, request2).await;
    assert_eq!(status2, StatusCode::OK);
    let body2 = response2.expect("response body");
    assert!(
        body2.data.iter().all(|s| s.id != target.id),
        "LIKE path cannot match 'on ch' as a contiguous substring",
    );
}
