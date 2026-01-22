//! Integration tests for system integrations API (Admin only)

#[path = "../common/mod.rs"]
mod common;

use codex::api::error::ErrorResponse;
use codex::api::routes::v1::dto::system_integrations::{
    CreateSystemIntegrationRequest, IntegrationStatusResponse, IntegrationTestResult,
    SystemIntegrationDto, SystemIntegrationsListResponse, UpdateSystemIntegrationRequest,
};
use codex::db::repositories::UserRepository;
use codex::utils::password;
use common::*;
use hyper::StatusCode;
use serde_json::json;

/// Set up a test encryption key for credential encryption
fn setup_test_encryption_key() {
    // Set a test encryption key if not already set
    if std::env::var("CODEX_ENCRYPTION_KEY").is_err() {
        std::env::set_var(
            "CODEX_ENCRYPTION_KEY",
            "AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8=",
        );
    }
}

// Helper to create user and token
async fn create_user_and_token(
    db: &sea_orm::DatabaseConnection,
    state: &codex::api::extractors::AuthState,
    username: &str,
    is_admin: bool,
) -> (uuid::Uuid, String) {
    let password_hash = password::hash_password("password123").unwrap();
    let user = create_test_user(
        username,
        &format!("{}@example.com", username),
        &password_hash,
        is_admin,
    );
    let created = UserRepository::create(db, &user).await.unwrap();
    let token = state
        .jwt_service
        .generate_token(created.id, created.username.clone(), created.get_role())
        .unwrap();
    (created.id, token)
}

// Helper to create an integration via API
async fn create_test_integration(
    state: &std::sync::Arc<codex::api::extractors::AuthState>,
    admin_token: &str,
    name: &str,
) -> SystemIntegrationDto {
    setup_test_encryption_key();
    let app = create_test_router(state.clone()).await;
    let body = CreateSystemIntegrationRequest {
        name: name.to_string(),
        display_name: format!("{} Display", name),
        integration_type: "metadata_provider".to_string(),
        credentials: Some(json!({"api_key": "test_key"})),
        config: Some(json!({"rate_limit": 60})),
        enabled: false,
    };
    let request = post_json_request_with_auth("/api/v1/admin/integrations", &body, admin_token);
    let (status, response): (StatusCode, Option<SystemIntegrationDto>) =
        make_json_request(app, request).await;
    assert_eq!(status, StatusCode::CREATED);
    response.unwrap()
}

// ============================================================================
// List Integrations Tests
// ============================================================================

#[tokio::test]
async fn test_list_integrations_empty() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let (_, admin_token) = create_user_and_token(&db, &state, "admin", true).await;
    let app = create_test_router(state).await;

    let request = get_request_with_auth("/api/v1/admin/integrations", &admin_token);
    let (status, response): (StatusCode, Option<SystemIntegrationsListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let list = response.unwrap();
    assert!(list.integrations.is_empty());
    assert_eq!(list.total, 0);
}

#[tokio::test]
async fn test_list_integrations_with_data() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let (_, admin_token) = create_user_and_token(&db, &state, "admin", true).await;

    // Create some integrations
    create_test_integration(&state, &admin_token, "mangaupdates").await;
    create_test_integration(&state, &admin_token, "anilist").await;

    let app = create_test_router(state).await;
    let request = get_request_with_auth("/api/v1/admin/integrations", &admin_token);
    let (status, response): (StatusCode, Option<SystemIntegrationsListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let list = response.unwrap();
    assert_eq!(list.integrations.len(), 2);
    assert_eq!(list.total, 2);
}

#[tokio::test]
async fn test_list_integrations_unauthorized() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let app = create_test_router(state).await;

    let request = get_request("/api/v1/admin/integrations");
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_list_integrations_non_admin_forbidden() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let (_, user_token) = create_user_and_token(&db, &state, "regularuser", false).await;
    let app = create_test_router(state).await;

    let request = get_request_with_auth("/api/v1/admin/integrations", &user_token);
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::FORBIDDEN);
}

// ============================================================================
// Create Integration Tests
// ============================================================================

#[tokio::test]
async fn test_create_integration_success() {
    setup_test_encryption_key();
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let (_, admin_token) = create_user_and_token(&db, &state, "admin", true).await;
    let app = create_test_router(state).await;

    let body = CreateSystemIntegrationRequest {
        name: "mangaupdates".to_string(),
        display_name: "MangaUpdates".to_string(),
        integration_type: "metadata_provider".to_string(),
        credentials: Some(json!({"api_key": "test_key_123"})),
        config: Some(json!({"rate_limit_per_minute": 60, "timeout_seconds": 30})),
        enabled: false,
    };

    let request = post_json_request_with_auth("/api/v1/admin/integrations", &body, &admin_token);
    let (status, body_bytes) = make_request(app, request).await;

    if status != StatusCode::CREATED {
        let body_str = String::from_utf8_lossy(&body_bytes);
        panic!("Expected CREATED, got {}: {}", status, body_str);
    }

    let response: SystemIntegrationDto = serde_json::from_slice(&body_bytes).unwrap();

    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(response.name, "mangaupdates");
    assert_eq!(response.display_name, "MangaUpdates");
    assert_eq!(response.integration_type, "metadata_provider");
    assert!(response.has_credentials); // Credentials were set
    assert!(!response.enabled);
    assert_eq!(response.health_status, "unknown");
}

#[tokio::test]
async fn test_create_integration_without_credentials() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let (_, admin_token) = create_user_and_token(&db, &state, "admin", true).await;
    let app = create_test_router(state).await;

    let body = CreateSystemIntegrationRequest {
        name: "test_provider".to_string(),
        display_name: "Test Provider".to_string(),
        integration_type: "notification".to_string(),
        credentials: None,
        config: Some(json!({})),
        enabled: true,
    };

    let request = post_json_request_with_auth("/api/v1/admin/integrations", &body, &admin_token);
    let (status, response): (StatusCode, Option<SystemIntegrationDto>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::CREATED);
    let integration = response.unwrap();
    assert!(!integration.has_credentials); // No credentials
    assert!(integration.enabled);
}

#[tokio::test]
async fn test_create_integration_invalid_name_uppercase() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let (_, admin_token) = create_user_and_token(&db, &state, "admin", true).await;
    let app = create_test_router(state).await;

    let body = CreateSystemIntegrationRequest {
        name: "MangaUpdates".to_string(), // Uppercase not allowed
        display_name: "MangaUpdates".to_string(),
        integration_type: "metadata_provider".to_string(),
        credentials: None,
        config: None,
        enabled: false,
    };

    let request = post_json_request_with_auth("/api/v1/admin/integrations", &body, &admin_token);
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_create_integration_invalid_name_with_dash() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let (_, admin_token) = create_user_and_token(&db, &state, "admin", true).await;
    let app = create_test_router(state).await;

    let body = CreateSystemIntegrationRequest {
        name: "manga-updates".to_string(), // Dash not allowed
        display_name: "MangaUpdates".to_string(),
        integration_type: "metadata_provider".to_string(),
        credentials: None,
        config: None,
        enabled: false,
    };

    let request = post_json_request_with_auth("/api/v1/admin/integrations", &body, &admin_token);
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_create_integration_invalid_type() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let (_, admin_token) = create_user_and_token(&db, &state, "admin", true).await;
    let app = create_test_router(state).await;

    let body = CreateSystemIntegrationRequest {
        name: "test_provider".to_string(),
        display_name: "Test".to_string(),
        integration_type: "invalid_type".to_string(), // Invalid type
        credentials: None,
        config: None,
        enabled: false,
    };

    let request = post_json_request_with_auth("/api/v1/admin/integrations", &body, &admin_token);
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_create_integration_duplicate_name() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let (_, admin_token) = create_user_and_token(&db, &state, "admin", true).await;

    // Create first integration
    create_test_integration(&state, &admin_token, "mangaupdates").await;

    // Try to create another with the same name
    let app = create_test_router(state).await;
    let body = CreateSystemIntegrationRequest {
        name: "mangaupdates".to_string(),
        display_name: "Another MangaUpdates".to_string(),
        integration_type: "metadata_provider".to_string(),
        credentials: None,
        config: None,
        enabled: false,
    };

    let request = post_json_request_with_auth("/api/v1/admin/integrations", &body, &admin_token);
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::CONFLICT);
}

#[tokio::test]
async fn test_create_integration_non_admin_forbidden() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let (_, user_token) = create_user_and_token(&db, &state, "regularuser", false).await;
    let app = create_test_router(state).await;

    let body = CreateSystemIntegrationRequest {
        name: "test".to_string(),
        display_name: "Test".to_string(),
        integration_type: "metadata_provider".to_string(),
        credentials: None,
        config: None,
        enabled: false,
    };

    let request = post_json_request_with_auth("/api/v1/admin/integrations", &body, &user_token);
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::FORBIDDEN);
}

// ============================================================================
// Get Integration Tests
// ============================================================================

#[tokio::test]
async fn test_get_integration_success() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let (_, admin_token) = create_user_and_token(&db, &state, "admin", true).await;

    let created = create_test_integration(&state, &admin_token, "mangaupdates").await;

    let app = create_test_router(state).await;
    let request = get_request_with_auth(
        &format!("/api/v1/admin/integrations/{}", created.id),
        &admin_token,
    );
    let (status, response): (StatusCode, Option<SystemIntegrationDto>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let integration = response.unwrap();
    assert_eq!(integration.id, created.id);
    assert_eq!(integration.name, "mangaupdates");
}

#[tokio::test]
async fn test_get_integration_not_found() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let (_, admin_token) = create_user_and_token(&db, &state, "admin", true).await;
    let app = create_test_router(state).await;

    let fake_id = uuid::Uuid::new_v4();
    let request = get_request_with_auth(
        &format!("/api/v1/admin/integrations/{}", fake_id),
        &admin_token,
    );
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_get_integration_credentials_never_returned() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let (_, admin_token) = create_user_and_token(&db, &state, "admin", true).await;

    let created = create_test_integration(&state, &admin_token, "mangaupdates").await;

    let app = create_test_router(state).await;
    let request = get_request_with_auth(
        &format!("/api/v1/admin/integrations/{}", created.id),
        &admin_token,
    );
    let (status, body) = make_request(app, request).await;

    assert_eq!(status, StatusCode::OK);

    // Parse as raw JSON to check for credentials field
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(json.get("credentials").is_none()); // No credentials field in response
    assert!(json.get("hasCredentials").is_some()); // But has_credentials flag is present
    assert_eq!(json["hasCredentials"], true);
}

// ============================================================================
// Update Integration Tests
// ============================================================================

#[tokio::test]
async fn test_update_integration_display_name() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let (_, admin_token) = create_user_and_token(&db, &state, "admin", true).await;

    let created = create_test_integration(&state, &admin_token, "mangaupdates").await;

    let app = create_test_router(state).await;
    let body = UpdateSystemIntegrationRequest {
        display_name: Some("MangaUpdates API v2".to_string()),
        credentials: None,
        config: None,
    };
    let request = patch_json_request_with_auth(
        &format!("/api/v1/admin/integrations/{}", created.id),
        &body,
        &admin_token,
    );
    let (status, response): (StatusCode, Option<SystemIntegrationDto>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let integration = response.unwrap();
    assert_eq!(integration.display_name, "MangaUpdates API v2");
}

#[tokio::test]
async fn test_update_integration_config() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let (_, admin_token) = create_user_and_token(&db, &state, "admin", true).await;

    let created = create_test_integration(&state, &admin_token, "mangaupdates").await;

    let app = create_test_router(state).await;
    let body = UpdateSystemIntegrationRequest {
        display_name: None,
        credentials: None,
        config: Some(json!({"rate_limit": 120, "timeout": 60})),
    };
    let request = patch_json_request_with_auth(
        &format!("/api/v1/admin/integrations/{}", created.id),
        &body,
        &admin_token,
    );
    let (status, response): (StatusCode, Option<SystemIntegrationDto>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let integration = response.unwrap();
    assert_eq!(integration.config["rate_limit"], 120);
    assert_eq!(integration.config["timeout"], 60);
}

#[tokio::test]
async fn test_update_integration_not_found() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let (_, admin_token) = create_user_and_token(&db, &state, "admin", true).await;
    let app = create_test_router(state).await;

    let fake_id = uuid::Uuid::new_v4();
    let body = UpdateSystemIntegrationRequest {
        display_name: Some("New Name".to_string()),
        credentials: None,
        config: None,
    };
    let request = patch_json_request_with_auth(
        &format!("/api/v1/admin/integrations/{}", fake_id),
        &body,
        &admin_token,
    );
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

// ============================================================================
// Delete Integration Tests
// ============================================================================

#[tokio::test]
async fn test_delete_integration_success() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let (_, admin_token) = create_user_and_token(&db, &state, "admin", true).await;

    let created = create_test_integration(&state, &admin_token, "mangaupdates").await;

    // Delete
    {
        let app = create_test_router(state.clone()).await;
        let request = delete_request_with_auth(
            &format!("/api/v1/admin/integrations/{}", created.id),
            &admin_token,
        );
        let (status, _body) = make_request(app, request).await;
        assert_eq!(status, StatusCode::NO_CONTENT);
    }

    // Verify it's gone
    {
        let app = create_test_router(state).await;
        let request = get_request_with_auth(
            &format!("/api/v1/admin/integrations/{}", created.id),
            &admin_token,
        );
        let (status, _): (StatusCode, Option<ErrorResponse>) =
            make_json_request(app, request).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }
}

#[tokio::test]
async fn test_delete_integration_not_found() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let (_, admin_token) = create_user_and_token(&db, &state, "admin", true).await;
    let app = create_test_router(state).await;

    let fake_id = uuid::Uuid::new_v4();
    let request = delete_request_with_auth(
        &format!("/api/v1/admin/integrations/{}", fake_id),
        &admin_token,
    );
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_delete_integration_non_admin_forbidden() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let (_, admin_token) = create_user_and_token(&db, &state, "admin", true).await;
    let (_, user_token) = create_user_and_token(&db, &state, "regularuser", false).await;

    let created = create_test_integration(&state, &admin_token, "mangaupdates").await;

    let app = create_test_router(state).await;
    let request = delete_request_with_auth(
        &format!("/api/v1/admin/integrations/{}", created.id),
        &user_token,
    );
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::FORBIDDEN);
}

// ============================================================================
// Enable/Disable Integration Tests
// ============================================================================

#[tokio::test]
async fn test_enable_integration() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let (_, admin_token) = create_user_and_token(&db, &state, "admin", true).await;

    let created = create_test_integration(&state, &admin_token, "mangaupdates").await;
    assert!(!created.enabled); // Starts disabled

    let app = create_test_router(state).await;
    let request = post_request_with_auth(
        &format!("/api/v1/admin/integrations/{}/enable", created.id),
        &admin_token,
    );
    let (status, response): (StatusCode, Option<IntegrationStatusResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let result = response.unwrap();
    assert!(result.integration.enabled);
    assert!(result.message.contains("enabled"));
}

#[tokio::test]
async fn test_disable_integration() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let (_, admin_token) = create_user_and_token(&db, &state, "admin", true).await;

    let created = create_test_integration(&state, &admin_token, "mangaupdates").await;

    // First enable it
    {
        let app = create_test_router(state.clone()).await;
        let request = post_request_with_auth(
            &format!("/api/v1/admin/integrations/{}/enable", created.id),
            &admin_token,
        );
        let (status, _): (StatusCode, Option<IntegrationStatusResponse>) =
            make_json_request(app, request).await;
        assert_eq!(status, StatusCode::OK);
    }

    // Then disable it
    let app = create_test_router(state).await;
    let request = post_request_with_auth(
        &format!("/api/v1/admin/integrations/{}/disable", created.id),
        &admin_token,
    );
    let (status, response): (StatusCode, Option<IntegrationStatusResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let result = response.unwrap();
    assert!(!result.integration.enabled);
    assert!(result.message.contains("disabled"));
}

#[tokio::test]
async fn test_enable_integration_not_found() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let (_, admin_token) = create_user_and_token(&db, &state, "admin", true).await;
    let app = create_test_router(state).await;

    let fake_id = uuid::Uuid::new_v4();
    let request = post_request_with_auth(
        &format!("/api/v1/admin/integrations/{}/enable", fake_id),
        &admin_token,
    );
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

// ============================================================================
// Test Integration Connection Tests
// ============================================================================

#[tokio::test]
async fn test_test_integration() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let (_, admin_token) = create_user_and_token(&db, &state, "admin", true).await;

    let created = create_test_integration(&state, &admin_token, "mangaupdates").await;

    let app = create_test_router(state).await;
    let request = post_request_with_auth(
        &format!("/api/v1/admin/integrations/{}/test", created.id),
        &admin_token,
    );
    let (status, response): (StatusCode, Option<IntegrationTestResult>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let result = response.unwrap();
    // Currently returns placeholder response
    assert!(result.success);
    assert!(!result.message.is_empty());
}

#[tokio::test]
async fn test_test_integration_not_found() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let (_, admin_token) = create_user_and_token(&db, &state, "admin", true).await;
    let app = create_test_router(state).await;

    let fake_id = uuid::Uuid::new_v4();
    let request = post_request_with_auth(
        &format!("/api/v1/admin/integrations/{}/test", fake_id),
        &admin_token,
    );
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

// ============================================================================
// Integration Type Validation Tests
// ============================================================================

#[tokio::test]
async fn test_create_integration_all_valid_types() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let (_, admin_token) = create_user_and_token(&db, &state, "admin", true).await;

    let valid_types = ["metadata_provider", "notification", "storage", "sync"];

    for (i, integration_type) in valid_types.iter().enumerate() {
        let app = create_test_router(state.clone()).await;
        let body = CreateSystemIntegrationRequest {
            name: format!("provider_{}", i),
            display_name: format!("Provider {}", i),
            integration_type: integration_type.to_string(),
            credentials: None,
            config: None,
            enabled: false,
        };
        let request =
            post_json_request_with_auth("/api/v1/admin/integrations", &body, &admin_token);
        let (status, _): (StatusCode, Option<SystemIntegrationDto>) =
            make_json_request(app, request).await;
        assert_eq!(
            status,
            StatusCode::CREATED,
            "Failed to create integration type: {}",
            integration_type
        );
    }
}
