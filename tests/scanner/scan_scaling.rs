//! Regression tests for scanner memory scaling.
//!
//! The scanner used to deep-clone the whole-library `existing_books_map`
//! once per series when fanning out parallel series processing. Because all
//! per-series futures are constructed up front, every clone was alive at the
//! same time, making peak memory `O(series * existing_books)`. On a real
//! library (hundreds of series, tens of thousands of books) that allocated
//! several GB and got the process OOM-killed mid-scan.
//!
//! The map is read-only during series processing, so it is now shared behind
//! an `Arc`. These tests exercise the multi-series + existing-books code path
//! (the one that did the per-series cloning) and assert the scan still
//! produces correct results: every book is seen once, a re-scan of an
//! unchanged library creates nothing, and lookups against the shared map
//! still resolve. A strict peak-memory assertion would be flaky in CI, so the
//! guarantee here is behavioural (the `Arc` change must not alter results);
//! the memory profile is validated manually against the real library.

#[path = "../common/mod.rs"]
mod common;

use codex::db::ScanningStrategy;
use codex::db::repositories::{BookRepository, LibraryRepository};
use codex::scanner::{ScanMode, scan_library};
use common::*;
use std::fs;
use tempfile::TempDir;

/// Build a library on disk with `series_count` series folders, each holding
/// `books_per_series` CBZ files. Returns the created library model.
async fn setup_many_series_library(
    db: &sea_orm::DatabaseConnection,
    temp_dir: &TempDir,
    series_count: usize,
    books_per_series: usize,
) -> codex::db::entities::libraries::Model {
    let library_path = temp_dir.path().join("scaling_library");
    fs::create_dir_all(&library_path).unwrap();

    // A single small CBZ we copy into every slot — content is irrelevant here,
    // we care about the number of distinct (series, book) paths.
    let template_cbz = create_test_cbz(temp_dir, 1, false);

    for s in 0..series_count {
        let series_path = library_path.join(format!("Series {:02}", s));
        fs::create_dir_all(&series_path).unwrap();
        for b in 0..books_per_series {
            let book_path = series_path.join(format!("Series {:02} v{:02}.cbz", s, b));
            fs::copy(&template_cbz, &book_path).unwrap();
        }
    }

    LibraryRepository::create(
        db,
        "Scaling Library",
        library_path.to_str().unwrap(),
        ScanningStrategy::Default,
    )
    .await
    .unwrap()
}

/// A normal re-scan of an unchanged multi-series library is the exact path that
/// cloned `existing_books_map` per series. After the first scan populates the
/// library, the second scan must see every file as existing/unchanged: it
/// creates nothing, and the total book count is unchanged.
#[tokio::test]
async fn test_rescan_many_series_with_existing_books_is_stable() {
    let (db_wrapper, temp_dir) = setup_test_db_wrapper().await;
    let db = db_wrapper.sea_orm_connection();

    let series_count = 25;
    let books_per_series = 4;
    let expected_books = series_count * books_per_series;

    let library = setup_many_series_library(db, &temp_dir, series_count, books_per_series).await;

    // First scan: cold library, nothing existing yet — creates everything.
    let first = scan_library(db, library.id, ScanMode::Normal, None, None, None, None)
        .await
        .unwrap();
    assert_eq!(
        first.books_created, expected_books,
        "first scan should create one book per file"
    );
    assert_eq!(
        first.series_created, series_count,
        "first scan should create one series per folder"
    );

    // Second scan: every book now exists. This is the path that built one full
    // clone of the existing-books map per series. With the Arc fix it shares a
    // single copy; behaviour must be identical — no new books created.
    let second = scan_library(db, library.id, ScanMode::Normal, None, None, None, None)
        .await
        .unwrap();
    assert_eq!(
        second.books_created, 0,
        "re-scan of an unchanged library must not create any books"
    );
    assert_eq!(
        second.files_processed, expected_books,
        "re-scan must still hash/visit every file"
    );

    // The shared map must still resolve every path: the library holds exactly
    // the expected number of books, with no duplicates from the second scan.
    let book_count = BookRepository::count_by_library(db, library.id)
        .await
        .unwrap();
    assert_eq!(
        book_count as usize, expected_books,
        "library must hold exactly one book per file after two scans"
    );

    db_wrapper.close().await;
}
