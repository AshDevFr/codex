#[path = "../common/mod.rs"]
mod common;

use codex::api::dto::api_key::{
    ApiKeyDto, CreateApiKeyRequest, CreateApiKeyResponse, UpdateApiKeyRequest,
};
use codex::api::permissions::{Permission, ADMIN_PERMISSIONS, READONLY_PERMISSIONS};
use codex::db::repositories::{ApiKeyRepository, UserRepository};
use codex::utils::password;
use common::*;
use hyper::StatusCode;

/// Helper to create a user with API key permissions and return their JWT token
async fn create_user_with_api_key_perms(
    db: &sea_orm::DatabaseConnection,
    state: &std::sync::Arc<codex::api::extractors::AppState>,
    username: &str,
    is_admin: bool,
) -> (uuid::Uuid, String) {
    let hashed_password = password::hash_password("password123").unwrap();
    let mut permissions = if is_admin {
        ADMIN_PERMISSIONS.iter().cloned().collect()
    } else {
        let mut perms = READONLY_PERMISSIONS
            .iter()
            .cloned()
            .collect::<std::collections::HashSet<_>>();
        perms.insert(Permission::ApiKeysRead);
        perms.insert(Permission::ApiKeysWrite);
        perms.insert(Permission::ApiKeysDelete);
        perms
    };

    let permissions_vec: Vec<Permission> = permissions.iter().cloned().collect();
    let permissions_strings: Vec<String> = permissions_vec
        .iter()
        .map(|p| {
            serde_json::to_string(p)
                .unwrap()
                .trim_matches('"')
                .to_string()
        })
        .collect();
    let user = create_test_user_with_permissions(
        username,
        &format!("{}@test.com", username),
        &hashed_password,
        is_admin,
        permissions_strings,
    );

    let created_user = UserRepository::create(db, &user).await.unwrap();
    let token = state
        .jwt_service
        .generate_token(created_user.id, created_user.username.clone(), is_admin)
        .unwrap();

    (created_user.id, token)
}

// ============================================================================
// List API Keys Tests
// ============================================================================

#[tokio::test]
async fn test_list_api_keys() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_app_state(db.clone()).await;
    let app = create_test_router_with_app_state(state.clone());

    let (user_id, token) = create_user_with_api_key_perms(&db, &state, "testuser", false).await;

    // Create some API keys for the user
    let api_key1 = create_test_api_key(
        user_id,
        "Key 1",
        "hash1",
        "codex_abc",
        serde_json::json!([]),
    );
    let api_key2 = create_test_api_key(
        user_id,
        "Key 2",
        "hash2",
        "codex_def",
        serde_json::json!([]),
    );
    ApiKeyRepository::create(&db, &api_key1).await.unwrap();
    ApiKeyRepository::create(&db, &api_key2).await.unwrap();

    let request = get_request_with_auth("/api/v1/api-keys", &token);
    let (status, response): (StatusCode, Option<Vec<ApiKeyDto>>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let keys = response.expect("Expected API keys response");
    assert_eq!(keys.len(), 2, "Should return 2 API keys");
    assert!(keys.iter().any(|k| k.name == "Key 1"));
    assert!(keys.iter().any(|k| k.name == "Key 2"));
}

#[tokio::test]
async fn test_list_api_keys_requires_permission() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_app_state(db.clone()).await;
    let app = create_test_router_with_app_state(state.clone());

    let hashed_password = password::hash_password("password123").unwrap();
    let user = create_test_user("testuser", "test@test.com", &hashed_password, false);
    let created_user = UserRepository::create(&db, &user).await.unwrap();
    let token = state
        .jwt_service
        .generate_token(created_user.id, created_user.username.clone(), false)
        .unwrap();

    let request = get_request_with_auth("/api/v1/api-keys", &token);
    let (status, _): (StatusCode, Option<Vec<ApiKeyDto>>) = make_json_request(app, request).await;

    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "Should require ApiKeysRead permission"
    );
}

#[tokio::test]
async fn test_list_api_keys_only_shows_own() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_app_state(db.clone()).await;
    let app = create_test_router_with_app_state(state.clone());

    let (user1_id, token1) = create_user_with_api_key_perms(&db, &state, "user1", false).await;
    let (user2_id, _token2) = create_user_with_api_key_perms(&db, &state, "user2", false).await;

    // Create keys for both users
    let api_key1 = create_test_api_key(
        user1_id,
        "User1 Key",
        "hash1",
        "codex_abc",
        serde_json::json!([]),
    );
    let api_key2 = create_test_api_key(
        user2_id,
        "User2 Key",
        "hash2",
        "codex_def",
        serde_json::json!([]),
    );
    ApiKeyRepository::create(&db, &api_key1).await.unwrap();
    ApiKeyRepository::create(&db, &api_key2).await.unwrap();

    // User1 should only see their own key
    let request = get_request_with_auth("/api/v1/api-keys", &token1);
    let (status, response): (StatusCode, Option<Vec<ApiKeyDto>>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let keys = response.expect("Expected API keys response");
    assert_eq!(keys.len(), 1, "Should only return user1's key");
    assert_eq!(keys[0].name, "User1 Key");
}

// ============================================================================
// Get API Key Tests
// ============================================================================

#[tokio::test]
async fn test_get_api_key() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_app_state(db.clone()).await;
    let app = create_test_router_with_app_state(state.clone());

    let (user_id, token) = create_user_with_api_key_perms(&db, &state, "testuser", false).await;

    let api_key = create_test_api_key(
        user_id,
        "Test Key",
        "hash1",
        "codex_abc",
        serde_json::json!([]),
    );
    let created = ApiKeyRepository::create(&db, &api_key).await.unwrap();

    let request = get_request_with_auth(&format!("/api/v1/api-keys/{}", created.id), &token);
    let (status, response): (StatusCode, Option<ApiKeyDto>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let key = response.expect("Expected API key response");
    assert_eq!(key.id, created.id);
    assert_eq!(key.name, "Test Key");
    assert_eq!(key.user_id, user_id);
}

#[tokio::test]
async fn test_get_api_key_not_found() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_app_state(db.clone()).await;
    let app = create_test_router_with_app_state(state.clone());

    let (_user_id, token) = create_user_with_api_key_perms(&db, &state, "testuser", false).await;

    let fake_id = uuid::Uuid::new_v4();
    let request = get_request_with_auth(&format!("/api/v1/api-keys/{}", fake_id), &token);
    let (status, _): (StatusCode, Option<ApiKeyDto>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_get_api_key_other_user_forbidden() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_app_state(db.clone()).await;
    let app = create_test_router_with_app_state(state.clone());

    let (user1_id, _token1) = create_user_with_api_key_perms(&db, &state, "user1", false).await;
    let (_user2_id, token2) = create_user_with_api_key_perms(&db, &state, "user2", false).await;

    let api_key = create_test_api_key(
        user1_id,
        "User1 Key",
        "hash1",
        "codex_abc",
        serde_json::json!([]),
    );
    let created = ApiKeyRepository::create(&db, &api_key).await.unwrap();

    // User2 tries to access User1's key
    let request = get_request_with_auth(&format!("/api/v1/api-keys/{}", created.id), &token2);
    let (status, _): (StatusCode, Option<ApiKeyDto>) = make_json_request(app, request).await;

    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "Should not allow accessing other user's key"
    );
}

// ============================================================================
// Create API Key Tests
// ============================================================================

#[tokio::test]
async fn test_create_api_key() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_app_state(db.clone()).await;
    let app = create_test_router_with_app_state(state.clone());

    let (user_id, token) = create_user_with_api_key_perms(&db, &state, "testuser", false).await;

    let create_request = CreateApiKeyRequest {
        name: "My API Key".to_string(),
        permissions: None,
        expires_at: None,
    };

    let request = post_json_request_with_auth("/api/v1/api-keys", &create_request, &token);
    let (status, response): (StatusCode, Option<CreateApiKeyResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::CREATED);
    let response_data = response.expect("Expected API key creation response");
    assert_eq!(response_data.api_key.name, "My API Key");
    assert_eq!(response_data.api_key.user_id, user_id);
    assert!(!response_data.key.is_empty(), "Should return plaintext key");
    assert!(
        response_data.key.starts_with("codex_"),
        "Key should start with codex_"
    );

    // Verify key was created in database
    let db_key = ApiKeyRepository::get_by_id(&db, response_data.api_key.id)
        .await
        .unwrap()
        .expect("Key should exist in database");
    assert_eq!(db_key.name, "My API Key");
    assert_eq!(db_key.user_id, user_id);
}

#[tokio::test]
async fn test_create_api_key_with_custom_permissions() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_app_state(db.clone()).await;
    let app = create_test_router_with_app_state(state.clone());

    let (_user_id, token) = create_user_with_api_key_perms(&db, &state, "testuser", false).await;

    let create_request = CreateApiKeyRequest {
        name: "Custom Perm Key".to_string(),
        permissions: Some(vec!["libraries-read".to_string(), "books-read".to_string()]),
        expires_at: None,
    };

    let request = post_json_request_with_auth("/api/v1/api-keys", &create_request, &token);
    let (status, response): (StatusCode, Option<CreateApiKeyResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::CREATED);
    let response_data = response.expect("Expected API key creation response");

    // Verify permissions were set correctly
    let permissions: Vec<String> =
        serde_json::from_value(response_data.api_key.permissions.clone()).unwrap();
    assert_eq!(permissions.len(), 2);
    assert!(permissions.contains(&"libraries-read".to_string()));
    assert!(permissions.contains(&"books-read".to_string()));
}

#[tokio::test]
async fn test_create_api_key_with_expiration() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_app_state(db.clone()).await;
    let app = create_test_router_with_app_state(state.clone());

    let (_user_id, token) = create_user_with_api_key_perms(&db, &state, "testuser", false).await;

    let expires_at = chrono::Utc::now() + chrono::Duration::days(30);
    let create_request = CreateApiKeyRequest {
        name: "Expiring Key".to_string(),
        permissions: None,
        expires_at: Some(expires_at),
    };

    let request = post_json_request_with_auth("/api/v1/api-keys", &create_request, &token);
    let (status, response): (StatusCode, Option<CreateApiKeyResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::CREATED);
    let response_data = response.expect("Expected API key creation response");
    assert!(response_data.api_key.expires_at.is_some());
    assert_eq!(
        response_data.api_key.expires_at.unwrap().timestamp(),
        expires_at.timestamp()
    );
}

#[tokio::test]
async fn test_create_api_key_requires_permission() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_app_state(db.clone()).await;
    let app = create_test_router_with_app_state(state.clone());

    let hashed_password = password::hash_password("password123").unwrap();
    let user = create_test_user("testuser", "test@test.com", &hashed_password, false);
    let created_user = UserRepository::create(&db, &user).await.unwrap();
    let token = state
        .jwt_service
        .generate_token(created_user.id, created_user.username.clone(), false)
        .unwrap();

    let create_request = CreateApiKeyRequest {
        name: "My API Key".to_string(),
        permissions: None,
        expires_at: None,
    };

    let request = post_json_request_with_auth("/api/v1/api-keys", &create_request, &token);
    let (status, _): (StatusCode, Option<CreateApiKeyResponse>) =
        make_json_request(app, request).await;

    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "Should require ApiKeysWrite permission"
    );
}

#[tokio::test]
async fn test_create_api_key_cannot_grant_unauthorized_permissions() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_app_state(db.clone()).await;
    let app = create_test_router_with_app_state(state.clone());

    let (_user_id, token) = create_user_with_api_key_perms(&db, &state, "testuser", false).await;

    // Try to grant a permission the user doesn't have
    let create_request = CreateApiKeyRequest {
        name: "Bad Key".to_string(),
        permissions: Some(vec!["users-write".to_string()]), // User doesn't have this
        expires_at: None,
    };

    let request = post_json_request_with_auth("/api/v1/api-keys", &create_request, &token);
    let (status, _): (StatusCode, Option<CreateApiKeyResponse>) =
        make_json_request(app, request).await;

    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "Should not allow granting permissions user doesn't have"
    );
}

#[tokio::test]
async fn test_create_api_key_always_associated_with_creator() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_app_state(db.clone()).await;
    let app = create_test_router_with_app_state(state.clone());

    let (user_id, token) = create_user_with_api_key_perms(&db, &state, "testuser", false).await;

    let create_request = CreateApiKeyRequest {
        name: "My Key".to_string(),
        permissions: None,
        expires_at: None,
    };

    let request = post_json_request_with_auth("/api/v1/api-keys", &create_request, &token);
    let (status, response): (StatusCode, Option<CreateApiKeyResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::CREATED);
    let response_data = response.expect("Expected API key creation response");
    assert_eq!(
        response_data.api_key.user_id, user_id,
        "API key should be associated with creating user"
    );
}

// ============================================================================
// Update API Key Tests
// ============================================================================

#[tokio::test]
async fn test_update_api_key() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_app_state(db.clone()).await;
    let app = create_test_router_with_app_state(state.clone());

    let (user_id, token) = create_user_with_api_key_perms(&db, &state, "testuser", false).await;

    let api_key = create_test_api_key(
        user_id,
        "Original Name",
        "hash1",
        "codex_abc",
        serde_json::json!([]),
    );
    let created = ApiKeyRepository::create(&db, &api_key).await.unwrap();

    let update_request = UpdateApiKeyRequest {
        name: Some("Updated Name".to_string()),
        permissions: None,
        is_active: None,
        expires_at: None,
    };

    let request = put_json_request_with_auth(
        &format!("/api/v1/api-keys/{}", created.id),
        &update_request,
        &token,
    );
    let (status, response): (StatusCode, Option<ApiKeyDto>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let updated = response.expect("Expected updated API key");
    assert_eq!(updated.name, "Updated Name");
    assert_eq!(updated.id, created.id);
}

#[tokio::test]
async fn test_update_api_key_permissions() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_app_state(db.clone()).await;
    let app = create_test_router_with_app_state(state.clone());

    let (user_id, token) = create_user_with_api_key_perms(&db, &state, "testuser", false).await;

    let api_key = create_test_api_key(
        user_id,
        "Test Key",
        "hash1",
        "codex_abc",
        serde_json::json!([]),
    );
    let created = ApiKeyRepository::create(&db, &api_key).await.unwrap();

    let update_request = UpdateApiKeyRequest {
        name: None,
        permissions: Some(vec!["libraries-read".to_string(), "books-read".to_string()]),
        is_active: None,
        expires_at: None,
    };

    let request = put_json_request_with_auth(
        &format!("/api/v1/api-keys/{}", created.id),
        &update_request,
        &token,
    );
    let (status, response): (StatusCode, Option<ApiKeyDto>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let updated = response.expect("Expected updated API key");
    let permissions: Vec<String> = serde_json::from_value(updated.permissions).unwrap();
    assert_eq!(permissions.len(), 2);
    assert!(permissions.contains(&"libraries-read".to_string()));
}

#[tokio::test]
async fn test_update_api_key_deactivate() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_app_state(db.clone()).await;
    let app = create_test_router_with_app_state(state.clone());

    let (user_id, token) = create_user_with_api_key_perms(&db, &state, "testuser", false).await;

    let api_key = create_test_api_key(
        user_id,
        "Test Key",
        "hash1",
        "codex_abc",
        serde_json::json!([]),
    );
    let created = ApiKeyRepository::create(&db, &api_key).await.unwrap();

    let update_request = UpdateApiKeyRequest {
        name: None,
        permissions: None,
        is_active: Some(false),
        expires_at: None,
    };

    let request = put_json_request_with_auth(
        &format!("/api/v1/api-keys/{}", created.id),
        &update_request,
        &token,
    );
    let (status, response): (StatusCode, Option<ApiKeyDto>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let updated = response.expect("Expected updated API key");
    assert!(!updated.is_active, "Key should be deactivated");

    // Verify in database
    let db_key = ApiKeyRepository::get_by_id(&db, created.id)
        .await
        .unwrap()
        .unwrap();
    assert!(!db_key.is_active);
}

#[tokio::test]
async fn test_update_api_key_other_user_forbidden() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_app_state(db.clone()).await;
    let app = create_test_router_with_app_state(state.clone());

    let (user1_id, _token1) = create_user_with_api_key_perms(&db, &state, "user1", false).await;
    let (_user2_id, token2) = create_user_with_api_key_perms(&db, &state, "user2", false).await;

    let api_key = create_test_api_key(
        user1_id,
        "User1 Key",
        "hash1",
        "codex_abc",
        serde_json::json!([]),
    );
    let created = ApiKeyRepository::create(&db, &api_key).await.unwrap();

    let update_request = UpdateApiKeyRequest {
        name: Some("Hacked".to_string()),
        permissions: None,
        is_active: None,
        expires_at: None,
    };

    let request = put_json_request_with_auth(
        &format!("/api/v1/api-keys/{}", created.id),
        &update_request,
        &token2,
    );
    let (status, _): (StatusCode, Option<ApiKeyDto>) = make_json_request(app, request).await;

    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "Should not allow updating other user's key"
    );
}

// ============================================================================
// Delete API Key Tests
// ============================================================================

#[tokio::test]
async fn test_delete_api_key() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_app_state(db.clone()).await;
    let app = create_test_router_with_app_state(state.clone());

    let (user_id, token) = create_user_with_api_key_perms(&db, &state, "testuser", false).await;

    let api_key = create_test_api_key(
        user_id,
        "Test Key",
        "hash1",
        "codex_abc",
        serde_json::json!([]),
    );
    let created = ApiKeyRepository::create(&db, &api_key).await.unwrap();

    let request = delete_request_with_auth(&format!("/api/v1/api-keys/{}", created.id), &token);
    let (status, _body) = make_request(app.clone(), request).await;

    assert_eq!(status, StatusCode::NO_CONTENT);

    // Verify key was deleted
    let deleted = ApiKeyRepository::get_by_id(&db, created.id).await.unwrap();
    assert!(deleted.is_none(), "Key should be deleted from database");
}

#[tokio::test]
async fn test_delete_api_key_not_found() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_app_state(db.clone()).await;
    let app = create_test_router_with_app_state(state.clone());

    let (_user_id, token) = create_user_with_api_key_perms(&db, &state, "testuser", false).await;

    let fake_id = uuid::Uuid::new_v4();
    let request = delete_request_with_auth(&format!("/api/v1/api-keys/{}", fake_id), &token);
    let (status, _body) = make_request(app, request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_delete_api_key_other_user_forbidden() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_app_state(db.clone()).await;
    let app = create_test_router_with_app_state(state.clone());

    let (user1_id, _token1) = create_user_with_api_key_perms(&db, &state, "user1", false).await;
    let (_user2_id, token2) = create_user_with_api_key_perms(&db, &state, "user2", false).await;

    let api_key = create_test_api_key(
        user1_id,
        "User1 Key",
        "hash1",
        "codex_abc",
        serde_json::json!([]),
    );
    let created = ApiKeyRepository::create(&db, &api_key).await.unwrap();

    let request = delete_request_with_auth(&format!("/api/v1/api-keys/{}", created.id), &token2);
    let (status, _body) = make_request(app, request).await;

    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "Should not allow deleting other user's key"
    );
}

#[tokio::test]
async fn test_delete_api_key_requires_permission() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_app_state(db.clone()).await;
    let app = create_test_router_with_app_state(state.clone());

    let hashed_password = password::hash_password("password123").unwrap();
    let user = create_test_user("testuser", "test@test.com", &hashed_password, false);
    let created_user = UserRepository::create(&db, &user).await.unwrap();
    let token = state
        .jwt_service
        .generate_token(created_user.id, created_user.username.clone(), false)
        .unwrap();

    let fake_id = uuid::Uuid::new_v4();
    let request = delete_request_with_auth(&format!("/api/v1/api-keys/{}", fake_id), &token);
    let (status, _body) = make_request(app, request).await;

    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "Should require ApiKeysDelete permission"
    );
}

// ============================================================================
// Admin Access Tests
// ============================================================================

#[tokio::test]
async fn test_admin_can_access_own_keys() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_app_state(db.clone()).await;
    let app = create_test_router_with_app_state(state.clone());

    let (user_id, token) = create_user_with_api_key_perms(&db, &state, "admin", true).await;

    let api_key = create_test_api_key(
        user_id,
        "Admin Key",
        "hash1",
        "codex_abc",
        serde_json::json!([]),
    );
    let created = ApiKeyRepository::create(&db, &api_key).await.unwrap();

    let request = get_request_with_auth(&format!("/api/v1/api-keys/{}", created.id), &token);
    let (status, response): (StatusCode, Option<ApiKeyDto>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let key = response.expect("Expected API key response");
    assert_eq!(key.id, created.id);
}

// ============================================================================
// API Key Authentication Tests
// ============================================================================

#[tokio::test]
async fn test_created_api_key_can_authenticate() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_app_state(db.clone()).await;
    let app = create_test_router_with_app_state(state.clone());

    let (_user_id, token) = create_user_with_api_key_perms(&db, &state, "testuser", false).await;

    // Create an API key
    let create_request = CreateApiKeyRequest {
        name: "Auth Test Key".to_string(),
        permissions: None,
        expires_at: None,
    };

    let request = post_json_request_with_auth("/api/v1/api-keys", &create_request, &token);
    let (status, response): (StatusCode, Option<CreateApiKeyResponse>) =
        make_json_request(app.clone(), request).await;

    assert_eq!(status, StatusCode::CREATED);
    let response_data = response.expect("Expected API key creation response");
    let api_key = response_data.key;

    // Use the API key to authenticate
    let auth_request = get_request_with_api_key("/api/v1/api-keys", &api_key);
    let (auth_status, auth_response): (StatusCode, Option<Vec<ApiKeyDto>>) =
        make_json_request(app, auth_request).await;

    assert_eq!(
        auth_status,
        StatusCode::OK,
        "API key should authenticate successfully"
    );
    let keys = auth_response.expect("Expected API keys response");
    assert!(keys.iter().any(|k| k.id == response_data.api_key.id));
}
