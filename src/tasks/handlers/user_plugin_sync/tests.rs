use super::*;
use crate::db::ScanningStrategy;
use crate::db::entities::{books, users};
use crate::db::repositories::{
    BookRepository, LibraryRepository, ReadProgressRepository, SeriesExternalIdRepository,
    SeriesMetadataRepository, SeriesRepository, UserRepository, UserSeriesRatingRepository,
};
use crate::db::test_helpers::create_test_db;
use crate::services::plugin::sync::{SyncEntry, SyncProgress, SyncReadingStatus};
use chrono::Utc;

/// Helper to create a test user in the database
async fn create_test_user(db: &sea_orm::DatabaseConnection) -> users::Model {
    let user = users::Model {
        id: Uuid::new_v4(),
        username: format!("syncuser_{}", Uuid::new_v4()),
        email: format!("sync_{}@example.com", Uuid::new_v4()),
        password_hash: "hash123".to_string(),
        role: "reader".to_string(),
        is_active: true,
        email_verified: false,
        permissions: serde_json::json!([]),
        created_at: Utc::now(),
        updated_at: Utc::now(),
        last_login_at: None,
    };
    UserRepository::create(db, &user).await.unwrap()
}

/// Helper to create a book in a series with a given page count
async fn create_test_book(
    db: &sea_orm::DatabaseConnection,
    series_id: Uuid,
    library_id: Uuid,
    index: usize,
    page_count: i32,
) -> books::Model {
    let book = books::Model {
        id: Uuid::new_v4(),
        series_id,
        library_id,
        file_path: format!("/test/book_{}_{}.cbz", index, Uuid::new_v4()),
        file_name: format!("book_{}.cbz", index),
        file_size: 1024,
        file_hash: format!("hash_{}_{}", index, Uuid::new_v4()),
        partial_hash: String::new(),
        format: "cbz".to_string(),
        page_count,
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
    BookRepository::create(db, &book, None).await.unwrap()
}

#[test]
fn test_handler_creation() {
    // Handler requires a PluginManager, verify the struct is constructed correctly
    // (actual integration test would need a real PluginManager)
}

#[test]
fn test_sync_result_serialization() {
    let result = UserPluginSyncResult {
        plugin_id: Uuid::new_v4(),
        user_id: Uuid::new_v4(),
        external_username: Some("manga_reader".to_string()),
        pushed: 5,
        pulled: 10,
        matched: 8,
        applied: 6,
        push_failures: 1,
        pull_incomplete: false,
        pull_error: None,
        push_error: None,
        skipped_reason: None,
    };

    let json = serde_json::to_value(&result).unwrap();
    assert_eq!(json["externalUsername"], "manga_reader");
    assert_eq!(json["pushed"], 5);
    assert_eq!(json["pulled"], 10);
    assert_eq!(json["matched"], 8);
    assert_eq!(json["applied"], 6);
    assert_eq!(json["pushFailures"], 1);
    assert!(!json["pullIncomplete"].as_bool().unwrap());
    assert!(!json.as_object().unwrap().contains_key("skippedReason"));
    assert!(!json.as_object().unwrap().contains_key("pullError"));
    assert!(!json.as_object().unwrap().contains_key("pushError"));
}

#[test]
fn test_sync_result_with_errors() {
    let result = UserPluginSyncResult {
        plugin_id: Uuid::new_v4(),
        user_id: Uuid::new_v4(),
        external_username: Some("user".to_string()),
        pushed: 3,
        pulled: 0,
        matched: 0,
        applied: 0,
        push_failures: 0,
        pull_incomplete: false,
        pull_error: Some("AniList API error: 400 Bad Request".to_string()),
        push_error: None,
        skipped_reason: None,
    };

    let json = serde_json::to_value(&result).unwrap();
    assert_eq!(json["pullError"], "AniList API error: 400 Bad Request");
    assert!(!json.as_object().unwrap().contains_key("pushError"));
    assert_eq!(json["pushed"], 3);
    assert_eq!(json["pulled"], 0);

    // Round-trip
    let deserialized: UserPluginSyncResult = serde_json::from_value(json).unwrap();
    assert_eq!(
        deserialized.pull_error,
        Some("AniList API error: 400 Bad Request".to_string())
    );
    assert!(deserialized.push_error.is_none());
}

#[test]
fn test_sync_result_with_both_errors() {
    let result = UserPluginSyncResult {
        plugin_id: Uuid::new_v4(),
        user_id: Uuid::new_v4(),
        external_username: None,
        pushed: 0,
        pulled: 0,
        matched: 0,
        applied: 0,
        push_failures: 0,
        pull_incomplete: false,
        pull_error: Some("Pull failed".to_string()),
        push_error: Some("Push failed".to_string()),
        skipped_reason: None,
    };

    let json = serde_json::to_value(&result).unwrap();
    assert_eq!(json["pullError"], "Pull failed");
    assert_eq!(json["pushError"], "Push failed");
}

#[test]
fn test_sync_result_skipped() {
    let result = UserPluginSyncResult {
        plugin_id: Uuid::new_v4(),
        user_id: Uuid::new_v4(),
        external_username: None,
        pushed: 0,
        pulled: 0,
        matched: 0,
        applied: 0,
        push_failures: 0,
        pull_incomplete: false,
        pull_error: None,
        push_error: None,
        skipped_reason: Some("plugin_not_enabled".to_string()),
    };

    let json = serde_json::to_value(&result).unwrap();
    assert_eq!(json["skippedReason"], "plugin_not_enabled");
    assert!(!json.as_object().unwrap().contains_key("externalUsername"));
    assert_eq!(json["pushed"], 0);
    assert_eq!(json["pulled"], 0);
    assert_eq!(json["matched"], 0);
    assert_eq!(json["applied"], 0);
}

#[test]
fn test_sync_result_deserialization() {
    let json = serde_json::json!({
        "pluginId": "00000000-0000-0000-0000-000000000001",
        "userId": "00000000-0000-0000-0000-000000000002",
        "externalUsername": "test_user",
        "pushed": 3,
        "pulled": 7,
        "matched": 5,
        "applied": 4,
        "pushFailures": 0,
        "pullIncomplete": true,
    });

    let result: UserPluginSyncResult = serde_json::from_value(json).unwrap();
    assert_eq!(result.external_username, Some("test_user".to_string()));
    assert_eq!(result.pushed, 3);
    assert_eq!(result.pulled, 7);
    assert_eq!(result.matched, 5);
    assert_eq!(result.applied, 4);
    assert!(result.pull_incomplete);
    assert!(result.skipped_reason.is_none());
}

#[test]
fn test_sync_result_pull_incomplete() {
    let result = UserPluginSyncResult {
        plugin_id: Uuid::new_v4(),
        user_id: Uuid::new_v4(),
        external_username: Some("user".to_string()),
        pushed: 0,
        pulled: 500,
        matched: 300,
        applied: 250,
        push_failures: 0,
        pull_incomplete: true,
        pull_error: None,
        push_error: None,
        skipped_reason: None,
    };

    let json = serde_json::to_value(&result).unwrap();
    assert!(json["pullIncomplete"].as_bool().unwrap());
    assert_eq!(json["pulled"], 500);
    assert_eq!(json["matched"], 300);
    assert_eq!(json["applied"], 250);
}

#[test]
fn test_sync_result_applied_field() {
    let result = UserPluginSyncResult {
        plugin_id: Uuid::new_v4(),
        user_id: Uuid::new_v4(),
        external_username: None,
        pushed: 0,
        pulled: 10,
        matched: 5,
        applied: 3,
        push_failures: 0,
        pull_incomplete: false,
        pull_error: None,
        push_error: None,
        skipped_reason: None,
    };

    let json = serde_json::to_value(&result).unwrap();
    assert_eq!(json["applied"], 3);

    // Verify round-trip
    let deserialized: UserPluginSyncResult = serde_json::from_value(json).unwrap();
    assert_eq!(deserialized.applied, 3);
}

#[tokio::test]
async fn test_match_and_apply_no_source() {
    let (db, _temp_dir) = create_test_db().await;
    let user_id = Uuid::new_v4();

    let entries = vec![SyncEntry {
        external_id: "12345".to_string(),
        status: SyncReadingStatus::Reading,
        progress: None,
        score: None,
        started_at: None,
        completed_at: None,
        notes: None,
        latest_updated_at: None,
        title: None,
    }];

    let (matched, applied) = pull::match_and_apply_pulled_entries(
        db.sea_orm_connection(),
        &entries,
        None,
        user_id,
        Uuid::new_v4(),
        false,
    )
    .await;
    assert_eq!(matched, 0);
    assert_eq!(applied, 0);
}

#[tokio::test]
async fn test_match_and_apply_with_matches() {
    let (db, _temp_dir) = create_test_db().await;

    let library = LibraryRepository::create(
        db.sea_orm_connection(),
        "Test Library",
        "/test/path",
        ScanningStrategy::Default,
    )
    .await
    .unwrap();

    let series = SeriesRepository::create(db.sea_orm_connection(), library.id, "My Manga", None)
        .await
        .unwrap();

    // Create an api:anilist external ID for the series
    SeriesExternalIdRepository::create(
        db.sea_orm_connection(),
        series.id,
        "api:anilist",
        "12345",
        None,
        None,
    )
    .await
    .unwrap();

    let user_id = Uuid::new_v4();

    let entries = vec![
        SyncEntry {
            external_id: "12345".to_string(), // matches
            status: SyncReadingStatus::Reading,
            progress: None,
            score: None,
            started_at: None,
            completed_at: None,
            notes: None,
            latest_updated_at: None,
            title: None,
        },
        SyncEntry {
            external_id: "99999".to_string(), // no match
            status: SyncReadingStatus::Completed,
            progress: None,
            score: None,
            started_at: None,
            completed_at: None,
            notes: None,
            latest_updated_at: None,
            title: None,
        },
    ];

    let (matched, _applied) = pull::match_and_apply_pulled_entries(
        db.sea_orm_connection(),
        &entries,
        Some("api:anilist"),
        user_id,
        Uuid::new_v4(),
        false,
    )
    .await;
    assert_eq!(matched, 1);
}

#[tokio::test]
async fn test_match_and_apply_pulled_entries_applies_progress() {
    let (db, _temp_dir) = create_test_db().await;

    let library = LibraryRepository::create(
        db.sea_orm_connection(),
        "Test Library",
        "/test/path",
        ScanningStrategy::Default,
    )
    .await
    .unwrap();

    let series = SeriesRepository::create(db.sea_orm_connection(), library.id, "Test Manga", None)
        .await
        .unwrap();

    // Create 5 books in the series
    for i in 1..=5 {
        create_test_book(db.sea_orm_connection(), series.id, library.id, i, 100).await;
    }

    SeriesExternalIdRepository::create(
        db.sea_orm_connection(),
        series.id,
        "api:anilist",
        "42",
        None,
        None,
    )
    .await
    .unwrap();

    let user = create_test_user(db.sea_orm_connection()).await;
    let user_id = user.id;

    // Pull entry says 3 chapters read
    let entries = vec![SyncEntry {
        external_id: "42".to_string(),
        status: SyncReadingStatus::Reading,
        progress: Some(SyncProgress {
            chapters: Some(3),
            volumes: None,
            pages: None,
            total_chapters: None,
            total_volumes: None,
        }),
        score: None,
        started_at: None,
        completed_at: None,
        notes: None,
        latest_updated_at: None,
        title: None,
    }];

    let (matched, applied) = pull::match_and_apply_pulled_entries(
        db.sea_orm_connection(),
        &entries,
        Some("api:anilist"),
        user_id,
        Uuid::new_v4(),
        false,
    )
    .await;
    assert_eq!(matched, 1);
    assert_eq!(applied, 3);

    // Verify: first 3 books should be marked as read
    let books_list = BookRepository::list_by_series(db.sea_orm_connection(), series.id, false)
        .await
        .unwrap();
    for (i, book) in books_list.iter().enumerate() {
        let progress =
            ReadProgressRepository::get_by_user_and_book(db.sea_orm_connection(), user_id, book.id)
                .await
                .unwrap();
        if i < 3 {
            assert!(progress.is_some(), "Book {} should have progress", i);
            assert!(
                progress.unwrap().completed,
                "Book {} should be completed",
                i
            );
        } else {
            assert!(progress.is_none(), "Book {} should have no progress", i);
        }
    }
}

#[tokio::test]
async fn test_match_and_apply_skips_already_read() {
    let (db, _temp_dir) = create_test_db().await;

    let library = LibraryRepository::create(
        db.sea_orm_connection(),
        "Test Library",
        "/test/path",
        ScanningStrategy::Default,
    )
    .await
    .unwrap();

    let series = SeriesRepository::create(db.sea_orm_connection(), library.id, "Test Manga", None)
        .await
        .unwrap();

    // Create 3 books
    let mut book_ids = Vec::new();
    for i in 1..=3 {
        let book = create_test_book(db.sea_orm_connection(), series.id, library.id, i, 50).await;
        book_ids.push(book.id);
    }

    let user = create_test_user(db.sea_orm_connection()).await;
    let user_id = user.id;

    // Pre-mark book 1 as read
    ReadProgressRepository::mark_as_read(db.sea_orm_connection(), user_id, book_ids[0], 50)
        .await
        .unwrap();

    SeriesExternalIdRepository::create(
        db.sea_orm_connection(),
        series.id,
        "api:anilist",
        "99",
        None,
        None,
    )
    .await
    .unwrap();

    // Pull says completed (all 3 chapters)
    let entries = vec![SyncEntry {
        external_id: "99".to_string(),
        status: SyncReadingStatus::Completed,
        progress: Some(SyncProgress {
            chapters: Some(3),
            volumes: None,
            pages: None,
            total_chapters: None,
            total_volumes: None,
        }),
        score: None,
        started_at: None,
        completed_at: None,
        notes: None,
        latest_updated_at: None,
        title: None,
    }];

    let (matched, applied) = pull::match_and_apply_pulled_entries(
        db.sea_orm_connection(),
        &entries,
        Some("api:anilist"),
        user_id,
        Uuid::new_v4(),
        false,
    )
    .await;
    assert_eq!(matched, 1);
    // Only 2 books newly applied (book 1 was already read)
    assert_eq!(applied, 2);
}

/// Default Codex sync settings for tests (matches production defaults)
fn default_codex_settings() -> CodexSyncSettings {
    CodexSyncSettings {
        include_completed: true,
        include_in_progress: true,
        count_partial_progress: false,
        sync_ratings: true,
        search_fallback: false,
    }
}

#[tokio::test]
async fn test_build_push_entries_with_progress() {
    let (db, _temp_dir) = create_test_db().await;

    let library = LibraryRepository::create(
        db.sea_orm_connection(),
        "Test Library",
        "/test/path",
        ScanningStrategy::Default,
    )
    .await
    .unwrap();

    let series = SeriesRepository::create(db.sea_orm_connection(), library.id, "Push Manga", None)
        .await
        .unwrap();

    // Create 4 books
    let mut test_books = Vec::new();
    for i in 1..=4 {
        let book = create_test_book(db.sea_orm_connection(), series.id, library.id, i, 100).await;
        test_books.push(book);
    }

    let user = create_test_user(db.sea_orm_connection()).await;
    let user_id = user.id;

    // Mark first 2 books as read
    ReadProgressRepository::mark_as_read(db.sea_orm_connection(), user_id, test_books[0].id, 100)
        .await
        .unwrap();
    ReadProgressRepository::mark_as_read(db.sea_orm_connection(), user_id, test_books[1].id, 100)
        .await
        .unwrap();

    SeriesExternalIdRepository::create(
        db.sea_orm_connection(),
        series.id,
        "api:anilist",
        "777",
        None,
        None,
    )
    .await
    .unwrap();

    let entries = push::build_push_entries(
        db.sea_orm_connection(),
        user_id,
        "api:anilist",
        Uuid::new_v4(),
        &default_codex_settings(),
    )
    .await;

    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].external_id, "777");
    assert_eq!(entries[0].status, SyncReadingStatus::Reading);
    // "volumes" mode sends only volumes (not chapters, to avoid misleading activity)
    assert_eq!(entries[0].progress.as_ref().unwrap().volumes, Some(2));
    assert!(entries[0].progress.as_ref().unwrap().chapters.is_none());
}

#[tokio::test]
async fn test_build_push_entries_all_completed() {
    let (db, _temp_dir) = create_test_db().await;

    let library = LibraryRepository::create(
        db.sea_orm_connection(),
        "Test Library",
        "/test/path",
        ScanningStrategy::Default,
    )
    .await
    .unwrap();

    let series = SeriesRepository::create(db.sea_orm_connection(), library.id, "Done Manga", None)
        .await
        .unwrap();

    // Create 2 books
    let mut test_books = Vec::new();
    for i in 1..=2 {
        let book = create_test_book(db.sea_orm_connection(), series.id, library.id, i, 50).await;
        test_books.push(book);
    }

    let user = create_test_user(db.sea_orm_connection()).await;
    let user_id = user.id;

    // Mark all books as read
    for book in &test_books {
        ReadProgressRepository::mark_as_read(db.sea_orm_connection(), user_id, book.id, 50)
            .await
            .unwrap();
    }

    SeriesExternalIdRepository::create(
        db.sea_orm_connection(),
        series.id,
        "api:anilist",
        "888",
        None,
        None,
    )
    .await
    .unwrap();

    let entries = push::build_push_entries(
        db.sea_orm_connection(),
        user_id,
        "api:anilist",
        Uuid::new_v4(),
        &default_codex_settings(),
    )
    .await;

    assert_eq!(entries.len(), 1);
    // Always push as Reading — we can't know total chapter count from external service
    assert_eq!(entries[0].status, SyncReadingStatus::Reading);
    // "volumes" mode sends only volumes (not chapters, to avoid misleading activity)
    assert_eq!(entries[0].progress.as_ref().unwrap().volumes, Some(2));
    assert!(entries[0].progress.as_ref().unwrap().chapters.is_none());
    assert!(entries[0].completed_at.is_none());
}

#[tokio::test]
async fn test_build_push_entries_skips_no_progress() {
    let (db, _temp_dir) = create_test_db().await;

    let library = LibraryRepository::create(
        db.sea_orm_connection(),
        "Test Library",
        "/test/path",
        ScanningStrategy::Default,
    )
    .await
    .unwrap();

    let series =
        SeriesRepository::create(db.sea_orm_connection(), library.id, "Unread Manga", None)
            .await
            .unwrap();

    // Create a book with no progress
    create_test_book(db.sea_orm_connection(), series.id, library.id, 1, 100).await;

    SeriesExternalIdRepository::create(
        db.sea_orm_connection(),
        series.id,
        "api:anilist",
        "999",
        None,
        None,
    )
    .await
    .unwrap();

    let user_id = Uuid::new_v4();

    let entries = push::build_push_entries(
        db.sea_orm_connection(),
        user_id,
        "api:anilist",
        Uuid::new_v4(),
        &default_codex_settings(),
    )
    .await;

    // No progress → should skip
    assert!(entries.is_empty());
}

#[test]
fn test_sync_mode_parsing_default_is_both() {
    // When config has no syncMode key, default to "both"
    let config = serde_json::json!({});
    let sync_mode = config
        .get("syncMode")
        .and_then(|v| v.as_str())
        .unwrap_or("both");
    assert_eq!(sync_mode, "both");
    let do_pull = sync_mode == "both" || sync_mode == "pull";
    let do_push = sync_mode == "both" || sync_mode == "push";
    assert!(do_pull);
    assert!(do_push);
}

#[test]
fn test_sync_mode_parsing_pull_only() {
    let config = serde_json::json!({"syncMode": "pull"});
    let sync_mode = config
        .get("syncMode")
        .and_then(|v| v.as_str())
        .unwrap_or("both");
    assert_eq!(sync_mode, "pull");
    let do_pull = sync_mode == "both" || sync_mode == "pull";
    let do_push = sync_mode == "both" || sync_mode == "push";
    assert!(do_pull);
    assert!(!do_push);
}

#[test]
fn test_sync_mode_parsing_push_only() {
    let config = serde_json::json!({"syncMode": "push"});
    let sync_mode = config
        .get("syncMode")
        .and_then(|v| v.as_str())
        .unwrap_or("both");
    assert_eq!(sync_mode, "push");
    let do_pull = sync_mode == "both" || sync_mode == "pull";
    let do_push = sync_mode == "both" || sync_mode == "push";
    assert!(!do_pull);
    assert!(do_push);
}

#[test]
fn test_sync_mode_parsing_both_explicit() {
    let config = serde_json::json!({"syncMode": "both"});
    let sync_mode = config
        .get("syncMode")
        .and_then(|v| v.as_str())
        .unwrap_or("both");
    assert_eq!(sync_mode, "both");
    let do_pull = sync_mode == "both" || sync_mode == "pull";
    let do_push = sync_mode == "both" || sync_mode == "push";
    assert!(do_pull);
    assert!(do_push);
}

#[test]
fn test_sync_mode_parsing_invalid_value_disables_both() {
    // An unrecognized syncMode value should disable both pull and push
    let config = serde_json::json!({"syncMode": "invalid"});
    let sync_mode = config
        .get("syncMode")
        .and_then(|v| v.as_str())
        .unwrap_or("both");
    assert_eq!(sync_mode, "invalid");
    let do_pull = sync_mode == "both" || sync_mode == "pull";
    let do_push = sync_mode == "both" || sync_mode == "push";
    assert!(!do_pull);
    assert!(!do_push);
}

#[test]
fn test_sync_mode_parsing_non_string_falls_back_to_both() {
    // If syncMode is a non-string value, as_str() returns None → default "both"
    let config = serde_json::json!({"syncMode": 123});
    let sync_mode = config
        .get("syncMode")
        .and_then(|v| v.as_str())
        .unwrap_or("both");
    assert_eq!(sync_mode, "both");
}

#[tokio::test]
async fn test_match_and_apply_empty() {
    let (db, _temp_dir) = create_test_db().await;

    let (matched, applied) = pull::match_and_apply_pulled_entries(
        db.sea_orm_connection(),
        &[],
        Some("api:anilist"),
        Uuid::new_v4(),
        Uuid::new_v4(),
        false,
    )
    .await;
    assert_eq!(matched, 0);
    assert_eq!(applied, 0);
}

#[test]
fn test_codex_settings_defaults() {
    let config = serde_json::json!({});
    let settings = CodexSyncSettings::from_user_config(&config);
    assert!(settings.include_completed);
    assert!(settings.include_in_progress);
    assert!(!settings.count_partial_progress);
    assert!(settings.sync_ratings); // default is now true
}

#[test]
fn test_codex_settings_from_user_config() {
    let config = serde_json::json!({
        "_codex": {
            "includeCompleted": false,
            "includeInProgress": true,
            "countPartialProgress": true,
            "syncRatings": false,
        }
    });
    let settings = CodexSyncSettings::from_user_config(&config);
    assert!(!settings.include_completed);
    assert!(settings.include_in_progress);
    assert!(settings.count_partial_progress);
    assert!(!settings.sync_ratings);
}

#[tokio::test]
async fn test_build_push_entries_skip_completed_series() {
    let (db, _temp_dir) = create_test_db().await;

    let library = LibraryRepository::create(
        db.sea_orm_connection(),
        "Test Library",
        "/test/path",
        ScanningStrategy::Default,
    )
    .await
    .unwrap();

    let series =
        SeriesRepository::create(db.sea_orm_connection(), library.id, "Done Manga 2", None)
            .await
            .unwrap();

    // Create 2 books, mark both as read (= completed)
    let mut test_books = Vec::new();
    for i in 1..=2 {
        let book = create_test_book(db.sea_orm_connection(), series.id, library.id, i, 50).await;
        test_books.push(book);
    }

    let user = create_test_user(db.sea_orm_connection()).await;
    for book in &test_books {
        ReadProgressRepository::mark_as_read(db.sea_orm_connection(), user.id, book.id, 50)
            .await
            .unwrap();
    }

    SeriesExternalIdRepository::create(
        db.sea_orm_connection(),
        series.id,
        "api:anilist",
        "222",
        None,
        None,
    )
    .await
    .unwrap();

    // Disable including completed series
    let settings = CodexSyncSettings {
        include_completed: false,
        ..default_codex_settings()
    };

    let entries = push::build_push_entries(
        db.sea_orm_connection(),
        user.id,
        "api:anilist",
        Uuid::new_v4(),
        &settings,
    )
    .await;

    assert!(entries.is_empty(), "Completed series should be skipped");
}

#[tokio::test]
async fn test_build_push_entries_skip_in_progress_series() {
    let (db, _temp_dir) = create_test_db().await;

    let library = LibraryRepository::create(
        db.sea_orm_connection(),
        "Test Library",
        "/test/path",
        ScanningStrategy::Default,
    )
    .await
    .unwrap();

    let series = SeriesRepository::create(db.sea_orm_connection(), library.id, "WIP Manga", None)
        .await
        .unwrap();

    // Create 3 books, mark only 1 as read (= in-progress)
    let mut test_books = Vec::new();
    for i in 1..=3 {
        let book = create_test_book(db.sea_orm_connection(), series.id, library.id, i, 50).await;
        test_books.push(book);
    }

    let user = create_test_user(db.sea_orm_connection()).await;
    ReadProgressRepository::mark_as_read(db.sea_orm_connection(), user.id, test_books[0].id, 50)
        .await
        .unwrap();

    SeriesExternalIdRepository::create(
        db.sea_orm_connection(),
        series.id,
        "api:anilist",
        "333",
        None,
        None,
    )
    .await
    .unwrap();

    // Disable including in-progress series
    let settings = CodexSyncSettings {
        include_in_progress: false,
        ..default_codex_settings()
    };

    let entries = push::build_push_entries(
        db.sea_orm_connection(),
        user.id,
        "api:anilist",
        Uuid::new_v4(),
        &settings,
    )
    .await;

    assert!(entries.is_empty(), "In-progress series should be skipped");
}

#[tokio::test]
async fn test_build_push_entries_count_in_progress_volumes() {
    let (db, _temp_dir) = create_test_db().await;

    let library = LibraryRepository::create(
        db.sea_orm_connection(),
        "Test Library",
        "/test/path",
        ScanningStrategy::Default,
    )
    .await
    .unwrap();

    let series = SeriesRepository::create(db.sea_orm_connection(), library.id, "IP Manga", None)
        .await
        .unwrap();

    // Create 4 books
    let mut test_books = Vec::new();
    for i in 1..=4 {
        let book = create_test_book(db.sea_orm_connection(), series.id, library.id, i, 100).await;
        test_books.push(book);
    }

    let user = create_test_user(db.sea_orm_connection()).await;

    // Mark book 1 as fully read
    ReadProgressRepository::mark_as_read(db.sea_orm_connection(), user.id, test_books[0].id, 100)
        .await
        .unwrap();

    // Mark book 2 as partially read (in-progress)
    ReadProgressRepository::upsert(
        db.sea_orm_connection(),
        user.id,
        test_books[1].id,
        50,    // current_page
        false, // not completed
    )
    .await
    .unwrap();

    SeriesExternalIdRepository::create(
        db.sea_orm_connection(),
        series.id,
        "api:anilist",
        "444",
        None,
        None,
    )
    .await
    .unwrap();

    // Without partial progress: should count only completed (1)
    let settings = default_codex_settings();
    let entries = push::build_push_entries(
        db.sea_orm_connection(),
        user.id,
        "api:anilist",
        Uuid::new_v4(),
        &settings,
    )
    .await;
    assert_eq!(entries.len(), 1);
    // Server always sends volumes (not chapters)
    assert_eq!(entries[0].progress.as_ref().unwrap().volumes, Some(1));
    assert!(entries[0].progress.as_ref().unwrap().chapters.is_none());

    // With partial progress: should count completed + in-progress (2)
    let settings_with_partial = CodexSyncSettings {
        count_partial_progress: true,
        ..default_codex_settings()
    };
    let entries = push::build_push_entries(
        db.sea_orm_connection(),
        user.id,
        "api:anilist",
        Uuid::new_v4(),
        &settings_with_partial,
    )
    .await;
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].progress.as_ref().unwrap().volumes, Some(2));
    assert!(entries[0].progress.as_ref().unwrap().chapters.is_none());
}

#[tokio::test]
async fn test_apply_pulled_entry_uses_volumes() {
    let (db, _temp_dir) = create_test_db().await;

    let library = LibraryRepository::create(
        db.sea_orm_connection(),
        "Test Library",
        "/test/path",
        ScanningStrategy::Default,
    )
    .await
    .unwrap();

    let series = SeriesRepository::create(db.sea_orm_connection(), library.id, "Vol Manga", None)
        .await
        .unwrap();

    // Create 5 books
    for i in 1..=5 {
        create_test_book(db.sea_orm_connection(), series.id, library.id, i, 100).await;
    }

    let user = create_test_user(db.sea_orm_connection()).await;

    // Pull entry with volumes=2 (no chapters)
    let entry = SyncEntry {
        external_id: "55".to_string(),
        status: SyncReadingStatus::Reading,
        progress: Some(SyncProgress {
            chapters: None,
            volumes: Some(2),
            pages: None,
            total_chapters: None,
            total_volumes: None,
        }),
        score: None,
        started_at: None,
        completed_at: None,
        notes: None,
        latest_updated_at: None,
        title: None,
    };

    // Build pre-fetched maps for apply_pulled_entry (via match_and_apply which calls it)
    // We test via match_and_apply since apply_pulled_entry is private to pull module
    SeriesExternalIdRepository::create(
        db.sea_orm_connection(),
        series.id,
        "api:anilist",
        "55",
        None,
        None,
    )
    .await
    .unwrap();

    let (matched, applied) = pull::match_and_apply_pulled_entries(
        db.sea_orm_connection(),
        &[entry],
        Some("api:anilist"),
        user.id,
        Uuid::new_v4(),
        false,
    )
    .await;
    assert_eq!(matched, 1);
    assert_eq!(applied, 2);

    // Verify first 2 books are marked as read
    let books = BookRepository::list_by_series(db.sea_orm_connection(), series.id, false)
        .await
        .unwrap();
    for (i, book) in books.iter().enumerate() {
        let progress =
            ReadProgressRepository::get_by_user_and_book(db.sea_orm_connection(), user.id, book.id)
                .await
                .unwrap();
        if i < 2 {
            assert!(progress.is_some(), "Book {} should have progress", i);
            assert!(
                progress.unwrap().completed,
                "Book {} should be completed",
                i
            );
        } else {
            assert!(progress.is_none(), "Book {} should have no progress", i);
        }
    }
}

// =========================================================================
// Rating sync tests
// =========================================================================

#[test]
fn test_codex_settings_sync_ratings_default() {
    let config = serde_json::json!({});
    let settings = CodexSyncSettings::from_user_config(&config);
    assert!(settings.sync_ratings); // default is now true
}

#[test]
fn test_codex_settings_sync_ratings_disabled() {
    let config = serde_json::json!({"_codex": {"syncRatings": false}});
    let settings = CodexSyncSettings::from_user_config(&config);
    assert!(!settings.sync_ratings);
}

#[tokio::test]
async fn test_build_push_entries_includes_rating() {
    let (db, _temp_dir) = create_test_db().await;

    let library = LibraryRepository::create(
        db.sea_orm_connection(),
        "Test Library",
        "/test/path",
        ScanningStrategy::Default,
    )
    .await
    .unwrap();

    let series = SeriesRepository::create(db.sea_orm_connection(), library.id, "Rated Manga", None)
        .await
        .unwrap();

    let book = create_test_book(db.sea_orm_connection(), series.id, library.id, 1, 100).await;

    let user = create_test_user(db.sea_orm_connection()).await;

    ReadProgressRepository::mark_as_read(db.sea_orm_connection(), user.id, book.id, 100)
        .await
        .unwrap();

    // Create a rating for this series
    UserSeriesRatingRepository::create(
        db.sea_orm_connection(),
        user.id,
        series.id,
        85,
        Some("Excellent manga!".to_string()),
    )
    .await
    .unwrap();

    SeriesExternalIdRepository::create(
        db.sea_orm_connection(),
        series.id,
        "api:anilist",
        "555",
        None,
        None,
    )
    .await
    .unwrap();

    let settings = default_codex_settings(); // sync_ratings=true by default

    let entries = push::build_push_entries(
        db.sea_orm_connection(),
        user.id,
        "api:anilist",
        Uuid::new_v4(),
        &settings,
    )
    .await;

    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].score, Some(85.0));
    assert_eq!(entries[0].notes, Some("Excellent manga!".to_string()));
}

#[tokio::test]
async fn test_build_push_entries_no_rating_when_disabled() {
    let (db, _temp_dir) = create_test_db().await;

    let library = LibraryRepository::create(
        db.sea_orm_connection(),
        "Test Library",
        "/test/path",
        ScanningStrategy::Default,
    )
    .await
    .unwrap();

    let series =
        SeriesRepository::create(db.sea_orm_connection(), library.id, "Rated Manga 2", None)
            .await
            .unwrap();

    let book = create_test_book(db.sea_orm_connection(), series.id, library.id, 1, 100).await;

    let user = create_test_user(db.sea_orm_connection()).await;

    ReadProgressRepository::mark_as_read(db.sea_orm_connection(), user.id, book.id, 100)
        .await
        .unwrap();

    // Create a rating, but sync_ratings is false
    UserSeriesRatingRepository::create(db.sea_orm_connection(), user.id, series.id, 85, None)
        .await
        .unwrap();

    SeriesExternalIdRepository::create(
        db.sea_orm_connection(),
        series.id,
        "api:anilist",
        "556",
        None,
        None,
    )
    .await
    .unwrap();

    let settings = CodexSyncSettings {
        sync_ratings: false,
        ..default_codex_settings()
    };

    let entries = push::build_push_entries(
        db.sea_orm_connection(),
        user.id,
        "api:anilist",
        Uuid::new_v4(),
        &settings,
    )
    .await;

    assert_eq!(entries.len(), 1);
    assert!(entries[0].score.is_none());
    assert!(entries[0].notes.is_none());
}

#[tokio::test]
async fn test_build_push_entries_no_rating_for_unrated() {
    let (db, _temp_dir) = create_test_db().await;

    let library = LibraryRepository::create(
        db.sea_orm_connection(),
        "Test Library",
        "/test/path",
        ScanningStrategy::Default,
    )
    .await
    .unwrap();

    let series =
        SeriesRepository::create(db.sea_orm_connection(), library.id, "Unrated Manga", None)
            .await
            .unwrap();

    let book = create_test_book(db.sea_orm_connection(), series.id, library.id, 1, 100).await;

    let user = create_test_user(db.sea_orm_connection()).await;

    ReadProgressRepository::mark_as_read(db.sea_orm_connection(), user.id, book.id, 100)
        .await
        .unwrap();

    // No rating created

    SeriesExternalIdRepository::create(
        db.sea_orm_connection(),
        series.id,
        "api:anilist",
        "557",
        None,
        None,
    )
    .await
    .unwrap();

    let settings = default_codex_settings(); // sync_ratings=true by default

    let entries = push::build_push_entries(
        db.sea_orm_connection(),
        user.id,
        "api:anilist",
        Uuid::new_v4(),
        &settings,
    )
    .await;

    assert_eq!(entries.len(), 1);
    assert!(entries[0].score.is_none());
    assert!(entries[0].notes.is_none());
}

#[tokio::test]
async fn test_apply_pulled_rating_no_existing() {
    let (db, _temp_dir) = create_test_db().await;

    let library = LibraryRepository::create(
        db.sea_orm_connection(),
        "Test Library",
        "/test/path",
        ScanningStrategy::Default,
    )
    .await
    .unwrap();

    let series = SeriesRepository::create(db.sea_orm_connection(), library.id, "Pull Manga", None)
        .await
        .unwrap();

    create_test_book(db.sea_orm_connection(), series.id, library.id, 1, 100).await;

    let user = create_test_user(db.sea_orm_connection()).await;

    SeriesExternalIdRepository::create(
        db.sea_orm_connection(),
        series.id,
        "api:anilist",
        "600",
        None,
        None,
    )
    .await
    .unwrap();

    let entries = vec![SyncEntry {
        external_id: "600".to_string(),
        status: SyncReadingStatus::Reading,
        progress: Some(SyncProgress {
            chapters: Some(1),
            volumes: None,
            pages: None,
            total_chapters: None,
            total_volumes: None,
        }),
        score: Some(75.0),
        started_at: None,
        completed_at: None,
        notes: Some("Good so far".to_string()),
        latest_updated_at: None,
        title: None,
    }];

    let (matched, _applied) = pull::match_and_apply_pulled_entries(
        db.sea_orm_connection(),
        &entries,
        Some("api:anilist"),
        user.id,
        Uuid::new_v4(),
        true, // sync_ratings=true
    )
    .await;

    assert_eq!(matched, 1);

    // Verify rating was created
    let rating = UserSeriesRatingRepository::get_by_user_and_series(
        db.sea_orm_connection(),
        user.id,
        series.id,
    )
    .await
    .unwrap();
    assert!(rating.is_some());
    let rating = rating.unwrap();
    assert_eq!(rating.rating, 75);
    assert_eq!(rating.notes, Some("Good so far".to_string()));
}

#[tokio::test]
async fn test_apply_pulled_rating_existing_not_overwritten() {
    let (db, _temp_dir) = create_test_db().await;

    let library = LibraryRepository::create(
        db.sea_orm_connection(),
        "Test Library",
        "/test/path",
        ScanningStrategy::Default,
    )
    .await
    .unwrap();

    let series =
        SeriesRepository::create(db.sea_orm_connection(), library.id, "Rated Manga 3", None)
            .await
            .unwrap();

    create_test_book(db.sea_orm_connection(), series.id, library.id, 1, 100).await;

    let user = create_test_user(db.sea_orm_connection()).await;

    // Pre-create a Codex rating
    UserSeriesRatingRepository::create(
        db.sea_orm_connection(),
        user.id,
        series.id,
        90,
        Some("My notes".to_string()),
    )
    .await
    .unwrap();

    SeriesExternalIdRepository::create(
        db.sea_orm_connection(),
        series.id,
        "api:anilist",
        "601",
        None,
        None,
    )
    .await
    .unwrap();

    // Pull entry with different score
    let entries = vec![SyncEntry {
        external_id: "601".to_string(),
        status: SyncReadingStatus::Reading,
        progress: Some(SyncProgress {
            chapters: Some(1),
            volumes: None,
            pages: None,
            total_chapters: None,
            total_volumes: None,
        }),
        score: Some(60.0),
        started_at: None,
        completed_at: None,
        notes: Some("AniList notes".to_string()),
        latest_updated_at: None,
        title: None,
    }];

    let (_matched, _applied) = pull::match_and_apply_pulled_entries(
        db.sea_orm_connection(),
        &entries,
        Some("api:anilist"),
        user.id,
        Uuid::new_v4(),
        true,
    )
    .await;

    // Verify Codex rating was NOT overwritten
    let rating = UserSeriesRatingRepository::get_by_user_and_series(
        db.sea_orm_connection(),
        user.id,
        series.id,
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(rating.rating, 90); // Original Codex rating preserved
    assert_eq!(rating.notes, Some("My notes".to_string()));
}

#[tokio::test]
async fn test_apply_pulled_rating_disabled() {
    let (db, _temp_dir) = create_test_db().await;

    let library = LibraryRepository::create(
        db.sea_orm_connection(),
        "Test Library",
        "/test/path",
        ScanningStrategy::Default,
    )
    .await
    .unwrap();

    let series =
        SeriesRepository::create(db.sea_orm_connection(), library.id, "No Sync Manga", None)
            .await
            .unwrap();

    create_test_book(db.sea_orm_connection(), series.id, library.id, 1, 100).await;

    let user = create_test_user(db.sea_orm_connection()).await;

    SeriesExternalIdRepository::create(
        db.sea_orm_connection(),
        series.id,
        "api:anilist",
        "602",
        None,
        None,
    )
    .await
    .unwrap();

    let entries = vec![SyncEntry {
        external_id: "602".to_string(),
        status: SyncReadingStatus::Reading,
        progress: Some(SyncProgress {
            chapters: Some(1),
            volumes: None,
            pages: None,
            total_chapters: None,
            total_volumes: None,
        }),
        score: Some(80.0),
        started_at: None,
        completed_at: None,
        notes: Some("Should not be stored".to_string()),
        latest_updated_at: None,
        title: None,
    }];

    let (_matched, _applied) = pull::match_and_apply_pulled_entries(
        db.sea_orm_connection(),
        &entries,
        Some("api:anilist"),
        user.id,
        Uuid::new_v4(),
        false, // sync_ratings=false
    )
    .await;

    // Verify no rating was created
    let rating = UserSeriesRatingRepository::get_by_user_and_series(
        db.sea_orm_connection(),
        user.id,
        series.id,
    )
    .await
    .unwrap();
    assert!(rating.is_none());
}

// =========================================================================
// New tests: latestUpdatedAt, totalVolumes, always-sends-volumes
// =========================================================================

#[tokio::test]
async fn test_build_push_entries_populates_latest_updated_at() {
    let (db, _temp_dir) = create_test_db().await;

    let library = LibraryRepository::create(
        db.sea_orm_connection(),
        "Test Library",
        "/test/path",
        ScanningStrategy::Default,
    )
    .await
    .unwrap();

    let series =
        SeriesRepository::create(db.sea_orm_connection(), library.id, "Updated Manga", None)
            .await
            .unwrap();

    let book = create_test_book(db.sea_orm_connection(), series.id, library.id, 1, 100).await;

    let user = create_test_user(db.sea_orm_connection()).await;

    ReadProgressRepository::mark_as_read(db.sea_orm_connection(), user.id, book.id, 100)
        .await
        .unwrap();

    SeriesExternalIdRepository::create(
        db.sea_orm_connection(),
        series.id,
        "api:anilist",
        "800",
        None,
        None,
    )
    .await
    .unwrap();

    let entries = push::build_push_entries(
        db.sea_orm_connection(),
        user.id,
        "api:anilist",
        Uuid::new_v4(),
        &default_codex_settings(),
    )
    .await;

    assert_eq!(entries.len(), 1);
    assert!(
        entries[0].latest_updated_at.is_some(),
        "latestUpdatedAt should be populated when there is reading progress"
    );
}

#[tokio::test]
async fn test_build_push_entries_populates_total_volumes() {
    let (db, _temp_dir) = create_test_db().await;

    let library = LibraryRepository::create(
        db.sea_orm_connection(),
        "Test Library",
        "/test/path",
        ScanningStrategy::Default,
    )
    .await
    .unwrap();

    let series = SeriesRepository::create(db.sea_orm_connection(), library.id, "Total Manga", None)
        .await
        .unwrap();

    // Create 2 books
    let mut test_books = Vec::new();
    for i in 1..=2 {
        let book = create_test_book(db.sea_orm_connection(), series.id, library.id, i, 100).await;
        test_books.push(book);
    }

    // Set total_book_count=3 in metadata (more than the 2 local books)
    SeriesMetadataRepository::update_total_book_count(db.sea_orm_connection(), series.id, Some(3))
        .await
        .unwrap();

    let user = create_test_user(db.sea_orm_connection()).await;

    // Mark 1 book as read
    ReadProgressRepository::mark_as_read(db.sea_orm_connection(), user.id, test_books[0].id, 100)
        .await
        .unwrap();

    SeriesExternalIdRepository::create(
        db.sea_orm_connection(),
        series.id,
        "api:anilist",
        "801",
        None,
        None,
    )
    .await
    .unwrap();

    let entries = push::build_push_entries(
        db.sea_orm_connection(),
        user.id,
        "api:anilist",
        Uuid::new_v4(),
        &default_codex_settings(),
    )
    .await;

    assert_eq!(entries.len(), 1);
    assert_eq!(
        entries[0].progress.as_ref().unwrap().total_volumes,
        Some(3),
        "totalVolumes should come from series metadata total_book_count"
    );
}

#[tokio::test]
async fn test_build_push_entries_always_sends_volumes() {
    let (db, _temp_dir) = create_test_db().await;

    let library = LibraryRepository::create(
        db.sea_orm_connection(),
        "Test Library",
        "/test/path",
        ScanningStrategy::Default,
    )
    .await
    .unwrap();

    let series =
        SeriesRepository::create(db.sea_orm_connection(), library.id, "Volumes Manga", None)
            .await
            .unwrap();

    let mut test_books = Vec::new();
    for i in 1..=3 {
        let book = create_test_book(db.sea_orm_connection(), series.id, library.id, i, 100).await;
        test_books.push(book);
    }

    let user = create_test_user(db.sea_orm_connection()).await;

    ReadProgressRepository::mark_as_read(db.sea_orm_connection(), user.id, test_books[0].id, 100)
        .await
        .unwrap();
    ReadProgressRepository::mark_as_read(db.sea_orm_connection(), user.id, test_books[1].id, 100)
        .await
        .unwrap();

    SeriesExternalIdRepository::create(
        db.sea_orm_connection(),
        series.id,
        "api:anilist",
        "802",
        None,
        None,
    )
    .await
    .unwrap();

    let entries = push::build_push_entries(
        db.sea_orm_connection(),
        user.id,
        "api:anilist",
        Uuid::new_v4(),
        &default_codex_settings(),
    )
    .await;

    assert_eq!(entries.len(), 1);
    let progress = entries[0].progress.as_ref().unwrap();
    assert_eq!(
        progress.volumes,
        Some(2),
        "Server should always send books-read as volumes"
    );
    assert!(
        progress.chapters.is_none(),
        "chapters should be None — server always sends volumes"
    );
}

// =========================================================================
// search_fallback settings and unmatched entries tests
// =========================================================================

#[test]
fn test_codex_settings_search_fallback_default() {
    let config = serde_json::json!({});
    let settings = CodexSyncSettings::from_user_config(&config);
    assert!(
        !settings.search_fallback,
        "search_fallback should default to false"
    );
}

#[test]
fn test_codex_settings_search_fallback_enabled() {
    let config = serde_json::json!({"_codex": {"searchFallback": true}});
    let settings = CodexSyncSettings::from_user_config(&config);
    assert!(settings.search_fallback);
}

#[test]
fn test_codex_settings_search_fallback_disabled() {
    let config = serde_json::json!({"_codex": {"searchFallback": false}});
    let settings = CodexSyncSettings::from_user_config(&config);
    assert!(!settings.search_fallback);
}

#[tokio::test]
async fn test_build_push_entries_includes_unmatched_with_search_fallback() {
    let (db, _temp_dir) = create_test_db().await;

    let library = LibraryRepository::create(
        db.sea_orm_connection(),
        "Test Library",
        "/test/path",
        ScanningStrategy::Default,
    )
    .await
    .unwrap();

    // Series A: has external ID
    let series_a =
        SeriesRepository::create(db.sea_orm_connection(), library.id, "Matched Manga", None)
            .await
            .unwrap();

    SeriesExternalIdRepository::create(
        db.sea_orm_connection(),
        series_a.id,
        "api:anilist",
        "100",
        None,
        None,
    )
    .await
    .unwrap();

    let book_a = create_test_book(db.sea_orm_connection(), series_a.id, library.id, 1, 50).await;

    // Series B: NO external ID, but has metadata title and reading progress
    let series_b =
        SeriesRepository::create(db.sea_orm_connection(), library.id, "Unmatched Manga", None)
            .await
            .unwrap();

    let book_b = create_test_book(db.sea_orm_connection(), series_b.id, library.id, 1, 100).await;

    let user = create_test_user(db.sea_orm_connection()).await;

    // Mark books as read in both series
    ReadProgressRepository::mark_as_read(db.sea_orm_connection(), user.id, book_a.id, 50)
        .await
        .unwrap();
    ReadProgressRepository::mark_as_read(db.sea_orm_connection(), user.id, book_b.id, 100)
        .await
        .unwrap();

    // With search_fallback=false: only matched series
    let settings_no_fallback = default_codex_settings();
    let entries = push::build_push_entries(
        db.sea_orm_connection(),
        user.id,
        "api:anilist",
        Uuid::new_v4(),
        &settings_no_fallback,
    )
    .await;
    assert_eq!(
        entries.len(),
        1,
        "Only matched series without search_fallback"
    );
    assert_eq!(entries[0].external_id, "100");

    // With search_fallback=true: matched + unmatched series
    let settings_with_fallback = CodexSyncSettings {
        search_fallback: true,
        ..default_codex_settings()
    };
    let entries = push::build_push_entries(
        db.sea_orm_connection(),
        user.id,
        "api:anilist",
        Uuid::new_v4(),
        &settings_with_fallback,
    )
    .await;
    assert_eq!(
        entries.len(),
        2,
        "Both matched and unmatched with search_fallback"
    );

    // Check the unmatched entry
    let unmatched = entries.iter().find(|e| e.external_id.is_empty());
    assert!(
        unmatched.is_some(),
        "Unmatched entry should have empty external_id"
    );
    let unmatched = unmatched.unwrap();
    assert_eq!(
        unmatched.title,
        Some("Unmatched Manga".to_string()),
        "Unmatched entry should have title from metadata"
    );
    assert!(unmatched.progress.is_some());
    assert_eq!(unmatched.progress.as_ref().unwrap().volumes, Some(1));
}

#[tokio::test]
async fn test_build_push_entries_unmatched_skips_no_metadata() {
    let (db, _temp_dir) = create_test_db().await;

    let library = LibraryRepository::create(
        db.sea_orm_connection(),
        "Test Library",
        "/test/path",
        ScanningStrategy::Default,
    )
    .await
    .unwrap();

    // Series with NO external ID and reading progress but also no series metadata title
    // Note: SeriesRepository::create auto-creates metadata with the series name,
    // so this series will have metadata. We test that it shows up.
    let series = SeriesRepository::create(db.sea_orm_connection(), library.id, "Has Title", None)
        .await
        .unwrap();

    let book = create_test_book(db.sea_orm_connection(), series.id, library.id, 1, 50).await;

    let user = create_test_user(db.sea_orm_connection()).await;
    ReadProgressRepository::mark_as_read(db.sea_orm_connection(), user.id, book.id, 50)
        .await
        .unwrap();

    let settings = CodexSyncSettings {
        search_fallback: true,
        ..default_codex_settings()
    };

    let entries = push::build_push_entries(
        db.sea_orm_connection(),
        user.id,
        "api:anilist",
        Uuid::new_v4(),
        &settings,
    )
    .await;

    // Should include the unmatched entry since it has metadata (from series creation)
    assert_eq!(entries.len(), 1);
    assert!(entries[0].external_id.is_empty());
    assert_eq!(entries[0].title, Some("Has Title".to_string()));
}

#[tokio::test]
async fn test_build_push_entries_populates_title_for_matched() {
    let (db, _temp_dir) = create_test_db().await;

    let library = LibraryRepository::create(
        db.sea_orm_connection(),
        "Test Library",
        "/test/path",
        ScanningStrategy::Default,
    )
    .await
    .unwrap();

    let series = SeriesRepository::create(
        db.sea_orm_connection(),
        library.id,
        "Title Test Manga",
        None,
    )
    .await
    .unwrap();

    let book = create_test_book(db.sea_orm_connection(), series.id, library.id, 1, 100).await;

    let user = create_test_user(db.sea_orm_connection()).await;
    ReadProgressRepository::mark_as_read(db.sea_orm_connection(), user.id, book.id, 100)
        .await
        .unwrap();

    SeriesExternalIdRepository::create(
        db.sea_orm_connection(),
        series.id,
        "api:anilist",
        "900",
        None,
        None,
    )
    .await
    .unwrap();

    let entries = push::build_push_entries(
        db.sea_orm_connection(),
        user.id,
        "api:anilist",
        Uuid::new_v4(),
        &default_codex_settings(),
    )
    .await;

    assert_eq!(entries.len(), 1);
    assert_eq!(
        entries[0].title,
        Some("Title Test Manga".to_string()),
        "Matched entries should also have title populated from metadata"
    );
}
