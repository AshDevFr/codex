use chrono::Utc;
use codex::db::repositories::{BookRepository, LibraryRepository, SeriesRepository};
use codex::models::ScanningStrategy;
use sea_orm::DatabaseConnection;
use uuid::Uuid;

pub use codex::db::entities::{api_keys, books, libraries, pages, series, users};

/// Create a test user model with default values
pub fn create_test_user(
    username: &str,
    email: &str,
    password_hash: &str,
    is_admin: bool,
) -> users::Model {
    let role = if is_admin { "admin" } else { "reader" };
    users::Model {
        id: Uuid::new_v4(),
        username: username.to_string(),
        email: email.to_string(),
        password_hash: password_hash.to_string(),
        role: role.to_string(),
        is_active: true,
        email_verified: false,
        permissions: serde_json::json!([]),
        created_at: Utc::now(),
        updated_at: Utc::now(),
        last_login_at: None,
    }
}

/// Create a test user model with specific permissions
pub fn create_test_user_with_permissions(
    username: &str,
    email: &str,
    password_hash: &str,
    is_admin: bool,
    permissions: Vec<String>,
) -> users::Model {
    let role = if is_admin { "admin" } else { "reader" };
    users::Model {
        id: Uuid::new_v4(),
        username: username.to_string(),
        email: email.to_string(),
        password_hash: password_hash.to_string(),
        role: role.to_string(),
        is_active: true,
        email_verified: false,
        permissions: serde_json::json!(permissions),
        created_at: Utc::now(),
        updated_at: Utc::now(),
        last_login_at: None,
    }
}

/// Create a test book model with default values
pub fn create_test_book(
    series_id: Uuid,
    library_id: Uuid,
    file_path: &str,
    file_name: &str,
    file_hash: &str,
    format: &str,
    page_count: i32,
) -> books::Model {
    let now = Utc::now();
    books::Model {
        id: Uuid::new_v4(),
        series_id,
        library_id,
        file_path: file_path.to_string(),
        file_name: file_name.to_string(),
        file_size: 1024,
        file_hash: file_hash.to_string(),
        partial_hash: String::new(),
        format: format.to_string(),
        page_count,
        deleted: false,
        analyzed: false, // Default to not analyzed
        analysis_error: None,
        modified_at: now,
        created_at: now,
        updated_at: now,
        thumbnail_path: None,
        thumbnail_generated_at: None,
    }
}

/// Create a test page model with default values
pub fn create_test_page(
    book_id: Uuid,
    page_number: i32,
    file_name: &str,
    format: &str,
) -> pages::Model {
    pages::Model {
        id: Uuid::new_v4(),
        book_id,
        page_number,
        file_name: file_name.to_string(),
        format: format.to_string(),
        width: 800,
        height: 1200,
        file_size: 1024,
        created_at: Utc::now(),
    }
}

/// Create a test API key model with default values
pub fn create_test_api_key(
    user_id: Uuid,
    name: &str,
    key_hash: &str,
    key_prefix: &str,
    permissions: serde_json::Value,
) -> api_keys::Model {
    let now = Utc::now();
    api_keys::Model {
        id: Uuid::new_v4(),
        user_id,
        name: name.to_string(),
        key_hash: key_hash.to_string(),
        key_prefix: key_prefix.to_string(),
        permissions,
        is_active: true,
        expires_at: None,
        last_used_at: None,
        created_at: now,
        updated_at: now,
    }
}

/// Create a test library in the database
pub async fn create_test_library(
    db: &DatabaseConnection,
    name: &str,
    path: &str,
) -> libraries::Model {
    LibraryRepository::create(db, name, path, ScanningStrategy::Default)
        .await
        .unwrap()
}

/// Create a test library with specific book strategy
pub async fn create_test_library_with_strategies(
    db: &DatabaseConnection,
    name: &str,
    path: &str,
    series_strategy: codex::models::SeriesStrategy,
    book_strategy: codex::models::BookStrategy,
) -> libraries::Model {
    use codex::db::repositories::library::CreateLibraryParams;

    let params = CreateLibraryParams::new(name, path)
        .with_series_strategy(series_strategy)
        .with_book_strategy(book_strategy);

    LibraryRepository::create_with_params(db, params)
        .await
        .unwrap()
}

/// Create a test series in the database
pub async fn create_test_series(
    db: &DatabaseConnection,
    library: &libraries::Model,
    name: &str,
) -> series::Model {
    SeriesRepository::create(db, library.id, name, None)
        .await
        .unwrap()
}

/// Create a test book in the database with a specific file hash
pub async fn create_test_book_with_hash(
    db: &DatabaseConnection,
    _library: &libraries::Model,
    series: &series::Model,
    _title: &str,
    file_path: &str,
    file_hash: &str,
) -> books::Model {
    let book = books::Model {
        id: Uuid::new_v4(),
        series_id: series.id,
        library_id: series.library_id,
        file_path: file_path.to_string(),
        file_name: file_path
            .split('/')
            .next_back()
            .unwrap_or(file_path)
            .to_string(),
        file_size: 1024,
        file_hash: file_hash.to_string(),
        partial_hash: String::new(),
        format: "cbz".to_string(),
        page_count: 10,
        deleted: false,
        analyzed: false,
        analysis_error: None,
        modified_at: Utc::now(),
        created_at: Utc::now(),
        thumbnail_path: None,
        thumbnail_generated_at: None,
        updated_at: Utc::now(),
    };

    // Note: title is now stored in book_metadata table, not in books table
    BookRepository::create(db, &book, None).await.unwrap()
}
