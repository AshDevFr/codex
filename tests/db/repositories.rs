#[path = "../common/mod.rs"]
mod common;

use chrono::Utc;
use codex::db::entities::{books, libraries, read_progress, series, users};
use codex::db::repositories::{
    BookRepository, LibraryRepository, PageRepository, SeriesRepository,
};
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
    let series = SeriesRepository::create(conn, library.id, "Test Series")
        .await
        .unwrap();

    // Create book
    let book_model = create_test_book(
        series.id,
        "/test/book.cbz",
        "book.cbz",
        "test_hash",
        "cbz",
        10,
    );

    let book = BookRepository::create(conn, &book_model).await.unwrap();

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
    assert_eq!(series_result.unwrap().name, "Test Series");

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

    let series = SeriesRepository::create(conn, library.id, "Test Series")
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

    let series = SeriesRepository::create(conn, library.id, "Series")
        .await
        .unwrap();

    let book_model = create_test_book(series.id, "/book.cbz", "book.cbz", "test_hash", "cbz", 10);
    let book = BookRepository::create(conn, &book_model).await.unwrap();

    // Create read progress using ActiveModel
    let progress = read_progress::ActiveModel {
        id: Set(Uuid::new_v4()),
        user_id: Set(user.id),
        book_id: Set(book.id),
        current_page: Set(1),
        completed: Set(false),
        started_at: Set(Utc::now()),
        updated_at: Set(Utc::now()),
        completed_at: Set(None),
    };
    let progress = progress.insert(conn).await.unwrap();

    // Query progress with user and book info using SeaORM
    let progress_result = read_progress::Entity::find_by_id(progress.id.clone())
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

    let series = SeriesRepository::create(conn, library.id, "Series")
        .await
        .unwrap();

    // Insert first book
    let book1_model = create_test_book(
        series.id,
        "/same/path/book.cbz",
        "book.cbz",
        "hash1",
        "cbz",
        10,
    );
    BookRepository::create(conn, &book1_model).await.unwrap();

    // Try to insert second book with same file path (should fail)
    let book2_model = create_test_book(
        series.id,
        "/same/path/book.cbz",
        "book.cbz",
        "hash2",
        "cbz",
        10,
    );

    let result = BookRepository::create(conn, &book2_model).await;
    assert!(result.is_err());

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

    let series = SeriesRepository::create(conn, library.id, "Series")
        .await
        .unwrap();

    // Insert book with specific hash
    let test_hash = "abc123hash";
    let book_model = create_test_book(series.id, "/book1.cbz", "book1.cbz", test_hash, "cbz", 10);
    BookRepository::create(conn, &book_model).await.unwrap();

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

    let series = SeriesRepository::create(conn, library.id, "Series")
        .await
        .unwrap();

    let book_model = create_test_book(series.id, "/book.cbz", "book.cbz", "test_hash", "cbz", 10);
    let book = BookRepository::create(conn, &book_model).await.unwrap();

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
