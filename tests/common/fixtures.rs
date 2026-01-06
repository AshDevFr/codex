use chrono::Utc;
use uuid::Uuid;

pub use codex::db::entities::{api_keys, books, pages, users};

/// Create a test user model with default values
pub fn create_test_user(
    username: &str,
    email: &str,
    password_hash: &str,
    is_admin: bool,
) -> users::Model {
    users::Model {
        id: Uuid::new_v4(),
        username: username.to_string(),
        email: email.to_string(),
        password_hash: password_hash.to_string(),
        is_admin,
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
    users::Model {
        id: Uuid::new_v4(),
        username: username.to_string(),
        email: email.to_string(),
        password_hash: password_hash.to_string(),
        is_admin,
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
        title: None,
        number: None,
        file_path: file_path.to_string(),
        file_name: file_name.to_string(),
        file_size: 1024,
        file_hash: file_hash.to_string(),
        format: format.to_string(),
        page_count,
        deleted: false,
        modified_at: now,
        created_at: now,
        updated_at: now,
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
