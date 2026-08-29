//! Regression tests for book numbering when a series is populated in stages.
//!
//! Positional numbers ("this book is the 32nd file in the series") are only
//! meaningful against the complete set of books. Resolving them per book while
//! the scanner is still inserting siblings hands out the same position twice
//! and leaves gaps, so positional numbering belongs to the renumber pass alone.

use anyhow::Result;
use chrono::Utc;
use codex::db::entities::{books, series};
use codex::db::repositories::{
    BookMetadataRepository, BookRepository, LibraryRepository, SeriesRepository, TaskRepository,
    library::CreateLibraryParams,
};
use codex::models::NumberStrategy;
use codex::scanner::analyze_book;
use codex::tasks::TaskWorker;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use tempfile::TempDir;
use uuid::Uuid;
use zip::ZipWriter;
use zip::write::SimpleFileOptions;

#[path = "../common/mod.rs"]
mod common;
use common::{files::create_test_png, setup_test_db_wrapper};

/// Write a minimal CBZ with no ComicInfo.xml, so nothing but the filename and
/// the file's position can supply a number.
fn write_bare_cbz(dir: &Path, file_name: &str) -> PathBuf {
    let path = dir.join(file_name);
    let mut zip = ZipWriter::new(File::create(&path).unwrap());
    let options: SimpleFileOptions =
        SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);

    for i in 1..=2 {
        zip.start_file(format!("page{:03}.png", i), options)
            .unwrap();
        zip.write_all(&create_test_png(10, 10)).unwrap();
    }
    zip.finish().unwrap();
    path
}

/// Write a CBZ carrying an explicit ComicInfo `<Number>`.
fn write_cbz_with_number(dir: &Path, file_name: &str, number: u32) -> PathBuf {
    let path = dir.join(file_name);
    let mut zip = ZipWriter::new(File::create(&path).unwrap());
    let options: SimpleFileOptions =
        SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);

    let xml = format!(
        r#"<?xml version="1.0"?>
<ComicInfo>
  <Series>Staged Series</Series>
  <Number>{number}</Number>
  <PageCount>2</PageCount>
</ComicInfo>"#
    );
    zip.start_file("ComicInfo.xml", options).unwrap();
    zip.write_all(xml.as_bytes()).unwrap();

    for i in 1..=2 {
        zip.start_file(format!("page{:03}.png", i), options)
            .unwrap();
        zip.write_all(&create_test_png(10, 10)).unwrap();
    }
    zip.finish().unwrap();
    path
}

async fn setup_library(
    db: &codex::db::Database,
    strategy: NumberStrategy,
) -> Result<(codex::db::entities::libraries::Model, series::Model)> {
    let conn = db.sea_orm_connection();
    let params =
        CreateLibraryParams::new("Staged Library", "/staged").with_number_strategy(strategy);
    let library = LibraryRepository::create_with_params(conn, params).await?;
    let series = SeriesRepository::create(conn, library.id, "Staged Series", None).await?;
    Ok((library, series))
}

/// Insert a book row for an on-disk file and analyze it, exactly as the
/// AnalyzeBook task does after the scanner flushes a batch.
async fn insert_and_analyze(
    db: &codex::db::Database,
    library_id: Uuid,
    series_id: Uuid,
    path: &Path,
) -> Result<books::Model> {
    let created = insert_book(db, library_id, series_id, path).await?;
    analyze_book(db.sea_orm_connection(), created.id, false, None).await?;
    Ok(created)
}

/// Insert a book row the way a scan does, without analyzing it. The book has no
/// `book_metadata` row until analysis creates one.
async fn insert_book(
    db: &codex::db::Database,
    library_id: Uuid,
    series_id: Uuid,
    path: &Path,
) -> Result<books::Model> {
    let conn = db.sea_orm_connection();
    let file_name = path.file_name().unwrap().to_string_lossy().to_string();

    let book = books::Model {
        id: Uuid::new_v4(),
        series_id,
        library_id,
        path: path.to_string_lossy().to_string(),
        file_name: file_name.clone(),
        file_size: 0,
        file_hash: format!("hash-{file_name}"),
        partial_hash: String::new(),
        format: "cbz".to_string(),
        page_count: 0,
        deleted: false,
        analyzed: false,
        analysis_error: None,
        analysis_errors: None,
        modified_at: Utc::now(),
        created_at: Utc::now(),
        thumbnail_path: None,
        thumbnail_generated_at: None,
        koreader_hash: None,
        epub_positions: None,
        epub_spine_items: None,
        updated_at: Utc::now(),
    };

    let created = BookRepository::create(conn, &book, None).await?;
    Ok(created)
}

/// Run every queued task to completion, the way the worker pool does after a scan.
async fn drain_tasks(conn: &sea_orm::DatabaseConnection) {
    let worker =
        TaskWorker::new(conn.clone()).with_poll_interval(std::time::Duration::from_millis(10));

    for _ in 0..50 {
        let stats = TaskRepository::get_stats(conn).await.unwrap();
        if stats.pending == 0 {
            break;
        }
        worker.process_once().await.ok();
    }
}

async fn number_of(db: &codex::db::Database, book_id: Uuid) -> Option<f32> {
    BookMetadataRepository::get_by_book_id(db.sea_orm_connection(), book_id)
        .await
        .unwrap()
        .and_then(|m| m.number)
        .and_then(|n| n.to_string().parse::<f32>().ok())
}

/// A file_order library must not let per-book analysis write a number at all:
/// the position it would compute is only valid for the row set that happens to
/// exist at that instant.
#[tokio::test]
async fn test_file_order_analysis_defers_numbering_to_renumber_task() -> Result<()> {
    let (db, _db_dir) = setup_test_db_wrapper().await;
    let files = TempDir::new()?;
    let (library, series) = setup_library(&db, NumberStrategy::FileOrder).await?;

    let path = write_bare_cbz(files.path(), "Staged Series - ch 001.cbz");
    let book = insert_and_analyze(&db, library.id, series.id, &path).await?;

    assert_eq!(
        number_of(&db, book.id).await,
        None,
        "file_order analysis must leave `number` unset for the renumber pass to own"
    );

    let pending = TaskRepository::list(
        db.sea_orm_connection(),
        Some("pending".to_string()),
        None,
        Some(100),
    )
    .await?;
    assert!(
        pending
            .iter()
            .any(|t| t.task_type == "renumber_series" && t.series_id == Some(series.id)),
        "analysis should enqueue a renumber_series task for the series, got {:?}",
        pending.iter().map(|t| &t.task_type).collect::<Vec<_>>()
    );

    Ok(())
}

/// The reported bug: books analyzed against a partial series get positions from
/// a smaller snapshot, so a late-sorting file lands on a number that a later
/// batch hands out again. After the renumber pass the numbers must be a unique,
/// contiguous 1..N in natural filename order.
#[tokio::test]
async fn test_staged_analysis_yields_unique_contiguous_numbers() -> Result<()> {
    let (db, _db_dir) = setup_test_db_wrapper().await;
    let files = TempDir::new()?;
    let conn = db.sea_orm_connection();
    let (library, series) = setup_library(&db, NumberStrategy::FileOrder).await?;

    // First flush: chapters 1-5 plus a far-later chapter that sorts 6th here
    // but 11th once the series is complete.
    let mut first_batch: Vec<String> = (1..=5)
        .map(|i| format!("Staged Series - ch {:03}.cbz", i))
        .collect();
    first_batch.push("Staged Series - ch 070.cbz".to_string());

    for name in &first_batch {
        let path = write_bare_cbz(files.path(), name);
        insert_and_analyze(&db, library.id, series.id, &path).await?;
    }

    // Second flush: chapters 6-10. Under the old behaviour "ch 006" is the 6th
    // file in the completed set and collides with "ch 070".
    for i in 6..=10 {
        let name = format!("Staged Series - ch {:03}.cbz", i);
        let path = write_bare_cbz(files.path(), &name);
        insert_and_analyze(&db, library.id, series.id, &path).await?;
    }

    // Drain the queue: the numbering must come from the renumber task the
    // analysis path enqueued, not from anything analysis wrote itself.
    drain_tasks(conn).await;

    let books = BookRepository::list_by_series(conn, series.id, false).await?;
    assert_eq!(books.len(), 11);

    let mut numbered: Vec<(String, i64)> = Vec::new();
    for book in &books {
        let number = number_of(&db, book.id)
            .await
            .unwrap_or_else(|| panic!("book '{}' has no number after renumber", book.file_name));
        numbered.push((book.file_name.clone(), number as i64));
    }

    let mut assigned: Vec<i64> = numbered.iter().map(|(_, n)| *n).collect();
    assigned.sort_unstable();
    assert_eq!(
        assigned,
        (1..=11).collect::<Vec<_>>(),
        "numbers must be unique and contiguous, got {numbered:?}"
    );

    numbered.sort_by(|a, b| codex::utils::natural_cmp_filename(&a.0, &b.0));
    for (index, (file_name, number)) in numbered.iter().enumerate() {
        assert_eq!(
            *number,
            index as i64 + 1,
            "'{file_name}' should be #{} in natural order, got {number}",
            index + 1
        );
    }

    Ok(())
}

/// Smart resolves metadata and filename numbers from the file alone, which is
/// race-free. Those must survive analysis without a renumber task being queued
/// to overwrite them.
#[tokio::test]
async fn test_smart_keeps_file_derived_number_without_renumber_task() -> Result<()> {
    let (db, _db_dir) = setup_test_db_wrapper().await;
    let files = TempDir::new()?;
    let (library, series) = setup_library(&db, NumberStrategy::Smart).await?;

    let path = write_cbz_with_number(files.path(), "Staged Series - ch 070.cbz", 70);
    let book = insert_and_analyze(&db, library.id, series.id, &path).await?;

    assert_eq!(
        number_of(&db, book.id).await,
        Some(70.0),
        "smart must keep the ComicInfo number rather than a positional one"
    );

    let pending = TaskRepository::list(
        db.sea_orm_connection(),
        Some("pending".to_string()),
        None,
        Some(100),
    )
    .await?;
    assert!(
        !pending
            .iter()
            .any(|t| t.task_type == "renumber_series" && t.series_id == Some(series.id)),
        "no renumber needed when the file itself supplied the number"
    );

    Ok(())
}

/// A renumber pass reads the series once, at the top, and skips any book that
/// has no `book_metadata` row yet. A book whose analysis finishes while that
/// pass is in flight is therefore invisible to it, and the follow-up pass the
/// analysis tries to queue is swallowed by the dedup on (task type, series),
/// which matches tasks in `processing` as well as `pending`. Nothing runs
/// afterwards, so the book keeps a null number for good.
///
/// The visible symptom is a series numbered 1, 2, 5, 6, 7 with two books
/// showing no number at all — gaps that never fill in, not even on a rescan
/// that finds nothing new.
#[tokio::test]
async fn test_book_analyzed_during_a_renumber_pass_still_ends_up_numbered() -> Result<()> {
    let (db, _db_dir) = setup_test_db_wrapper().await;
    let files = TempDir::new()?;
    let conn = db.sea_orm_connection();
    let (library, series) = setup_library(&db, NumberStrategy::FileOrder).await?;

    // Five books already analyzed and numbered by a pass that ran to completion.
    for i in 1..=5 {
        let name = format!("Staged Series - ch {:03}.cbz", i);
        let path = write_bare_cbz(files.path(), &name);
        insert_and_analyze(&db, library.id, series.id, &path).await?;
    }
    drain_tasks(conn).await;

    // A sixth book lands. The scan inserts the row and queues the pass; its
    // analysis has not run yet, so it has no metadata row.
    let path = write_bare_cbz(files.path(), "Staged Series - ch 006.cbz");
    let late = insert_book(&db, library.id, series.id, &path).await?;
    TaskRepository::enqueue(
        conn,
        codex::tasks::types::TaskType::RenumberSeries {
            series_id: series.id,
        },
        None,
    )
    .await?;

    // The worker claims the pass and runs it. `ch 006` has no metadata row at
    // this point, so the pass leaves it alone.
    let claimed = TaskRepository::claim_next(conn, "worker-renumber", 60)
        .await?
        .expect("the renumber task should be claimable");
    assert_eq!(claimed.task_type, "renumber_series");
    codex::scanner::renumber_series_books(conn, series.id, library.id).await?;

    // The late book's analysis finishes while the pass is still marked
    // `processing`, which is exactly the window the scanner leaves open.
    analyze_book(conn, late.id, false, None).await?;

    TaskRepository::mark_completed(conn, claimed.id, None).await?;

    // Whatever the queue holds now must be enough to finish the job.
    drain_tasks(conn).await;

    assert_eq!(
        number_of(&db, late.id).await,
        Some(6.0),
        "a book analyzed during a renumber pass must still get numbered"
    );

    Ok(())
}

/// Re-analysis must not blank a number the renumber pass already assigned.
///
/// A positional strategy resolves nothing from the file alone, so analysis has
/// nothing better to write than the number already there. Writing null instead
/// makes every book in a deep scan show no number until the pass catches up,
/// and loses the number outright for any book whose follow-up pass is missed.
#[tokio::test]
async fn test_reanalysis_keeps_the_number_the_pass_assigned() -> Result<()> {
    let (db, _db_dir) = setup_test_db_wrapper().await;
    let files = TempDir::new()?;
    let conn = db.sea_orm_connection();
    let (library, series) = setup_library(&db, NumberStrategy::FileOrder).await?;

    let path = write_bare_cbz(files.path(), "Staged Series - ch 001.cbz");
    let book = insert_and_analyze(&db, library.id, series.id, &path).await?;
    drain_tasks(conn).await;
    assert_eq!(number_of(&db, book.id).await, Some(1.0));

    // A deep scan re-analyzes every book.
    analyze_book(conn, book.id, true, None).await?;

    assert_eq!(
        number_of(&db, book.id).await,
        Some(1.0),
        "re-analysis must leave the assigned number alone, not blank it until a pass runs"
    );

    Ok(())
}
