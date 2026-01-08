#[path = "../common/mod.rs"]
mod common;

use codex::api::dto::setup::{
    ConfigureSettingsRequest, InitializeSetupRequest, InitializeSetupResponse, SetupStatusResponse,
};
use codex::api::error::ErrorResponse;
use codex::db::repositories::{SettingsRepository, UserRepository};
use codex::utils::password;
use common::*;
use hyper::StatusCode;
use serde_json::json;
use std::collections::HashMap;
use uuid::Uuid;

// ============================================================================
// Setup Status Tests
// ============================================================================

#[tokio::test]
async fn test_setup_status_no_users() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let app = create_test_router(state).await;

    // Fresh database with no users
    let request = get_request("/api/v1/setup/status");
    let (status, response): (StatusCode, Option<SetupStatusResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let status_response = response.unwrap();
    assert!(
        status_response.setup_required,
        "Setup should be required when no users exist"
    );
    assert!(
        !status_response.has_users,
        "has_users should be false when no users exist"
    );
}

#[tokio::test]
async fn test_setup_status_with_users() {
    let (db, _temp_dir) = setup_test_db().await;

    // Create a test user
    let password_hash = password::hash_password("password123").unwrap();
    let user = create_test_user("testuser", "test@example.com", &password_hash, false);
    UserRepository::create(&db, &user).await.unwrap();

    let state = create_test_auth_state(db).await;
    let app = create_test_router(state).await;

    // Database with a user
    let request = get_request("/api/v1/setup/status");
    let (status, response): (StatusCode, Option<SetupStatusResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let status_response = response.unwrap();
    assert!(
        !status_response.setup_required,
        "Setup should not be required when users exist"
    );
    assert!(
        status_response.has_users,
        "has_users should be true when users exist"
    );
}

// ============================================================================
// Initialize Setup Tests
// ============================================================================

#[tokio::test]
async fn test_initialize_setup_creates_admin_user() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let app = create_test_router(state).await;

    let init_request = InitializeSetupRequest {
        username: "admin".to_string(),
        email: "admin@example.com".to_string(),
        password: "securepassword123".to_string(),
    };

    let request = hyper::Request::builder()
        .method("POST")
        .uri("/api/v1/setup/initialize")
        .header("Content-Type", "application/json")
        .body(serde_json::to_string(&init_request).unwrap())
        .unwrap();

    let (status, response): (StatusCode, Option<InitializeSetupResponse>) =
        make_json_request(app, request).await;

    assert_eq!(
        status,
        StatusCode::OK,
        "Setup initialization should succeed"
    );

    let init_response = response.unwrap();

    // Verify response structure
    assert!(
        !init_response.access_token.is_empty(),
        "Should return access token"
    );
    assert_eq!(init_response.token_type, "Bearer");
    assert_eq!(init_response.expires_in, 24 * 3600);

    // Verify user details
    assert_eq!(init_response.user.username, "admin");
    assert_eq!(init_response.user.email, "admin@example.com");
    assert!(init_response.user.is_admin, "First user should be admin");
    assert!(
        init_response.user.email_verified,
        "First user should have email verified"
    );

    // Verify user was created in database
    let db_user = UserRepository::get_by_username(&db, "admin")
        .await
        .expect("Failed to query database")
        .expect("User should exist in database");

    assert_eq!(db_user.username, "admin");
    assert_eq!(db_user.email, "admin@example.com");
    assert!(db_user.is_admin, "User should be admin");
    assert!(db_user.is_active, "User should be active");
    assert!(db_user.email_verified, "User should have email verified");
}

#[tokio::test]
async fn test_initialize_setup_validation() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db).await;
    let app = create_test_router(state).await;

    // Test empty username
    let request_body = json!({
        "username": "",
        "email": "admin@example.com",
        "password": "securepassword123"
    });

    let request = hyper::Request::builder()
        .method("POST")
        .uri("/api/v1/setup/initialize")
        .header("Content-Type", "application/json")
        .body(request_body.to_string())
        .unwrap();

    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, request).await;

    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "Should reject empty username"
    );
}

#[tokio::test]
async fn test_initialize_setup_fails_when_users_exist() {
    let (db, _temp_dir) = setup_test_db().await;

    // Create a test user first
    let password_hash = password::hash_password("password123").unwrap();
    let user = create_test_user("existing", "existing@example.com", &password_hash, false);
    UserRepository::create(&db, &user).await.unwrap();

    let state = create_test_auth_state(db).await;
    let app = create_test_router(state).await;

    let init_request = InitializeSetupRequest {
        username: "admin".to_string(),
        email: "admin@example.com".to_string(),
        password: "securepassword123".to_string(),
    };

    let request = hyper::Request::builder()
        .method("POST")
        .uri("/api/v1/setup/initialize")
        .header("Content-Type", "application/json")
        .body(serde_json::to_string(&init_request).unwrap())
        .unwrap();

    let (status, response): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app, request).await;

    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "Setup should fail when users already exist"
    );

    let error = response.unwrap();
    assert!(
        error.error.contains("Setup already completed")
            || error.message.contains("Setup already completed"),
        "Error message should indicate setup is already completed"
    );
}

// ============================================================================
// Configure Settings Tests
// ============================================================================

#[tokio::test]
async fn test_configure_settings_requires_auth() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db).await;
    let app = create_test_router(state).await;

    let settings_request = ConfigureSettingsRequest {
        settings: HashMap::new(),
        skip_configuration: true,
    };

    let request = hyper::Request::builder()
        .method("PATCH")
        .uri("/api/v1/setup/settings")
        .header("Content-Type", "application/json")
        .body(serde_json::to_string(&settings_request).unwrap())
        .unwrap();

    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, request).await;

    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "Should require authentication"
    );
}

#[tokio::test]
async fn test_configure_settings_requires_admin() {
    let (db, _temp_dir) = setup_test_db().await;

    // Create a non-admin user
    let password_hash = password::hash_password("password123").unwrap();
    let user = create_test_user("user", "user@example.com", &password_hash, false);
    UserRepository::create(&db, &user).await.unwrap();

    let state = create_test_auth_state(db).await;
    let app = create_test_router(state.clone()).await;

    // Generate token for non-admin user
    let token = state
        .jwt_service
        .generate_token(user.id, user.username.clone(), false)
        .unwrap();

    let settings_request = ConfigureSettingsRequest {
        settings: HashMap::new(),
        skip_configuration: true,
    };

    let request = hyper::Request::builder()
        .method("PATCH")
        .uri("/api/v1/setup/settings")
        .header("Authorization", format!("Bearer {}", token))
        .header("Content-Type", "application/json")
        .body(serde_json::to_string(&settings_request).unwrap())
        .unwrap();

    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, request).await;

    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "Should require admin privileges"
    );
}
