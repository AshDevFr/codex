use codex::config::{DatabaseConfig, DatabaseType, SQLiteConfig};
use codex::db::{Book, Database, Library, Page, ReadProgress, ScanningStrategy, Series, User};
use tempfile::TempDir;

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
    (db, temp_dir)
}

// ============================================================================
// Library CRUD Tests
// ============================================================================

#[tokio::test]
async fn test_library_insert_and_select() {
    let (db, _temp_dir) = create_test_db().await;
    let pool = db.sqlite_pool().unwrap();

    let library = Library::new(
        "Test Library".to_string(),
        "/test/path".to_string(),
        ScanningStrategy::KomgaCompatible,
    );

    // Insert
    sqlx::query(
        r#"
        INSERT INTO libraries (id, name, path, scanning_strategy, scanning_config, created_at, updated_at, last_scanned_at)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(library.id.to_string())
    .bind(&library.name)
    .bind(&library.path)
    .bind(&library.scanning_strategy)
    .bind(&library.scanning_config)
    .bind(library.created_at.to_rfc3339())
    .bind(library.updated_at.to_rfc3339())
    .bind(library.last_scanned_at.map(|dt| dt.to_rfc3339()))
    .execute(pool)
    .await
    .unwrap();

    // Select
    let row: (String, String, String) = sqlx::query_as(
        "SELECT id, name, path FROM libraries WHERE id = ?"
    )
    .bind(library.id.to_string())
    .fetch_one(pool)
    .await
    .unwrap();

    assert_eq!(row.0, library.id.to_string());
    assert_eq!(row.1, "Test Library");
    assert_eq!(row.2, "/test/path");

    db.close().await;
}

#[tokio::test]
async fn test_library_update() {
    let (db, _temp_dir) = create_test_db().await;
    let pool = db.sqlite_pool().unwrap();

    let library = Library::new(
        "Original Name".to_string(),
        "/original/path".to_string(),
        ScanningStrategy::KomgaCompatible,
    );

    // Insert
    sqlx::query(
        "INSERT INTO libraries (id, name, path, scanning_strategy, scanning_config, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?)"
    )
    .bind(library.id.to_string())
    .bind(&library.name)
    .bind(&library.path)
    .bind(&library.scanning_strategy)
    .bind(&library.scanning_config)
    .bind(library.created_at.to_rfc3339())
    .bind(library.updated_at.to_rfc3339())
    .execute(pool)
    .await
    .unwrap();

    // Update
    sqlx::query("UPDATE libraries SET name = ? WHERE id = ?")
        .bind("Updated Name")
        .bind(library.id.to_string())
        .execute(pool)
        .await
        .unwrap();

    // Verify
    let name: (String,) = sqlx::query_as("SELECT name FROM libraries WHERE id = ?")
        .bind(library.id.to_string())
        .fetch_one(pool)
        .await
        .unwrap();

    assert_eq!(name.0, "Updated Name");

    db.close().await;
}

#[tokio::test]
async fn test_library_delete() {
    let (db, _temp_dir) = create_test_db().await;
    let pool = db.sqlite_pool().unwrap();

    let library = Library::new(
        "To Delete".to_string(),
        "/delete/path".to_string(),
        ScanningStrategy::KomgaCompatible,
    );

    // Insert
    sqlx::query(
        "INSERT INTO libraries (id, name, path, scanning_strategy, scanning_config, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?)"
    )
    .bind(library.id.to_string())
    .bind(&library.name)
    .bind(&library.path)
    .bind(&library.scanning_strategy)
    .bind(&library.scanning_config)
    .bind(library.created_at.to_rfc3339())
    .bind(library.updated_at.to_rfc3339())
    .execute(pool)
    .await
    .unwrap();

    // Delete
    sqlx::query("DELETE FROM libraries WHERE id = ?")
        .bind(library.id.to_string())
        .execute(pool)
        .await
        .unwrap();

    // Verify deleted
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM libraries WHERE id = ?")
        .bind(library.id.to_string())
        .fetch_one(pool)
        .await
        .unwrap();

    assert_eq!(count.0, 0);

    db.close().await;
}

// ============================================================================
// Series and Book Relationship Tests
// ============================================================================

#[tokio::test]
async fn test_series_book_relationship() {
    let (db, _temp_dir) = create_test_db().await;
    let pool = db.sqlite_pool().unwrap();

    // Create library
    let library = Library::new(
        "Test Library".to_string(),
        "/test".to_string(),
        ScanningStrategy::KomgaCompatible,
    );

    sqlx::query(
        "INSERT INTO libraries (id, name, path, scanning_strategy, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)"
    )
    .bind(library.id.to_string())
    .bind(&library.name)
    .bind(&library.path)
    .bind(&library.scanning_strategy)
    .bind(library.created_at.to_rfc3339())
    .bind(library.updated_at.to_rfc3339())
    .execute(pool)
    .await
    .unwrap();

    // Create series
    let series = Series::new(library.id, "Test Series".to_string());

    sqlx::query(
        "INSERT INTO series (id, library_id, name, normalized_name, book_count, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?)"
    )
    .bind(series.id.to_string())
    .bind(series.library_id.to_string())
    .bind(&series.name)
    .bind(&series.normalized_name)
    .bind(series.book_count)
    .bind(series.created_at.to_rfc3339())
    .bind(series.updated_at.to_rfc3339())
    .execute(pool)
    .await
    .unwrap();

    // Create book
    let book = Book::new(
        series.id,
        "/test/book.cbz".to_string(),
        "book.cbz".to_string(),
    );

    sqlx::query(
        r#"
        INSERT INTO books (id, series_id, file_path, file_name, file_size, file_hash, format, page_count, modified_at, created_at, updated_at)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#
    )
    .bind(book.id.to_string())
    .bind(book.series_id.to_string())
    .bind(&book.file_path)
    .bind(&book.file_name)
    .bind(book.file_size)
    .bind(&book.file_hash)
    .bind(&book.format)
    .bind(book.page_count)
    .bind(book.modified_at.to_rfc3339())
    .bind(book.created_at.to_rfc3339())
    .bind(book.updated_at.to_rfc3339())
    .execute(pool)
    .await
    .unwrap();

    // Query book with series join
    let result: (String, String, String) = sqlx::query_as(
        "SELECT books.id, books.file_name, series.name FROM books JOIN series ON books.series_id = series.id WHERE books.id = ?"
    )
    .bind(book.id.to_string())
    .fetch_one(pool)
    .await
    .unwrap();

    assert_eq!(result.0, book.id.to_string());
    assert_eq!(result.1, "book.cbz");
    assert_eq!(result.2, "Test Series");

    db.close().await;
}

#[tokio::test]
async fn test_cascade_delete_library_to_series() {
    let (db, _temp_dir) = create_test_db().await;
    let pool = db.sqlite_pool().unwrap();

    // Create library and series
    let library = Library::new(
        "Test Library".to_string(),
        "/test".to_string(),
        ScanningStrategy::KomgaCompatible,
    );

    sqlx::query(
        "INSERT INTO libraries (id, name, path, scanning_strategy, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)"
    )
    .bind(library.id.to_string())
    .bind(&library.name)
    .bind(&library.path)
    .bind(&library.scanning_strategy)
    .bind(library.created_at.to_rfc3339())
    .bind(library.updated_at.to_rfc3339())
    .execute(pool)
    .await
    .unwrap();

    let series = Series::new(library.id, "Test Series".to_string());

    sqlx::query(
        "INSERT INTO series (id, library_id, name, normalized_name, book_count, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?)"
    )
    .bind(series.id.to_string())
    .bind(series.library_id.to_string())
    .bind(&series.name)
    .bind(&series.normalized_name)
    .bind(series.book_count)
    .bind(series.created_at.to_rfc3339())
    .bind(series.updated_at.to_rfc3339())
    .execute(pool)
    .await
    .unwrap();

    // Delete library (should cascade to series)
    sqlx::query("DELETE FROM libraries WHERE id = ?")
        .bind(library.id.to_string())
        .execute(pool)
        .await
        .unwrap();

    // Verify series was also deleted
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM series WHERE id = ?")
        .bind(series.id.to_string())
        .fetch_one(pool)
        .await
        .unwrap();

    assert_eq!(count.0, 0);

    db.close().await;
}

// ============================================================================
// User and Read Progress Tests
// ============================================================================

#[tokio::test]
async fn test_user_read_progress() {
    let (db, _temp_dir) = create_test_db().await;
    let pool = db.sqlite_pool().unwrap();

    // Create user
    let user = User::new(
        "testuser".to_string(),
        "test@example.com".to_string(),
        "hashed_password".to_string(),
    );

    sqlx::query(
        "INSERT INTO users (id, username, email, password_hash, is_admin, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?)"
    )
    .bind(user.id.to_string())
    .bind(&user.username)
    .bind(&user.email)
    .bind(&user.password_hash)
    .bind(user.is_admin)
    .bind(user.created_at.to_rfc3339())
    .bind(user.updated_at.to_rfc3339())
    .execute(pool)
    .await
    .unwrap();

    // Create library, series, and book for progress tracking
    let library = Library::new("Lib".to_string(), "/lib".to_string(), ScanningStrategy::Flat);
    let series = Series::new(library.id, "Series".to_string());
    let book = Book::new(series.id, "/book.cbz".to_string(), "book.cbz".to_string());

    sqlx::query(
        "INSERT INTO libraries (id, name, path, scanning_strategy, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)"
    )
    .bind(library.id.to_string())
    .bind(&library.name)
    .bind(&library.path)
    .bind(&library.scanning_strategy)
    .bind(library.created_at.to_rfc3339())
    .bind(library.updated_at.to_rfc3339())
    .execute(pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO series (id, library_id, name, normalized_name, book_count, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?)"
    )
    .bind(series.id.to_string())
    .bind(series.library_id.to_string())
    .bind(&series.name)
    .bind(&series.normalized_name)
    .bind(series.book_count)
    .bind(series.created_at.to_rfc3339())
    .bind(series.updated_at.to_rfc3339())
    .execute(pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO books (id, series_id, file_path, file_name, file_size, file_hash, format, page_count, modified_at, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
    )
    .bind(book.id.to_string())
    .bind(book.series_id.to_string())
    .bind(&book.file_path)
    .bind(&book.file_name)
    .bind(book.file_size)
    .bind(&book.file_hash)
    .bind(&book.format)
    .bind(book.page_count)
    .bind(book.modified_at.to_rfc3339())
    .bind(book.created_at.to_rfc3339())
    .bind(book.updated_at.to_rfc3339())
    .execute(pool)
    .await
    .unwrap();

    // Create read progress
    let progress = ReadProgress::new(user.id, book.id);

    sqlx::query(
        "INSERT INTO read_progress (id, user_id, book_id, current_page, completed, started_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?)"
    )
    .bind(progress.id.to_string())
    .bind(progress.user_id.to_string())
    .bind(progress.book_id.to_string())
    .bind(progress.current_page)
    .bind(progress.completed)
    .bind(progress.started_at.to_rfc3339())
    .bind(progress.updated_at.to_rfc3339())
    .execute(pool)
    .await
    .unwrap();

    // Query progress with user and book info
    let result: (String, String, i32) = sqlx::query_as(
        r#"
        SELECT users.username, books.file_name, read_progress.current_page
        FROM read_progress
        JOIN users ON read_progress.user_id = users.id
        JOIN books ON read_progress.book_id = books.id
        WHERE read_progress.id = ?
        "#
    )
    .bind(progress.id.to_string())
    .fetch_one(pool)
    .await
    .unwrap();

    assert_eq!(result.0, "testuser");
    assert_eq!(result.1, "book.cbz");
    assert_eq!(result.2, 1);

    db.close().await;
}

// ============================================================================
// Constraint Tests
// ============================================================================

#[tokio::test]
async fn test_unique_username_constraint() {
    let (db, _temp_dir) = create_test_db().await;
    let pool = db.sqlite_pool().unwrap();

    let user1 = User::new(
        "testuser".to_string(),
        "test1@example.com".to_string(),
        "hash1".to_string(),
    );

    // Insert first user
    sqlx::query(
        "INSERT INTO users (id, username, email, password_hash, is_admin, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?)"
    )
    .bind(user1.id.to_string())
    .bind(&user1.username)
    .bind(&user1.email)
    .bind(&user1.password_hash)
    .bind(user1.is_admin)
    .bind(user1.created_at.to_rfc3339())
    .bind(user1.updated_at.to_rfc3339())
    .execute(pool)
    .await
    .unwrap();

    // Try to insert second user with same username (should fail)
    let user2 = User::new(
        "testuser".to_string(), // Same username
        "test2@example.com".to_string(),
        "hash2".to_string(),
    );

    let result = sqlx::query(
        "INSERT INTO users (id, username, email, password_hash, is_admin, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?)"
    )
    .bind(user2.id.to_string())
    .bind(&user2.username)
    .bind(&user2.email)
    .bind(&user2.password_hash)
    .bind(user2.is_admin)
    .bind(user2.created_at.to_rfc3339())
    .bind(user2.updated_at.to_rfc3339())
    .execute(pool)
    .await;

    assert!(result.is_err());

    db.close().await;
}

#[tokio::test]
async fn test_unique_file_path_constraint() {
    let (db, _temp_dir) = create_test_db().await;
    let pool = db.sqlite_pool().unwrap();

    // Setup library and series
    let library = Library::new("Lib".to_string(), "/lib".to_string(), ScanningStrategy::Flat);
    let series = Series::new(library.id, "Series".to_string());

    sqlx::query(
        "INSERT INTO libraries (id, name, path, scanning_strategy, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)"
    )
    .bind(library.id.to_string())
    .bind(&library.name)
    .bind(&library.path)
    .bind(&library.scanning_strategy)
    .bind(library.created_at.to_rfc3339())
    .bind(library.updated_at.to_rfc3339())
    .execute(pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO series (id, library_id, name, normalized_name, book_count, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?)"
    )
    .bind(series.id.to_string())
    .bind(series.library_id.to_string())
    .bind(&series.name)
    .bind(&series.normalized_name)
    .bind(series.book_count)
    .bind(series.created_at.to_rfc3339())
    .bind(series.updated_at.to_rfc3339())
    .execute(pool)
    .await
    .unwrap();

    // Insert first book
    let book1 = Book::new(
        series.id,
        "/same/path/book.cbz".to_string(),
        "book.cbz".to_string(),
    );

    sqlx::query(
        "INSERT INTO books (id, series_id, file_path, file_name, file_size, file_hash, format, page_count, modified_at, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
    )
    .bind(book1.id.to_string())
    .bind(book1.series_id.to_string())
    .bind(&book1.file_path)
    .bind(&book1.file_name)
    .bind(book1.file_size)
    .bind(&book1.file_hash)
    .bind(&book1.format)
    .bind(book1.page_count)
    .bind(book1.modified_at.to_rfc3339())
    .bind(book1.created_at.to_rfc3339())
    .bind(book1.updated_at.to_rfc3339())
    .execute(pool)
    .await
    .unwrap();

    // Try to insert second book with same file path (should fail)
    let book2 = Book::new(
        series.id,
        "/same/path/book.cbz".to_string(), // Same path
        "book.cbz".to_string(),
    );

    let result = sqlx::query(
        "INSERT INTO books (id, series_id, file_path, file_name, file_size, file_hash, format, page_count, modified_at, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
    )
    .bind(book2.id.to_string())
    .bind(book2.series_id.to_string())
    .bind(&book2.file_path)
    .bind(&book2.file_name)
    .bind(book2.file_size)
    .bind(&book2.file_hash)
    .bind(&book2.format)
    .bind(book2.page_count)
    .bind(book2.modified_at.to_rfc3339())
    .bind(book2.created_at.to_rfc3339())
    .bind(book2.updated_at.to_rfc3339())
    .execute(pool)
    .await;

    assert!(result.is_err());

    db.close().await;
}

// ============================================================================
// Index Performance Tests
// ============================================================================

#[tokio::test]
async fn test_file_hash_index_lookup() {
    let (db, _temp_dir) = create_test_db().await;
    let pool = db.sqlite_pool().unwrap();

    // Setup
    let library = Library::new("Lib".to_string(), "/lib".to_string(), ScanningStrategy::Flat);
    let series = Series::new(library.id, "Series".to_string());

    sqlx::query(
        "INSERT INTO libraries (id, name, path, scanning_strategy, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)"
    )
    .bind(library.id.to_string())
    .bind(&library.name)
    .bind(&library.path)
    .bind(&library.scanning_strategy)
    .bind(library.created_at.to_rfc3339())
    .bind(library.updated_at.to_rfc3339())
    .execute(pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO series (id, library_id, name, normalized_name, book_count, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?)"
    )
    .bind(series.id.to_string())
    .bind(series.library_id.to_string())
    .bind(&series.name)
    .bind(&series.normalized_name)
    .bind(series.book_count)
    .bind(series.created_at.to_rfc3339())
    .bind(series.updated_at.to_rfc3339())
    .execute(pool)
    .await
    .unwrap();

    // Insert books with specific hash
    let test_hash = "abc123hash";
    let mut book = Book::new(series.id, "/book1.cbz".to_string(), "book1.cbz".to_string());
    book.file_hash = test_hash.to_string();

    sqlx::query(
        "INSERT INTO books (id, series_id, file_path, file_name, file_size, file_hash, format, page_count, modified_at, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
    )
    .bind(book.id.to_string())
    .bind(book.series_id.to_string())
    .bind(&book.file_path)
    .bind(&book.file_name)
    .bind(book.file_size)
    .bind(&book.file_hash)
    .bind(&book.format)
    .bind(book.page_count)
    .bind(book.modified_at.to_rfc3339())
    .bind(book.created_at.to_rfc3339())
    .bind(book.updated_at.to_rfc3339())
    .execute(pool)
    .await
    .unwrap();

    // Query by hash (should use index)
    let result: (String,) = sqlx::query_as("SELECT file_name FROM books WHERE file_hash = ?")
        .bind(test_hash)
        .fetch_one(pool)
        .await
        .unwrap();

    assert_eq!(result.0, "book1.cbz");

    db.close().await;
}

// ============================================================================
// Page Tests
// ============================================================================

#[tokio::test]
async fn test_pages_insert_and_query() {
    let (db, _temp_dir) = create_test_db().await;
    let pool = db.sqlite_pool().unwrap();

    // Setup
    let library = Library::new("Lib".to_string(), "/lib".to_string(), ScanningStrategy::Flat);
    let series = Series::new(library.id, "Series".to_string());
    let book = Book::new(series.id, "/book.cbz".to_string(), "book.cbz".to_string());

    sqlx::query(
        "INSERT INTO libraries (id, name, path, scanning_strategy, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)"
    )
    .bind(library.id.to_string())
    .bind(&library.name)
    .bind(&library.path)
    .bind(&library.scanning_strategy)
    .bind(library.created_at.to_rfc3339())
    .bind(library.updated_at.to_rfc3339())
    .execute(pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO series (id, library_id, name, normalized_name, book_count, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?)"
    )
    .bind(series.id.to_string())
    .bind(series.library_id.to_string())
    .bind(&series.name)
    .bind(&series.normalized_name)
    .bind(series.book_count)
    .bind(series.created_at.to_rfc3339())
    .bind(series.updated_at.to_rfc3339())
    .execute(pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO books (id, series_id, file_path, file_name, file_size, file_hash, format, page_count, modified_at, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
    )
    .bind(book.id.to_string())
    .bind(book.series_id.to_string())
    .bind(&book.file_path)
    .bind(&book.file_name)
    .bind(book.file_size)
    .bind(&book.file_hash)
    .bind(&book.format)
    .bind(book.page_count)
    .bind(book.modified_at.to_rfc3339())
    .bind(book.created_at.to_rfc3339())
    .bind(book.updated_at.to_rfc3339())
    .execute(pool)
    .await
    .unwrap();

    // Insert pages
    for i in 1..=3 {
        let page = Page::new(book.id, i, format!("page{:03}.jpg", i));

        sqlx::query(
            "INSERT INTO pages (id, book_id, page_number, file_name, format, width, height, file_size, created_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(page.id.to_string())
        .bind(page.book_id.to_string())
        .bind(page.page_number)
        .bind(&page.file_name)
        .bind(&page.format)
        .bind(page.width)
        .bind(page.height)
        .bind(page.file_size)
        .bind(page.created_at.to_rfc3339())
        .execute(pool)
        .await
        .unwrap();
    }

    // Query pages ordered by page_number
    let pages: Vec<(i32, String)> = sqlx::query_as(
        "SELECT page_number, file_name FROM pages WHERE book_id = ? ORDER BY page_number"
    )
    .bind(book.id.to_string())
    .fetch_all(pool)
    .await
    .unwrap();

    assert_eq!(pages.len(), 3);
    assert_eq!(pages[0].0, 1);
    assert_eq!(pages[0].1, "page001.jpg");
    assert_eq!(pages[2].0, 3);
    assert_eq!(pages[2].1, "page003.jpg");

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
    let pool = db2.sqlite_pool().unwrap();
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM libraries")
        .fetch_one(pool)
        .await
        .unwrap();

    assert_eq!(count.0, 0);

    db2.close().await;
}

#[tokio::test]
async fn test_health_check() {
    let (db, _temp_dir) = create_test_db().await;

    // Health check should pass
    assert!(db.health_check().await.is_ok());

    db.close().await;
}
