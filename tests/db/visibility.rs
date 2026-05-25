//! Repository-level tests for the `SeriesVisibility` SQL filter.
//!
//! Every list/search method on `BookRepository` and `SeriesRepository`
//! that the API uses for paginated reads must honor the visibility filter.
//! These tests pin that contract method-by-method so a regression in any
//! one of them shows up immediately, without needing to spin up the full
//! HTTP layer.

#[path = "../common/mod.rs"]
mod common;

use chrono::Utc;
use codex::db::ScanningStrategy;
use codex::db::entities::{books, libraries, series};
use codex::db::repositories::{
    BookMetadataRepository, BookRepository, LibraryRepository, ReadProgressRepository,
    SeriesRepository, SeriesVisibility, UserRepository,
};
use codex::models::sort::{BookSortField, BookSortParam, SeriesSortField, SeriesSortParam};
use codex::utils::password;
use common::*;
use sea_orm::DatabaseConnection;
use uuid::Uuid;

struct VisibilityFixture {
    library: libraries::Model,
    visible_series: series::Model,
    hidden_series: series::Model,
    visible_book: books::Model,
    hidden_book: books::Model,
    user_id: Uuid,
}

/// Build a small library with two series. `visible_series` is the one a
/// user with a deny-only filter would still see; `hidden_series` is the
/// one excluded via `SeriesVisibility::excluded_series_ids`.
async fn build_fixture(db: &DatabaseConnection) -> VisibilityFixture {
    let library = LibraryRepository::create(
        db,
        "Lib",
        &format!("/lib-{}", Uuid::new_v4()),
        ScanningStrategy::Default,
    )
    .await
    .unwrap();

    let visible_series = SeriesRepository::create(db, library.id, "Visible Series", None)
        .await
        .unwrap();
    let hidden_series = SeriesRepository::create(db, library.id, "Hidden Series", None)
        .await
        .unwrap();

    // SeriesRepository::create already inserts a series_metadata row, so
    // don't seed metadata again here — that would violate the UNIQUE
    // constraint on `series_id`.

    let visible_book = make_book(db, visible_series.id, library.id, "visible.cbz").await;
    let hidden_book = make_book(db, hidden_series.id, library.id, "hidden.cbz").await;

    let password_hash = password::hash_password("pw").unwrap();
    let user = create_test_user("u", "u@example.com", &password_hash, false);
    let user = UserRepository::create(db, &user).await.unwrap();

    VisibilityFixture {
        library,
        visible_series,
        hidden_series,
        visible_book,
        hidden_book,
        user_id: user.id,
    }
}

async fn make_book(
    db: &DatabaseConnection,
    series_id: Uuid,
    library_id: Uuid,
    file_name: &str,
) -> books::Model {
    let now = Utc::now();
    let id = Uuid::new_v4();
    let model = books::Model {
        id,
        series_id,
        library_id,
        path: format!("/lib/{}", file_name),
        file_name: file_name.to_string(),
        file_size: 1024,
        file_hash: format!("hash-{}", id),
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
        Some(file_name.to_string()),
        None,
    )
    .await
    .unwrap();
    created
}

/// `SeriesVisibility` that hides `hidden_series` via the deny set.
fn deny_only(fx: &VisibilityFixture) -> SeriesVisibility {
    SeriesVisibility {
        excluded_series_ids: vec![fx.hidden_series.id],
        allowed_series_ids: None,
    }
}

/// `SeriesVisibility` whitelist that allows only `visible_series`.
fn whitelist_only(fx: &VisibilityFixture) -> SeriesVisibility {
    SeriesVisibility {
        excluded_series_ids: vec![],
        allowed_series_ids: Some(vec![fx.visible_series.id]),
    }
}

// ============================================================================
// BookRepository methods
// ============================================================================

#[tokio::test]
async fn book_list_recently_added_honors_deny() {
    let (db, _temp) = setup_test_db().await;
    let fx = build_fixture(&db).await;
    let vis = deny_only(&fx);

    let (books, total) = BookRepository::list_recently_added(&db, None, false, 0, 10, Some(&vis))
        .await
        .unwrap();

    assert_eq!(total, 1);
    assert_eq!(books.len(), 1);
    assert_eq!(books[0].id, fx.visible_book.id);
}

#[tokio::test]
async fn book_list_recently_added_honors_whitelist() {
    let (db, _temp) = setup_test_db().await;
    let fx = build_fixture(&db).await;
    let vis = whitelist_only(&fx);

    let (books, total) = BookRepository::list_recently_added(&db, None, false, 0, 10, Some(&vis))
        .await
        .unwrap();

    assert_eq!(total, 1);
    assert_eq!(books[0].id, fx.visible_book.id);
}

#[tokio::test]
async fn book_list_recently_added_empty_whitelist_short_circuits() {
    let (db, _temp) = setup_test_db().await;
    let _fx = build_fixture(&db).await;
    let vis = SeriesVisibility {
        excluded_series_ids: vec![],
        allowed_series_ids: Some(vec![]),
    };

    let (books, total) = BookRepository::list_recently_added(&db, None, false, 0, 10, Some(&vis))
        .await
        .unwrap();

    assert_eq!(total, 0);
    assert!(books.is_empty());
}

#[tokio::test]
async fn book_list_all_honors_deny() {
    let (db, _temp) = setup_test_db().await;
    let fx = build_fixture(&db).await;
    let vis = deny_only(&fx);

    let (books, total) = BookRepository::list_all(&db, false, 0, 10, Some(&vis))
        .await
        .unwrap();

    assert_eq!(total, 1);
    assert_eq!(books.len(), 1);
    assert_eq!(books[0].id, fx.visible_book.id);
}

#[tokio::test]
async fn book_list_by_ids_honors_deny() {
    let (db, _temp) = setup_test_db().await;
    let fx = build_fixture(&db).await;
    let vis = deny_only(&fx);

    let ids = vec![fx.visible_book.id, fx.hidden_book.id];
    let (books, total) = BookRepository::list_by_ids(&db, &ids, false, 0, 10, Some(&vis))
        .await
        .unwrap();

    assert_eq!(total, 1);
    assert_eq!(books.len(), 1);
    assert_eq!(books[0].id, fx.visible_book.id);
}

#[tokio::test]
async fn book_list_by_ids_sorted_honors_deny() {
    let (db, _temp) = setup_test_db().await;
    let fx = build_fixture(&db).await;
    let vis = deny_only(&fx);

    let ids = vec![fx.visible_book.id, fx.hidden_book.id];
    let sort = BookSortParam {
        field: BookSortField::Title,
        direction: codex::models::sort::SortDirection::Asc,
    };
    let (books, total) =
        BookRepository::list_by_ids_sorted(&db, &ids, &sort, None, false, 0, 10, Some(&vis))
            .await
            .unwrap();

    assert_eq!(total, 1);
    assert_eq!(books.len(), 1);
    assert_eq!(books[0].id, fx.visible_book.id);
}

#[tokio::test]
async fn book_hydrate_by_ids_honors_deny() {
    let (db, _temp) = setup_test_db().await;
    let fx = build_fixture(&db).await;
    let vis = deny_only(&fx);

    let ids = vec![fx.visible_book.id, fx.hidden_book.id];
    let books = BookRepository::hydrate_by_ids(&db, None, &ids, false, Some(&vis))
        .await
        .unwrap();

    assert_eq!(books.len(), 1);
    assert_eq!(books[0].id, fx.visible_book.id);
}

#[tokio::test]
async fn book_list_by_library_sorted_honors_deny() {
    let (db, _temp) = setup_test_db().await;
    let fx = build_fixture(&db).await;
    let vis = deny_only(&fx);

    let sort = BookSortParam::default();
    let (books, total) =
        BookRepository::list_by_library_sorted(&db, fx.library.id, &sort, false, 0, 10, Some(&vis))
            .await
            .unwrap();

    assert_eq!(total, 1);
    assert_eq!(books.len(), 1);
    assert_eq!(books[0].id, fx.visible_book.id);
}

#[tokio::test]
async fn book_list_with_progress_honors_deny() {
    let (db, _temp) = setup_test_db().await;
    let fx = build_fixture(&db).await;
    let vis = deny_only(&fx);

    // Give the user progress on both books so they would otherwise show up.
    ReadProgressRepository::upsert(&db, fx.user_id, fx.visible_book.id, 1, false)
        .await
        .unwrap();
    ReadProgressRepository::upsert(&db, fx.user_id, fx.hidden_book.id, 1, false)
        .await
        .unwrap();

    let (books, total) =
        BookRepository::list_with_progress(&db, fx.user_id, None, None, 0, 10, Some(&vis))
            .await
            .unwrap();

    assert_eq!(total, 1);
    assert_eq!(books.len(), 1);
    assert_eq!(books[0].id, fx.visible_book.id);
}

#[tokio::test]
async fn book_list_recently_read_honors_deny() {
    let (db, _temp) = setup_test_db().await;
    let fx = build_fixture(&db).await;
    let vis = deny_only(&fx);

    ReadProgressRepository::upsert(&db, fx.user_id, fx.visible_book.id, 1, false)
        .await
        .unwrap();
    ReadProgressRepository::upsert(&db, fx.user_id, fx.hidden_book.id, 1, false)
        .await
        .unwrap();

    let books = BookRepository::list_recently_read(&db, fx.user_id, None, 10, Some(&vis))
        .await
        .unwrap();

    assert_eq!(books.len(), 1);
    assert_eq!(books[0].id, fx.visible_book.id);
}

#[tokio::test]
async fn book_list_on_deck_honors_deny() {
    let (db, _temp) = setup_test_db().await;
    let fx = build_fixture(&db).await;

    // Add a second book to each series and mark the first one completed so
    // that the second is on-deck for that series.
    let visible_book2 = make_book(&db, fx.visible_series.id, fx.library.id, "visible2.cbz").await;
    let hidden_book2 = make_book(&db, fx.hidden_series.id, fx.library.id, "hidden2.cbz").await;
    ReadProgressRepository::upsert(&db, fx.user_id, fx.visible_book.id, 10, true)
        .await
        .unwrap();
    ReadProgressRepository::upsert(&db, fx.user_id, fx.hidden_book.id, 10, true)
        .await
        .unwrap();

    let vis = deny_only(&fx);
    let (books, total) = BookRepository::list_on_deck(&db, fx.user_id, None, 0, 10, Some(&vis))
        .await
        .unwrap();

    assert_eq!(
        total, 1,
        "only the visible-series on-deck book should remain"
    );
    assert_eq!(books.len(), 1);
    assert_eq!(books[0].id, visible_book2.id);
    assert_ne!(books[0].id, hidden_book2.id);
}

#[tokio::test]
async fn book_search_by_title_honors_deny() {
    let (db, _temp) = setup_test_db().await;
    let fx = build_fixture(&db).await;
    let vis = deny_only(&fx);

    let (books, total) =
        BookRepository::search_by_title(&db, "cbz", None, None, false, Some((0, 10)), Some(&vis))
            .await
            .unwrap();

    assert_eq!(total, 1);
    assert_eq!(books.len(), 1);
    assert_eq!(books[0].id, fx.visible_book.id);
}

#[tokio::test]
async fn book_search_by_name_honors_deny() {
    let (db, _temp) = setup_test_db().await;
    let fx = build_fixture(&db).await;
    let vis = deny_only(&fx);

    let books = BookRepository::search_by_name(&db, "cbz", Some(&vis))
        .await
        .unwrap();

    assert_eq!(books.len(), 1);
    assert_eq!(books[0].id, fx.visible_book.id);
}

#[tokio::test]
async fn book_get_adjacent_in_series_hides_denied_series() {
    let (db, _temp) = setup_test_db().await;
    let fx = build_fixture(&db).await;
    let vis = deny_only(&fx);

    let (prev, next) = BookRepository::get_adjacent_in_series(&db, fx.hidden_book.id, Some(&vis))
        .await
        .unwrap();

    assert!(prev.is_none(), "denied series must return no prev neighbor");
    assert!(next.is_none(), "denied series must return no next neighbor");
}

// ============================================================================
// SeriesRepository methods
// ============================================================================

#[tokio::test]
async fn series_list_by_library_honors_deny() {
    let (db, _temp) = setup_test_db().await;
    let fx = build_fixture(&db).await;
    let vis = deny_only(&fx);

    let series_list = SeriesRepository::list_by_library(&db, fx.library.id, Some(&vis))
        .await
        .unwrap();

    assert_eq!(series_list.len(), 1);
    assert_eq!(series_list[0].id, fx.visible_series.id);
}

#[tokio::test]
async fn series_list_all_honors_deny() {
    let (db, _temp) = setup_test_db().await;
    let fx = build_fixture(&db).await;
    let vis = deny_only(&fx);

    let series_list = SeriesRepository::list_all(&db, Some(&vis)).await.unwrap();

    assert_eq!(series_list.len(), 1);
    assert_eq!(series_list[0].id, fx.visible_series.id);
}

#[tokio::test]
async fn series_list_recently_added_honors_deny() {
    let (db, _temp) = setup_test_db().await;
    let fx = build_fixture(&db).await;
    let vis = deny_only(&fx);

    let series_list = SeriesRepository::list_recently_added(&db, None, 10, Some(&vis))
        .await
        .unwrap();

    assert_eq!(series_list.len(), 1);
    assert_eq!(series_list[0].id, fx.visible_series.id);
}

#[tokio::test]
async fn series_list_recently_updated_honors_deny() {
    let (db, _temp) = setup_test_db().await;
    let fx = build_fixture(&db).await;
    let vis = deny_only(&fx);

    let series_list = SeriesRepository::list_recently_updated(&db, None, 10, Some(&vis))
        .await
        .unwrap();

    assert_eq!(series_list.len(), 1);
    assert_eq!(series_list[0].id, fx.visible_series.id);
}

#[tokio::test]
async fn series_list_by_library_sorted_honors_deny() {
    let (db, _temp) = setup_test_db().await;
    let fx = build_fixture(&db).await;
    let vis = deny_only(&fx);

    let sort = SeriesSortParam {
        field: SeriesSortField::Name,
        direction: codex::models::sort::SortDirection::Asc,
    };
    let series_list = SeriesRepository::list_by_library_sorted(
        &db,
        fx.library.id,
        &sort,
        None,
        0,
        10,
        Some(&vis),
    )
    .await
    .unwrap();

    assert_eq!(series_list.len(), 1);
    assert_eq!(series_list[0].id, fx.visible_series.id);
}

#[tokio::test]
async fn series_list_by_ids_sorted_honors_deny() {
    let (db, _temp) = setup_test_db().await;
    let fx = build_fixture(&db).await;
    let vis = deny_only(&fx);

    let ids = vec![fx.visible_series.id, fx.hidden_series.id];
    let sort = SeriesSortParam::default();
    let (series_list, total) =
        SeriesRepository::list_by_ids_sorted(&db, &ids, &sort, None, 0, 10, Some(&vis))
            .await
            .unwrap();

    assert_eq!(total, 1);
    assert_eq!(series_list.len(), 1);
    assert_eq!(series_list[0].id, fx.visible_series.id);
}

#[tokio::test]
async fn series_search_by_title_honors_deny() {
    let (db, _temp) = setup_test_db().await;
    let fx = build_fixture(&db).await;
    let vis = deny_only(&fx);

    let (series_list, total) =
        SeriesRepository::search_by_title(&db, "Series", None, None, Some((0, 10)), Some(&vis))
            .await
            .unwrap();

    assert_eq!(total, 1);
    assert_eq!(series_list.len(), 1);
    assert_eq!(series_list[0].id, fx.visible_series.id);
}

#[tokio::test]
async fn series_search_by_name_honors_deny() {
    let (db, _temp) = setup_test_db().await;
    let fx = build_fixture(&db).await;
    let vis = deny_only(&fx);

    let series_list = SeriesRepository::search_by_name(&db, "Series", Some(&vis))
        .await
        .unwrap();

    assert_eq!(series_list.len(), 1);
    assert_eq!(series_list[0].id, fx.visible_series.id);
}

#[tokio::test]
async fn series_list_in_progress_honors_deny() {
    let (db, _temp) = setup_test_db().await;
    let fx = build_fixture(&db).await;
    let vis = deny_only(&fx);

    ReadProgressRepository::upsert(&db, fx.user_id, fx.visible_book.id, 1, false)
        .await
        .unwrap();
    ReadProgressRepository::upsert(&db, fx.user_id, fx.hidden_book.id, 1, false)
        .await
        .unwrap();

    let series_list = SeriesRepository::list_in_progress(&db, fx.user_id, None, Some(&vis))
        .await
        .unwrap();

    assert_eq!(series_list.len(), 1);
    assert_eq!(series_list[0].id, fx.visible_series.id);
}

// ============================================================================
// None visibility = no filtering (regression: visibility=None must not narrow)
// ============================================================================

#[tokio::test]
async fn none_visibility_returns_everything() {
    let (db, _temp) = setup_test_db().await;
    let fx = build_fixture(&db).await;

    let (books, total) = BookRepository::list_recently_added(&db, None, false, 0, 10, None)
        .await
        .unwrap();
    assert_eq!(total, 2);
    assert_eq!(books.len(), 2);

    let series_list = SeriesRepository::list_all(&db, None).await.unwrap();
    assert_eq!(series_list.len(), 2);
    // make sure the fixture's library/hidden_series field is actually used by
    // the assertion path so the struct isn't trimmed to dead fields.
    let _ = fx.hidden_series.id;
}
