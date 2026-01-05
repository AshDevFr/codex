#[path = "../common/mod.rs"]
mod common;

use codex::api::dto::auth::{LoginRequest, LoginResponse};
use codex::api::error::ErrorResponse;
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
    let state = create_test_auth_state(db);
    let app = create_test_router(state);

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

    let state = create_test_auth_state(db);
    let app = create_test_router(state);

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
    assert!(!login_response.user.is_admin);
}

#[tokio::test]
async fn test_login_success_with_email() {
    let (db, _temp_dir) = setup_test_db().await;

    // Create a test user
    let password = "secure_password_123";
    let password_hash = password::hash_password(password).unwrap();
    let user = create_test_user("emailuser", "email@example.com", &password_hash, false);
    UserRepository::create(&db, &user).await.unwrap();

    let state = create_test_auth_state(db);
    let app = create_test_router(state);

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

    let state = create_test_auth_state(db);
    let app = create_test_router(state);

    let login_request = LoginRequest {
        username: "admin".to_string(),
        password: password.to_string(),
    };

    let request = post_json_request("/api/v1/auth/login", &login_request);
    let (status, response): (StatusCode, Option<LoginResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let login_response = response.unwrap();
    assert!(login_response.user.is_admin);
}

#[tokio::test]
async fn test_login_wrong_password() {
    let (db, _temp_dir) = setup_test_db().await;

    // Create a test user
    let password = "correct_password";
    let password_hash = password::hash_password(password).unwrap();
    let user = create_test_user("user", "user@example.com", &password_hash, false);
    UserRepository::create(&db, &user).await.unwrap();

    let state = create_test_auth_state(db);
    let app = create_test_router(state);

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
    let state = create_test_auth_state(db);
    let app = create_test_router(state);

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

    let state = create_test_auth_state(db);
    let app = create_test_router(state);

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
    let state = create_test_auth_state(db);
    let app = create_test_router(state);

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

    let state = create_test_auth_state(db);

    // Generate token
    let token = state
        .jwt_service
        .generate_token(
            created_user.id,
            created_user.username.clone(),
            created_user.is_admin,
        )
        .unwrap();

    let app = create_test_router(state);

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
    let state = create_test_auth_state(db);
    let app = create_test_router(state);

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
    let state = create_test_auth_state(db);
    let app = create_test_router(state);

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
