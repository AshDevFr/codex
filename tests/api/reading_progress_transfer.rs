//! Integration tests for `GET /api/v1/reading-progress/export` and
//! `POST /api/v1/reading-progress/import`.
//!
//! These exercise the properties the feature exists for: a library split
//! (export, delete the old library, rescan under new roots, import) must land
//! state on the new book ids; importing the same file twice must not
//! duplicate anything; a dry run must write nothing; and a book the importing
//! user cannot see must resolve as unmatched rather than leak through a
//! permission error.

#[path = "../common/mod.rs"]
mod common;

use chrono::{Duration, Utc};
use codex::api::routes::v1::dto::{
    BookDisposition, ConflictPolicy, ExportBookDto, ExportCompletionDto, ExportExternalIdDto,
    ExportProgressDto, ExportSeriesDto, ExportSessionDto, ExportWantToReadDto, FieldOutcome,
    HashMode, ImportReadingProgressRequest, ImportReadingProgressResponse, READING_PROGRESS_FORMAT,
    READING_PROGRESS_VERSION, ReadingProgressExportDocument, SeriesDisposition,
};
use codex::db::ScanningStrategy;
use codex::db::entities::reading_sessions::SessionKind;
use codex::db::entities::user_sharing_tags::AccessMode;
use codex::db::entities::{
    books, read_completions, read_progress, reading_sessions, user_series_ratings,
};
use codex::db::repositories::{
    BookRepository, LibraryRepository, NewSession, ReadCompletionRepository,
    ReadProgressRepository, SeriesExternalIdRepository, SeriesRepository, SharingTagRepository,
    UserRepository, UserSeriesRatingRepository,
};
use codex::utils::password;
use common::*;
use hyper::StatusCode;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use uuid::Uuid;

async fn admin_and_token(
    db: &DatabaseConnection,
    state: &codex::api::extractors::AuthState,
    username: &str,
) -> (Uuid, String) {
    let password_hash = password::hash_password("pw123456").unwrap();
    let user = create_test_user(
        username,
        &format!("{username}@example.com"),
        &password_hash,
        true,
    );
    let created = UserRepository::create(db, &user).await.unwrap();
    let token = state
        .jwt_service
        .generate_token(created.id, created.username.clone(), created.get_role())
        .unwrap();
    (created.id, token)
}

fn book_model(
    series_id: Uuid,
    library_id: Uuid,
    path: &str,
    file_name: &str,
    hash: &str,
) -> books::Model {
    books::Model {
        id: Uuid::new_v4(),
        series_id,
        library_id,
        path: path.to_string(),
        file_name: file_name.to_string(),
        file_size: 100,
        file_hash: hash.to_string(),
        partial_hash: String::new(),
        format: "cbz".to_string(),
        page_count: 20,
        deleted: false,
        analyzed: !hash.is_empty(),
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
    }
}

async fn table_counts(db: &DatabaseConnection) -> (usize, usize, usize, usize) {
    (
        read_progress::Entity::find().all(db).await.unwrap().len(),
        read_completions::Entity::find()
            .all(db)
            .await
            .unwrap()
            .len(),
        reading_sessions::Entity::find()
            .all(db)
            .await
            .unwrap()
            .len(),
        user_series_ratings::Entity::find()
            .all(db)
            .await
            .unwrap()
            .len(),
    )
}

fn minimal_book_doc(path: &str, file_name: &str, hash: &str, current_page: i32) -> ExportBookDto {
    let now = Utc::now();
    ExportBookDto {
        path: path.to_string(),
        file_name: file_name.to_string(),
        file_hash: hash.to_string(),
        partial_hash: String::new(),
        progress: Some(ExportProgressDto {
            current_page,
            progress_percentage: None,
            completed: false,
            started_at: now - Duration::hours(1),
            updated_at: now,
            completed_at: None,
            r2_progression: None,
        }),
        completions: vec![],
        sessions: None,
        want_to_read: None,
    }
}

fn document(
    series: Vec<ExportSeriesDto>,
    includes_sessions: bool,
) -> ReadingProgressExportDocument {
    ReadingProgressExportDocument {
        format: READING_PROGRESS_FORMAT.to_string(),
        version: READING_PROGRESS_VERSION,
        exported_at: Utc::now(),
        includes_sessions,
        series,
    }
}

fn import_request(
    file: ReadingProgressExportDocument,
    dry_run: bool,
) -> ImportReadingProgressRequest {
    ImportReadingProgressRequest {
        dry_run,
        hash_mode: HashMode::Verify,
        source_preference: None,
        conflict_policy: ConflictPolicy::Overwrite,
        reattach_sessions: true,
        accept_stem_matches: false,
        library_ids: None,
        restore_want_to_read: true,
        file,
    }
}

#[tokio::test]
async fn export_returns_a_downloadable_document_with_a_content_disposition_header() {
    let (db, _tmp) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (user_id, token) = admin_and_token(&db, &state, "exporter").await;

    let library = LibraryRepository::create(&db, "Lib", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();
    let series = SeriesRepository::create(&db, library.id, "Naruto", None)
        .await
        .unwrap();
    let book = BookRepository::create(
        &db,
        &book_model(
            series.id,
            library.id,
            "/lib/Naruto/v01.cbz",
            "v01.cbz",
            "h1",
        ),
        None,
    )
    .await
    .unwrap();
    ReadProgressRepository::upsert(&db, user_id, book.id, 5, false)
        .await
        .unwrap();

    let app = create_test_router(state.clone()).await;
    let request = get_request_with_auth("/api/v1/reading-progress/export", &token);
    let (status, headers, body) = make_full_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let disposition = headers
        .get("content-disposition")
        .expect("content-disposition header");
    assert!(
        disposition
            .to_str()
            .unwrap()
            .contains("codex-reading-progress")
    );

    let doc: ReadingProgressExportDocument = serde_json::from_slice(&body).unwrap();
    assert_eq!(doc.format, READING_PROGRESS_FORMAT);
    assert_eq!(doc.series.len(), 1);
    assert_eq!(doc.series[0].library_relative_path, "Naruto");
    assert_eq!(doc.series[0].books[0].path, "v01.cbz");
}

/// The scenario the feature exists for: two series exported from one library,
/// the library deleted, the same files rescanned under two new library roots,
/// and the import landing progress, completions, sessions, and the series
/// rating on the new ids.
#[tokio::test]
async fn library_split_round_trip_moves_state_to_the_new_books() {
    let (db, _tmp) = setup_test_db().await;
    exercise_library_split_round_trip(&db).await;
}

async fn exercise_library_split_round_trip(db: &DatabaseConnection) {
    let state = create_test_auth_state(db.clone()).await;
    let (user_id, token) = admin_and_token(db, &state, "splitter").await;

    // --- Old layout: one library, two series ---
    let old_library = LibraryRepository::create(db, "Old", "/old", ScanningStrategy::Default)
        .await
        .unwrap();
    let old_series_a = SeriesRepository::create(db, old_library.id, "Naruto", None)
        .await
        .unwrap();
    let old_series_b = SeriesRepository::create(db, old_library.id, "Bleach", None)
        .await
        .unwrap();
    let old_book_a = BookRepository::create(
        db,
        &book_model(
            old_series_a.id,
            old_library.id,
            "/old/Naruto/v01.cbz",
            "v01.cbz",
            "hash-naruto",
        ),
        None,
    )
    .await
    .unwrap();
    let old_book_b = BookRepository::create(
        db,
        &book_model(
            old_series_b.id,
            old_library.id,
            "/old/Bleach/v01.cbz",
            "v01.cbz",
            "hash-bleach",
        ),
        None,
    )
    .await
    .unwrap();

    // Progress, a completion, a session, and a rating on series A.
    ReadProgressRepository::upsert(db, user_id, old_book_a.id, 12, false)
        .await
        .unwrap();
    let completion = ReadCompletionRepository::record(
        db,
        user_id,
        old_book_a.id,
        Utc::now() - Duration::days(1),
        Utc::now(),
    )
    .await
    .unwrap();
    let session = NewSession::from_client(
        Uuid::new_v4(),
        user_id,
        old_book_a.id,
        "device-1",
        Some("Test Device".to_string()),
        SessionKind::Progress,
        Some(600_000),
        Some(12),
        Utc::now() - Duration::minutes(10),
        Utc::now(),
    )
    .with_page(12);
    ReadProgressRepository::record_session(db, session)
        .await
        .unwrap();
    UserSeriesRatingRepository::create(db, user_id, old_series_a.id, 90, Some("great".to_string()))
        .await
        .unwrap();

    // Progress on series B too, to prove both series move.
    ReadProgressRepository::upsert(db, user_id, old_book_b.id, 3, false)
        .await
        .unwrap();

    // --- Export before the split ---
    let app = create_test_router(state.clone()).await;
    let request = get_request_with_auth(
        "/api/v1/reading-progress/export?includeSessions=true",
        &token,
    );
    let (status, exported): (StatusCode, Option<ReadingProgressExportDocument>) =
        make_json_request(app, request).await;
    assert_eq!(status, StatusCode::OK);
    let exported = exported.expect("export body");
    assert_eq!(exported.series.len(), 2);

    // --- The split: delete the old library, rescan under two new roots ---
    LibraryRepository::delete(db, old_library.id).await.unwrap();

    let new_library_a = LibraryRepository::create(db, "New A", "/new-a", ScanningStrategy::Default)
        .await
        .unwrap();
    let new_series_a = SeriesRepository::create(db, new_library_a.id, "Naruto", None)
        .await
        .unwrap();
    let new_book_a = BookRepository::create(
        db,
        &book_model(
            new_series_a.id,
            new_library_a.id,
            "/new-a/Naruto/v01.cbz",
            "v01.cbz",
            "hash-naruto",
        ),
        None,
    )
    .await
    .unwrap();

    let new_library_b = LibraryRepository::create(db, "New B", "/new-b", ScanningStrategy::Default)
        .await
        .unwrap();
    let new_series_b = SeriesRepository::create(db, new_library_b.id, "Bleach", None)
        .await
        .unwrap();
    let new_book_b = BookRepository::create(
        db,
        &book_model(
            new_series_b.id,
            new_library_b.id,
            "/new-b/Bleach/v01.cbz",
            "v01.cbz",
            "hash-bleach",
        ),
        None,
    )
    .await
    .unwrap();

    // --- Import into the new layout ---
    let app = create_test_router(state.clone()).await;
    let request = post_json_request_with_auth(
        "/api/v1/reading-progress/import",
        &import_request(exported, false),
        &token,
    );
    let (status, response): (StatusCode, Option<ImportReadingProgressResponse>) =
        make_json_request(app, request).await;
    assert_eq!(status, StatusCode::OK);
    let response = response.expect("import response");
    assert!(!response.dry_run);
    assert_eq!(response.summary.series_matched, 2);
    assert_eq!(response.summary.series_committed, 2);
    assert_eq!(response.summary.books_matched, 2);

    // Progress landed on the new books.
    let progress_a = ReadProgressRepository::get_by_user_and_book(db, user_id, new_book_a.id)
        .await
        .unwrap()
        .expect("progress on new book A");
    assert_eq!(progress_a.current_page, 12);
    let progress_b = ReadProgressRepository::get_by_user_and_book(db, user_id, new_book_b.id)
        .await
        .unwrap()
        .expect("progress on new book B");
    assert_eq!(progress_b.current_page, 3);

    // The completion and session, both originally recorded on the deleted
    // book, were reattached (same id) rather than re-inserted.
    let completion_row = read_completions::Entity::find_by_id(completion.id)
        .one(db)
        .await
        .unwrap()
        .expect("completion row still exists");
    assert_eq!(completion_row.book_id, Some(new_book_a.id));

    // Two sessions land here: the explicit one recorded above, plus the
    // legacy-write session `ReadProgressRepository::upsert` records under the
    // hood for its own progress update. Both must be reattached exactly once
    // each, never duplicated.
    let sessions_for_a = reading_sessions::Entity::find()
        .filter(reading_sessions::Column::BookId.eq(new_book_a.id))
        .all(db)
        .await
        .unwrap();
    assert_eq!(
        sessions_for_a.len(),
        2,
        "every orphaned session must be reattached exactly once"
    );

    // The rating followed series A to its new id.
    let rating = UserSeriesRatingRepository::get_by_user_and_series(db, user_id, new_series_a.id)
        .await
        .unwrap()
        .expect("rating on new series A");
    assert_eq!(rating.rating, 90);
}

#[tokio::test]
async fn importing_the_same_document_twice_does_not_change_row_counts() {
    let (db, _tmp) = setup_test_db().await;
    exercise_idempotent_import(&db).await;
}

async fn exercise_idempotent_import(db: &DatabaseConnection) {
    let state = create_test_auth_state(db.clone()).await;
    let (_user_id, token) = admin_and_token(db, &state, "idempotent").await;

    let library = LibraryRepository::create(db, "Lib", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();
    let series = SeriesRepository::create(db, library.id, "Series", None)
        .await
        .unwrap();
    let _book = BookRepository::create(
        db,
        &book_model(
            series.id,
            library.id,
            "/lib/Series/v01.cbz",
            "v01.cbz",
            "h1",
        ),
        None,
    )
    .await
    .unwrap();

    let mut book_doc = minimal_book_doc("v01.cbz", "v01.cbz", "h1", 7);
    book_doc.completions.push(ExportCompletionDto {
        id: Uuid::new_v4(),
        started_at: Utc::now() - Duration::days(1),
        completed_at: Utc::now(),
    });
    book_doc.sessions = Some(vec![ExportSessionDto {
        id: Uuid::new_v4(),
        device_id: "device-1".to_string(),
        device_name: None,
        pass: 1,
        kind: "progress".to_string(),
        to_page: Some(7),
        to_percentage: None,
        active_duration_ms: Some(60_000),
        duration_source: "measured".to_string(),
        pages_read: Some(7),
        client_started_at: Utc::now() - Duration::minutes(10),
        client_ended_at: Utc::now(),
        server_recorded_at: Utc::now(),
    }]);

    let series_doc = ExportSeriesDto {
        external_ids: vec![],
        library_relative_path: "Series".to_string(),
        name: "Series".to_string(),
        rating: Some(77),
        notes: None,
        rating_updated_at: None,
        books: vec![book_doc],
        want_to_read: None,
    };
    let doc = document(vec![series_doc], true);

    // Relative to what is already there: the PostgreSQL run shares one
    // database across several scenarios.
    let before = table_counts(db).await;

    let app = create_test_router(state.clone()).await;
    let request = post_json_request_with_auth(
        "/api/v1/reading-progress/import",
        &import_request(doc.clone(), false),
        &token,
    );
    let (status, _): (StatusCode, Option<ImportReadingProgressResponse>) =
        make_json_request(app, request).await;
    assert_eq!(status, StatusCode::OK);

    let counts_after_first = table_counts(db).await;
    assert_eq!(
        counts_after_first,
        (before.0 + 1, before.1 + 1, before.2 + 1, before.3 + 1),
        "the first import writes one row to each table"
    );

    let app = create_test_router(state.clone()).await;
    let request = post_json_request_with_auth(
        "/api/v1/reading-progress/import",
        &import_request(doc, false),
        &token,
    );
    let (status, _): (StatusCode, Option<ImportReadingProgressResponse>) =
        make_json_request(app, request).await;
    assert_eq!(status, StatusCode::OK);

    let counts_after_second = table_counts(db).await;
    assert_eq!(
        counts_after_first, counts_after_second,
        "importing the same document twice must not change row counts"
    );
}

#[tokio::test]
async fn dry_run_reports_matches_but_writes_nothing() {
    let (db, _tmp) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_user_id, token) = admin_and_token(&db, &state, "dryrunner").await;

    let library = LibraryRepository::create(&db, "Lib", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();
    let series = SeriesRepository::create(&db, library.id, "Series", None)
        .await
        .unwrap();
    BookRepository::create(
        &db,
        &book_model(
            series.id,
            library.id,
            "/lib/Series/v01.cbz",
            "v01.cbz",
            "h1",
        ),
        None,
    )
    .await
    .unwrap();

    let series_doc = ExportSeriesDto {
        external_ids: vec![],
        library_relative_path: "Series".to_string(),
        name: "Series".to_string(),
        rating: Some(50),
        notes: None,
        rating_updated_at: None,
        books: vec![minimal_book_doc("v01.cbz", "v01.cbz", "h1", 9)],
        want_to_read: None,
    };
    let doc = document(vec![series_doc], false);

    let before = table_counts(&db).await;

    let app = create_test_router(state.clone()).await;
    let request = post_json_request_with_auth(
        "/api/v1/reading-progress/import",
        &import_request(doc, true),
        &token,
    );
    let (status, response): (StatusCode, Option<ImportReadingProgressResponse>) =
        make_json_request(app, request).await;
    assert_eq!(status, StatusCode::OK);
    let response = response.expect("dry run response");
    assert!(response.dry_run);
    assert_eq!(response.summary.series_matched, 1);
    assert_eq!(response.summary.books_matched, 1);
    assert_eq!(
        response.summary.series_committed, 0,
        "a dry run commits nothing"
    );

    let after = table_counts(&db).await;
    assert_eq!(before, after, "a dry run must leave the database unchanged");
}

/// The one security-relevant requirement: a book behind a sharing-tag deny
/// resolves as unmatched, not as a permission error, and nothing is written.
#[tokio::test]
async fn a_book_the_importing_user_cannot_see_resolves_as_unmatched_and_nothing_is_written() {
    let (db, _tmp) = setup_test_db().await;
    exercise_visibility_denies_unmatched(&db).await;
    exercise_two_entries_one_book(&db).await;
    exercise_same_instance_split(&db).await;
}

async fn exercise_visibility_denies_unmatched(db: &DatabaseConnection) {
    // Relative, because the PostgreSQL run shares one database across scenarios.
    let before = table_counts(db).await;
    let state = create_test_auth_state(db.clone()).await;
    let (user_id, token) = admin_and_token(db, &state, "restricted-reader").await;

    let library = LibraryRepository::create(db, "Lib", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();
    let series = SeriesRepository::create(db, library.id, "Hidden", None)
        .await
        .unwrap();
    BookRepository::create(
        db,
        &book_model(
            series.id,
            library.id,
            "/lib/Hidden/v01.cbz",
            "v01.cbz",
            "h1",
        ),
        None,
    )
    .await
    .unwrap();

    let tag = SharingTagRepository::create(db, &format!("restricted-{}", Uuid::new_v4()), None)
        .await
        .unwrap();
    SharingTagRepository::add_tag_to_series(db, series.id, tag.id)
        .await
        .unwrap();
    SharingTagRepository::set_user_grant(db, user_id, tag.id, AccessMode::Deny)
        .await
        .unwrap();

    let series_doc = ExportSeriesDto {
        external_ids: vec![],
        library_relative_path: "Hidden".to_string(),
        name: "Hidden".to_string(),
        rating: Some(100),
        notes: None,
        rating_updated_at: None,
        books: vec![minimal_book_doc("v01.cbz", "v01.cbz", "h1", 1)],
        want_to_read: None,
    };
    let doc = document(vec![series_doc], false);

    let app = create_test_router(state.clone()).await;
    let request = post_json_request_with_auth(
        "/api/v1/reading-progress/import",
        &import_request(doc, false),
        &token,
    );
    let (status, response): (StatusCode, Option<ImportReadingProgressResponse>) =
        make_json_request(app, request).await;

    // Never a permission error: the response is a normal 200 that simply
    // could not resolve anything, which does not confirm the series exists.
    assert_eq!(status, StatusCode::OK);
    let response = response.expect("import response");
    assert_eq!(response.series.len(), 1);
    assert_eq!(response.series[0].disposition, SeriesDisposition::Unmatched);
    assert_eq!(
        response.series[0].books[0].disposition,
        BookDisposition::Unmatched
    );
    assert!(!response.series[0].books[0].applied);

    let counts = table_counts(db).await;
    assert_eq!(
        counts, before,
        "nothing may be written against an invisible book"
    );
}

#[tokio::test]
async fn unknown_format_is_rejected_with_400() {
    let (db, _tmp) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_user_id, token) = admin_and_token(&db, &state, "u1").await;

    let mut doc = document(vec![], false);
    doc.format = "something-else".to_string();

    let app = create_test_router(state.clone()).await;
    let request = post_json_request_with_auth(
        "/api/v1/reading-progress/import",
        &import_request(doc, true),
        &token,
    );
    let (status, body) = make_request(app, request).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let text = String::from_utf8_lossy(&body);
    assert!(
        text.to_lowercase().contains("format"),
        "error should name the problem: {text}"
    );
}

#[tokio::test]
async fn a_version_newer_than_supported_is_rejected_with_400() {
    let (db, _tmp) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_user_id, token) = admin_and_token(&db, &state, "u2").await;

    let mut doc = document(vec![], false);
    doc.version = READING_PROGRESS_VERSION + 1;

    let app = create_test_router(state.clone()).await;
    let request = post_json_request_with_auth(
        "/api/v1/reading-progress/import",
        &import_request(doc, true),
        &token,
    );
    let (status, body) = make_request(app, request).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let text = String::from_utf8_lossy(&body);
    assert!(
        text.to_lowercase().contains("version"),
        "error should name the problem: {text}"
    );
}

/// The narrow reattach path called out explicitly: record history, hard-delete
/// the book with `books::Entity::delete_by_id`, then import and assert the
/// orphaned session and completion are reattached rather than re-inserted.
#[tokio::test]
async fn hard_deleting_a_book_then_reimporting_reattaches_its_orphaned_history() {
    let (db, _tmp) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (user_id, token) = admin_and_token(&db, &state, "reattacher").await;

    let library = LibraryRepository::create(&db, "Lib", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();
    let series = SeriesRepository::create(&db, library.id, "Series", None)
        .await
        .unwrap();
    let book = BookRepository::create(
        &db,
        &book_model(
            series.id,
            library.id,
            "/lib/Series/v01.cbz",
            "v01.cbz",
            "h1",
        ),
        None,
    )
    .await
    .unwrap();

    let completion = ReadCompletionRepository::record(
        &db,
        user_id,
        book.id,
        Utc::now() - Duration::days(1),
        Utc::now(),
    )
    .await
    .unwrap();
    let session = NewSession::from_client(
        Uuid::new_v4(),
        user_id,
        book.id,
        "device-1",
        None,
        SessionKind::Progress,
        Some(60_000),
        Some(5),
        Utc::now() - Duration::minutes(5),
        Utc::now(),
    )
    .with_page(5);
    ReadProgressRepository::record_session(&db, session)
        .await
        .unwrap();

    // Export while the book still exists, so the file has something to
    // match back against.
    let app = create_test_router(state.clone()).await;
    let request = get_request_with_auth(
        "/api/v1/reading-progress/export?includeSessions=true",
        &token,
    );
    let (status, exported): (StatusCode, Option<ReadingProgressExportDocument>) =
        make_json_request(app, request).await;
    assert_eq!(status, StatusCode::OK);
    let exported = exported.unwrap();

    // Hard-delete the book: the session and completion survive as orphans.
    books::Entity::delete_by_id(book.id)
        .exec(&db)
        .await
        .unwrap();

    // A rescan recreates the book at the same path with a new id.
    let rescanned = BookRepository::create(
        &db,
        &book_model(
            series.id,
            library.id,
            "/lib/Series/v01.cbz",
            "v01.cbz",
            "h1",
        ),
        None,
    )
    .await
    .unwrap();
    assert_ne!(rescanned.id, book.id);

    let app = create_test_router(state.clone()).await;
    let request = post_json_request_with_auth(
        "/api/v1/reading-progress/import",
        &import_request(exported, false),
        &token,
    );
    let (status, response): (StatusCode, Option<ImportReadingProgressResponse>) =
        make_json_request(app, request).await;
    assert_eq!(status, StatusCode::OK);
    let response = response.unwrap();
    assert_eq!(response.summary.completions_reattached, 1);
    assert_eq!(response.summary.sessions_reattached, 1);
    assert_eq!(response.summary.completions_inserted, 0);
    assert_eq!(response.summary.sessions_inserted, 0);

    let completion_row = read_completions::Entity::find_by_id(completion.id)
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(completion_row.book_id, Some(rescanned.id));

    let session_count = reading_sessions::Entity::find()
        .all(&db)
        .await
        .unwrap()
        .len();
    assert_eq!(
        session_count, 1,
        "the session must be reattached, not duplicated"
    );
}

// ============================================================================
// Regressions: values the normal write paths reject, colliding entries, the
// same-instance split, and a history larger than axum's default body limit.
// ============================================================================

fn series_doc(path: &str, name: &str, books: Vec<ExportBookDto>) -> ExportSeriesDto {
    ExportSeriesDto {
        external_ids: vec![],
        library_relative_path: path.to_string(),
        name: name.to_string(),
        rating: None,
        notes: None,
        rating_updated_at: None,
        books,
        want_to_read: None,
    }
}

/// Ratings feed an average every user sees, so a file must not get around
/// the range the rating endpoint enforces.
#[tokio::test]
async fn a_rating_outside_1_to_100_is_rejected_with_400() {
    let (db, _tmp) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_user_id, token) = admin_and_token(&db, &state, "rater").await;

    for rating in [0, 101, 2_000_000_000] {
        let mut series = series_doc("S", "S", vec![]);
        series.rating = Some(rating);
        let app = create_test_router(state.clone()).await;
        let request = post_json_request_with_auth(
            "/api/v1/reading-progress/import",
            &import_request(document(vec![series], false), true),
            &token,
        );
        let (status, _body) = make_request(app, request).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "rating {rating}");
    }
}

/// A moved file leaves its old row soft-deleted, the export carries both, and
/// both resolve to the one current book. The second entry is decided against
/// the first under the conflict policy instead of colliding with it, so the
/// series commits and the dry run predicted exactly that.
#[tokio::test]
async fn two_entries_landing_on_one_book_are_resolved_by_the_policy() {
    let (db, _tmp) = setup_test_db().await;
    exercise_two_entries_one_book(&db).await;
}

async fn exercise_two_entries_one_book(db: &DatabaseConnection) {
    let state = create_test_auth_state(db.clone()).await;
    let (user_id, token) = admin_and_token(db, &state, "mover").await;

    let library = LibraryRepository::create(db, "Lib", "/twice", ScanningStrategy::Default)
        .await
        .unwrap();
    let series = SeriesRepository::create(db, library.id, "Twice Told", None)
        .await
        .unwrap();
    let book = BookRepository::create(
        db,
        &book_model(
            series.id,
            library.id,
            "/twice/Twice Told/Vol 01/v01.cbz",
            "v01.cbz",
            "",
        ),
        None,
    )
    .await
    .unwrap();

    let mut older = minimal_book_doc("v01.cbz", "v01.cbz", "", 5);
    older.progress.as_mut().unwrap().updated_at = Utc::now() - Duration::days(2);
    let newer = minimal_book_doc("Vol 01/v01.cbz", "v01.cbz", "", 9);
    let doc = document(
        vec![series_doc("Twice Told", "Twice Told", vec![newer, older])],
        false,
    );

    let mut request = import_request(doc, true);
    request.conflict_policy = ConflictPolicy::Newest;
    let app = create_test_router(state.clone()).await;
    let (status, preview): (StatusCode, Option<ImportReadingProgressResponse>) = make_json_request(
        app,
        post_json_request_with_auth("/api/v1/reading-progress/import", &request, &token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let preview = preview.expect("dry-run body");
    let outcomes: Vec<_> = preview.series[0].books.iter().map(|b| b.progress).collect();
    assert_eq!(
        outcomes,
        vec![Some(FieldOutcome::Inserted), Some(FieldOutcome::Skipped)],
        "the older entry loses to the one planned before it"
    );

    request.dry_run = false;
    let app = create_test_router(state.clone()).await;
    let (status, applied): (StatusCode, Option<ImportReadingProgressResponse>) = make_json_request(
        app,
        post_json_request_with_auth("/api/v1/reading-progress/import", &request, &token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let applied = applied.expect("import body");
    assert!(applied.series[0].committed, "{:?}", applied.series[0].error);

    let progress = ReadProgressRepository::get_by_user_and_book(db, user_id, book.id)
        .await
        .unwrap()
        .expect("progress landed");
    assert_eq!(progress.current_page, 9, "the newer position wins");
}

/// Splitting on the same instance: the files move, a rescan soft-deletes the
/// old books, and the reader imports before deleting the old library. The
/// leftover series must not capture the match, and history still sitting on
/// the soft-deleted books moves to the new ones rather than being skipped.
#[tokio::test]
async fn importing_before_the_old_library_is_deleted_moves_history_to_the_new_books() {
    let (db, _tmp) = setup_test_db().await;
    exercise_same_instance_split(&db).await;
}

async fn exercise_same_instance_split(db: &DatabaseConnection) {
    use sea_orm::{ActiveModelTrait, Set};

    let state = create_test_auth_state(db.clone()).await;
    let (user_id, token) = admin_and_token(db, &state, "same-instance").await;

    let old_library = LibraryRepository::create(db, "Manga", "/manga", ScanningStrategy::Default)
        .await
        .unwrap();
    let old_series = SeriesRepository::create(db, old_library.id, "Split Series", None)
        .await
        .unwrap();
    SeriesRepository::update_path(db, old_series.id, "shonen/Split Series".to_string())
        .await
        .unwrap();
    let old_book = BookRepository::create(
        db,
        &book_model(
            old_series.id,
            old_library.id,
            "/manga/shonen/Split Series/v01.cbz",
            "v01.cbz",
            "hash-split",
        ),
        None,
    )
    .await
    .unwrap();

    let completion =
        ReadCompletionRepository::record(db, user_id, old_book.id, Utc::now(), Utc::now())
            .await
            .unwrap();
    let session_id = Uuid::new_v4();
    ReadProgressRepository::record_session(
        db,
        NewSession::from_client(
            session_id,
            user_id,
            old_book.id,
            "device-1",
            None,
            SessionKind::Progress,
            Some(60_000),
            Some(4),
            Utc::now() - Duration::minutes(5),
            Utc::now(),
        )
        .with_page(4),
    )
    .await
    .unwrap();

    // The files move to a new library; the old library's rescan marks the
    // book deleted but nobody has deleted the old library yet.
    let app = create_test_router(state.clone()).await;
    let (_, exported): (StatusCode, Option<ReadingProgressExportDocument>) = make_json_request(
        app,
        get_request_with_auth("/api/v1/reading-progress/export", &token),
    )
    .await;
    let exported = exported.expect("export body");

    let mut gone: books::ActiveModel = old_book.clone().into();
    gone.deleted = Set(true);
    gone.update(db).await.unwrap();

    let new_library = LibraryRepository::create(db, "Shonen", "/shonen", ScanningStrategy::Default)
        .await
        .unwrap();
    let new_series = SeriesRepository::create(db, new_library.id, "Split Series", None)
        .await
        .unwrap();
    let new_book = BookRepository::create(
        db,
        &book_model(
            new_series.id,
            new_library.id,
            "/shonen/Split Series/v01.cbz",
            "v01.cbz",
            "hash-split",
        ),
        None,
    )
    .await
    .unwrap();

    let app = create_test_router(state.clone()).await;
    let (status, response): (StatusCode, Option<ImportReadingProgressResponse>) =
        make_json_request(
            app,
            post_json_request_with_auth(
                "/api/v1/reading-progress/import",
                &import_request(exported, false),
                &token,
            ),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let response = response.expect("import body");
    assert_eq!(
        response.series[0].matched_series_id,
        Some(new_series.id),
        "the leftover series with nothing on disk must not capture the match"
    );
    assert!(response.series[0].committed);

    let completion = read_completions::Entity::find_by_id(completion.id)
        .one(db)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(completion.book_id, Some(new_book.id));
    let session = reading_sessions::Entity::find_by_id(session_id)
        .one(db)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(session.book_id, Some(new_book.id));
}

/// A large history has to fit: axum's 2 MB default would stop around 2,500
/// books with their sessions. A dry run keeps this fast; the limit is the
/// point.
#[tokio::test]
async fn an_import_larger_than_two_megabytes_is_accepted() {
    let (db, _tmp) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_user_id, token) = admin_and_token(&db, &state, "heavy").await;

    let now = Utc::now();
    let sessions: Vec<ExportSessionDto> = (0..12_000)
        .map(|i| ExportSessionDto {
            id: Uuid::new_v4(),
            device_id: "device-with-a-reasonably-long-identifier".to_string(),
            device_name: Some("A reader with a descriptive name".to_string()),
            pass: 1,
            kind: "progress".to_string(),
            to_page: Some(i),
            to_percentage: None,
            active_duration_ms: Some(60_000),
            duration_source: "measured".to_string(),
            pages_read: Some(1),
            client_started_at: now - Duration::minutes(1),
            client_ended_at: now,
            server_recorded_at: now,
        })
        .collect();
    let mut book = minimal_book_doc("v01.cbz", "v01.cbz", "", 1);
    book.sessions = Some(sessions);
    let request = import_request(
        document(vec![series_doc("Heavy", "Heavy", vec![book])], true),
        true,
    );
    let body = serde_json::to_vec(&request).unwrap();
    assert!(
        body.len() > 2 * 1024 * 1024,
        "fixture must exceed the default"
    );

    let app = create_test_router(state).await;
    let (status, _body) = make_request(
        app,
        post_json_request_with_auth("/api/v1/reading-progress/import", &request, &token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
}

/// The scenarios that matter most, against PostgreSQL, sequenced in one test
/// on purpose: `setup_test_db_postgres` truncates a database shared by the
/// whole run, so two PostgreSQL tests running at once would delete each
/// other's fixtures.
#[tokio::test]
#[ignore] // Requires PostgreSQL test database
async fn reading_progress_transfer_postgres() {
    let Some(db) = setup_test_db_postgres().await else {
        eprintln!("PostgreSQL test database not available, skipping");
        return;
    };

    exercise_library_split_round_trip(&db).await;
    exercise_idempotent_import(&db).await;
    exercise_visibility_denies_unmatched(&db).await;
}

// ---------------------------------------------------------------------------
// Series matching by external id.
//
// This is the step the format exists for: an id survives a rename and a move,
// which neither the relative path nor the normalized name does. Everything
// below builds a target whose path *and* name differ from the export, so the
// two weaker steps cannot succeed and only the id can explain a match.
// ---------------------------------------------------------------------------

/// A series in the library, with an external id attached, whose path and name
/// deliberately differ from whatever the export will carry.
async fn series_with_external_id(
    db: &DatabaseConnection,
    library_id: Uuid,
    name: &str,
    source: &str,
    external_id: &str,
) -> Uuid {
    let series = SeriesRepository::create(db, library_id, name, None)
        .await
        .unwrap();
    SeriesExternalIdRepository::create(db, series.id, source, external_id, None, None)
        .await
        .unwrap();
    BookRepository::create(
        db,
        &book_model(
            series.id,
            library_id,
            &format!("/lib/{name}/v01.cbz"),
            "v01.cbz",
            "",
        ),
        None,
    )
    .await
    .unwrap();
    series.id
}

/// An exported series carrying ids but a path and name that match nothing.
fn series_doc_with_ids(ids: Vec<(&str, &str)>) -> ExportSeriesDto {
    ExportSeriesDto {
        external_ids: ids
            .into_iter()
            .map(|(source, id)| ExportExternalIdDto {
                source: source.to_string(),
                id: id.to_string(),
            })
            .collect(),
        library_relative_path: "a/path/that/matches/nothing".to_string(),
        name: "A Name That Matches Nothing".to_string(),
        rating: Some(64),
        notes: None,
        rating_updated_at: Some(Utc::now()),
        books: vec![],
        want_to_read: None,
    }
}

async fn import_with(
    state: &std::sync::Arc<codex::api::extractors::AuthState>,
    token: &str,
    doc: ReadingProgressExportDocument,
    sources: Option<Vec<String>>,
) -> ImportReadingProgressResponse {
    let app = create_test_router(state.clone()).await;
    let mut body = import_request(doc, true);
    body.source_preference = sources;
    let request = post_json_request_with_auth("/api/v1/reading-progress/import", &body, token);
    let (status, response): (StatusCode, Option<ImportReadingProgressResponse>) =
        make_json_request(app, request).await;
    assert_eq!(status, StatusCode::OK);
    response.expect("import response")
}

/// The defect this change fixes: a caller that does not name any source still
/// gets id matching. An empty default silently downgraded every import to the
/// two weakest steps, and no caller in the tree ever passed anything else.
#[tokio::test]
async fn omitting_the_source_preference_still_matches_on_external_id() {
    let (db, _tmp) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_user, token) = admin_and_token(&db, &state, "id-default").await;
    let library = LibraryRepository::create(&db, "Lib", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    let target = series_with_external_id(
        &db,
        library.id,
        "Renamed Series",
        "plugin:mangabaka",
        "12345",
    )
    .await;

    let doc = document(
        vec![series_doc_with_ids(vec![("plugin:mangabaka", "12345")])],
        false,
    );
    let report = import_with(&state, &token, doc, None).await;

    assert_eq!(report.summary.series_matched, 1);
    assert_eq!(report.series[0].disposition, SeriesDisposition::Matched);
    assert_eq!(report.series[0].matched_series_id, Some(target));
}

/// An explicit empty list still means "skip ids", so a caller can deliberately
/// fall through to path and name.
#[tokio::test]
async fn an_explicit_empty_source_preference_skips_external_ids() {
    let (db, _tmp) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_user, token) = admin_and_token(&db, &state, "id-optout").await;
    let library = LibraryRepository::create(&db, "Lib", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    series_with_external_id(
        &db,
        library.id,
        "Renamed Series",
        "plugin:mangabaka",
        "12345",
    )
    .await;

    let doc = document(
        vec![series_doc_with_ids(vec![("plugin:mangabaka", "12345")])],
        false,
    );
    let report = import_with(&state, &token, doc, Some(vec![])).await;

    assert_eq!(report.summary.series_unmatched, 1);
}

/// Priority order is the point of a *list*: when the file carries two ids that
/// resolve to different series, the order decides which one wins.
#[tokio::test]
async fn the_preference_order_decides_between_two_matching_sources() {
    let (db, _tmp) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_user, token) = admin_and_token(&db, &state, "id-order").await;
    let library = LibraryRepository::create(&db, "Lib", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    let baka =
        series_with_external_id(&db, library.id, "Via Mangabaka", "plugin:mangabaka", "111").await;
    let anilist =
        series_with_external_id(&db, library.id, "Via Anilist", "plugin:anilist", "222").await;

    let ids = vec![("plugin:mangabaka", "111"), ("plugin:anilist", "222")];

    let baka_first = import_with(
        &state,
        &token,
        document(vec![series_doc_with_ids(ids.clone())], false),
        Some(vec!["plugin:mangabaka".into(), "plugin:anilist".into()]),
    )
    .await;
    assert_eq!(baka_first.series[0].matched_series_id, Some(baka));

    let anilist_first = import_with(
        &state,
        &token,
        document(vec![series_doc_with_ids(ids)], false),
        Some(vec!["plugin:anilist".into(), "plugin:mangabaka".into()]),
    )
    .await;
    assert_eq!(anilist_first.series[0].matched_series_id, Some(anilist));
}

/// A preferred source the file does not carry is skipped rather than ending
/// the search, so naming a source order costs nothing when a file is sparse.
#[tokio::test]
async fn a_source_absent_from_the_file_falls_through_to_the_next() {
    let (db, _tmp) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_user, token) = admin_and_token(&db, &state, "id-sparse").await;
    let library = LibraryRepository::create(&db, "Lib", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    let target =
        series_with_external_id(&db, library.id, "Only Anilist", "plugin:anilist", "222").await;

    let doc = document(
        vec![series_doc_with_ids(vec![("plugin:anilist", "222")])],
        false,
    );
    let report = import_with(
        &state,
        &token,
        doc,
        Some(vec!["plugin:mangabaka".into(), "plugin:anilist".into()]),
    )
    .await;

    assert_eq!(report.series[0].matched_series_id, Some(target));
}

/// Two series sharing one id is a real state (a bad plugin match, or a split
/// that duplicated a series). Guessing between them would write a reader's
/// history onto the wrong book, so it reports and writes nothing.
#[tokio::test]
async fn two_series_sharing_an_external_id_are_ambiguous() {
    let (db, _tmp) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_user, token) = admin_and_token(&db, &state, "id-ambiguous").await;
    let library = LibraryRepository::create(&db, "Lib", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    series_with_external_id(&db, library.id, "First Copy", "plugin:mangabaka", "12345").await;
    series_with_external_id(&db, library.id, "Second Copy", "plugin:mangabaka", "12345").await;

    let doc = document(
        vec![series_doc_with_ids(vec![("plugin:mangabaka", "12345")])],
        false,
    );
    let report = import_with(&state, &token, doc, None).await;

    assert_eq!(report.series[0].disposition, SeriesDisposition::Ambiguous);
    assert_eq!(report.summary.series_ambiguous, 1);
}

// ---------------------------------------------------------------------------
// Library scoping.
// ---------------------------------------------------------------------------

/// Scoping the import to the target library is what lets it run while the old
/// copy of a series is still on disk. Unscoped, both copies are live, both
/// match the same normalized name, and the series reports `ambiguous` with
/// nothing written.
#[tokio::test]
async fn scoping_the_import_to_a_library_resolves_a_duplicate_that_is_otherwise_ambiguous() {
    let (db, _tmp) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_user, token) = admin_and_token(&db, &state, "lib-scope").await;

    let old_lib = LibraryRepository::create(&db, "Old", "/old", ScanningStrategy::Default)
        .await
        .unwrap();
    let new_lib = LibraryRepository::create(&db, "New", "/new", ScanningStrategy::Default)
        .await
        .unwrap();

    // The same series, live in both libraries: the files were copied rather
    // than moved, or the old library has not been rescanned yet.
    for (lib, root) in [(old_lib.id, "old"), (new_lib.id, "new")] {
        let series = SeriesRepository::create(&db, lib, "Naruto", None)
            .await
            .unwrap();
        BookRepository::create(
            &db,
            &book_model(
                series.id,
                lib,
                &format!("/{root}/Naruto/v01.cbz"),
                "v01.cbz",
                "",
            ),
            None,
        )
        .await
        .unwrap();
    }

    let series_doc = ExportSeriesDto {
        external_ids: vec![],
        library_relative_path: "Naruto".to_string(),
        name: "Naruto".to_string(),
        rating: Some(50),
        notes: None,
        rating_updated_at: Some(Utc::now()),
        books: vec![],
        want_to_read: None,
    };

    // Unscoped: both copies compete.
    let app = create_test_router(state.clone()).await;
    let body = import_request(document(vec![series_doc.clone()], false), true);
    let request = post_json_request_with_auth("/api/v1/reading-progress/import", &body, &token);
    let (_status, unscoped): (StatusCode, Option<ImportReadingProgressResponse>) =
        make_json_request(app, request).await;
    assert_eq!(
        unscoped.expect("report").series[0].disposition,
        SeriesDisposition::Ambiguous
    );

    // Scoped to the new library: only one candidate remains.
    let app = create_test_router(state.clone()).await;
    let mut body = import_request(document(vec![series_doc], false), true);
    body.library_ids = Some(vec![new_lib.id]);
    let request = post_json_request_with_auth("/api/v1/reading-progress/import", &body, &token);
    let (_status, scoped): (StatusCode, Option<ImportReadingProgressResponse>) =
        make_json_request(app, request).await;
    assert_eq!(
        scoped.expect("report").series[0].disposition,
        SeriesDisposition::Matched
    );
}

/// Exporting a subset carries only that library's series.
#[tokio::test]
async fn exporting_with_library_ids_omits_the_other_libraries() {
    let (db, _tmp) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (user_id, token) = admin_and_token(&db, &state, "lib-export").await;

    let mut wanted_library = None;
    for name in ["Kept", "Excluded"] {
        let lib =
            LibraryRepository::create(&db, name, &format!("/{name}"), ScanningStrategy::Default)
                .await
                .unwrap();
        let series = SeriesRepository::create(&db, lib.id, name, None)
            .await
            .unwrap();
        let book = BookRepository::create(
            &db,
            &book_model(
                series.id,
                lib.id,
                &format!("/{name}/{name}/v01.cbz"),
                "v01.cbz",
                "",
            ),
            None,
        )
        .await
        .unwrap();
        ReadProgressRepository::upsert(&db, user_id, book.id, 3, false)
            .await
            .unwrap();
        if name == "Kept" {
            wanted_library = Some(lib.id);
        }
    }

    let app = create_test_router(state.clone()).await;
    let request = get_request_with_auth(
        &format!(
            "/api/v1/reading-progress/export?libraryIds={}",
            wanted_library.unwrap()
        ),
        &token,
    );
    let (status, document): (StatusCode, Option<ReadingProgressExportDocument>) =
        make_json_request(app, request).await;
    assert_eq!(status, StatusCode::OK);

    let document = document.expect("export document");
    let names: Vec<&str> = document.series.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, vec!["Kept"]);
}

/// A malformed id is a 400, not a silently wider export.
#[tokio::test]
async fn a_malformed_library_id_is_rejected_with_400() {
    let (db, _tmp) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_user, token) = admin_and_token(&db, &state, "lib-bad").await;

    let app = create_test_router(state.clone()).await;
    let request = get_request_with_auth(
        "/api/v1/reading-progress/export?libraryIds=not-a-uuid",
        &token,
    );
    let (status, _body): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

/// Scoping makes a series match that would otherwise be ambiguous, which is
/// the point of it. But the reader's sessions still sit on the old library's
/// live books, so they are *not* moved: progress goes across and the reading
/// history stays behind. That is a half-migration, and the report has to say
/// so rather than reading as a clean success.
#[tokio::test]
async fn rows_on_a_live_book_elsewhere_are_reported_as_stranded() {
    let (db, _tmp) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (user_id, token) = admin_and_token(&db, &state, "stranded").await;

    let old_lib = LibraryRepository::create(&db, "Old", "/old", ScanningStrategy::Default)
        .await
        .unwrap();
    let new_lib = LibraryRepository::create(&db, "New", "/new", ScanningStrategy::Default)
        .await
        .unwrap();

    // The same series in both libraries, both live: the files were copied
    // rather than moved, so the old library still has them on disk.
    let mut old_book = None;
    for (lib, root) in [(old_lib.id, "old"), (new_lib.id, "new")] {
        let series = SeriesRepository::create(&db, lib, "Naruto", None)
            .await
            .unwrap();
        let book = BookRepository::create(
            &db,
            &book_model(
                series.id,
                lib,
                &format!("/{root}/Naruto/v01.cbz"),
                "v01.cbz",
                "",
            ),
            None,
        )
        .await
        .unwrap();
        if root == "old" {
            old_book = Some(book.id);
        }
    }

    // A session already banked against the old library's still-live book.
    let session_id = Uuid::new_v4();
    let now = Utc::now();
    ReadProgressRepository::record_session(
        &db,
        NewSession::from_client(
            session_id,
            user_id,
            old_book.unwrap(),
            "device-1",
            None,
            SessionKind::Progress,
            Some(60_000),
            Some(5),
            now - Duration::minutes(10),
            now,
        )
        .with_page(5),
    )
    .await
    .unwrap();

    let series_doc = ExportSeriesDto {
        external_ids: vec![],
        library_relative_path: "Naruto".to_string(),
        name: "Naruto".to_string(),
        rating: None,
        notes: None,
        rating_updated_at: None,
        books: vec![ExportBookDto {
            path: "v01.cbz".to_string(),
            file_name: "v01.cbz".to_string(),
            file_hash: String::new(),
            partial_hash: String::new(),
            progress: None,
            completions: vec![],
            sessions: Some(vec![ExportSessionDto {
                id: session_id,
                device_id: "device-1".to_string(),
                device_name: None,
                pass: 1,
                kind: "progress".to_string(),
                to_page: Some(5),
                to_percentage: None,
                active_duration_ms: Some(60_000),
                duration_source: "measured".to_string(),
                pages_read: Some(5),
                client_started_at: now - Duration::minutes(10),
                client_ended_at: now,
                server_recorded_at: now,
            }]),
            want_to_read: None,
        }],
        want_to_read: None,
    };

    let app = create_test_router(state.clone()).await;
    let mut body = import_request(document(vec![series_doc], true), true);
    body.library_ids = Some(vec![new_lib.id]);
    let request = post_json_request_with_auth("/api/v1/reading-progress/import", &body, &token);
    let (status, response): (StatusCode, Option<ImportReadingProgressResponse>) =
        make_json_request(app, request).await;
    assert_eq!(status, StatusCode::OK);
    let report = response.expect("import response");

    // The series matched, so this reads as a success without the notice.
    assert_eq!(report.series[0].disposition, SeriesDisposition::Matched);
    assert_eq!(report.summary.rows_stranded, 1);
    assert_eq!(report.series[0].books[0].sessions.stranded, 1);
    assert!(
        report.notices.iter().any(|n| n.contains("left behind")),
        "a half-migration must be stated, got: {:?}",
        report.notices
    );
}

// ---------------------------------------------------------------------------
// Reporting coverage of the destination.
//
// A split exports a whole old library and imports it into one of several new
// ones, so most file series are *meant* to miss. Counting them as unmatched
// made a complete import read like a failure. A scoped import instead reports
// how much of the destination it covered.
// ---------------------------------------------------------------------------

/// A live series with one live book, in the given library.
async fn live_series(db: &DatabaseConnection, library_id: Uuid, root: &str, name: &str) -> Uuid {
    let series = SeriesRepository::create(db, library_id, name, None)
        .await
        .unwrap();
    BookRepository::create(
        db,
        &book_model(
            series.id,
            library_id,
            &format!("/{root}/{name}/v01.cbz"),
            "v01.cbz",
            "",
        ),
        None,
    )
    .await
    .unwrap();
    series.id
}

fn named_series_doc(name: &str, relative_path: &str) -> ExportSeriesDto {
    ExportSeriesDto {
        external_ids: vec![],
        library_relative_path: relative_path.to_string(),
        name: name.to_string(),
        rating: None,
        notes: None,
        rating_updated_at: None,
        books: vec![],
        want_to_read: None,
    }
}

async fn preview(
    state: &std::sync::Arc<codex::api::extractors::AuthState>,
    token: &str,
    series: Vec<ExportSeriesDto>,
    library_ids: Option<Vec<Uuid>>,
) -> ImportReadingProgressResponse {
    let app = create_test_router(state.clone()).await;
    let mut body = import_request(document(series, false), true);
    body.library_ids = library_ids;
    let request = post_json_request_with_auth("/api/v1/reading-progress/import", &body, token);
    let (status, response): (StatusCode, Option<ImportReadingProgressResponse>) =
        make_json_request(app, request).await;
    assert_eq!(status, StatusCode::OK);
    response.expect("import response")
}

/// The shape of the user's first real import: the file covers far more than
/// the destination, and every destination series matched.
#[tokio::test]
async fn a_scoped_import_reports_coverage_of_the_destination() {
    let (db, _tmp) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_user, token) = admin_and_token(&db, &state, "coverage").await;

    let shonen = LibraryRepository::create(&db, "Shonen", "/shonen", ScanningStrategy::Default)
        .await
        .unwrap();
    let other = LibraryRepository::create(&db, "Other", "/other", ScanningStrategy::Default)
        .await
        .unwrap();
    live_series(&db, shonen.id, "shonen", "Naruto").await;
    live_series(&db, shonen.id, "shonen", "Bleach").await;
    live_series(&db, other.id, "other", "Monster").await;

    let report = preview(
        &state,
        &token,
        vec![
            named_series_doc("Naruto", "Naruto"),
            named_series_doc("Bleach", "Bleach"),
            named_series_doc("Monster", "Monster"),
            named_series_doc("Nowhere At All", "Nowhere At All"),
        ],
        Some(vec![shonen.id]),
    )
    .await;

    assert_eq!(report.summary.series_in_selected_libraries, Some(2));
    assert_eq!(report.summary.series_matched_distinct, 2);
    assert_eq!(report.summary.books_in_selected_libraries, Some(2));
}

/// Two file series can resolve to one destination series by name. Counting
/// file matches would then report more matches than the destination holds,
/// so coverage counts distinct destination series.
#[tokio::test]
async fn two_file_series_landing_on_one_destination_series_count_once() {
    let (db, _tmp) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_user, token) = admin_and_token(&db, &state, "distinct").await;

    let lib = LibraryRepository::create(&db, "Lib", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();
    live_series(&db, lib.id, "lib", "Naruto").await;

    let report = preview(
        &state,
        &token,
        vec![
            named_series_doc("Naruto", "shonen/Naruto"),
            named_series_doc("Naruto", "old/Naruto"),
        ],
        Some(vec![lib.id]),
    )
    .await;

    assert_eq!(report.summary.series_matched, 2);
    assert_eq!(report.summary.series_matched_distinct, 1);
    assert_eq!(report.summary.series_in_selected_libraries, Some(1));
}

/// Unscoped, there is no meaningful destination to measure against: the
/// denominator would be every series the reader can see.
#[tokio::test]
async fn an_unscoped_import_has_no_destination_totals() {
    let (db, _tmp) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_user, token) = admin_and_token(&db, &state, "unscoped").await;

    let lib = LibraryRepository::create(&db, "Lib", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();
    live_series(&db, lib.id, "lib", "Naruto").await;

    let report = preview(
        &state,
        &token,
        vec![named_series_doc("Naruto", "Naruto")],
        None,
    )
    .await;

    assert_eq!(report.summary.series_in_selected_libraries, None);
    assert_eq!(report.summary.books_in_selected_libraries, None);
    assert_eq!(report.summary.series_matched_distinct, 1);
}

// ---------------------------------------------------------------------------
// Want-to-read.
//
// The queue is per-user and keyed by series or book, so it rides the match the
// import already performs. The case that matters most is the one that is easy
// to lose: a series queued but never started has no reading state at all, so
// an export seeded only from reading state would never carry it.
// ---------------------------------------------------------------------------

use codex::db::entities::want_to_read;
use codex::db::repositories::WantToReadRepository;
use codex::models::sort::WantToReadSort;

async fn queue_for(db: &DatabaseConnection, user: Uuid) -> Vec<want_to_read::Model> {
    WantToReadRepository::list(db, user, WantToReadSort::Custom)
        .await
        .unwrap()
}

async fn export_doc(
    state: &std::sync::Arc<codex::api::extractors::AuthState>,
    token: &str,
) -> ReadingProgressExportDocument {
    let app = create_test_router(state.clone()).await;
    let request = get_request_with_auth("/api/v1/reading-progress/export", token);
    let (status, doc): (StatusCode, Option<ReadingProgressExportDocument>) =
        make_json_request(app, request).await;
    assert_eq!(status, StatusCode::OK);
    doc.expect("export document")
}

async fn import_doc(
    state: &std::sync::Arc<codex::api::extractors::AuthState>,
    token: &str,
    doc: ReadingProgressExportDocument,
    dry_run: bool,
    restore: bool,
) -> ImportReadingProgressResponse {
    let app = create_test_router(state.clone()).await;
    let mut body = import_request(doc, dry_run);
    body.restore_want_to_read = restore;
    let request = post_json_request_with_auth("/api/v1/reading-progress/import", &body, token);
    let (status, response): (StatusCode, Option<ImportReadingProgressResponse>) =
        make_json_request(app, request).await;
    assert_eq!(status, StatusCode::OK);
    response.expect("import response")
}

/// The ordinary want-to-read case: queued, never opened. With no progress,
/// completion, session or rating, nothing else would put it in the file.
#[tokio::test]
async fn a_queued_series_with_no_reading_state_is_exported() {
    let (db, _tmp) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (user, token) = admin_and_token(&db, &state, "wtr-series").await;
    let lib = LibraryRepository::create(&db, "Lib", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();
    let series = live_series(&db, lib.id, "lib", "Unstarted").await;
    WantToReadRepository::add_series(&db, user, series)
        .await
        .unwrap();

    let doc = export_doc(&state, &token).await;

    let exported = doc
        .series
        .iter()
        .find(|s| s.name == "Unstarted")
        .expect("a queued series with no reading state must still be exported");
    assert!(exported.want_to_read.is_some());
}

/// A book can be queued on its own. It has to travel with its series so the
/// import can match it, even though nothing else was recorded against either.
#[tokio::test]
async fn a_queued_book_is_exported_with_its_series() {
    let (db, _tmp) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (user, token) = admin_and_token(&db, &state, "wtr-book").await;
    let lib = LibraryRepository::create(&db, "Lib", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();
    let series = live_series(&db, lib.id, "lib", "Anthology").await;
    let book = books::Entity::find()
        .filter(books::Column::SeriesId.eq(series))
        .one(&db)
        .await
        .unwrap()
        .expect("the series has a book");
    WantToReadRepository::add_book(&db, user, book.id)
        .await
        .unwrap();

    let doc = export_doc(&state, &token).await;

    let exported = doc
        .series
        .iter()
        .find(|s| s.name == "Anthology")
        .expect("the queued book's series must be exported");
    assert!(
        exported.want_to_read.is_none(),
        "the book was queued, not the series"
    );
    assert_eq!(exported.books.len(), 1);
    assert!(exported.books[0].want_to_read.is_some());
}

/// Imported entries go after the reader's existing queue, in their original
/// relative order, and nothing already queued moves.
#[tokio::test]
async fn restored_entries_follow_the_existing_queue_in_original_order() {
    let (db, _tmp) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (user, token) = admin_and_token(&db, &state, "wtr-order").await;
    let lib = LibraryRepository::create(&db, "Lib", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();
    let already_queued = live_series(&db, lib.id, "lib", "Already Queued").await;
    let second = live_series(&db, lib.id, "lib", "Second").await;
    let first = live_series(&db, lib.id, "lib", "First").await;
    WantToReadRepository::add_series(&db, user, already_queued)
        .await
        .unwrap();

    // Listed out of order in the file; the original positions say First comes
    // before Second.
    let queued_on = Utc::now() - Duration::days(30);
    let mut second_doc = named_series_doc("Second", "Second");
    second_doc.want_to_read = Some(ExportWantToReadDto {
        position: 9,
        added_at: queued_on,
    });
    let mut first_doc = named_series_doc("First", "First");
    first_doc.want_to_read = Some(ExportWantToReadDto {
        position: 4,
        added_at: queued_on,
    });

    let report = import_doc(
        &state,
        &token,
        document(vec![second_doc, first_doc], false),
        false,
        true,
    )
    .await;
    assert_eq!(report.summary.want_to_read_restored, 2);

    let queue = queue_for(&db, user).await;
    let order: Vec<Uuid> = queue.iter().filter_map(|e| e.series_id).collect();
    assert_eq!(order, vec![already_queued, first, second]);

    // The original date survives, so the newest/oldest sorts stay truthful.
    let restored = queue.iter().find(|e| e.series_id == Some(first)).unwrap();
    assert_eq!(restored.added_at.timestamp(), queued_on.timestamp());
}

/// Row ids are not reused for the queue, so idempotency comes from the entry
/// itself: a series already queued is left exactly where it is.
#[tokio::test]
async fn reimporting_does_not_duplicate_the_queue() {
    let (db, _tmp) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (user, token) = admin_and_token(&db, &state, "wtr-twice").await;
    let lib = LibraryRepository::create(&db, "Lib", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();
    live_series(&db, lib.id, "lib", "Once").await;

    let mut doc_series = named_series_doc("Once", "Once");
    doc_series.want_to_read = Some(ExportWantToReadDto {
        position: 0,
        added_at: Utc::now(),
    });
    let doc = document(vec![doc_series], false);

    import_doc(&state, &token, doc.clone(), false, true).await;
    let second = import_doc(&state, &token, doc, false, true).await;

    assert_eq!(queue_for(&db, user).await.len(), 1);
    assert_eq!(second.summary.want_to_read_restored, 0);
}

#[tokio::test]
async fn restoring_want_to_read_can_be_switched_off() {
    let (db, _tmp) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (user, token) = admin_and_token(&db, &state, "wtr-off").await;
    let lib = LibraryRepository::create(&db, "Lib", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();
    live_series(&db, lib.id, "lib", "Skipped").await;

    let mut doc_series = named_series_doc("Skipped", "Skipped");
    doc_series.want_to_read = Some(ExportWantToReadDto {
        position: 0,
        added_at: Utc::now(),
    });

    let report = import_doc(
        &state,
        &token,
        document(vec![doc_series], false),
        false,
        false,
    )
    .await;

    assert!(queue_for(&db, user).await.is_empty());
    assert_eq!(report.summary.want_to_read_restored, 0);
}

/// A dry run reports what it would restore and writes nothing, like every
/// other count in the report.
#[tokio::test]
async fn a_dry_run_reports_want_to_read_but_writes_nothing() {
    let (db, _tmp) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (user, token) = admin_and_token(&db, &state, "wtr-dry").await;
    let lib = LibraryRepository::create(&db, "Lib", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();
    live_series(&db, lib.id, "lib", "Previewed").await;

    let mut doc_series = named_series_doc("Previewed", "Previewed");
    doc_series.want_to_read = Some(ExportWantToReadDto {
        position: 0,
        added_at: Utc::now(),
    });

    let report = import_doc(
        &state,
        &token,
        document(vec![doc_series], false),
        true,
        true,
    )
    .await;

    assert_eq!(report.summary.want_to_read_restored, 1);
    assert!(queue_for(&db, user).await.is_empty());
}

/// `booksUnmatched` mixes books whose whole series belongs elsewhere with
/// books missed inside a series that matched. Only the first is reassuring,
/// so the report has to keep them apart.
#[tokio::test]
async fn books_missed_inside_a_matched_series_are_kept_apart_from_other_libraries() {
    let (db, _tmp) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_user, token) = admin_and_token(&db, &state, "book-split").await;
    let lib = LibraryRepository::create(&db, "Lib", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();
    live_series(&db, lib.id, "lib", "Here").await;

    let book = |path: &str| minimal_book_doc(path, path, "", 1);

    // A matched series: one book matches, one is genuinely missing.
    let mut here = named_series_doc("Here", "Here");
    here.books = vec![book("v01.cbz"), book("v99-missing.cbz")];
    // A series that belongs to another library entirely.
    let mut elsewhere = named_series_doc("Elsewhere", "Elsewhere");
    elsewhere.books = vec![book("a.cbz"), book("b.cbz")];

    let report = preview(&state, &token, vec![here, elsewhere], Some(vec![lib.id])).await;

    assert_eq!(report.summary.books_unmatched, 3);
    assert_eq!(report.summary.books_in_unmatched_series, 2);
    // What is left is the one that deserves attention.
    assert_eq!(
        report.summary.books_unmatched - report.summary.books_in_unmatched_series,
        1
    );
}
