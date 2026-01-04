use codex::config::{DatabaseConfig, DatabaseType, SQLiteConfig};
use codex::db::{Database, repositories::{LibraryRepository, SeriesRepository, BookRepository, PageRepository}};
use codex::db::entities::{libraries, series, books, pages, users, read_progress};
use codex::models::ScanningStrategy;
use sea_orm::{EntityTrait, QueryFilter, ColumnTrait, Set, ActiveModelTrait, PaginatorTrait};
use tempfile::TempDir;
use uuid::Uuid;
use chrono::Utc;

/// Helper to create a test database
async fn create_test_db() -> (Database, TempDir) {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");

    let config = DatabaseConfig {
        db_type: DatabaseType::SQLite,
        postgres: None,
        sqlite: Some(SQLiteConfig {
            path: db_path.to_str().unwrap().to_string(),
            pragmas: None,
        }),
    };

    let db = Database::new(&config).await.unwrap();
    db.run_migrations().await.unwrap();
    (db, temp_dir)
}

// ============================================================================
// Library CRUD Tests
// ============================================================================

#[tokio::test]
async fn test_library_insert_and_select() {
    let (db, _temp_dir) = create_test_db().await;
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
    let (db, _temp_dir) = create_test_db().await;
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
    LibraryRepository::update(conn, &updated_library).await.unwrap();

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
    let (db, _temp_dir) = create_test_db().await;
    let conn = db.sea_orm_connection();

    // Create library
    let library = LibraryRepository::create(
        conn,
        "To Delete",
        "/delete/path",
        ScanningStrategy::Default,
    )
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
    let (db, _temp_dir) = create_test_db().await;
    let conn = db.sea_orm_connection();

    // Create library
    let library = LibraryRepository::create(
        conn,
        "Test Library",
        "/test",
        ScanningStrategy::Default,
    )
    .await
    .unwrap();

    // Create series
    let series = SeriesRepository::create(conn, library.id, "Test Series")
        .await
        .unwrap();

    // Create book
    let now = Utc::now();
    let book_model = books::Model {
        id: Uuid::new_v4(),
        series_id: series.id,
        title: None,
        number: None,
        file_path: "/test/book.cbz".to_string(),
        file_name: "book.cbz".to_string(),
        file_size: 1024,
        file_hash: "test_hash".to_string(),
        format: "cbz".to_string(),
        page_count: 10,
        modified_at: now,
        created_at: now,
        updated_at: now,
    };

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
    let (db, _temp_dir) = create_test_db().await;
    let conn = db.sea_orm_connection();

    // Create library and series
    let library = LibraryRepository::create(
        conn,
        "Test Library",
        "/test",
        ScanningStrategy::Default,
    )
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
    let (db, _temp_dir) = create_test_db().await;
    let conn = db.sea_orm_connection();

    // Create user using ActiveModel
    let now = Utc::now();
    let user = users::ActiveModel {
        id: Set(Uuid::new_v4()),
        username: Set("testuser".to_string()),
        email: Set("test@example.com".to_string()),
        password_hash: Set("hashed_password".to_string()),
        is_admin: Set(false),
        created_at: Set(now),
        updated_at: Set(now),
        last_login_at: Set(None),
    };
    let user = user.insert(conn).await.unwrap();

    // Create library, series, and book for progress tracking
    let library = LibraryRepository::create(
        conn,
        "Lib",
        "/lib",
        ScanningStrategy::Default,
    )
    .await
    .unwrap();

    let series = SeriesRepository::create(conn, library.id, "Series")
        .await
        .unwrap();

    let now = Utc::now();
    let book_model = books::Model {
        id: Uuid::new_v4(),
        series_id: series.id,
        title: None,
        number: None,
        file_path: "/book.cbz".to_string(),
        file_name: "book.cbz".to_string(),
        file_size: 1024,
        file_hash: "test_hash".to_string(),
        format: "cbz".to_string(),
        page_count: 10,
        modified_at: now,
        created_at: now,
        updated_at: now,
    };
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
    let (db, _temp_dir) = create_test_db().await;
    let conn = db.sea_orm_connection();

    // Create first user
    let now = Utc::now();
    let user1 = users::ActiveModel {
        id: Set(Uuid::new_v4()),
        username: Set("testuser".to_string()),
        email: Set("test1@example.com".to_string()),
        password_hash: Set("hash1".to_string()),
        is_admin: Set(false),
        created_at: Set(now),
        updated_at: Set(now),
        last_login_at: Set(None),
    };
    user1.insert(conn).await.unwrap();

    // Try to insert second user with same username (should fail)
    let user2 = users::ActiveModel {
        id: Set(Uuid::new_v4()),
        username: Set("testuser".to_string()), // Same username
        email: Set("test2@example.com".to_string()),
        password_hash: Set("hash2".to_string()),
        is_admin: Set(false),
        created_at: Set(now),
        updated_at: Set(now),
        last_login_at: Set(None),
    };

    let result = user2.insert(conn).await;
    assert!(result.is_err());

    db.close().await;
}

#[tokio::test]
async fn test_unique_file_path_constraint() {
    let (db, _temp_dir) = create_test_db().await;
    let conn = db.sea_orm_connection();

    // Setup library and series
    let library = LibraryRepository::create(
        conn,
        "Lib",
        "/lib",
        ScanningStrategy::Default,
    )
    .await
    .unwrap();

    let series = SeriesRepository::create(conn, library.id, "Series")
        .await
        .unwrap();

    // Insert first book
    let now = Utc::now();
    let book1_model = books::Model {
        id: Uuid::new_v4(),
        series_id: series.id,
        title: None,
        number: None,
        file_path: "/same/path/book.cbz".to_string(),
        file_name: "book.cbz".to_string(),
        file_size: 1024,
        file_hash: "hash1".to_string(),
        format: "cbz".to_string(),
        page_count: 10,
        modified_at: now,
        created_at: now,
        updated_at: now,
    };
    BookRepository::create(conn, &book1_model).await.unwrap();

    // Try to insert second book with same file path (should fail)
    let book2_model = books::Model {
        id: Uuid::new_v4(),
        series_id: series.id,
        title: None,
        number: None,
        file_path: "/same/path/book.cbz".to_string(), // Same path
        file_name: "book.cbz".to_string(),
        file_size: 1024,
        file_hash: "hash2".to_string(),
        format: "cbz".to_string(),
        page_count: 10,
        modified_at: now,
        created_at: now,
        updated_at: now,
    };

    let result = BookRepository::create(conn, &book2_model).await;
    assert!(result.is_err());

    db.close().await;
}

// ============================================================================
// Index Performance Tests
// ============================================================================

#[tokio::test]
async fn test_file_hash_index_lookup() {
    let (db, _temp_dir) = create_test_db().await;
    let conn = db.sea_orm_connection();

    // Setup
    let library = LibraryRepository::create(
        conn,
        "Lib",
        "/lib",
        ScanningStrategy::Default,
    )
    .await
    .unwrap();

    let series = SeriesRepository::create(conn, library.id, "Series")
        .await
        .unwrap();

    // Insert book with specific hash
    let test_hash = "abc123hash";
    let now = Utc::now();
    let book_model = books::Model {
        id: Uuid::new_v4(),
        series_id: series.id,
        title: None,
        number: None,
        file_path: "/book1.cbz".to_string(),
        file_name: "book1.cbz".to_string(),
        file_size: 1024,
        file_hash: test_hash.to_string(),
        format: "cbz".to_string(),
        page_count: 10,
        modified_at: now,
        created_at: now,
        updated_at: now,
    };
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
    let (db, _temp_dir) = create_test_db().await;
    let conn = db.sea_orm_connection();

    // Setup
    let library = LibraryRepository::create(
        conn,
        "Lib",
        "/lib",
        ScanningStrategy::Default,
    )
    .await
    .unwrap();

    let series = SeriesRepository::create(conn, library.id, "Series")
        .await
        .unwrap();

    let now = Utc::now();
    let book_model = books::Model {
        id: Uuid::new_v4(),
        series_id: series.id,
        title: None,
        number: None,
        file_path: "/book.cbz".to_string(),
        file_name: "book.cbz".to_string(),
        file_size: 1024,
        file_hash: "test_hash".to_string(),
        format: "cbz".to_string(),
        page_count: 10,
        modified_at: now,
        created_at: now,
        updated_at: now,
    };
    let book = BookRepository::create(conn, &book_model).await.unwrap();

    // Insert pages
    for i in 1..=3 {
        let page_model = pages::Model {
            id: Uuid::new_v4(),
            book_id: book.id,
            page_number: i,
            file_name: format!("page{:03}.jpg", i),
            format: "jpeg".to_string(),
            width: 800,
            height: 1200,
            file_size: 1024,
            created_at: Utc::now(),
        };
        PageRepository::create(conn, &page_model).await.unwrap();
    }

    // Query pages ordered by page_number
    let pages_result = PageRepository::list_by_book(conn, book.id)
        .await
        .unwrap();

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
    let (db, temp_dir) = create_test_db().await;
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
    let (db, _temp_dir) = create_test_db().await;

    // Health check should pass
    assert!(db.health_check().await.is_ok());

    db.close().await;
}
