#[path = "../common/mod.rs"]
mod common;

use chrono::Utc;
use codex::config::{DatabaseConfig, DatabaseType, SQLiteConfig};
use codex::db::entities::{books, libraries, read_progress, series, users};
use codex::db::repositories::{
    BookRepository, LibraryRepository, PageRepository, SeriesMetadataRepository, SeriesRepository,
};
use codex::db::Database;
use codex::models::ScanningStrategy;
use common::*;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, IntoActiveModel, PaginatorTrait, QueryFilter, Set,
};
use uuid::Uuid;

// ============================================================================
// Library CRUD Tests
// ============================================================================

#[tokio::test]
async fn test_library_insert_and_select() {
    let (db, _temp_dir) = setup_test_db_wrapper().await;
    let conn = db.sea_orm_connection();

    // Create library using repository
    let library = LibraryRepository::create(
        conn,
        "Test Library",
        "/test/path",
        ScanningStrategy::Default,
    )
    .await
    .unwrap();

    // Select using repository
    let retrieved = LibraryRepository::get_by_id(conn, library.id)
        .await
        .unwrap()
        .unwrap();

    assert_eq!(retrieved.id, library.id);
    assert_eq!(retrieved.name, "Test Library");
    assert_eq!(retrieved.path, "/test/path");

    db.close().await;
}

#[tokio::test]
async fn test_library_update() {
    let (db, _temp_dir) = setup_test_db_wrapper().await;
    let conn = db.sea_orm_connection();

    // Create library
    let library = LibraryRepository::create(
        conn,
        "Original Name",
        "/original/path",
        ScanningStrategy::Default,
    )
    .await
    .unwrap();

    // Update using repository
    let mut updated_library = library.clone();
    updated_library.name = "Updated Name".to_string();
    LibraryRepository::update(conn, &updated_library)
        .await
        .unwrap();

    // Verify
    let retrieved = LibraryRepository::get_by_id(conn, library.id)
        .await
        .unwrap()
        .unwrap();

    assert_eq!(retrieved.name, "Updated Name");

    db.close().await;
}

#[tokio::test]
async fn test_library_delete() {
    let (db, _temp_dir) = setup_test_db_wrapper().await;
    let conn = db.sea_orm_connection();

    // Create library
    let library =
        LibraryRepository::create(conn, "To Delete", "/delete/path", ScanningStrategy::Default)
            .await
            .unwrap();

    // Delete
    LibraryRepository::delete(conn, library.id).await.unwrap();

    // Verify deleted
    let count = libraries::Entity::find()
        .filter(libraries::Column::Id.eq(library.id))
        .count(conn)
        .await
        .unwrap();

    assert_eq!(count, 0);

    db.close().await;
}

// ============================================================================
// Series and Book Relationship Tests
// ============================================================================

#[tokio::test]
async fn test_series_book_relationship() {
    let (db, _temp_dir) = setup_test_db_wrapper().await;
    let conn = db.sea_orm_connection();

    // Create library
    let library =
        LibraryRepository::create(conn, "Test Library", "/test", ScanningStrategy::Default)
            .await
            .unwrap();

    // Create series
    let series = SeriesRepository::create(conn, library.id, "Test Series", None)
        .await
        .unwrap();

    // Create book
    let book_model = create_test_book(
        series.id,
        library.id,
        "/test/book.cbz",
        "book.cbz",
        "test_hash",
        "cbz",
        10,
    );

    let book = BookRepository::create(conn, &book_model, None)
        .await
        .unwrap();

    // Query book with series join using SeaORM
    let book_with_series = books::Entity::find_by_id(book.id)
        .find_also_related(series::Entity)
        .one(conn)
        .await
        .unwrap()
        .unwrap();

    let (book_result, series_result) = book_with_series;
    assert_eq!(book_result.id, book.id);
    assert_eq!(book_result.file_name, "book.cbz");
    // Series name is now in series_metadata table
    assert!(series_result.is_some());

    db.close().await;
}

#[tokio::test]
async fn test_cascade_delete_library_to_series() {
    let (db, _temp_dir) = setup_test_db_wrapper().await;
    let conn = db.sea_orm_connection();

    // Create library and series
    let library =
        LibraryRepository::create(conn, "Test Library", "/test", ScanningStrategy::Default)
            .await
            .unwrap();

    let series = SeriesRepository::create(conn, library.id, "Test Series", None)
        .await
        .unwrap();

    // Delete library (should cascade to series)
    LibraryRepository::delete(conn, library.id).await.unwrap();

    // Verify series was also deleted
    let count = series::Entity::find()
        .filter(series::Column::Id.eq(series.id))
        .count(conn)
        .await
        .unwrap();

    assert_eq!(count, 0);

    db.close().await;
}

// ============================================================================
// User and Read Progress Tests
// ============================================================================

#[tokio::test]
async fn test_user_read_progress() {
    let (db, _temp_dir) = setup_test_db_wrapper().await;
    let conn = db.sea_orm_connection();

    // Create user using ActiveModel
    let user = create_test_user("testuser", "test@example.com", "hashed_password", false);
    let user = user.into_active_model().insert(conn).await.unwrap();

    // Create library, series, and book for progress tracking
    let library = LibraryRepository::create(conn, "Lib", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    let series = SeriesRepository::create(conn, library.id, "Series", None)
        .await
        .unwrap();

    let book_model = create_test_book(
        series.id,
        library.id,
        "/book.cbz",
        "book.cbz",
        "test_hash",
        "cbz",
        10,
    );
    let book = BookRepository::create(conn, &book_model, None)
        .await
        .unwrap();

    // Create read progress using ActiveModel
    let progress = read_progress::ActiveModel {
        id: Set(Uuid::new_v4()),
        user_id: Set(user.id),
        book_id: Set(book.id),
        current_page: Set(1),
        progress_percentage: Set(None),
        completed: Set(false),
        started_at: Set(Utc::now()),
        updated_at: Set(Utc::now()),
        completed_at: Set(None),
    };
    let progress = progress.insert(conn).await.unwrap();

    // Query progress with user and book info using SeaORM
    let progress_result = read_progress::Entity::find_by_id(progress.id)
        .find_also_related(users::Entity)
        .one(conn)
        .await
        .unwrap()
        .unwrap();

    let (progress_model, user_result) = progress_result;
    let book_result = books::Entity::find_by_id(book.id)
        .one(conn)
        .await
        .unwrap()
        .unwrap();

    assert_eq!(user_result.unwrap().username, "testuser");
    assert_eq!(book_result.file_name, "book.cbz");
    assert_eq!(progress_model.current_page, 1);

    db.close().await;
}

// ============================================================================
// Constraint Tests
// ============================================================================

#[tokio::test]
async fn test_unique_username_constraint() {
    let (db, _temp_dir) = setup_test_db_wrapper().await;
    let conn = db.sea_orm_connection();

    // Create first user
    let user1 = create_test_user("testuser", "test1@example.com", "hash1", false);
    user1.into_active_model().insert(conn).await.unwrap();

    // Try to insert second user with same username (should fail)
    let user2 = create_test_user("testuser", "test2@example.com", "hash2", false);

    let result = user2.into_active_model().insert(conn).await;
    assert!(result.is_err());

    db.close().await;
}

#[tokio::test]
async fn test_unique_file_path_constraint() {
    let (db, _temp_dir) = setup_test_db_wrapper().await;
    let conn = db.sea_orm_connection();

    // Setup library and series
    let library = LibraryRepository::create(conn, "Lib", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    let series = SeriesRepository::create(conn, library.id, "Series", None)
        .await
        .unwrap();

    // Insert first book
    let book1_model = create_test_book(
        series.id,
        library.id,
        "/same/path/book.cbz",
        "book.cbz",
        "hash1",
        "cbz",
        10,
    );
    BookRepository::create(conn, &book1_model, None)
        .await
        .unwrap();

    // Try to insert second book with same file path in same library (should fail)
    let book2_model = create_test_book(
        series.id,
        library.id,
        "/same/path/book.cbz",
        "book.cbz",
        "hash2",
        "cbz",
        10,
    );

    let result = BookRepository::create(conn, &book2_model, None).await;
    assert!(result.is_err());

    // But same file path in different library should succeed
    let library2 = LibraryRepository::create(conn, "Lib2", "/lib2", ScanningStrategy::Default)
        .await
        .unwrap();
    let series2 = SeriesRepository::create(conn, library2.id, "Series2", None)
        .await
        .unwrap();

    let book3_model = create_test_book(
        series2.id,
        library2.id,
        "/same/path/book.cbz", // Same path, different library
        "book.cbz",
        "hash3",
        "cbz",
        10,
    );

    let result = BookRepository::create(conn, &book3_model, None).await;
    assert!(result.is_ok()); // Should succeed - different library

    db.close().await;
}

// ============================================================================
// Index Performance Tests
// ============================================================================

#[tokio::test]
async fn test_file_hash_index_lookup() {
    let (db, _temp_dir) = setup_test_db_wrapper().await;
    let conn = db.sea_orm_connection();

    // Setup
    let library = LibraryRepository::create(conn, "Lib", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    let series = SeriesRepository::create(conn, library.id, "Series", None)
        .await
        .unwrap();

    // Insert book with specific hash
    let test_hash = "abc123hash";
    let book_model = create_test_book(
        series.id,
        library.id,
        "/book1.cbz",
        "book1.cbz",
        test_hash,
        "cbz",
        10,
    );
    BookRepository::create(conn, &book_model, None)
        .await
        .unwrap();

    // Query by hash (should use index)
    let result = books::Entity::find()
        .filter(books::Column::FileHash.eq(test_hash))
        .one(conn)
        .await
        .unwrap()
        .unwrap();

    assert_eq!(result.file_name, "book1.cbz");

    db.close().await;
}

// ============================================================================
// Page Tests
// ============================================================================

#[tokio::test]
async fn test_pages_insert_and_query() {
    let (db, _temp_dir) = setup_test_db_wrapper().await;
    let conn = db.sea_orm_connection();

    // Setup
    let library = LibraryRepository::create(conn, "Lib", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    let series = SeriesRepository::create(conn, library.id, "Series", None)
        .await
        .unwrap();

    let book_model = create_test_book(
        series.id,
        library.id,
        "/book.cbz",
        "book.cbz",
        "test_hash",
        "cbz",
        10,
    );
    let book = BookRepository::create(conn, &book_model, None)
        .await
        .unwrap();

    // Insert pages
    for i in 1..=3 {
        let page_model = create_test_page(book.id, i, &format!("page{:03}.jpg", i), "jpeg");
        PageRepository::create(conn, &page_model).await.unwrap();
    }

    // Query pages ordered by page_number
    let pages_result = PageRepository::list_by_book(conn, book.id).await.unwrap();

    assert_eq!(pages_result.len(), 3);
    assert_eq!(pages_result[0].page_number, 1);
    assert_eq!(pages_result[0].file_name, "page001.jpg");
    assert_eq!(pages_result[2].page_number, 3);
    assert_eq!(pages_result[2].file_name, "page003.jpg");

    db.close().await;
}

// ============================================================================
// Soft Delete Tests
// ============================================================================

#[tokio::test]
async fn test_mark_book_deleted() {
    let (db, _temp_dir) = setup_test_db_wrapper().await;
    let conn = db.sea_orm_connection();

    // Setup
    let library = LibraryRepository::create(conn, "Lib", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    let series = SeriesRepository::create(conn, library.id, "Series", None)
        .await
        .unwrap();

    let book_model = create_test_book(
        series.id,
        library.id,
        "/book.cbz",
        "book.cbz",
        "test_hash",
        "cbz",
        10,
    );
    let book = BookRepository::create(conn, &book_model, None)
        .await
        .unwrap();

    // Verify book is not deleted initially
    assert!(!book.deleted);

    // Mark book as deleted
    BookRepository::mark_deleted(conn, book.id, true, None)
        .await
        .unwrap();

    // Verify book is marked deleted
    let updated_book = books::Entity::find_by_id(book.id)
        .one(conn)
        .await
        .unwrap()
        .unwrap();

    assert!(updated_book.deleted);
    assert!(updated_book.updated_at > book.updated_at);

    db.close().await;
}

#[tokio::test]
async fn test_restore_deleted_book() {
    let (db, _temp_dir) = setup_test_db_wrapper().await;
    let conn = db.sea_orm_connection();

    // Setup
    let library = LibraryRepository::create(conn, "Lib", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    let series = SeriesRepository::create(conn, library.id, "Series", None)
        .await
        .unwrap();

    let book_model = create_test_book(
        series.id,
        library.id,
        "/book.cbz",
        "book.cbz",
        "test_hash",
        "cbz",
        10,
    );
    let book = BookRepository::create(conn, &book_model, None)
        .await
        .unwrap();

    // Mark book as deleted
    BookRepository::mark_deleted(conn, book.id, true, None)
        .await
        .unwrap();

    let deleted_book = books::Entity::find_by_id(book.id)
        .one(conn)
        .await
        .unwrap()
        .unwrap();
    assert!(deleted_book.deleted);

    // Restore the book
    BookRepository::mark_deleted(conn, book.id, false, None)
        .await
        .unwrap();

    // Verify book is restored
    let restored_book = books::Entity::find_by_id(book.id)
        .one(conn)
        .await
        .unwrap()
        .unwrap();

    assert!(!restored_book.deleted);
    assert!(restored_book.updated_at > deleted_book.updated_at);

    db.close().await;
}

#[tokio::test]
async fn test_list_by_series_filters_deleted_by_default() {
    let (db, _temp_dir) = setup_test_db_wrapper().await;
    let conn = db.sea_orm_connection();

    // Setup
    let library = LibraryRepository::create(conn, "Lib", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    let series = SeriesRepository::create(conn, library.id, "Series", None)
        .await
        .unwrap();

    // Create two books
    let book1_model = create_test_book(
        series.id,
        library.id,
        "/book1.cbz",
        "book1.cbz",
        "hash1",
        "cbz",
        10,
    );
    let book1 = BookRepository::create(conn, &book1_model, None)
        .await
        .unwrap();

    let book2_model = create_test_book(
        series.id,
        library.id,
        "/book2.cbz",
        "book2.cbz",
        "hash2",
        "cbz",
        10,
    );
    let book2 = BookRepository::create(conn, &book2_model, None)
        .await
        .unwrap();

    // Mark book1 as deleted
    BookRepository::mark_deleted(conn, book1.id, true, None)
        .await
        .unwrap();

    // List books without including deleted (default behavior)
    let books = BookRepository::list_by_series(conn, series.id, false)
        .await
        .unwrap();

    // Should only return book2
    assert_eq!(books.len(), 1);
    assert_eq!(books[0].id, book2.id);
    assert!(!books[0].deleted);

    db.close().await;
}

#[tokio::test]
async fn test_list_by_series_includes_deleted_when_requested() {
    let (db, _temp_dir) = setup_test_db_wrapper().await;
    let conn = db.sea_orm_connection();

    // Setup
    let library = LibraryRepository::create(conn, "Lib", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    let series = SeriesRepository::create(conn, library.id, "Series", None)
        .await
        .unwrap();

    // Create two books
    let book1_model = create_test_book(
        series.id,
        library.id,
        "/book1.cbz",
        "book1.cbz",
        "hash1",
        "cbz",
        10,
    );
    let book1 = BookRepository::create(conn, &book1_model, None)
        .await
        .unwrap();

    let book2_model = create_test_book(
        series.id,
        library.id,
        "/book2.cbz",
        "book2.cbz",
        "hash2",
        "cbz",
        10,
    );
    let _book2 = BookRepository::create(conn, &book2_model, None)
        .await
        .unwrap();

    // Mark book1 as deleted
    BookRepository::mark_deleted(conn, book1.id, true, None)
        .await
        .unwrap();

    // List books including deleted
    let books = BookRepository::list_by_series(conn, series.id, true)
        .await
        .unwrap();

    // Should return both books
    assert_eq!(books.len(), 2);

    // Verify one is deleted and one is not
    let deleted_count = books.iter().filter(|b| b.deleted).count();
    let active_count = books.iter().filter(|b| !b.deleted).count();

    assert_eq!(deleted_count, 1);
    assert_eq!(active_count, 1);

    db.close().await;
}

#[tokio::test]
async fn test_mark_deleted_nonexistent_book_fails() {
    let (db, _temp_dir) = setup_test_db_wrapper().await;
    let conn = db.sea_orm_connection();

    let fake_id = Uuid::new_v4();
    let result = BookRepository::mark_deleted(conn, fake_id, true, None).await;

    assert!(result.is_err());

    db.close().await;
}

// ============================================================================
// Series Fingerprint Tests
// ============================================================================

#[tokio::test]
async fn test_create_series_with_fingerprint() {
    let (db, _temp_dir) = setup_test_db_wrapper().await;
    let conn = db.sea_orm_connection();

    // Setup library
    let library = LibraryRepository::create(conn, "Lib", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    // Create series with fingerprint
    let fingerprint = "abc123fingerprint".to_string();
    let series = SeriesRepository::create_with_fingerprint(
        conn,
        library.id,
        "Test Series",
        Some(fingerprint.clone()),
        "/test/series".to_string(),
        None,
    )
    .await
    .unwrap();

    // Verify series was created with fingerprint
    // Name is now in series_metadata table
    assert_eq!(series.fingerprint.as_deref(), Some(fingerprint.as_str()));

    // Verify series_metadata was created with title
    use codex::db::repositories::SeriesMetadataRepository;
    let metadata = SeriesMetadataRepository::get_by_series_id(conn, series.id)
        .await
        .unwrap()
        .expect("Series metadata should exist");
    assert_eq!(metadata.title, "Test Series");

    db.close().await;
}

#[tokio::test]
async fn test_create_series_without_fingerprint() {
    let (db, _temp_dir) = setup_test_db_wrapper().await;
    let conn = db.sea_orm_connection();

    // Setup library
    let library = LibraryRepository::create(conn, "Lib", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    // Create series without fingerprint
    let series = SeriesRepository::create_with_fingerprint(
        conn,
        library.id,
        "Test Series",
        None,
        "/test/series".to_string(),
        None,
    )
    .await
    .unwrap();

    // Verify series was created without fingerprint
    // Name is now in series_metadata table
    assert!(series.fingerprint.is_none());

    // Verify series_metadata was created with title
    use codex::db::repositories::SeriesMetadataRepository;
    let metadata = SeriesMetadataRepository::get_by_series_id(conn, series.id)
        .await
        .unwrap()
        .expect("Series metadata should exist");
    assert_eq!(metadata.title, "Test Series");

    db.close().await;
}

#[tokio::test]
async fn test_update_series_name() {
    let (db, _temp_dir) = setup_test_db_wrapper().await;
    let conn = db.sea_orm_connection();

    // Setup library
    let library = LibraryRepository::create(conn, "Lib", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    // Create series with fingerprint
    let fingerprint = "test_fingerprint".to_string();
    let series = SeriesRepository::create_with_fingerprint(
        conn,
        library.id,
        "Original Name",
        Some(fingerprint.clone()),
        "/test/series".to_string(),
        None,
    )
    .await
    .unwrap();

    // Update series name
    SeriesRepository::update_name(conn, series.id, "Updated Name")
        .await
        .unwrap();

    // Verify name changed but fingerprint preserved
    let updated_series = series::Entity::find_by_id(series.id)
        .one(conn)
        .await
        .unwrap()
        .unwrap();

    // Name is stored in series_metadata.title
    let metadata = SeriesMetadataRepository::get_by_series_id(conn, series.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(metadata.title, "Updated Name");
    assert_eq!(
        updated_series.fingerprint.as_deref(),
        Some(fingerprint.as_str())
    );

    db.close().await;
}

#[tokio::test]
async fn test_update_series_fingerprint() {
    let (db, _temp_dir) = setup_test_db_wrapper().await;
    let conn = db.sea_orm_connection();

    // Setup library
    let library = LibraryRepository::create(conn, "Lib", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    // Create series without fingerprint
    let series = SeriesRepository::create(conn, library.id, "Test Series", None)
        .await
        .unwrap();

    assert!(series.fingerprint.is_none());

    // Add fingerprint to existing series
    let new_fingerprint = "new_fingerprint".to_string();
    SeriesRepository::update_fingerprint(conn, series.id, Some(new_fingerprint.clone()))
        .await
        .unwrap();

    // Verify fingerprint was added
    let updated_series = series::Entity::find_by_id(series.id)
        .one(conn)
        .await
        .unwrap()
        .unwrap();

    assert_eq!(
        updated_series.fingerprint.as_deref(),
        Some(new_fingerprint.as_str())
    );
    // Name is stored in series_metadata.title
    let metadata = SeriesMetadataRepository::get_by_series_id(conn, series.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(metadata.title, "Test Series"); // Name unchanged

    db.close().await;
}

#[tokio::test]
async fn test_update_series_fingerprint_to_none() {
    let (db, _temp_dir) = setup_test_db_wrapper().await;
    let conn = db.sea_orm_connection();

    // Setup library
    let library = LibraryRepository::create(conn, "Lib", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    // Create series with fingerprint
    let series = SeriesRepository::create_with_fingerprint(
        conn,
        library.id,
        "Test Series",
        Some("fingerprint".to_string()),
        "/test/series".to_string(),
        None,
    )
    .await
    .unwrap();

    // Remove fingerprint
    SeriesRepository::update_fingerprint(conn, series.id, None)
        .await
        .unwrap();

    // Verify fingerprint was removed
    let updated_series = series::Entity::find_by_id(series.id)
        .one(conn)
        .await
        .unwrap()
        .unwrap();

    assert!(updated_series.fingerprint.is_none());

    db.close().await;
}

#[tokio::test]
async fn test_update_name_nonexistent_series_fails() {
    let (db, _temp_dir) = setup_test_db_wrapper().await;
    let conn = db.sea_orm_connection();

    let fake_id = Uuid::new_v4();
    let result = SeriesRepository::update_name(conn, fake_id, "New Name").await;

    assert!(result.is_err());

    db.close().await;
}

#[tokio::test]
async fn test_update_fingerprint_nonexistent_series_fails() {
    let (db, _temp_dir) = setup_test_db_wrapper().await;
    let conn = db.sea_orm_connection();

    let fake_id = Uuid::new_v4();
    let result =
        SeriesRepository::update_fingerprint(conn, fake_id, Some("fingerprint".to_string())).await;

    assert!(result.is_err());

    db.close().await;
}

// ============================================================================
// Connection Edge Cases
// ============================================================================

#[tokio::test]
async fn test_database_reconnect() {
    let (db, temp_dir) = setup_test_db_wrapper().await;
    let db_path = temp_dir.path().join("test.db");

    // Close first connection
    db.close().await;

    // Reconnect to same database (should not run migrations again)
    let config = DatabaseConfig {
        db_type: DatabaseType::SQLite,
        postgres: None,
        sqlite: Some(SQLiteConfig {
            path: db_path.to_str().unwrap().to_string(),
            pragmas: None,
            ..SQLiteConfig::default()
        }),
    };

    let db2 = Database::new(&config).await.unwrap();

    // Should be able to query tables
    let count = libraries::Entity::find()
        .count(db2.sea_orm_connection())
        .await
        .unwrap();

    assert_eq!(count, 0);

    db2.close().await;
}

#[tokio::test]
async fn test_health_check() {
    let (db, _temp_dir) = setup_test_db_wrapper().await;

    // Health check should pass
    assert!(db.health_check().await.is_ok());

    db.close().await;
}

// ============================================================================
// Library & Series New Fields Integration Tests
// ============================================================================

#[tokio::test]
async fn test_library_reading_direction_fields() {
    let (db, _temp_dir) = setup_test_db_wrapper().await;
    let conn = db.sea_orm_connection();

    // Create library
    let mut library =
        LibraryRepository::create(conn, "Manga Library", "/manga", ScanningStrategy::Default)
            .await
            .unwrap();

    // Verify default reading direction
    assert_eq!(library.default_reading_direction, "LEFT_TO_RIGHT");

    // Update to manga reading direction
    library.default_reading_direction = "RIGHT_TO_LEFT".to_string();
    LibraryRepository::update(conn, &library).await.unwrap();

    // Verify update persisted
    let retrieved = LibraryRepository::get_by_id(conn, library.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(retrieved.default_reading_direction, "RIGHT_TO_LEFT");

    db.close().await;
}

#[tokio::test]
async fn test_library_format_filtering() {
    let (db, _temp_dir) = setup_test_db_wrapper().await;
    let conn = db.sea_orm_connection();

    // Create library with format restrictions
    let mut library =
        LibraryRepository::create(conn, "Comics Only", "/comics", ScanningStrategy::Default)
            .await
            .unwrap();

    // Initially no format restrictions
    assert_eq!(library.allowed_formats, None);

    // Set to only allow CBZ and CBR
    library.allowed_formats = Some(r#"["CBZ","CBR"]"#.to_string());
    LibraryRepository::update(conn, &library).await.unwrap();

    // Verify it persisted
    let retrieved = LibraryRepository::get_by_id(conn, library.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        retrieved.allowed_formats,
        Some(r#"["CBZ","CBR"]"#.to_string())
    );

    db.close().await;
}

#[tokio::test]
async fn test_library_excluded_patterns() {
    let (db, _temp_dir) = setup_test_db_wrapper().await;
    let conn = db.sea_orm_connection();

    let mut library =
        LibraryRepository::create(conn, "Clean Library", "/clean", ScanningStrategy::Default)
            .await
            .unwrap();

    // Set exclusion patterns
    let patterns = ".DS_Store\nThumbs.db\n@eaDir/*\n*.tmp";
    library.excluded_patterns = Some(patterns.to_string());
    LibraryRepository::update(conn, &library).await.unwrap();

    // Verify persistence
    let retrieved = LibraryRepository::get_by_id(conn, library.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(retrieved.excluded_patterns, Some(patterns.to_string()));

    db.close().await;
}

#[tokio::test]
async fn test_series_reading_direction_override() {
    let (db, _temp_dir) = setup_test_db_wrapper().await;
    let conn = db.sea_orm_connection();

    // Create manga library (RTL by default)
    let mut library =
        LibraryRepository::create(conn, "Manga Library", "/manga", ScanningStrategy::Default)
            .await
            .unwrap();
    library.default_reading_direction = "RIGHT_TO_LEFT".to_string();
    LibraryRepository::update(conn, &library).await.unwrap();

    // Create series that inherits library default (reading_direction = None in metadata)
    let series1 = SeriesRepository::create(conn, library.id, "Regular Manga", None)
        .await
        .unwrap();
    let metadata1 = SeriesMetadataRepository::get_by_series_id(conn, series1.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(metadata1.reading_direction, None);

    // Create series with explicit override for webtoon
    let series2 = SeriesRepository::create(conn, library.id, "Webtoon", None)
        .await
        .unwrap();
    SeriesMetadataRepository::update_reading_direction(
        conn,
        series2.id,
        Some("TOP_TO_BOTTOM".to_string()),
    )
    .await
    .unwrap();

    // Verify both series persisted correctly
    let retrieved_metadata1 = SeriesMetadataRepository::get_by_series_id(conn, series1.id)
        .await
        .unwrap()
        .unwrap();
    let retrieved_metadata2 = SeriesMetadataRepository::get_by_series_id(conn, series2.id)
        .await
        .unwrap()
        .unwrap();

    assert_eq!(retrieved_metadata1.reading_direction, None); // Inherits library's RTL
    assert_eq!(
        retrieved_metadata2.reading_direction,
        Some("TOP_TO_BOTTOM".to_string())
    );

    db.close().await;
}

#[tokio::test]
async fn test_series_reading_direction_clear() {
    let (db, _temp_dir) = setup_test_db_wrapper().await;
    let conn = db.sea_orm_connection();

    let library =
        LibraryRepository::create(conn, "Test Library", "/test", ScanningStrategy::Default)
            .await
            .unwrap();

    // Create series with explicit direction
    let series = SeriesRepository::create(conn, library.id, "Test Series", None)
        .await
        .unwrap();
    SeriesMetadataRepository::update_reading_direction(
        conn,
        series.id,
        Some("TOP_TO_BOTTOM".to_string()),
    )
    .await
    .unwrap();

    // Clear it to revert to library default
    SeriesMetadataRepository::update_reading_direction(conn, series.id, None)
        .await
        .unwrap();

    // Verify it's cleared
    let retrieved_metadata = SeriesMetadataRepository::get_by_series_id(conn, series.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(retrieved_metadata.reading_direction, None);

    db.close().await;
}

#[tokio::test]
async fn test_purge_deleted_in_library_purges_empty_series_when_enabled() {
    use codex::db::repositories::SettingsRepository;

    let (db, _temp_dir) = setup_test_db_wrapper().await;
    let conn = db.sea_orm_connection();

    // Setup: Create library and series
    let library = LibraryRepository::create(conn, "Lib", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    let series1 = SeriesRepository::create(conn, library.id, "Series 1", None)
        .await
        .unwrap();
    let series2 = SeriesRepository::create(conn, library.id, "Series 2", None)
        .await
        .unwrap();

    // Create books in both series
    let book1_model = create_test_book(
        series1.id,
        library.id,
        "/book1.cbz",
        "book1.cbz",
        "hash1",
        "cbz",
        10,
    );
    let book1 = BookRepository::create(conn, &book1_model, None)
        .await
        .unwrap();

    let book2_model = create_test_book(
        series2.id,
        library.id,
        "/book2.cbz",
        "book2.cbz",
        "hash2",
        "cbz",
        10,
    );
    let _book2 = BookRepository::create(conn, &book2_model, None)
        .await
        .unwrap();

    // Mark all books in series1 as deleted
    BookRepository::mark_deleted(conn, book1.id, true, None)
        .await
        .unwrap();

    // Ensure setting is enabled (default should be true)
    let setting = SettingsRepository::get(conn, "purge.purge_empty_series")
        .await
        .unwrap();
    if let Some(s) = setting {
        if s.value != "true" {
            // Set it to true for this test
            let user_id = Uuid::new_v4();
            SettingsRepository::set(
                conn,
                "purge.purge_empty_series",
                "true".to_string(),
                user_id,
                None,
                None,
            )
            .await
            .unwrap();
        }
    }

    // Purge deleted books - should also delete series1 since it's now empty
    let deleted_count = BookRepository::purge_deleted_in_library(conn, library.id, None)
        .await
        .unwrap();

    assert_eq!(deleted_count, 1, "Should have purged 1 book");

    // Verify series1 was deleted
    let series1_after = SeriesRepository::get_by_id(conn, series1.id).await.unwrap();
    assert!(series1_after.is_none(), "Series 1 should have been deleted");

    // Verify series2 still exists
    let series2_after = SeriesRepository::get_by_id(conn, series2.id).await.unwrap();
    assert!(series2_after.is_some(), "Series 2 should still exist");

    db.close().await;
}

#[tokio::test]
async fn test_purge_deleted_in_library_keeps_empty_series_when_disabled() {
    use codex::db::repositories::SettingsRepository;

    let (db, _temp_dir) = setup_test_db_wrapper().await;
    let conn = db.sea_orm_connection();

    // Setup: Create library and series
    let library = LibraryRepository::create(conn, "Lib", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    let series1 = SeriesRepository::create(conn, library.id, "Series 1", None)
        .await
        .unwrap();

    // Create book in series
    let book1_model = create_test_book(
        series1.id,
        library.id,
        "/book1.cbz",
        "book1.cbz",
        "hash1",
        "cbz",
        10,
    );
    let book1 = BookRepository::create(conn, &book1_model, None)
        .await
        .unwrap();

    // Mark book as deleted
    BookRepository::mark_deleted(conn, book1.id, true, None)
        .await
        .unwrap();

    // Disable the setting
    let user_id = Uuid::new_v4();
    SettingsRepository::set(
        conn,
        "purge.purge_empty_series",
        "false".to_string(),
        user_id,
        None,
        None,
    )
    .await
    .unwrap();

    // Purge deleted books - should NOT delete series1 even though it's empty
    let deleted_count = BookRepository::purge_deleted_in_library(conn, library.id, None)
        .await
        .unwrap();

    assert_eq!(deleted_count, 1, "Should have purged 1 book");

    // Verify series1 still exists
    let series1_after = SeriesRepository::get_by_id(conn, series1.id).await.unwrap();
    assert!(
        series1_after.is_some(),
        "Series 1 should still exist when setting is disabled"
    );

    db.close().await;
}

#[tokio::test]
async fn test_purge_deleted_in_series_purges_series_when_empty_and_enabled() {
    use codex::db::repositories::SettingsRepository;

    let (db, _temp_dir) = setup_test_db_wrapper().await;
    let conn = db.sea_orm_connection();

    // Setup: Create library and series
    let library = LibraryRepository::create(conn, "Lib", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    let series = SeriesRepository::create(conn, library.id, "Series", None)
        .await
        .unwrap();

    // Create book in series
    let book_model = create_test_book(
        series.id,
        library.id,
        "/book.cbz",
        "book.cbz",
        "hash1",
        "cbz",
        10,
    );
    let book = BookRepository::create(conn, &book_model, None)
        .await
        .unwrap();

    // Mark book as deleted
    BookRepository::mark_deleted(conn, book.id, true, None)
        .await
        .unwrap();

    // Ensure setting is enabled
    let user_id = Uuid::new_v4();
    SettingsRepository::set(
        conn,
        "purge.purge_empty_series",
        "true".to_string(),
        user_id,
        None,
        None,
    )
    .await
    .unwrap();

    // Purge deleted books in series - should also delete the series
    let deleted_count = BookRepository::purge_deleted_in_series(conn, series.id, None)
        .await
        .unwrap();

    assert_eq!(deleted_count, 1, "Should have purged 1 book");

    // Verify series was deleted
    let series_after = SeriesRepository::get_by_id(conn, series.id).await.unwrap();
    assert!(series_after.is_none(), "Series should have been deleted");

    db.close().await;
}

#[tokio::test]
async fn test_purge_deleted_in_series_keeps_series_when_empty_but_setting_disabled() {
    use codex::db::repositories::SettingsRepository;

    let (db, _temp_dir) = setup_test_db_wrapper().await;
    let conn = db.sea_orm_connection();

    // Setup: Create library and series
    let library = LibraryRepository::create(conn, "Lib", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    let series = SeriesRepository::create(conn, library.id, "Series", None)
        .await
        .unwrap();

    // Create book in series
    let book_model = create_test_book(
        series.id,
        library.id,
        "/book.cbz",
        "book.cbz",
        "hash1",
        "cbz",
        10,
    );
    let book = BookRepository::create(conn, &book_model, None)
        .await
        .unwrap();

    // Mark book as deleted
    BookRepository::mark_deleted(conn, book.id, true, None)
        .await
        .unwrap();

    // Disable the setting
    let user_id = Uuid::new_v4();
    SettingsRepository::set(
        conn,
        "purge.purge_empty_series",
        "false".to_string(),
        user_id,
        None,
        None,
    )
    .await
    .unwrap();

    // Purge deleted books in series - should NOT delete the series
    let deleted_count = BookRepository::purge_deleted_in_series(conn, series.id, None)
        .await
        .unwrap();

    assert_eq!(deleted_count, 1, "Should have purged 1 book");

    // Verify series still exists
    let series_after = SeriesRepository::get_by_id(conn, series.id).await.unwrap();
    assert!(
        series_after.is_some(),
        "Series should still exist when setting is disabled"
    );

    db.close().await;
}

#[tokio::test]
async fn test_purge_deleted_in_series_keeps_series_when_not_empty() {
    use codex::db::repositories::SettingsRepository;

    let (db, _temp_dir) = setup_test_db_wrapper().await;
    let conn = db.sea_orm_connection();

    // Setup: Create library and series
    let library = LibraryRepository::create(conn, "Lib", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    let series = SeriesRepository::create(conn, library.id, "Series", None)
        .await
        .unwrap();

    // Create two books in series
    let book1_model = create_test_book(
        series.id,
        library.id,
        "/book1.cbz",
        "book1.cbz",
        "hash1",
        "cbz",
        10,
    );
    let book1 = BookRepository::create(conn, &book1_model, None)
        .await
        .unwrap();

    let book2_model = create_test_book(
        series.id,
        library.id,
        "/book2.cbz",
        "book2.cbz",
        "hash2",
        "cbz",
        10,
    );
    let _book2 = BookRepository::create(conn, &book2_model, None)
        .await
        .unwrap();

    // Mark only one book as deleted
    BookRepository::mark_deleted(conn, book1.id, true, None)
        .await
        .unwrap();

    // Ensure setting is enabled
    let user_id = Uuid::new_v4();
    SettingsRepository::set(
        conn,
        "purge.purge_empty_series",
        "true".to_string(),
        user_id,
        None,
        None,
    )
    .await
    .unwrap();

    // Purge deleted books in series - should NOT delete the series since it still has books
    let deleted_count = BookRepository::purge_deleted_in_series(conn, series.id, None)
        .await
        .unwrap();

    assert_eq!(deleted_count, 1, "Should have purged 1 book");

    // Verify series still exists (has remaining books)
    let series_after = SeriesRepository::get_by_id(conn, series.id).await.unwrap();
    assert!(
        series_after.is_some(),
        "Series should still exist since it has remaining books"
    );

    db.close().await;
}
