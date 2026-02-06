#[path = "../common/mod.rs"]
mod common;

use codex::api::error::ErrorResponse;
use codex::api::routes::v1::dto::auth::{LoginRequest, LoginResponse};
use codex::db::repositories::UserRepository;
use codex::utils::password;
use common::*;
use hyper::StatusCode;

// ============================================================================
// Health Check Tests
// ============================================================================

#[tokio::test]
async fn test_health_check_endpoint() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db).await;
    let app = create_test_router(state).await;

    let request = get_request("/health");
    let (status, body) = make_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let response = String::from_utf8(body.to_vec()).unwrap();
    assert_eq!(response, "OK");
}

// ============================================================================
// Login Endpoint Tests
// ============================================================================

#[tokio::test]
async fn test_login_success_with_username() {
    let (db, _temp_dir) = setup_test_db().await;

    // Create a test user
    let password = "secure_password_123";
    let password_hash = password::hash_password(password).unwrap();
    let user = create_test_user("testuser", "test@example.com", &password_hash, false);
    UserRepository::create(&db, &user).await.unwrap();

    let state = create_test_auth_state(db).await;
    let app = create_test_router(state).await;

    // Attempt login with username
    let login_request = LoginRequest {
        username: "testuser".to_string(),
        password: password.to_string(),
    };

    let request = post_json_request("/api/v1/auth/login", &login_request);
    let (status, response): (StatusCode, Option<LoginResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let login_response = response.unwrap();
    assert!(!login_response.access_token.is_empty());
    assert_eq!(login_response.token_type, "Bearer");
    assert_eq!(login_response.expires_in, 86400); // 24 hours in seconds
    assert_eq!(login_response.user.username, "testuser");
    assert_eq!(login_response.user.email, "test@example.com");
    assert_eq!(login_response.user.role, "reader");
}

#[tokio::test]
async fn test_login_success_with_email() {
    let (db, _temp_dir) = setup_test_db().await;

    // Create a test user
    let password = "secure_password_123";
    let password_hash = password::hash_password(password).unwrap();
    let user = create_test_user("emailuser", "email@example.com", &password_hash, false);
    UserRepository::create(&db, &user).await.unwrap();

    let state = create_test_auth_state(db).await;
    let app = create_test_router(state).await;

    // Attempt login with email
    let login_request = LoginRequest {
        username: "email@example.com".to_string(),
        password: password.to_string(),
    };

    let request = post_json_request("/api/v1/auth/login", &login_request);
    let (status, response): (StatusCode, Option<LoginResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let login_response = response.unwrap();
    assert!(!login_response.access_token.is_empty());
    assert_eq!(login_response.user.email, "email@example.com");
}

#[tokio::test]
async fn test_login_admin_user() {
    let (db, _temp_dir) = setup_test_db().await;

    // Create an admin user
    let password = "admin_password";
    let password_hash = password::hash_password(password).unwrap();
    let user = create_test_user("admin", "admin@example.com", &password_hash, true);
    UserRepository::create(&db, &user).await.unwrap();

    let state = create_test_auth_state(db).await;
    let app = create_test_router(state).await;

    let login_request = LoginRequest {
        username: "admin".to_string(),
        password: password.to_string(),
    };

    let request = post_json_request("/api/v1/auth/login", &login_request);
    let (status, response): (StatusCode, Option<LoginResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let login_response = response.unwrap();
    assert_eq!(login_response.user.role, "admin");
}

#[tokio::test]
async fn test_login_wrong_password() {
    let (db, _temp_dir) = setup_test_db().await;

    // Create a test user
    let password = "correct_password";
    let password_hash = password::hash_password(password).unwrap();
    let user = create_test_user("user", "user@example.com", &password_hash, false);
    UserRepository::create(&db, &user).await.unwrap();

    let state = create_test_auth_state(db).await;
    let app = create_test_router(state).await;

    // Attempt login with wrong password
    let login_request = LoginRequest {
        username: "user".to_string(),
        password: "wrong_password".to_string(),
    };

    let request = post_json_request("/api/v1/auth/login", &login_request);
    let (status, response): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let error = response.unwrap();
    assert_eq!(error.error, "Unauthorized");
    assert!(error.message.contains("Invalid"));
}

#[tokio::test]
async fn test_login_nonexistent_user() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db).await;
    let app = create_test_router(state).await;

    let login_request = LoginRequest {
        username: "nonexistent".to_string(),
        password: "password".to_string(),
    };

    let request = post_json_request("/api/v1/auth/login", &login_request);
    let (status, response): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let error = response.unwrap();
    assert_eq!(error.error, "Unauthorized");
}

#[tokio::test]
async fn test_login_inactive_user() {
    let (db, _temp_dir) = setup_test_db().await;

    // Create an inactive user
    let password = "password";
    let password_hash = password::hash_password(password).unwrap();
    let mut user = create_test_user("inactive", "inactive@example.com", &password_hash, false);
    user.is_active = false;
    UserRepository::create(&db, &user).await.unwrap();

    let state = create_test_auth_state(db).await;
    let app = create_test_router(state).await;

    let login_request = LoginRequest {
        username: "inactive".to_string(),
        password: password.to_string(),
    };

    let request = post_json_request("/api/v1/auth/login", &login_request);
    let (status, response): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let error = response.unwrap();
    assert!(error.message.contains("inactive") || error.message.contains("disabled"));
}

#[tokio::test]
async fn test_login_missing_fields() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db).await;
    let app = create_test_router(state).await;

    // Send malformed JSON
    let request = hyper::Request::builder()
        .method("POST")
        .uri("/api/v1/auth/login")
        .header("Content-Type", "application/json")
        .body("{\"username\":\"test\"}".to_string()) // Missing password
        .unwrap();

    let (status, _) = make_request(app, request).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}

// ============================================================================
// Logout Endpoint Tests
// ============================================================================

#[tokio::test]
async fn test_logout_with_valid_token() {
    let (db, _temp_dir) = setup_test_db().await;

    // Create and login a user
    let password = "password";
    let password_hash = password::hash_password(password).unwrap();
    let user = create_test_user("logoutuser", "logout@example.com", &password_hash, false);
    let created_user = UserRepository::create(&db, &user).await.unwrap();

    let state = create_test_auth_state(db).await;

    // Generate token
    let token = state
        .jwt_service
        .generate_token(
            created_user.id,
            created_user.username.clone(),
            created_user.get_role(),
        )
        .unwrap();

    let app = create_test_router(state).await;

    // Logout
    let request = hyper::Request::builder()
        .method("POST")
        .uri("/api/v1/auth/logout")
        .header("Authorization", format!("Bearer {}", token))
        .body(String::new())
        .unwrap();

    let (status, body) = make_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let response = String::from_utf8(body.to_vec()).unwrap();
    assert!(response.contains("Logged out successfully") || response.contains("success"));
}

#[tokio::test]
async fn test_logout_without_auth() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db).await;
    let app = create_test_router(state).await;

    let request = hyper::Request::builder()
        .method("POST")
        .uri("/api/v1/auth/logout")
        .body(String::new())
        .unwrap();

    let (status, response): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let error = response.unwrap();
    assert_eq!(error.error, "Unauthorized");
}

#[tokio::test]
async fn test_logout_with_invalid_token() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db).await;
    let app = create_test_router(state).await;

    let request = hyper::Request::builder()
        .method("POST")
        .uri("/api/v1/auth/logout")
        .header("Authorization", "Bearer invalid_token_here")
        .body(String::new())
        .unwrap();

    let (status, response): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let error = response.unwrap();
    assert_eq!(error.error, "Unauthorized");
}

// ============================================================================
// HTTP Basic Authentication Tests
// ============================================================================

#[tokio::test]
async fn test_basic_auth_success() {
    use base64::{Engine as _, engine::general_purpose::STANDARD};

    let (db, _temp_dir) = setup_test_db().await;

    // Create an admin user (admins have all permissions)
    let password = "secure_password_123";
    let password_hash = password::hash_password(password).unwrap();
    let user = create_test_user("basicadmin", "basicadmin@example.com", &password_hash, true);
    UserRepository::create(&db, &user).await.unwrap();

    let state = create_test_auth_state(db).await;
    let app = create_test_router(state).await;

    // Encode credentials in base64 (format: "username:password")
    let credentials = format!("{}:{}", "basicadmin", password);
    let encoded = STANDARD.encode(credentials.as_bytes());

    // Make authenticated request to logout endpoint (which requires auth but has no DB queries)
    let request = hyper::Request::builder()
        .method("POST")
        .uri("/api/v1/auth/logout")
        .header("Authorization", format!("Basic {}", encoded))
        .body(String::new())
        .unwrap();

    let (status, _) = make_request(app, request).await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn test_basic_auth_wrong_password() {
    use base64::{Engine as _, engine::general_purpose::STANDARD};

    let (db, _temp_dir) = setup_test_db().await;

    // Create a test user
    let password = "correct_password";
    let password_hash = password::hash_password(password).unwrap();
    let user = create_test_user("basicuser2", "basic2@example.com", &password_hash, false);
    UserRepository::create(&db, &user).await.unwrap();

    let state = create_test_auth_state(db).await;
    let app = create_test_router(state).await;

    // Encode wrong credentials
    let credentials = format!("{}:{}", "basicuser2", "wrong_password");
    let encoded = STANDARD.encode(credentials.as_bytes());

    let request = hyper::Request::builder()
        .method("POST")
        .uri("/api/v1/auth/logout")
        .header("Authorization", format!("Basic {}", encoded))
        .body(String::new())
        .unwrap();

    let (status, response): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let error = response.unwrap();
    assert_eq!(error.error, "Unauthorized");
}

#[tokio::test]
async fn test_basic_auth_nonexistent_user() {
    use base64::{Engine as _, engine::general_purpose::STANDARD};

    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db).await;
    let app = create_test_router(state).await;

    // Encode credentials for non-existent user
    let credentials = "nonexistent:password";
    let encoded = STANDARD.encode(credentials.as_bytes());

    let request = hyper::Request::builder()
        .method("POST")
        .uri("/api/v1/auth/logout")
        .header("Authorization", format!("Basic {}", encoded))
        .body(String::new())
        .unwrap();

    let (status, response): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let error = response.unwrap();
    assert_eq!(error.error, "Unauthorized");
}

#[tokio::test]
async fn test_basic_auth_invalid_encoding() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db).await;
    let app = create_test_router(state).await;

    // Use invalid base64 encoding
    let request = hyper::Request::builder()
        .method("POST")
        .uri("/api/v1/auth/logout")
        .header("Authorization", "Basic not_valid_base64!!!")
        .body(String::new())
        .unwrap();

    let (status, response): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let error = response.unwrap();
    assert_eq!(error.error, "Unauthorized");
}

#[tokio::test]
async fn test_basic_auth_invalid_format() {
    use base64::{Engine as _, engine::general_purpose::STANDARD};

    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db).await;
    let app = create_test_router(state).await;

    // Encode credentials without colon separator
    let credentials = "usernameonly";
    let encoded = STANDARD.encode(credentials.as_bytes());

    let request = hyper::Request::builder()
        .method("POST")
        .uri("/api/v1/auth/logout")
        .header("Authorization", format!("Basic {}", encoded))
        .body(String::new())
        .unwrap();

    let (status, response): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let error = response.unwrap();
    assert_eq!(error.error, "Unauthorized");
}

#[tokio::test]
async fn test_basic_auth_inactive_user() {
    use base64::{Engine as _, engine::general_purpose::STANDARD};

    let (db, _temp_dir) = setup_test_db().await;

    // Create an inactive user
    let password = "password";
    let password_hash = password::hash_password(password).unwrap();
    let mut user = create_test_user(
        "inactive_basic",
        "inactive_basic@example.com",
        &password_hash,
        false,
    );
    user.is_active = false;
    UserRepository::create(&db, &user).await.unwrap();

    let state = create_test_auth_state(db).await;
    let app = create_test_router(state).await;

    // Encode credentials
    let credentials = format!("{}:{}", "inactive_basic", password);
    let encoded = STANDARD.encode(credentials.as_bytes());

    let request = hyper::Request::builder()
        .method("POST")
        .uri("/api/v1/auth/logout")
        .header("Authorization", format!("Basic {}", encoded))
        .body(String::new())
        .unwrap();

    let (status, response): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let error = response.unwrap();
    assert!(error.message.contains("inactive"));
}

// Removed test_www_authenticate_header_on_401 test
// We no longer send WWW-Authenticate header to prevent browser basic auth popup

// ============================================================================
// Registration Disabled Tests
// ============================================================================

#[tokio::test]
async fn test_register_disabled_by_default() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let app = create_test_router(state).await;

    // Seed the auth.registration_enabled setting with false (default)
    use codex::db::repositories::SettingsRepository;

    // The setting should already be seeded as false by the migration
    // But let's verify it exists and is false
    let setting = SettingsRepository::get(&db, "auth.registration_enabled")
        .await
        .unwrap();

    if let Some(s) = setting {
        assert_eq!(
            s.value, "false",
            "Registration should be disabled by default"
        );
    }

    // Attempt to register
    let register_request = serde_json::json!({
        "username": "newuser",
        "email": "newuser@example.com",
        "password": "securepassword123"
    });

    let request = hyper::Request::builder()
        .method("POST")
        .uri("/api/v1/auth/register")
        .header("Content-Type", "application/json")
        .body(serde_json::to_string(&register_request).unwrap())
        .unwrap();

    let (status, response): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app, request).await;

    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "Registration should be forbidden when disabled"
    );
    let error = response.unwrap();
    assert!(
        error.message.contains("disabled") || error.error.contains("disabled"),
        "Error should indicate registration is disabled"
    );
}

#[tokio::test]
async fn test_register_succeeds_when_enabled() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let app = create_test_router(state).await;

    // Enable registration
    use codex::db::repositories::SettingsRepository;
    use uuid::Uuid;

    let admin_id = Uuid::new_v4();
    SettingsRepository::set(
        &db,
        "auth.registration_enabled",
        "true".to_string(),
        admin_id,
        Some("Enable registration for testing".to_string()),
        None,
    )
    .await
    .unwrap();

    // Now attempt to register
    let register_request = serde_json::json!({
        "username": "newuser",
        "email": "newuser@example.com",
        "password": "securepassword123"
    });

    let request = hyper::Request::builder()
        .method("POST")
        .uri("/api/v1/auth/register")
        .header("Content-Type", "application/json")
        .body(serde_json::to_string(&register_request).unwrap())
        .unwrap();

    let (status, _): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;

    // Registration should succeed (though may require email verification)
    assert!(
        status == StatusCode::OK || status == StatusCode::CREATED,
        "Registration should succeed when enabled, got status: {}",
        status
    );

    // Verify user was created
    let user = UserRepository::get_by_username(&db, "newuser")
        .await
        .unwrap();
    assert!(user.is_some(), "User should be created in database");
}
