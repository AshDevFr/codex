#[path = "../common/mod.rs"]
mod common;

use codex::api::error::ErrorResponse;
use codex::api::routes::v1::dto::setup::{
    ConfigureSettingsRequest, InitializeSetupRequest, InitializeSetupResponse, SetupStatusResponse,
};
use codex::db::repositories::UserRepository;
use codex::utils::password;
use common::*;
use hyper::StatusCode;
use serde_json::json;
use std::collections::HashMap;

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
    assert!(
        !status_response.registration_enabled,
        "registration_enabled should default to false"
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

#[tokio::test]
async fn test_setup_status_registration_enabled() {
    use codex::db::repositories::SettingsRepository;

    let (db, _temp_dir) = setup_test_db().await;

    // Enable registration via settings
    SettingsRepository::set(
        &db,
        "auth.registration_enabled",
        "true".to_string(),
        uuid::Uuid::new_v4(),
        Some("Test setup".to_string()),
        None,
    )
    .await
    .expect("Failed to set registration setting");

    let state = create_test_auth_state(db).await;
    let app = create_test_router(state).await;

    let request = get_request("/api/v1/setup/status");
    let (status, response): (StatusCode, Option<SetupStatusResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let status_response = response.unwrap();
    assert!(
        status_response.registration_enabled,
        "registration_enabled should be true when setting is enabled"
    );
}

#[tokio::test]
async fn test_setup_status_registration_disabled() {
    use codex::db::repositories::SettingsRepository;

    let (db, _temp_dir) = setup_test_db().await;

    // Explicitly disable registration via settings
    SettingsRepository::set(
        &db,
        "auth.registration_enabled",
        "false".to_string(),
        uuid::Uuid::new_v4(),
        Some("Test setup".to_string()),
        None,
    )
    .await
    .expect("Failed to set registration setting");

    let state = create_test_auth_state(db).await;
    let app = create_test_router(state).await;

    let request = get_request("/api/v1/setup/status");
    let (status, response): (StatusCode, Option<SetupStatusResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let status_response = response.unwrap();
    assert!(
        !status_response.registration_enabled,
        "registration_enabled should be false when setting is disabled"
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
        password: "SecurePassword123!".to_string(),
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
    assert_eq!(
        init_response.user.role, "admin",
        "First user should be admin"
    );
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
    assert_eq!(db_user.role, "admin", "User should have admin role");
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
        .generate_token(user.id, user.username.clone(), user.get_role())
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

// ============================================================================
// Password and Email Validation Tests
// ============================================================================

#[tokio::test]
async fn test_password_too_short() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db).await;
    let app = create_test_router(state).await;

    let request_body = json!({
        "username": "admin",
        "email": "admin@example.com",
        "password": "Short1!"
    });

    let request = hyper::Request::builder()
        .method("POST")
        .uri("/api/v1/setup/initialize")
        .header("Content-Type", "application/json")
        .body(request_body.to_string())
        .unwrap();

    let (status, response): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    let error = response.unwrap();
    assert!(
        error.error.contains("at least 8 characters")
            || error.message.contains("at least 8 characters"),
        "Error should mention password length requirement"
    );
}

#[tokio::test]
async fn test_password_missing_uppercase() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db).await;
    let app = create_test_router(state).await;

    let request_body = json!({
        "username": "admin",
        "email": "admin@example.com",
        "password": "lowercase123!"
    });

    let request = hyper::Request::builder()
        .method("POST")
        .uri("/api/v1/setup/initialize")
        .header("Content-Type", "application/json")
        .body(request_body.to_string())
        .unwrap();

    let (status, response): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    let error = response.unwrap();
    assert!(
        error.error.contains("uppercase") || error.message.contains("uppercase"),
        "Error should mention uppercase letter requirement"
    );
}

#[tokio::test]
async fn test_password_missing_lowercase() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db).await;
    let app = create_test_router(state).await;

    let request_body = json!({
        "username": "admin",
        "email": "admin@example.com",
        "password": "UPPERCASE123!"
    });

    let request = hyper::Request::builder()
        .method("POST")
        .uri("/api/v1/setup/initialize")
        .header("Content-Type", "application/json")
        .body(request_body.to_string())
        .unwrap();

    let (status, response): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    let error = response.unwrap();
    assert!(
        error.error.contains("lowercase") || error.message.contains("lowercase"),
        "Error should mention lowercase letter requirement"
    );
}

#[tokio::test]
async fn test_password_missing_number() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db).await;
    let app = create_test_router(state).await;

    let request_body = json!({
        "username": "admin",
        "email": "admin@example.com",
        "password": "Password!@#"
    });

    let request = hyper::Request::builder()
        .method("POST")
        .uri("/api/v1/setup/initialize")
        .header("Content-Type", "application/json")
        .body(request_body.to_string())
        .unwrap();

    let (status, response): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    let error = response.unwrap();
    assert!(
        error.error.contains("number") || error.message.contains("number"),
        "Error should mention number requirement"
    );
}

#[tokio::test]
async fn test_password_missing_special_character() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db).await;
    let app = create_test_router(state).await;

    let request_body = json!({
        "username": "admin",
        "email": "admin@example.com",
        "password": "Password123"
    });

    let request = hyper::Request::builder()
        .method("POST")
        .uri("/api/v1/setup/initialize")
        .header("Content-Type", "application/json")
        .body(request_body.to_string())
        .unwrap();

    let (status, response): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    let error = response.unwrap();
    assert!(
        error.error.contains("special character") || error.message.contains("special character"),
        "Error should mention special character requirement"
    );
}

#[tokio::test]
async fn test_valid_password_accepted() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db).await;
    let app = create_test_router(state).await;

    let request_body = json!({
        "username": "admin",
        "email": "admin@example.com",
        "password": "SecurePass123!"
    });

    let request = hyper::Request::builder()
        .method("POST")
        .uri("/api/v1/setup/initialize")
        .header("Content-Type", "application/json")
        .body(request_body.to_string())
        .unwrap();

    let (status, _): (StatusCode, Option<InitializeSetupResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK, "Valid password should be accepted");
}

#[tokio::test]
async fn test_invalid_email_format() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db).await;
    let app = create_test_router(state).await;

    // Test various invalid email formats
    let invalid_emails = vec![
        "notanemail",
        "missing@domain",
        "@nodomain.com",
        "no-at-sign.com",
        "double@@example.com",
    ];

    for invalid_email in invalid_emails {
        let request_body = json!({
            "username": "admin",
            "email": invalid_email,
            "password": "SecurePass123!"
        });

        let request = hyper::Request::builder()
            .method("POST")
            .uri("/api/v1/setup/initialize")
            .header("Content-Type", "application/json")
            .body(request_body.to_string())
            .unwrap();

        let (status, response): (StatusCode, Option<ErrorResponse>) =
            make_json_request(app.clone(), request).await;

        assert_eq!(
            status,
            StatusCode::BAD_REQUEST,
            "Email '{}' should be rejected",
            invalid_email
        );
        let error = response.unwrap();
        assert!(
            error.error.contains("email") || error.message.contains("email"),
            "Error should mention email validation for '{}'",
            invalid_email
        );
    }
}

#[tokio::test]
async fn test_valid_email_accepted() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db).await;
    let app = create_test_router(state).await;

    let request_body = json!({
        "username": "admin",
        "email": "valid.email@example.com",
        "password": "SecurePass123!"
    });

    let request = hyper::Request::builder()
        .method("POST")
        .uri("/api/v1/setup/initialize")
        .header("Content-Type", "application/json")
        .body(request_body.to_string())
        .unwrap();

    let (status, _): (StatusCode, Option<InitializeSetupResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK, "Valid email should be accepted");
}

// ============================================================================
// Cookie Authentication Tests
// ============================================================================

#[tokio::test]
async fn test_initialize_setup_sets_auth_cookie() {
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db).await;
    let app = create_test_router(state).await;

    let init_request = InitializeSetupRequest {
        username: "admin".to_string(),
        email: "admin@example.com".to_string(),
        password: "SecurePassword123!".to_string(),
    };

    let request = hyper::Request::builder()
        .method("POST")
        .uri("/api/v1/setup/initialize")
        .header("Content-Type", "application/json")
        .body(serde_json::to_string(&init_request).unwrap())
        .unwrap();

    // Make request and get full response with headers
    let response = app
        .oneshot(request)
        .await
        .expect("Failed to execute request");

    assert_eq!(response.status(), StatusCode::OK);

    // Check that Set-Cookie header is present with auth_token
    let set_cookie = response
        .headers()
        .get("set-cookie")
        .expect("Response should include Set-Cookie header");

    let cookie_value = set_cookie.to_str().expect("Cookie should be valid UTF-8");
    assert!(
        cookie_value.starts_with("auth_token="),
        "Cookie should be named 'auth_token', got: {}",
        cookie_value
    );
    assert!(
        cookie_value.contains("HttpOnly"),
        "Cookie should be HttpOnly for security"
    );
    assert!(
        cookie_value.contains("SameSite=Lax"),
        "Cookie should have SameSite=Lax"
    );
    assert!(cookie_value.contains("Path=/"), "Cookie should have Path=/");

    // Also verify the response body is correct
    let body = response
        .into_body()
        .collect()
        .await
        .expect("Failed to read body")
        .to_bytes();
    let init_response: InitializeSetupResponse =
        serde_json::from_slice(&body).expect("Failed to parse response");

    assert!(!init_response.access_token.is_empty());
    assert_eq!(init_response.user.username, "admin");
}
