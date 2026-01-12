//! Integration tests for user integrations API

#[path = "../common/mod.rs"]
mod common;

use codex::api::dto::user_integrations::{
    ConnectIntegrationRequest, ConnectIntegrationResponse, OAuthCallbackRequest,
    SyncTriggerResponse, UpdateIntegrationSettingsRequest, UserIntegrationDto,
    UserIntegrationsListResponse,
};
use codex::api::error::ErrorResponse;
use codex::db::repositories::UserRepository;
use codex::utils::password;
use common::*;
use hyper::StatusCode;
use serde_json::json;

/// Set up a test encryption key for credential encryption
fn setup_test_encryption_key() {
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
        .generate_token(created.id, created.username, created.is_admin)
        .unwrap();
    (created.id, token)
}

// Helper to connect an API key integration
async fn connect_api_key_integration(
    state: &std::sync::Arc<codex::api::extractors::AuthState>,
    token: &str,
    name: &str,
) -> UserIntegrationDto {
    setup_test_encryption_key();
    let app = create_test_router(state.clone()).await;
    let body = ConnectIntegrationRequest {
        integration_name: name.to_string(),
        redirect_uri: None,
        api_key: Some("test-api-key-12345".to_string()),
    };
    let request = post_json_request_with_auth("/api/v1/user/integrations", &body, token);
    let (status, response): (StatusCode, Option<ConnectIntegrationResponse>) =
        make_json_request(app, request).await;
    assert_eq!(status, StatusCode::OK);
    let resp = response.unwrap();
    assert!(resp.connected);
    resp.integration.unwrap()
}

// ============================================================================
// List Integrations Tests
// ============================================================================

#[tokio::test]
async fn test_list_integrations_empty() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let (_, token) = create_user_and_token(&db, &state, "testuser", false).await;
    let app = create_test_router(state).await;

    let request = get_request_with_auth("/api/v1/user/integrations", &token);
    let (status, response): (StatusCode, Option<UserIntegrationsListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let list = response.unwrap();
    assert!(list.integrations.is_empty());
    // Should have available integrations even if none connected
    assert!(!list.available.is_empty());
}

#[tokio::test]
async fn test_list_integrations_with_connected() {
    setup_test_encryption_key();
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let (_, token) = create_user_and_token(&db, &state, "testuser", false).await;

    // Connect an integration
    connect_api_key_integration(&state, &token, "mangadex").await;

    let app = create_test_router(state).await;
    let request = get_request_with_auth("/api/v1/user/integrations", &token);
    let (status, response): (StatusCode, Option<UserIntegrationsListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let list = response.unwrap();
    assert_eq!(list.integrations.len(), 1);
    assert_eq!(list.integrations[0].integration_name, "mangadex");

    // MangaDex should be marked as connected in available list
    let mangadex_available = list.available.iter().find(|a| a.name == "mangadex");
    assert!(mangadex_available.is_some());
    assert!(mangadex_available.unwrap().connected);
}

#[tokio::test]
async fn test_list_integrations_unauthorized() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let app = create_test_router(state).await;

    let request = get_request("/api/v1/user/integrations");
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_list_integrations_user_isolation() {
    setup_test_encryption_key();
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let (_, user1_token) = create_user_and_token(&db, &state, "user1", false).await;
    let (_, user2_token) = create_user_and_token(&db, &state, "user2", false).await;

    // User1 connects an integration
    connect_api_key_integration(&state, &user1_token, "mangadex").await;

    // User2 should not see user1's integration
    let app = create_test_router(state).await;
    let request = get_request_with_auth("/api/v1/user/integrations", &user2_token);
    let (status, response): (StatusCode, Option<UserIntegrationsListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let list = response.unwrap();
    assert!(list.integrations.is_empty());
}

// ============================================================================
// Get Single Integration Tests
// ============================================================================

#[tokio::test]
async fn test_get_integration_success() {
    setup_test_encryption_key();
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let (_, token) = create_user_and_token(&db, &state, "testuser", false).await;

    // Connect an integration
    let created = connect_api_key_integration(&state, &token, "kavita").await;

    let app = create_test_router(state).await;
    let request = get_request_with_auth("/api/v1/user/integrations/kavita", &token);
    let (status, response): (StatusCode, Option<UserIntegrationDto>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let integration = response.unwrap();
    assert_eq!(integration.id, created.id);
    assert_eq!(integration.integration_name, "kavita");
}

#[tokio::test]
async fn test_get_integration_not_found() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let (_, token) = create_user_and_token(&db, &state, "testuser", false).await;
    let app = create_test_router(state).await;

    let request = get_request_with_auth("/api/v1/user/integrations/anilist", &token);
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_get_integration_other_user_not_found() {
    setup_test_encryption_key();
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let (_, user1_token) = create_user_and_token(&db, &state, "user1", false).await;
    let (_, user2_token) = create_user_and_token(&db, &state, "user2", false).await;

    // User1 connects an integration
    connect_api_key_integration(&state, &user1_token, "mangadex").await;

    // User2 should not be able to access it
    let app = create_test_router(state).await;
    let request = get_request_with_auth("/api/v1/user/integrations/mangadex", &user2_token);
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

// ============================================================================
// Connect Integration Tests
// ============================================================================

#[tokio::test]
async fn test_connect_api_key_integration() {
    setup_test_encryption_key();
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let (_, token) = create_user_and_token(&db, &state, "testuser", false).await;
    let app = create_test_router(state).await;

    let body = ConnectIntegrationRequest {
        integration_name: "mangadex".to_string(),
        redirect_uri: None,
        api_key: Some("my-api-key".to_string()),
    };
    let request = post_json_request_with_auth("/api/v1/user/integrations", &body, &token);
    let (status, response): (StatusCode, Option<ConnectIntegrationResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let resp = response.unwrap();
    assert!(resp.connected);
    assert!(resp.auth_url.is_none());
    assert!(resp.integration.is_some());

    let integration = resp.integration.unwrap();
    assert_eq!(integration.integration_name, "mangadex");
    assert!(integration.enabled);
}

#[tokio::test]
async fn test_connect_oauth_integration() {
    setup_test_encryption_key();
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let (_, token) = create_user_and_token(&db, &state, "testuser", false).await;
    let app = create_test_router(state).await;

    let body = ConnectIntegrationRequest {
        integration_name: "anilist".to_string(),
        redirect_uri: Some("https://app.example.com/callback".to_string()),
        api_key: None,
    };
    let request = post_json_request_with_auth("/api/v1/user/integrations", &body, &token);
    let (status, response): (StatusCode, Option<ConnectIntegrationResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let resp = response.unwrap();
    assert!(!resp.connected); // OAuth flow not complete
    assert!(resp.auth_url.is_some()); // Should have auth URL
    assert!(resp.integration.is_none()); // Not connected yet
}

#[tokio::test]
async fn test_connect_oauth_missing_redirect_uri() {
    setup_test_encryption_key();
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let (_, token) = create_user_and_token(&db, &state, "testuser", false).await;
    let app = create_test_router(state).await;

    let body = ConnectIntegrationRequest {
        integration_name: "anilist".to_string(),
        redirect_uri: None, // Missing for OAuth
        api_key: None,
    };
    let request = post_json_request_with_auth("/api/v1/user/integrations", &body, &token);
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_connect_api_key_missing_key() {
    setup_test_encryption_key();
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let (_, token) = create_user_and_token(&db, &state, "testuser", false).await;
    let app = create_test_router(state).await;

    let body = ConnectIntegrationRequest {
        integration_name: "mangadex".to_string(),
        redirect_uri: None,
        api_key: None, // Missing for API key auth
    };
    let request = post_json_request_with_auth("/api/v1/user/integrations", &body, &token);
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_connect_unknown_integration() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let (_, token) = create_user_and_token(&db, &state, "testuser", false).await;
    let app = create_test_router(state).await;

    let body = ConnectIntegrationRequest {
        integration_name: "unknown_provider".to_string(),
        redirect_uri: None,
        api_key: Some("key".to_string()),
    };
    let request = post_json_request_with_auth("/api/v1/user/integrations", &body, &token);
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_connect_already_connected() {
    setup_test_encryption_key();
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let (_, token) = create_user_and_token(&db, &state, "testuser", false).await;

    // Connect once
    connect_api_key_integration(&state, &token, "mangadex").await;

    // Try to connect again
    let app = create_test_router(state).await;
    let body = ConnectIntegrationRequest {
        integration_name: "mangadex".to_string(),
        redirect_uri: None,
        api_key: Some("another-key".to_string()),
    };
    let request = post_json_request_with_auth("/api/v1/user/integrations", &body, &token);
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::CONFLICT);
}

// ============================================================================
// OAuth Callback Tests
// ============================================================================

#[tokio::test]
async fn test_oauth_callback_success() {
    setup_test_encryption_key();
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let (_, token) = create_user_and_token(&db, &state, "testuser", false).await;
    let app = create_test_router(state).await;

    let body = OAuthCallbackRequest {
        code: "test-auth-code".to_string(),
        state: "csrf-state".to_string(),
        redirect_uri: "https://app.example.com/callback".to_string(),
    };
    let request =
        post_json_request_with_auth("/api/v1/user/integrations/anilist/callback", &body, &token);
    let (status, response): (StatusCode, Option<UserIntegrationDto>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let integration = response.unwrap();
    assert_eq!(integration.integration_name, "anilist");
    assert!(integration.connected);
}

#[tokio::test]
async fn test_oauth_callback_non_oauth_provider() {
    setup_test_encryption_key();
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let (_, token) = create_user_and_token(&db, &state, "testuser", false).await;
    let app = create_test_router(state).await;

    let body = OAuthCallbackRequest {
        code: "test-auth-code".to_string(),
        state: "csrf-state".to_string(),
        redirect_uri: "https://app.example.com/callback".to_string(),
    };
    // MangaDex uses API key, not OAuth
    let request =
        post_json_request_with_auth("/api/v1/user/integrations/mangadex/callback", &body, &token);
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
}

// ============================================================================
// Update Integration Tests
// ============================================================================

#[tokio::test]
async fn test_update_integration_settings() {
    setup_test_encryption_key();
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let (_, token) = create_user_and_token(&db, &state, "testuser", false).await;

    // Connect first
    connect_api_key_integration(&state, &token, "kavita").await;

    // Update settings
    let app = create_test_router(state).await;
    let body = UpdateIntegrationSettingsRequest {
        display_name: None,
        enabled: None,
        settings: Some(json!({"sync_progress": true, "sync_ratings": false})),
    };
    let request = patch_json_request_with_auth("/api/v1/user/integrations/kavita", &body, &token);
    let (status, response): (StatusCode, Option<UserIntegrationDto>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let integration = response.unwrap();
    assert_eq!(integration.settings["sync_progress"], true);
    assert_eq!(integration.settings["sync_ratings"], false);
}

#[tokio::test]
async fn test_update_integration_display_name() {
    setup_test_encryption_key();
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let (_, token) = create_user_and_token(&db, &state, "testuser", false).await;

    // Connect first
    connect_api_key_integration(&state, &token, "mangadex").await;

    // Update display name
    let app = create_test_router(state).await;
    let body = UpdateIntegrationSettingsRequest {
        display_name: Some("My MangaDex".to_string()),
        enabled: None,
        settings: None,
    };
    let request = patch_json_request_with_auth("/api/v1/user/integrations/mangadex", &body, &token);
    let (status, response): (StatusCode, Option<UserIntegrationDto>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let integration = response.unwrap();
    assert_eq!(integration.display_name, Some("My MangaDex".to_string()));
}

#[tokio::test]
async fn test_update_integration_enable_disable() {
    setup_test_encryption_key();
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let (_, token) = create_user_and_token(&db, &state, "testuser", false).await;

    // Connect first
    let created = connect_api_key_integration(&state, &token, "kavita").await;
    assert!(created.enabled);

    // Disable
    let app = create_test_router(state.clone()).await;
    let body = UpdateIntegrationSettingsRequest {
        display_name: None,
        enabled: Some(false),
        settings: None,
    };
    let request = patch_json_request_with_auth("/api/v1/user/integrations/kavita", &body, &token);
    let (status, response): (StatusCode, Option<UserIntegrationDto>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let integration = response.unwrap();
    assert!(!integration.enabled);

    // Re-enable
    let app = create_test_router(state).await;
    let body = UpdateIntegrationSettingsRequest {
        display_name: None,
        enabled: Some(true),
        settings: None,
    };
    let request = patch_json_request_with_auth("/api/v1/user/integrations/kavita", &body, &token);
    let (status, response): (StatusCode, Option<UserIntegrationDto>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let integration = response.unwrap();
    assert!(integration.enabled);
}

#[tokio::test]
async fn test_update_integration_not_found() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let (_, token) = create_user_and_token(&db, &state, "testuser", false).await;
    let app = create_test_router(state).await;

    let body = UpdateIntegrationSettingsRequest {
        display_name: Some("Test".to_string()),
        enabled: None,
        settings: None,
    };
    let request = patch_json_request_with_auth("/api/v1/user/integrations/anilist", &body, &token);
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

// ============================================================================
// Disconnect Integration Tests
// ============================================================================

#[tokio::test]
async fn test_disconnect_integration() {
    setup_test_encryption_key();
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let (_, token) = create_user_and_token(&db, &state, "testuser", false).await;

    // Connect first
    connect_api_key_integration(&state, &token, "mangadex").await;

    // Disconnect
    let app = create_test_router(state.clone()).await;
    let request = delete_request_with_auth("/api/v1/user/integrations/mangadex", &token);
    let (status, _): (StatusCode, Option<()>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NO_CONTENT);

    // Verify it's gone
    let app = create_test_router(state).await;
    let request = get_request_with_auth("/api/v1/user/integrations/mangadex", &token);
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_disconnect_integration_not_found() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let (_, token) = create_user_and_token(&db, &state, "testuser", false).await;
    let app = create_test_router(state).await;

    let request = delete_request_with_auth("/api/v1/user/integrations/anilist", &token);
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_disconnect_other_user_integration() {
    setup_test_encryption_key();
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let (_, user1_token) = create_user_and_token(&db, &state, "user1", false).await;
    let (_, user2_token) = create_user_and_token(&db, &state, "user2", false).await;

    // User1 connects
    connect_api_key_integration(&state, &user1_token, "mangadex").await;

    // User2 tries to disconnect
    let app = create_test_router(state).await;
    let request = delete_request_with_auth("/api/v1/user/integrations/mangadex", &user2_token);
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

// ============================================================================
// Sync Tests
// ============================================================================

#[tokio::test]
async fn test_trigger_sync() {
    setup_test_encryption_key();
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let (_, token) = create_user_and_token(&db, &state, "testuser", false).await;

    // Connect first
    connect_api_key_integration(&state, &token, "kavita").await;

    // Trigger sync
    let app = create_test_router(state).await;
    let request = post_request_with_auth("/api/v1/user/integrations/kavita/sync", &token);
    let (status, response): (StatusCode, Option<SyncTriggerResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let resp = response.unwrap();
    assert!(resp.started);
}

#[tokio::test]
async fn test_trigger_sync_not_connected() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let (_, token) = create_user_and_token(&db, &state, "testuser", false).await;
    let app = create_test_router(state).await;

    let request = post_request_with_auth("/api/v1/user/integrations/anilist/sync", &token);
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_trigger_sync_disabled_integration() {
    setup_test_encryption_key();
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let (_, token) = create_user_and_token(&db, &state, "testuser", false).await;

    // Connect and disable
    connect_api_key_integration(&state, &token, "kavita").await;

    let app = create_test_router(state.clone()).await;
    let body = UpdateIntegrationSettingsRequest {
        display_name: None,
        enabled: Some(false),
        settings: None,
    };
    let request = patch_json_request_with_auth("/api/v1/user/integrations/kavita", &body, &token);
    let (status, _): (StatusCode, Option<UserIntegrationDto>) =
        make_json_request(app, request).await;
    assert_eq!(status, StatusCode::OK);

    // Try to sync disabled integration
    let app = create_test_router(state).await;
    let request = post_request_with_auth("/api/v1/user/integrations/kavita/sync", &token);
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
}

// ============================================================================
// Available Integrations Tests
// ============================================================================

#[tokio::test]
async fn test_available_integrations_list() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let (_, token) = create_user_and_token(&db, &state, "testuser", false).await;
    let app = create_test_router(state).await;

    let request = get_request_with_auth("/api/v1/user/integrations", &token);
    let (status, response): (StatusCode, Option<UserIntegrationsListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let list = response.unwrap();

    // Should have all known providers
    let names: Vec<&str> = list.available.iter().map(|a| a.name.as_str()).collect();
    assert!(names.contains(&"anilist"));
    assert!(names.contains(&"myanimelist"));
    assert!(names.contains(&"kitsu"));
    assert!(names.contains(&"mangadex"));
    assert!(names.contains(&"kavita"));

    // Check auth types
    let anilist = list.available.iter().find(|a| a.name == "anilist").unwrap();
    assert_eq!(anilist.auth_type, "oauth2");

    let mangadex = list
        .available
        .iter()
        .find(|a| a.name == "mangadex")
        .unwrap();
    assert_eq!(mangadex.auth_type, "api_key");

    // Check features
    assert!(!anilist.features.is_empty());
    assert!(anilist.features.contains(&"sync_progress".to_string()));
}
