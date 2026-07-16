//! Tests for the series/book export collectors.
//!
//! Verifies the DB-backed collection/read-list membership fields, including
//! that membership is only emitted for rows the caller resolved as visible.

#[path = "../common/mod.rs"]
mod common;

use codex::db::repositories::{BookRepository, CollectionRepository, ReadListRepository};
use codex::services::book_export_collector::{self, BookExportField, BookExportRow};
use codex::services::series_export_collector::{self, ExportField, SeriesExportRow};
use common::db::setup_test_db;
use common::fixtures::{create_test_book, create_test_library, create_test_series};
use uuid::Uuid;

#[tokio::test]
async fn test_series_export_includes_collections() {
    let (db, _t) = setup_test_db().await;
    let library = create_test_library(&db, "Lib", "/lib").await;
    let series_a = create_test_series(&db, &library, "Alpha").await;
    let series_b = create_test_series(&db, &library, "Bravo").await;

    let zeta = CollectionRepository::create(&db, "Zeta", None, false)
        .await
        .unwrap();
    let picks = CollectionRepository::create(&db, "Best Picks", None, false)
        .await
        .unwrap();
    CollectionRepository::add_series(&db, zeta.id, series_a.id)
        .await
        .unwrap();
    CollectionRepository::add_series(&db, picks.id, series_a.id)
        .await
        .unwrap();

    let mut rows: Vec<SeriesExportRow> = Vec::new();
    let count = series_export_collector::collect_batched(
        &db,
        Uuid::new_v4(),
        &[series_a.id, series_b.id],
        &[ExportField::SeriesName, ExportField::Collections],
        |row| rows.push(row),
    )
    .await
    .unwrap();

    assert_eq!(count, 2);
    let row_a = rows.iter().find(|r| r.series_name == "Alpha").unwrap();
    assert_eq!(row_a.collections.as_deref(), Some("Best Picks; Zeta"));
    // A series in no collection gets None (field omitted in JSON, empty in CSV).
    let row_b = rows.iter().find(|r| r.series_name == "Bravo").unwrap();
    assert_eq!(row_b.collections, None);
}

#[tokio::test]
async fn test_series_export_collections_not_selected() {
    let (db, _t) = setup_test_db().await;
    let library = create_test_library(&db, "Lib", "/lib").await;
    let series = create_test_series(&db, &library, "Alpha").await;

    let coll = CollectionRepository::create(&db, "Zeta", None, false)
        .await
        .unwrap();
    CollectionRepository::add_series(&db, coll.id, series.id)
        .await
        .unwrap();

    let mut rows: Vec<SeriesExportRow> = Vec::new();
    series_export_collector::collect_batched(
        &db,
        Uuid::new_v4(),
        &[series.id],
        &[ExportField::SeriesName],
        |row| rows.push(row),
    )
    .await
    .unwrap();

    // Unselected field stays None even when memberships exist.
    assert_eq!(rows[0].collections, None);
}

#[tokio::test]
async fn test_series_export_membership_only_for_visible_rows() {
    // A collection containing both a visible and a hidden series: the hidden
    // series must not produce a row, and the visible row still names the
    // shared collection. `collect_batched` operates on the pre-filtered ID
    // list from `resolve_series_ids`, so membership lookups never widen it.
    let (db, _t) = setup_test_db().await;
    let library = create_test_library(&db, "Lib", "/lib").await;
    let visible = create_test_series(&db, &library, "Visible").await;
    let hidden = create_test_series(&db, &library, "Hidden").await;

    let coll = CollectionRepository::create(&db, "Shared", None, false)
        .await
        .unwrap();
    for sid in [visible.id, hidden.id] {
        CollectionRepository::add_series(&db, coll.id, sid)
            .await
            .unwrap();
    }

    let mut rows: Vec<SeriesExportRow> = Vec::new();
    let count = series_export_collector::collect_batched(
        &db,
        Uuid::new_v4(),
        &[visible.id],
        &[ExportField::SeriesName, ExportField::Collections],
        |row| rows.push(row),
    )
    .await
    .unwrap();

    assert_eq!(count, 1);
    assert_eq!(rows[0].series_name, "Visible");
    assert_eq!(rows[0].collections.as_deref(), Some("Shared"));
}

#[tokio::test]
async fn test_book_export_includes_read_lists() {
    let (db, _t) = setup_test_db().await;
    let library = create_test_library(&db, "Lib", "/lib").await;
    let series = create_test_series(&db, &library, "Alpha").await;

    let book_a_model = create_test_book(
        series.id,
        library.id,
        "/lib/alpha/1.cbz",
        "a.cbz",
        "hash_a",
        "cbz",
        10,
    );
    let book_a = BookRepository::create(&db, &book_a_model, None)
        .await
        .unwrap();
    let book_b_model = create_test_book(
        series.id,
        library.id,
        "/lib/alpha/2.cbz",
        "b.cbz",
        "hash_b",
        "cbz",
        10,
    );
    let book_b = BookRepository::create(&db, &book_b_model, None)
        .await
        .unwrap();

    let omega = ReadListRepository::create(&db, "Omega Run", None, true)
        .await
        .unwrap();
    let arc = ReadListRepository::create(&db, "Arc One", None, true)
        .await
        .unwrap();
    ReadListRepository::add_book(&db, omega.id, book_a.id)
        .await
        .unwrap();
    ReadListRepository::add_book(&db, arc.id, book_a.id)
        .await
        .unwrap();

    let mut rows: Vec<BookExportRow> = Vec::new();
    let count = book_export_collector::collect_batched(
        &db,
        Uuid::new_v4(),
        &[book_a.id, book_b.id],
        &[BookExportField::BookName, BookExportField::ReadLists],
        |row| rows.push(row),
    )
    .await
    .unwrap();

    assert_eq!(count, 2);
    let row_a = rows.iter().find(|r| r.book_name == "a.cbz").unwrap();
    assert_eq!(row_a.read_lists.as_deref(), Some("Arc One; Omega Run"));
    let row_b = rows.iter().find(|r| r.book_name == "b.cbz").unwrap();
    assert_eq!(row_b.read_lists, None);
}
