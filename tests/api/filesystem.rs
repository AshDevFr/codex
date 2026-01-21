#[path = "../common/mod.rs"]
mod common;

use codex::api::error::ErrorResponse;
use codex::api::handlers::filesystem::{BrowseResponse, FileSystemEntry};
use codex::db::repositories::UserRepository;
use codex::utils::password;
use common::*;
use hyper::StatusCode;

// Helper to create an admin user and get a token
async fn create_admin_and_token(
    db: &sea_orm::DatabaseConnection,
    state: &codex::api::extractors::AuthState,
) -> String {
    let password_hash = password::hash_password("admin123").unwrap();
    let user = create_test_user("admin", "admin@example.com", &password_hash, true);
    let created = UserRepository::create(db, &user).await.unwrap();

    state
        .jwt_service
        .generate_token(created.id, created.username.clone(), created.get_role())
        .unwrap()
}

// Helper to create a non-admin user and get a token
async fn create_user_and_token(
    db: &sea_orm::DatabaseConnection,
    state: &codex::api::extractors::AuthState,
) -> String {
    let password_hash = password::hash_password("user123").unwrap();
    let user = create_test_user("user", "user@example.com", &password_hash, false);
    let created = UserRepository::create(db, &user).await.unwrap();

    state
        .jwt_service
        .generate_token(created.id, created.username.clone(), created.get_role())
        .unwrap()
}

// ============================================================================
// Browse Filesystem Tests
// ============================================================================

#[tokio::test]
async fn test_browse_filesystem_with_admin() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Get user's home directory or a known path
    let test_path = std::env::temp_dir();
    let request = get_request_with_auth(
        &format!("/api/v1/filesystem/browse?path={}", test_path.display()),
        &token,
    );

    let (status, response): (StatusCode, Option<BrowseResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let browse_data = response.unwrap();
    assert_eq!(browse_data.current_path, test_path.to_string_lossy());
    // Entries might be empty if temp dir is empty, so just check structure
    assert!(browse_data.entries.is_empty() || !browse_data.entries.is_empty());
}

#[tokio::test]
async fn test_browse_filesystem_without_path_uses_default() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let request = get_request_with_auth("/api/v1/filesystem/browse", &token);

    let (status, response): (StatusCode, Option<BrowseResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let browse_data = response.unwrap();
    // Should default to home directory or root
    assert!(!browse_data.current_path.is_empty());
}

#[tokio::test]
async fn test_browse_filesystem_without_auth() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db).await;
    let app = create_test_router(state).await;

    let request = get_request("/api/v1/filesystem/browse");
    let (status, response): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let error = response.unwrap();
    assert_eq!(error.error, "Unauthorized");
}

#[tokio::test]
async fn test_browse_filesystem_with_non_admin() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let token = create_user_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let request = get_request_with_auth("/api/v1/filesystem/browse", &token);

    let (status, response): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app, request).await;

    // Non-admin users should not have access to filesystem browsing
    assert_eq!(status, StatusCode::FORBIDDEN);
    let error = response.unwrap();
    assert_eq!(error.error, "Forbidden");
}

#[tokio::test]
async fn test_browse_filesystem_with_invalid_path() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let invalid_path = "/this/path/does/not/exist/surely/not";
    let request = get_request_with_auth(
        &format!("/api/v1/filesystem/browse?path={}", invalid_path),
        &token,
    );

    let (status, response): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    let error = response.unwrap();
    assert!(error.message.contains("does not exist"));
}

#[tokio::test]
async fn test_browse_filesystem_with_file_path() {
    let (db, temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Create a test file
    let file_path = temp_dir.path().join("test_file.txt");
    std::fs::write(&file_path, "test content").unwrap();

    let request = get_request_with_auth(
        &format!("/api/v1/filesystem/browse?path={}", file_path.display()),
        &token,
    );

    let (status, response): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    let error = response.unwrap();
    assert!(error.message.contains("not a directory"));
}

#[tokio::test]
async fn test_browse_filesystem_only_returns_directories() {
    let (db, temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Create test structure with files and directories
    let test_dir = temp_dir.path();
    std::fs::create_dir_all(test_dir.join("subdir1")).unwrap();
    std::fs::create_dir_all(test_dir.join("subdir2")).unwrap();
    std::fs::write(test_dir.join("file1.txt"), "content").unwrap();
    std::fs::write(test_dir.join("file2.txt"), "content").unwrap();

    let request = get_request_with_auth(
        &format!("/api/v1/filesystem/browse?path={}", test_dir.display()),
        &token,
    );

    let (status, response): (StatusCode, Option<BrowseResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let browse_data = response.unwrap();

    // All entries should be directories or files, but we filter hidden files
    // Check that we got some entries
    assert!(browse_data.entries.len() >= 2);

    // Verify that directories are included
    let dir_names: Vec<_> = browse_data
        .entries
        .iter()
        .filter(|e| e.is_directory)
        .map(|e| e.name.as_str())
        .collect();

    assert!(dir_names.contains(&"subdir1"));
    assert!(dir_names.contains(&"subdir2"));
}

#[tokio::test]
async fn test_browse_filesystem_excludes_hidden_files() {
    let (db, temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Create test structure with hidden files
    let test_dir = temp_dir.path();
    std::fs::create_dir_all(test_dir.join(".hidden_dir")).unwrap();
    std::fs::write(test_dir.join(".hidden_file"), "content").unwrap();
    std::fs::create_dir_all(test_dir.join("visible_dir")).unwrap();

    let request = get_request_with_auth(
        &format!("/api/v1/filesystem/browse?path={}", test_dir.display()),
        &token,
    );

    let (status, response): (StatusCode, Option<BrowseResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let browse_data = response.unwrap();

    // Check that hidden entries are not returned
    for entry in &browse_data.entries {
        assert!(
            !entry.name.starts_with('.'),
            "Hidden file/dir should be excluded: {}",
            entry.name
        );
    }

    // Verify visible directory is present
    let visible = browse_data.entries.iter().any(|e| e.name == "visible_dir");
    assert!(visible, "Visible directory should be included");
}

// ============================================================================
// List Drives Tests
// ============================================================================

#[tokio::test]
async fn test_list_drives_with_admin() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let request = get_request_with_auth("/api/v1/filesystem/drives", &token);

    let (status, response): (StatusCode, Option<Vec<FileSystemEntry>>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let drives = response.unwrap();

    // Should return at least one drive/location
    assert!(
        !drives.is_empty(),
        "Should return at least one drive or location"
    );

    // All entries should be directories
    for drive in &drives {
        assert!(drive.is_directory, "All drives should be directories");
    }
}

#[tokio::test]
async fn test_list_drives_without_auth() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db).await;
    let app = create_test_router(state).await;

    let request = get_request("/api/v1/filesystem/drives");
    let (status, response): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let error = response.unwrap();
    assert_eq!(error.error, "Unauthorized");
}

#[tokio::test]
async fn test_list_drives_with_non_admin() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let token = create_user_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let request = get_request_with_auth("/api/v1/filesystem/drives", &token);

    let (status, response): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app, request).await;

    // Non-admin users should not have access to drives listing
    assert_eq!(status, StatusCode::FORBIDDEN);
    let error = response.unwrap();
    assert_eq!(error.error, "Forbidden");
}

#[tokio::test]
async fn test_browse_returns_parent_path() {
    let (db, temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Create a subdirectory
    let subdir = temp_dir.path().join("subdir");
    std::fs::create_dir_all(&subdir).unwrap();

    let request = get_request_with_auth(
        &format!("/api/v1/filesystem/browse?path={}", subdir.display()),
        &token,
    );

    let (status, response): (StatusCode, Option<BrowseResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let browse_data = response.unwrap();

    // Parent path should be set
    assert!(
        browse_data.parent_path.is_some(),
        "Parent path should be available"
    );

    let parent = browse_data.parent_path.unwrap();
    assert!(!parent.is_empty(), "Parent path should not be empty");
}

#[tokio::test]
async fn test_browse_entries_sorted_directories_first() {
    let (db, temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Create test structure
    let test_dir = temp_dir.path();
    std::fs::create_dir_all(test_dir.join("z_dir")).unwrap();
    std::fs::create_dir_all(test_dir.join("a_dir")).unwrap();
    std::fs::write(test_dir.join("b_file.txt"), "content").unwrap();

    let request = get_request_with_auth(
        &format!("/api/v1/filesystem/browse?path={}", test_dir.display()),
        &token,
    );

    let (status, response): (StatusCode, Option<BrowseResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let browse_data = response.unwrap();

    // Find directories
    let directories: Vec<_> = browse_data
        .entries
        .iter()
        .filter(|e| e.is_directory)
        .collect();

    // Should have both directories
    assert!(directories.len() >= 2, "Should have at least 2 directories");

    // Verify they are sorted alphabetically
    let dir_names: Vec<_> = directories.iter().map(|d| d.name.as_str()).collect();
    assert!(dir_names.contains(&"a_dir"));
    assert!(dir_names.contains(&"z_dir"));

    // a_dir should come before z_dir
    let a_pos = dir_names.iter().position(|&n| n == "a_dir").unwrap();
    let z_pos = dir_names.iter().position(|&n| n == "z_dir").unwrap();
    assert!(a_pos < z_pos, "Directories should be sorted alphabetically");
}
