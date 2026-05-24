//! Populate a `FuzzyIndex` from the database.
//!
//! One pass loads every (non-deleted) series + its metadata, alt titles, and
//! authors; a second pass loads every (non-deleted) book + its metadata title.
//! The resulting vecs replace the index contents in-place.

use anyhow::{Context, Result};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use serde_json::Value;
use tracing::warn;
use uuid::Uuid;

use codex_db::entities::{book_metadata, books, prelude::*, series, series_metadata};
use codex_db::repositories::AlternateTitleRepository;

use super::index::{BookEntry, BookSources, FuzzyIndex, SeriesEntry, SeriesSources};

/// Fetch one series and assemble a `SeriesEntry`.
///
/// Returns `Ok(None)` when the series row does not exist (already deleted).
/// Used by the event listener to upsert a single series after a
/// `SeriesCreated` / `SeriesUpdated` / `SeriesMetadataUpdated` event.
pub async fn fetch_series_entry(
    db: &DatabaseConnection,
    series_id: Uuid,
) -> Result<Option<SeriesEntry>> {
    let Some(series_row) = Series::find_by_id(series_id)
        .one(db)
        .await
        .context("Failed to load series for fuzzy index upsert")?
    else {
        return Ok(None);
    };

    let metadata_row = SeriesMetadata::find_by_id(series_id)
        .one(db)
        .await
        .context("Failed to load series metadata for fuzzy index upsert")?;

    let alt_titles_by_series = AlternateTitleRepository::get_for_series_ids(db, &[series_id])
        .await
        .context("Failed to load alt titles for fuzzy index upsert")?;
    let alt_titles: Vec<String> = alt_titles_by_series
        .get(&series_id)
        .map(|v| v.iter().map(|t| t.title.clone()).collect())
        .unwrap_or_default();

    let (title, title_sort, authors) = match metadata_row {
        Some(m) => {
            let authors = parse_authors_names(m.authors_json.as_deref(), series_id);
            (m.title, m.title_sort, authors)
        }
        None => (series_row.name.clone(), None, Vec::new()),
    };

    Ok(Some(SeriesEntry::new(
        series_row.id,
        series_row.library_id,
        SeriesSources {
            title,
            title_sort,
            name: series_row.name,
            alt_titles,
            authors,
        },
    )))
}

/// Fetch one book and assemble a `BookEntry`.
///
/// Returns `Ok(None)` when the book row does not exist or is soft-deleted
/// (`books.deleted = true`); soft-deleted rows are intentionally absent
/// from the index, mirroring the builder's filter.
pub async fn fetch_book_entry(db: &DatabaseConnection, book_id: Uuid) -> Result<Option<BookEntry>> {
    let Some(book_row) = Books::find_by_id(book_id)
        .one(db)
        .await
        .context("Failed to load book for fuzzy index upsert")?
    else {
        return Ok(None);
    };
    if book_row.deleted {
        return Ok(None);
    }

    let metadata_row = BookMetadata::find_by_id(book_id)
        .one(db)
        .await
        .context("Failed to load book metadata for fuzzy index upsert")?;
    let title = metadata_row.and_then(|m| m.title);

    Ok(Some(BookEntry::new(
        book_row.id,
        book_row.series_id,
        book_row.library_id,
        BookSources {
            title,
            file_name: book_row.file_name,
        },
    )))
}

/// Build the entire index from scratch.
///
/// Returns the populated index. Logs counts and elapsed time.
pub async fn build_from_db(db: &DatabaseConnection) -> Result<FuzzyIndex> {
    let index = FuzzyIndex::empty();
    rebuild_into(&index, db).await?;
    Ok(index)
}

/// Rebuild `index` in place from the database. Replaces both the series and
/// books vecs. Used by Phase 2 lag-recovery and by `build_from_db`.
pub async fn rebuild_into(index: &FuzzyIndex, db: &DatabaseConnection) -> Result<()> {
    let started = std::time::Instant::now();

    let series_entries = load_series(db).await?;
    let book_entries = load_books(db).await?;

    let series_count = series_entries.len();
    let book_count = book_entries.len();

    index.replace_series(series_entries);
    index.replace_books(book_entries);

    let elapsed = started.elapsed();
    let approx_bytes = index.approx_memory_bytes();
    tracing::info!(
        target: "search::fuzzy",
        elapsed_ms = elapsed.as_millis() as u64,
        series = series_count,
        books = book_count,
        approx_mem_bytes = approx_bytes,
        "fuzzy search index built"
    );
    Ok(())
}

async fn load_series(db: &DatabaseConnection) -> Result<Vec<SeriesEntry>> {
    // Pull every series joined with its metadata. We do this in one round-trip
    // rather than series + metadata separately to keep startup latency low.
    let rows: Vec<(series::Model, Option<series_metadata::Model>)> = Series::find()
        .find_also_related(SeriesMetadata)
        .all(db)
        .await
        .context("Failed to load series for fuzzy index")?;

    let ids: Vec<Uuid> = rows.iter().map(|(s, _)| s.id).collect();
    let alt_titles_by_series = AlternateTitleRepository::get_for_series_ids(db, &ids)
        .await
        .context("Failed to load alternate titles for fuzzy index")?;

    let mut entries = Vec::with_capacity(rows.len());
    for (series_row, metadata_row) in rows {
        let alt_titles: Vec<String> = alt_titles_by_series
            .get(&series_row.id)
            .map(|v| v.iter().map(|t| t.title.clone()).collect())
            .unwrap_or_default();

        let (title, title_sort, authors) = match metadata_row {
            Some(m) => {
                let authors = parse_authors_names(m.authors_json.as_deref(), series_row.id);
                (m.title, m.title_sort, authors)
            }
            None => (series_row.name.clone(), None, Vec::new()),
        };

        entries.push(SeriesEntry::new(
            series_row.id,
            series_row.library_id,
            SeriesSources {
                title,
                title_sort,
                name: series_row.name,
                alt_titles,
                authors,
            },
        ));
    }
    Ok(entries)
}

async fn load_books(db: &DatabaseConnection) -> Result<Vec<BookEntry>> {
    // Skip soft-deleted books — they aren't visible in any user-facing list,
    // so indexing them would only waste memory and pollute results.
    let rows: Vec<(books::Model, Option<book_metadata::Model>)> = Books::find()
        .filter(books::Column::Deleted.eq(false))
        .find_also_related(BookMetadata)
        .all(db)
        .await
        .context("Failed to load books for fuzzy index")?;

    let mut entries = Vec::with_capacity(rows.len());
    for (book_row, metadata_row) in rows {
        let title = metadata_row.and_then(|m| m.title);
        entries.push(BookEntry::new(
            book_row.id,
            book_row.series_id,
            book_row.library_id,
            BookSources {
                title,
                file_name: book_row.file_name,
            },
        ));
    }
    Ok(entries)
}

/// Best-effort extraction of author names from the `authors_json` blob.
///
/// The shape is documented as `[{"name": "...", "role": "..."}, ...]` but real
/// data has been observed as a plain array of strings on older rows. Parse
/// defensively: extract whatever names we can find and skip the rest. A
/// malformed blob logs a warning and yields an empty list so the rest of the
/// series still indexes cleanly.
fn parse_authors_names(authors_json: Option<&str>, series_id: Uuid) -> Vec<String> {
    let Some(raw) = authors_json else {
        return Vec::new();
    };
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }
    let value: Value = match serde_json::from_str(trimmed) {
        Ok(v) => v,
        Err(err) => {
            warn!(
                target: "search::fuzzy",
                series_id = %series_id,
                error = %err,
                "skipping authors_json for fuzzy index: invalid JSON"
            );
            return Vec::new();
        }
    };
    let Value::Array(items) = value else {
        return Vec::new();
    };
    items
        .into_iter()
        .filter_map(|v| match v {
            Value::String(s) => {
                let s = s.trim().to_string();
                if s.is_empty() { None } else { Some(s) }
            }
            Value::Object(map) => map
                .get("name")
                .and_then(Value::as_str)
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty()),
            _ => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use codex_db::ScanningStrategy;
    use codex_db::entities::books;
    use codex_db::repositories::{
        AlternateTitleRepository as AltRepo, BookRepository, LibraryRepository,
        SeriesMetadataRepository, SeriesRepository,
    };
    use codex_db::test_helpers::create_test_db;

    fn book_model(series_id: Uuid, library_id: Uuid, path: &str, name: &str) -> books::Model {
        let now = Utc::now();
        books::Model {
            id: Uuid::new_v4(),
            series_id,
            library_id,
            path: path.to_string(),
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
        }
    }

    #[test]
    fn parses_authors_json_object_array() {
        let blob = r#"[
            {"name": "Eiichiro Oda", "role": "writer"},
            {"name": "Hirohiko Araki", "role": "writer"}
        ]"#;
        let names = parse_authors_names(Some(blob), Uuid::nil());
        assert_eq!(names, vec!["Eiichiro Oda", "Hirohiko Araki"]);
    }

    #[test]
    fn parses_authors_json_string_array() {
        let blob = r#"["Kentaro Miura", "Studio Gaga"]"#;
        let names = parse_authors_names(Some(blob), Uuid::nil());
        assert_eq!(names, vec!["Kentaro Miura", "Studio Gaga"]);
    }

    #[test]
    fn parses_authors_json_handles_garbage() {
        assert!(parse_authors_names(Some("not json"), Uuid::nil()).is_empty());
        assert!(parse_authors_names(Some(""), Uuid::nil()).is_empty());
        assert!(parse_authors_names(None, Uuid::nil()).is_empty());
        // Object instead of array → empty, but no panic.
        assert!(parse_authors_names(Some("{\"name\":\"x\"}"), Uuid::nil()).is_empty());
    }

    #[tokio::test]
    async fn builds_from_seeded_db_and_matches_gapped_query() {
        let (db, _temp) = create_test_db().await;
        let conn = db.sea_orm_connection();

        let library = LibraryRepository::create(
            conn,
            "Test Library",
            "/test/path",
            ScanningStrategy::Default,
        )
        .await
        .unwrap();

        // Seed a target series + two distractors. Metadata rows are created
        // implicitly by SeriesRepository::create using the series name as the
        // title; we then update the target's title explicitly so the metadata
        // title (not the directory name) drives the match.
        let target = SeriesRepository::create(conn, library.id, "One-Punch Man", None)
            .await
            .unwrap();
        AltRepo::create(conn, target.id, "Japanese", "ワンパンマン", None)
            .await
            .unwrap();

        SeriesRepository::create(conn, library.id, "Berserk", None)
            .await
            .unwrap();
        SeriesRepository::create(conn, library.id, "Naruto", None)
            .await
            .unwrap();

        let index = build_from_db(conn).await.unwrap();
        assert_eq!(index.series_count(), 3);

        let hits = index.search_series("on ch", 10, None);
        let top = hits.first().expect("at least one hit for 'on ch'");
        assert_eq!(
            top.0, target.id,
            "expected One-Punch Man to rank first for 'on ch', got {:?}",
            hits
        );

        // Alt title is reachable from the same haystack.
        let jp_hits = index.search_series("ワンパン", 10, None);
        assert_eq!(jp_hits.first().map(|h| h.0), Some(target.id));
    }

    #[tokio::test]
    async fn books_are_indexed_with_filename_and_title() {
        let (db, _temp) = create_test_db().await;
        let conn = db.sea_orm_connection();

        let library = LibraryRepository::create(
            conn,
            "Test Library",
            "/test/path",
            ScanningStrategy::Default,
        )
        .await
        .unwrap();
        let series = SeriesRepository::create(conn, library.id, "Berserk", None)
            .await
            .unwrap();

        let book_to_create = book_model(
            series.id,
            library.id,
            "/test/berserk/vol01.cbz",
            "berserk-volume-01.cbz",
        );
        let book = BookRepository::create(conn, &book_to_create, None)
            .await
            .unwrap();

        let index = build_from_db(conn).await.unwrap();
        assert_eq!(index.book_count(), 1);
        let hits = index.search_books("berserk volume", 10, None);
        let top = hits.first().expect("at least one book hit");
        assert_eq!(top.0, book.id);
        assert_eq!(top.1, series.id);
    }

    #[tokio::test]
    async fn deleted_books_are_skipped() {
        let (db, _temp) = create_test_db().await;
        let conn = db.sea_orm_connection();

        let library = LibraryRepository::create(
            conn,
            "Test Library",
            "/test/path",
            ScanningStrategy::Default,
        )
        .await
        .unwrap();
        let series = SeriesRepository::create(conn, library.id, "Berserk", None)
            .await
            .unwrap();
        let book = BookRepository::create(
            conn,
            &book_model(
                series.id,
                library.id,
                "/test/berserk/vol01.cbz",
                "berserk-volume-01.cbz",
            ),
            None,
        )
        .await
        .unwrap();
        BookRepository::mark_deleted(conn, book.id, true, None)
            .await
            .unwrap();

        let index = build_from_db(conn).await.unwrap();
        assert_eq!(index.book_count(), 0);
    }

    #[tokio::test]
    async fn metadata_title_drives_match() {
        // Series is created with directory name "one-punch-man" — replace its
        // metadata title with "One-Punch Man" and ensure that the
        // metadata-driven title is what we match against.
        let (db, _temp) = create_test_db().await;
        let conn = db.sea_orm_connection();

        let library = LibraryRepository::create(
            conn,
            "Test Library",
            "/test/path",
            ScanningStrategy::Default,
        )
        .await
        .unwrap();
        let target = SeriesRepository::create(conn, library.id, "one-punch-man", None)
            .await
            .unwrap();
        SeriesMetadataRepository::update_title(
            conn,
            target.id,
            "One-Punch Man".to_string(),
            None,
            None,
        )
        .await
        .unwrap();

        let index = build_from_db(conn).await.unwrap();
        // Searching by the metadata title should hit.
        let hits = index.search_series("One-Punch Man", 10, None);
        assert_eq!(hits.first().map(|h| h.0), Some(target.id));
    }
}
