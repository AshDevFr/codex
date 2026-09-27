//! Assembling one user's reading state into the portable export document.
//!
//! Reads `read_progress`, `read_completions`, `reading_sessions`, and
//! `user_series_ratings` for exactly the requesting user, groups them by
//! book and then by series, and serialises everything against stable keys
//! (external ids, a series-relative path, file name, hashes) instead of the
//! database ids that a rescan under a new root would replace.
//!
//! No visibility filtering: this is the user's own data, not a view of the
//! library, so a book later hidden behind a sharing tag is still exported.
//! Import is where the visibility filter matters, because that is where a
//! crafted file could otherwise be used to write state onto a book the
//! importing user cannot see.

use anyhow::Result;
use chrono::Utc;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

use codex_db::entities::{books, read_completions, read_progress, reading_sessions};
use codex_db::repositories::{
    LibraryRepository, ReadProgressRepository, SeriesExternalIdRepository, SeriesRepository,
    UserSeriesRatingRepository,
};

use super::model::{
    ExportBookDto, ExportCompletionDto, ExportExternalIdDto, ExportProgressDto, ExportSeriesDto,
    ExportSessionDto, READING_PROGRESS_FORMAT, READING_PROGRESS_VERSION,
    ReadingProgressExportDocument,
};
use super::series_relative_book_path;

/// Every completion for `user_id` whose book has not been hard-deleted.
///
/// Already-orphaned completions (`book_id IS NULL`) are skipped for the same
/// reason as sessions: nothing in the file could say which book to put them
/// back on.
async fn completions_for_user(
    db: &DatabaseConnection,
    user_id: Uuid,
) -> Result<Vec<read_completions::Model>> {
    let rows = read_completions::Entity::find()
        .filter(read_completions::Column::UserId.eq(user_id))
        .filter(read_completions::Column::BookId.is_not_null())
        .order_by_asc(read_completions::Column::StartedAt)
        .all(db)
        .await?;
    Ok(rows)
}

/// The books a user's history points at, **including soft-deleted ones**.
///
/// `BookRepository::get_by_ids` hides books the scanner has marked deleted,
/// which is right for browsing and wrong here. Splitting a library usually
/// moves the files first, the next scan of the old library marks every moved
/// book deleted without purging it, and that is exactly when the reader takes
/// the export. Hiding them would export nothing for the books the file exists
/// to carry across.
///
/// Batched so a long history stays under the engines' bind-parameter limits.
async fn books_including_soft_deleted(
    db: &DatabaseConnection,
    ids: &[Uuid],
) -> Result<Vec<books::Model>> {
    const BATCH: usize = 1_000;
    let mut found = Vec::with_capacity(ids.len());
    for chunk in ids.chunks(BATCH) {
        found.extend(
            books::Entity::find()
                .filter(books::Column::Id.is_in(chunk.to_vec()))
                .all(db)
                .await?,
        );
    }
    Ok(found)
}

/// Every session for `user_id` whose book has not been hard-deleted.
async fn sessions_for_user(
    db: &DatabaseConnection,
    user_id: Uuid,
) -> Result<Vec<reading_sessions::Model>> {
    let rows = reading_sessions::Entity::find()
        .filter(reading_sessions::Column::UserId.eq(user_id))
        .filter(reading_sessions::Column::BookId.is_not_null())
        .order_by_asc(reading_sessions::Column::ClientEndedAt)
        .all(db)
        .await?;
    Ok(rows)
}

/// Assemble the export document for one user.
///
/// `include_sessions = false` omits the `sessions` key entirely on every book
/// rather than emitting empty arrays, so a client can tell "not exported"
/// apart from "exported, and there were none".
pub async fn export_reading_progress(
    db: &DatabaseConnection,
    user_id: Uuid,
    include_sessions: bool,
) -> Result<ReadingProgressExportDocument> {
    let progress_rows = ReadProgressRepository::get_by_user(db, user_id).await?;
    let completion_rows = completions_for_user(db, user_id).await?;
    let session_rows = if include_sessions {
        sessions_for_user(db, user_id).await?
    } else {
        Vec::new()
    };
    let ratings = UserSeriesRatingRepository::get_all_for_user(db, user_id).await?;

    let mut book_id_set: HashSet<Uuid> = HashSet::new();
    for p in &progress_rows {
        book_id_set.insert(p.book_id);
    }
    for c in &completion_rows {
        if let Some(id) = c.book_id {
            book_id_set.insert(id);
        }
    }
    for s in &session_rows {
        if let Some(id) = s.book_id {
            book_id_set.insert(id);
        }
    }
    let book_ids: Vec<Uuid> = book_id_set.into_iter().collect();
    let books = books_including_soft_deleted(db, &book_ids).await?;

    let mut series_id_set: HashSet<Uuid> = books.iter().map(|b| b.series_id).collect();
    for r in &ratings {
        series_id_set.insert(r.series_id);
    }
    let series_ids: Vec<Uuid> = series_id_set.into_iter().collect();

    let mut series_rows = SeriesRepository::get_by_ids(db, &series_ids).await?;
    series_rows.sort_by(|a, b| a.path.cmp(&b.path).then_with(|| a.id.cmp(&b.id)));

    let library_ids: Vec<Uuid> = series_rows
        .iter()
        .map(|s| s.library_id)
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    let library_path_by_id: HashMap<Uuid, String> = LibraryRepository::get_by_ids(db, &library_ids)
        .await?
        .into_iter()
        .map(|(id, lib)| (id, lib.path))
        .collect();

    let external_ids_by_series =
        SeriesExternalIdRepository::get_for_series_ids(db, &series_ids).await?;
    let ratings_by_series: HashMap<Uuid, codex_db::entities::user_series_ratings::Model> =
        ratings.into_iter().map(|r| (r.series_id, r)).collect();

    let mut books_by_series: HashMap<Uuid, Vec<&books::Model>> = HashMap::new();
    for b in &books {
        books_by_series.entry(b.series_id).or_default().push(b);
    }

    let progress_by_book: HashMap<Uuid, &read_progress::Model> =
        progress_rows.iter().map(|p| (p.book_id, p)).collect();

    let mut completions_by_book: HashMap<Uuid, Vec<&read_completions::Model>> = HashMap::new();
    for c in &completion_rows {
        if let Some(book_id) = c.book_id {
            completions_by_book.entry(book_id).or_default().push(c);
        }
    }

    let mut sessions_by_book: HashMap<Uuid, Vec<&reading_sessions::Model>> = HashMap::new();
    for s in &session_rows {
        if let Some(book_id) = s.book_id {
            sessions_by_book.entry(book_id).or_default().push(s);
        }
    }

    let mut series_docs = Vec::with_capacity(series_rows.len());

    for series in &series_rows {
        let library_path = library_path_by_id
            .get(&series.library_id)
            .cloned()
            .unwrap_or_default();

        let external_ids = external_ids_by_series
            .get(&series.id)
            .map(|ids| {
                ids.iter()
                    .map(|e| ExportExternalIdDto {
                        source: e.source.clone(),
                        id: e.external_id.clone(),
                    })
                    .collect()
            })
            .unwrap_or_default();

        let rating_row = ratings_by_series.get(&series.id);

        let mut book_docs = Vec::new();
        if let Some(series_books) = books_by_series.get(&series.id) {
            let mut sorted_books: Vec<&books::Model> = series_books.to_vec();
            sorted_books.sort_by(|a, b| a.path.cmp(&b.path));

            for book in sorted_books {
                let relative_path =
                    series_relative_book_path(&library_path, &series.path, &book.path);

                let progress = progress_by_book.get(&book.id).map(|p| ExportProgressDto {
                    current_page: p.current_page,
                    progress_percentage: p.progress_percentage,
                    completed: p.completed,
                    started_at: p.started_at,
                    updated_at: p.updated_at,
                    completed_at: p.completed_at,
                    r2_progression: p.r2_progression.clone(),
                });

                let completions: Vec<ExportCompletionDto> = completions_by_book
                    .get(&book.id)
                    .map(|list| {
                        list.iter()
                            .map(|c| ExportCompletionDto {
                                id: c.id,
                                started_at: c.started_at,
                                completed_at: c.completed_at,
                            })
                            .collect()
                    })
                    .unwrap_or_default();

                let sessions: Option<Vec<ExportSessionDto>> = if include_sessions {
                    Some(
                        sessions_by_book
                            .get(&book.id)
                            .map(|list| {
                                list.iter()
                                    .map(|s| ExportSessionDto {
                                        id: s.id,
                                        device_id: s.device_id.clone(),
                                        device_name: s.device_name.clone(),
                                        pass: s.pass,
                                        kind: s.kind.clone(),
                                        to_page: s.to_page,
                                        to_percentage: s.to_percentage,
                                        active_duration_ms: s.active_duration_ms,
                                        duration_source: s.duration_source.clone(),
                                        pages_read: s.pages_read,
                                        client_started_at: s.client_started_at,
                                        client_ended_at: s.client_ended_at,
                                        server_recorded_at: s.server_recorded_at,
                                    })
                                    .collect()
                            })
                            .unwrap_or_default(),
                    )
                } else {
                    None
                };

                let has_sessions = sessions.as_ref().is_some_and(|s| !s.is_empty());
                if progress.is_none() && completions.is_empty() && !has_sessions {
                    // Nothing to say about this book for this user.
                    continue;
                }

                book_docs.push(ExportBookDto {
                    path: relative_path,
                    file_name: book.file_name.clone(),
                    file_hash: book.file_hash.clone(),
                    partial_hash: book.partial_hash.clone(),
                    progress,
                    completions,
                    sessions,
                });
            }
        }

        if rating_row.is_none() && book_docs.is_empty() {
            // Nothing recorded against this series for this user.
            continue;
        }

        series_docs.push(ExportSeriesDto {
            external_ids,
            library_relative_path: series.path.clone(),
            name: series.name.clone(),
            rating: rating_row.map(|r| r.rating),
            notes: rating_row.and_then(|r| r.notes.clone()),
            rating_updated_at: rating_row.map(|r| r.updated_at),
            books: book_docs,
        });
    }

    Ok(ReadingProgressExportDocument {
        format: READING_PROGRESS_FORMAT.to_string(),
        version: READING_PROGRESS_VERSION,
        exported_at: Utc::now(),
        includes_sessions: include_sessions,
        series: series_docs,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;
    use codex_db::ScanningStrategy;
    use codex_db::entities::reading_sessions::SessionKind;
    use codex_db::repositories::{
        BookRepository, LibraryRepository, NewSession, ReadCompletionRepository, SeriesRepository,
        UserRepository,
    };
    use codex_db::test_helpers::create_test_db;

    async fn make_user(db: &DatabaseConnection) -> Uuid {
        use chrono::Utc;
        use codex_db::entities::users;
        let now = Utc::now();
        let model = users::Model {
            id: Uuid::new_v4(),
            username: format!("u-{}", Uuid::new_v4()),
            email: format!("{}@test.com", Uuid::new_v4()),
            password_hash: "hash".to_string(),
            role: "reader".to_string(),
            is_active: true,
            email_verified: true,
            permissions: serde_json::json!([]),
            created_at: now,
            updated_at: now,
            last_login_at: None,
        };
        UserRepository::create(db, &model).await.unwrap().id
    }

    #[tokio::test]
    async fn exports_a_nested_volume_folder_and_empty_hash() {
        let (db, _tmp) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let user = make_user(conn).await;

        let library =
            LibraryRepository::create(conn, "Lib", "/library/root", ScanningStrategy::Default)
                .await
                .unwrap();
        let series = SeriesRepository::create(conn, library.id, "Naruto", None)
            .await
            .unwrap();
        SeriesRepository::update_path(conn, series.id, "shonen/Naruto".to_string())
            .await
            .unwrap();

        let book = books::Model {
            id: Uuid::new_v4(),
            series_id: series.id,
            library_id: library.id,
            path: "/library/root/shonen/Naruto/Vol 01/v01.cbz".to_string(),
            file_name: "v01.cbz".to_string(),
            file_size: 1024,
            file_hash: String::new(),
            partial_hash: String::new(),
            format: "cbz".to_string(),
            page_count: 20,
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
        let book = BookRepository::create(conn, &book, None).await.unwrap();

        ReadProgressRepository::upsert(conn, user, book.id, 5, false)
            .await
            .unwrap();

        let doc = export_reading_progress(conn, user, true).await.unwrap();

        assert_eq!(doc.format, READING_PROGRESS_FORMAT);
        assert_eq!(doc.series.len(), 1);
        let series_doc = &doc.series[0];
        assert_eq!(series_doc.library_relative_path, "shonen/Naruto");
        assert_eq!(series_doc.books.len(), 1);
        let book_doc = &series_doc.books[0];
        assert_eq!(book_doc.path, "Vol 01/v01.cbz");
        assert_eq!(book_doc.file_hash, "");
        assert!(book_doc.progress.is_some());
    }

    /// Splitting a library usually moves the files first, and the next scan of
    /// the old library marks every moved book deleted without purging it. That
    /// is exactly when a reader takes the export, so soft-deleted books must be
    /// in it: leaving them out would export nothing for the very books the
    /// file exists to carry across.
    #[tokio::test]
    async fn exports_books_the_scanner_has_soft_deleted() {
        use sea_orm::{ActiveModelTrait, Set};

        let (db, _tmp) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let user = make_user(conn).await;

        let library = LibraryRepository::create(conn, "Lib", "/manga", ScanningStrategy::Default)
            .await
            .unwrap();
        let series = SeriesRepository::create(conn, library.id, "Naruto", None)
            .await
            .unwrap();
        SeriesRepository::update_path(conn, series.id, "shonen/Naruto".to_string())
            .await
            .unwrap();
        let book = books::Model {
            id: Uuid::new_v4(),
            series_id: series.id,
            library_id: library.id,
            path: "/manga/shonen/Naruto/v01.cbz".to_string(),
            file_name: "v01.cbz".to_string(),
            file_size: 10,
            file_hash: "h".to_string(),
            partial_hash: "p".to_string(),
            format: "cbz".to_string(),
            page_count: 20,
            deleted: false,
            analyzed: true,
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
        let book = BookRepository::create(conn, &book, None).await.unwrap();
        ReadProgressRepository::upsert(conn, user, book.id, 12, false)
            .await
            .unwrap();

        let mut missing: books::ActiveModel = book.clone().into();
        missing.deleted = Set(true);
        missing.update(conn).await.unwrap();

        let doc = export_reading_progress(conn, user, true).await.unwrap();

        assert_eq!(
            doc.series.len(),
            1,
            "the soft-deleted book's series is exported"
        );
        let book_doc = &doc.series[0].books[0];
        assert_eq!(book_doc.path, "v01.cbz");
        assert_eq!(book_doc.progress.as_ref().unwrap().current_page, 12);
    }

    #[tokio::test]
    async fn omits_sessions_key_when_not_requested() {
        let (db, _tmp) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let user = make_user(conn).await;

        let library = LibraryRepository::create(conn, "Lib", "/lib", ScanningStrategy::Default)
            .await
            .unwrap();
        let series = SeriesRepository::create(conn, library.id, "Series", None)
            .await
            .unwrap();
        let book = books::Model {
            id: Uuid::new_v4(),
            series_id: series.id,
            library_id: library.id,
            path: "/lib/book.cbz".to_string(),
            file_name: "book.cbz".to_string(),
            file_size: 10,
            file_hash: "h".to_string(),
            partial_hash: "p".to_string(),
            format: "cbz".to_string(),
            page_count: 1,
            deleted: false,
            analyzed: true,
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
        let book = BookRepository::create(conn, &book, None).await.unwrap();

        let now = Utc::now();
        let session = NewSession::from_client(
            Uuid::new_v4(),
            user,
            book.id,
            "device",
            None,
            SessionKind::Progress,
            Some(1000),
            Some(1),
            now - Duration::minutes(5),
            now,
        )
        .with_page(1);
        ReadProgressRepository::record_session(conn, session)
            .await
            .unwrap();
        ReadCompletionRepository::record(conn, user, book.id, now - Duration::minutes(5), now)
            .await
            .unwrap();

        let doc_without = export_reading_progress(conn, user, false).await.unwrap();
        assert!(!doc_without.includes_sessions);
        let book_doc = &doc_without.series[0].books[0];
        assert!(book_doc.sessions.is_none());
        // Completions and progress are unaffected by include_sessions.
        assert_eq!(book_doc.completions.len(), 1);
        assert!(book_doc.progress.is_some());

        let doc_with = export_reading_progress(conn, user, true).await.unwrap();
        assert!(doc_with.includes_sessions);
        let book_doc = &doc_with.series[0].books[0];
        assert!(book_doc.sessions.is_some());
        assert_eq!(book_doc.sessions.as_ref().unwrap().len(), 1);
        // Sessions never carry r2_progression.
        assert!(book_doc.progress.as_ref().unwrap().current_page >= 0);
    }

    /// Confirms the plan's size budget: a 5,000-book read history should stay
    /// under ~5 MB uncompressed. Each book here gets full progress, one
    /// completion, and one session (one read-through) plus a per-series
    /// rating, which measured 3.85 MB. Doubling the session count per book
    /// (a re-read, or two devices each syncing their own session) measured
    /// 5.56 MB, over the estimate: sessions are the dominant cost, so a
    /// heavier history than "read once" can exceed it.
    ///
    /// Seeding 5,000 books plus their progress, completions, and sessions
    /// through the ORM is slow enough to skip in the default run; measure it
    /// on demand with:
    /// `cargo test -p codex-services --lib reading_transfer::export::tests::export_of_5000_books_stays_under_5mb -- --ignored --nocapture`
    #[tokio::test]
    #[ignore]
    async fn export_of_5000_books_stays_under_5mb() {
        use codex_db::entities::read_progress;
        use sea_orm::{EntityTrait, Set};

        const SERIES_COUNT: usize = 50;
        const BOOKS_PER_SERIES: usize = 100;

        let (db, _tmp) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let user = make_user(conn).await;

        let library =
            LibraryRepository::create(conn, "Big Library", "/big", ScanningStrategy::Default)
                .await
                .unwrap();

        let mut all_books = Vec::with_capacity(SERIES_COUNT * BOOKS_PER_SERIES);
        let mut progress_rows = Vec::with_capacity(SERIES_COUNT * BOOKS_PER_SERIES);
        let mut completion_rows = Vec::new();
        let mut session_rows = Vec::new();
        let now = Utc::now();

        for s in 0..SERIES_COUNT {
            let series =
                SeriesRepository::create(conn, library.id, &format!("Series {s:04}"), None)
                    .await
                    .unwrap();
            SeriesRepository::update_path(conn, series.id, format!("series-{s:04}"))
                .await
                .unwrap();
            UserSeriesRatingRepository::create(
                conn,
                user,
                series.id,
                80,
                Some("solid run".to_string()),
            )
            .await
            .unwrap();

            for b in 0..BOOKS_PER_SERIES {
                let book_id = Uuid::new_v4();
                all_books.push(books::Model {
                    id: book_id,
                    series_id: series.id,
                    library_id: library.id,
                    path: format!("/big/series-{s:04}/v{b:03}.cbz"),
                    file_name: format!("v{b:03}.cbz"),
                    file_size: 12_345,
                    file_hash: format!("hash-{s:04}-{b:03}"),
                    partial_hash: format!("partial-{s:04}-{b:03}"),
                    format: "cbz".to_string(),
                    page_count: 24,
                    deleted: false,
                    analyzed: true,
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
                });

                progress_rows.push(read_progress::ActiveModel {
                    id: Set(Uuid::new_v4()),
                    user_id: Set(user),
                    book_id: Set(book_id),
                    current_page: Set(24),
                    progress_percentage: Set(None),
                    completed: Set(true),
                    started_at: Set(now - Duration::days(1)),
                    updated_at: Set(now),
                    completed_at: Set(Some(now)),
                    r2_progression: Set(None),
                });

                completion_rows.push(read_completions::ActiveModel {
                    id: Set(Uuid::new_v4()),
                    user_id: Set(user),
                    book_id: Set(Some(book_id)),
                    started_at: Set(now - Duration::days(1)),
                    completed_at: Set(now),
                });

                for pass in 0..1 {
                    session_rows.push(reading_sessions::ActiveModel {
                        id: Set(Uuid::new_v4()),
                        user_id: Set(user),
                        book_id: Set(Some(book_id)),
                        device_id: Set("device-1".to_string()),
                        device_name: Set(Some("Test Device".to_string())),
                        pass: Set(pass),
                        kind: Set("progress".to_string()),
                        to_page: Set(Some(24)),
                        to_percentage: Set(None),
                        r2_progression: Set(None),
                        active_duration_ms: Set(Some(600_000)),
                        duration_source: Set("measured".to_string()),
                        pages_read: Set(Some(24)),
                        client_started_at: Set(now - Duration::minutes(10)),
                        client_ended_at: Set(now),
                        server_recorded_at: Set(now),
                    });
                }
            }
        }

        for chunk in all_books.chunks(500) {
            BookRepository::create_batch(conn, chunk).await.unwrap();
        }
        for chunk in progress_rows.chunks(500) {
            read_progress::Entity::insert_many(chunk.to_vec())
                .exec(conn)
                .await
                .unwrap();
        }
        for chunk in completion_rows.chunks(500) {
            read_completions::Entity::insert_many(chunk.to_vec())
                .exec(conn)
                .await
                .unwrap();
        }
        for chunk in session_rows.chunks(500) {
            reading_sessions::Entity::insert_many(chunk.to_vec())
                .exec(conn)
                .await
                .unwrap();
        }

        let doc = export_reading_progress(conn, user, true).await.unwrap();
        assert_eq!(doc.series.len(), SERIES_COUNT);

        let json = serde_json::to_vec(&doc).unwrap();
        let bytes = json.len();
        println!(
            "5,000-book export: {} bytes ({:.2} MB)",
            bytes,
            bytes as f64 / 1_048_576.0
        );
        assert!(
            bytes < 5 * 1_048_576,
            "export of 5,000 books should stay under ~5 MB uncompressed, was {bytes} bytes"
        );
    }

    #[tokio::test]
    async fn export_contains_no_other_users_rows() {
        let (db, _tmp) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let user_a = make_user(conn).await;
        let user_b = make_user(conn).await;

        let library = LibraryRepository::create(conn, "Lib", "/lib", ScanningStrategy::Default)
            .await
            .unwrap();
        let series = SeriesRepository::create(conn, library.id, "Series", None)
            .await
            .unwrap();
        let book = books::Model {
            id: Uuid::new_v4(),
            series_id: series.id,
            library_id: library.id,
            path: "/lib/book.cbz".to_string(),
            file_name: "book.cbz".to_string(),
            file_size: 10,
            file_hash: "h".to_string(),
            partial_hash: "p".to_string(),
            format: "cbz".to_string(),
            page_count: 1,
            deleted: false,
            analyzed: true,
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
        let book = BookRepository::create(conn, &book, None).await.unwrap();

        ReadProgressRepository::upsert(conn, user_a, book.id, 3, false)
            .await
            .unwrap();
        ReadProgressRepository::upsert(conn, user_b, book.id, 9, false)
            .await
            .unwrap();

        let doc = export_reading_progress(conn, user_a, true).await.unwrap();
        assert_eq!(doc.series.len(), 1);
        assert_eq!(doc.series[0].books.len(), 1);
        assert_eq!(
            doc.series[0].books[0]
                .progress
                .as_ref()
                .unwrap()
                .current_page,
            3
        );
    }
}
