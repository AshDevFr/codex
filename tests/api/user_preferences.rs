//! Integration tests for user preferences endpoints

#[path = "../common/mod.rs"]
mod common;

use codex::api::dto::user_preferences::{
    BulkSetPreferencesRequest, DeletePreferenceResponse, SetPreferenceRequest,
    SetPreferencesResponse, UserPreferenceDto, UserPreferencesResponse,
};
use codex::api::error::ErrorResponse;
use codex::db::repositories::UserRepository;
use codex::utils::password;
use common::*;
use hyper::StatusCode;
use serde_json::json;
use std::collections::HashMap;

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

// ============================================================================
// Get All Preferences Tests
// ============================================================================

#[tokio::test]
async fn test_get_all_preferences_empty() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let (_, token) = create_user_and_token(&db, &state, "testuser", false).await;
    let app = create_test_router(state).await;

    let request = get_request_with_auth("/api/v1/user/preferences", &token);
    let (status, response): (StatusCode, Option<UserPreferencesResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let prefs = response.unwrap();
    assert!(prefs.preferences.is_empty());
}

#[tokio::test]
async fn test_get_all_preferences_with_data() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let (_, token) = create_user_and_token(&db, &state, "testuser", false).await;

    // Set some preferences first
    {
        let app = create_test_router(state.clone()).await;
        let body = SetPreferenceRequest {
            value: json!("dark"),
        };
        let request =
            put_json_request_with_auth("/api/v1/user/preferences/ui.theme", &body, &token);
        let (status, _): (StatusCode, Option<UserPreferenceDto>) =
            make_json_request(app, request).await;
        assert_eq!(status, StatusCode::OK);
    }
    {
        let app = create_test_router(state.clone()).await;
        let body = SetPreferenceRequest { value: json!(150) };
        let request =
            put_json_request_with_auth("/api/v1/user/preferences/reader.zoom", &body, &token);
        let (status, _): (StatusCode, Option<UserPreferenceDto>) =
            make_json_request(app, request).await;
        assert_eq!(status, StatusCode::OK);
    }

    // Get all preferences
    let app = create_test_router(state).await;
    let request = get_request_with_auth("/api/v1/user/preferences", &token);
    let (status, response): (StatusCode, Option<UserPreferencesResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let prefs = response.unwrap();
    assert_eq!(prefs.preferences.len(), 2);

    // Check that both preferences exist
    let keys: Vec<&str> = prefs.preferences.iter().map(|p| p.key.as_str()).collect();
    assert!(keys.contains(&"ui.theme"));
    assert!(keys.contains(&"reader.zoom"));
}

#[tokio::test]
async fn test_get_all_preferences_unauthorized() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let app = create_test_router(state).await;

    let request = get_request("/api/v1/user/preferences");
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_preferences_isolated_per_user() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let (_, token1) = create_user_and_token(&db, &state, "user1", false).await;
    let (_, token2) = create_user_and_token(&db, &state, "user2", false).await;

    // User 1 sets a preference
    {
        let app = create_test_router(state.clone()).await;
        let body = SetPreferenceRequest {
            value: json!("dark"),
        };
        let request =
            put_json_request_with_auth("/api/v1/user/preferences/ui.theme", &body, &token1);
        let (status, _): (StatusCode, Option<UserPreferenceDto>) =
            make_json_request(app, request).await;
        assert_eq!(status, StatusCode::OK);
    }

    // User 2 should see no preferences
    {
        let app = create_test_router(state.clone()).await;
        let request = get_request_with_auth("/api/v1/user/preferences", &token2);
        let (status, response): (StatusCode, Option<UserPreferencesResponse>) =
            make_json_request(app, request).await;

        assert_eq!(status, StatusCode::OK);
        assert!(response.unwrap().preferences.is_empty());
    }

    // User 1 should see their preference
    {
        let app = create_test_router(state.clone()).await;
        let request = get_request_with_auth("/api/v1/user/preferences", &token1);
        let (status, response): (StatusCode, Option<UserPreferencesResponse>) =
            make_json_request(app, request).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(response.unwrap().preferences.len(), 1);
    }
}

// ============================================================================
// Get Single Preference Tests
// ============================================================================

#[tokio::test]
async fn test_get_single_preference() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let (_, token) = create_user_and_token(&db, &state, "testuser", false).await;

    // Set preference first
    {
        let app = create_test_router(state.clone()).await;
        let body = SetPreferenceRequest {
            value: json!("dark"),
        };
        let request =
            put_json_request_with_auth("/api/v1/user/preferences/ui.theme", &body, &token);
        let (status, _): (StatusCode, Option<UserPreferenceDto>) =
            make_json_request(app, request).await;
        assert_eq!(status, StatusCode::OK);
    }

    // Get the preference
    let app = create_test_router(state).await;
    let request = get_request_with_auth("/api/v1/user/preferences/ui.theme", &token);
    let (status, response): (StatusCode, Option<UserPreferenceDto>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let pref = response.unwrap();
    assert_eq!(pref.key, "ui.theme");
    assert_eq!(pref.value, json!("dark"));
    assert_eq!(pref.value_type, "string");
}

#[tokio::test]
async fn test_get_single_preference_not_found() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let (_, token) = create_user_and_token(&db, &state, "testuser", false).await;
    let app = create_test_router(state).await;

    let request = get_request_with_auth("/api/v1/user/preferences/nonexistent.key", &token);
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

// ============================================================================
// Set Single Preference Tests
// ============================================================================

#[tokio::test]
async fn test_set_preference_string() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let (_, token) = create_user_and_token(&db, &state, "testuser", false).await;
    let app = create_test_router(state).await;

    let body = SetPreferenceRequest {
        value: json!("dark"),
    };
    let request = put_json_request_with_auth("/api/v1/user/preferences/ui.theme", &body, &token);
    let (status, response): (StatusCode, Option<UserPreferenceDto>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let pref = response.unwrap();
    assert_eq!(pref.key, "ui.theme");
    assert_eq!(pref.value, json!("dark"));
    assert_eq!(pref.value_type, "string");
}

#[tokio::test]
async fn test_set_preference_integer() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let (_, token) = create_user_and_token(&db, &state, "testuser", false).await;
    let app = create_test_router(state).await;

    let body = SetPreferenceRequest { value: json!(150) };
    let request = put_json_request_with_auth("/api/v1/user/preferences/reader.zoom", &body, &token);
    let (status, response): (StatusCode, Option<UserPreferenceDto>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let pref = response.unwrap();
    assert_eq!(pref.key, "reader.zoom");
    assert_eq!(pref.value, json!(150));
    assert_eq!(pref.value_type, "integer");
}

#[tokio::test]
async fn test_set_preference_float() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let (_, token) = create_user_and_token(&db, &state, "testuser", false).await;
    let app = create_test_router(state).await;

    let body = SetPreferenceRequest { value: json!(1.5) };
    let request =
        put_json_request_with_auth("/api/v1/user/preferences/reader.scale", &body, &token);
    let (status, response): (StatusCode, Option<UserPreferenceDto>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let pref = response.unwrap();
    assert_eq!(pref.key, "reader.scale");
    assert_eq!(pref.value, json!(1.5));
    assert_eq!(pref.value_type, "float");
}

#[tokio::test]
async fn test_set_preference_boolean() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let (_, token) = create_user_and_token(&db, &state, "testuser", false).await;
    let app = create_test_router(state).await;

    let body = SetPreferenceRequest { value: json!(true) };
    let request = put_json_request_with_auth(
        "/api/v1/user/preferences/ui.sidebar_collapsed",
        &body,
        &token,
    );
    let (status, response): (StatusCode, Option<UserPreferenceDto>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let pref = response.unwrap();
    assert_eq!(pref.key, "ui.sidebar_collapsed");
    assert_eq!(pref.value, json!(true));
    assert_eq!(pref.value_type, "boolean");
}

#[tokio::test]
async fn test_set_preference_json() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let (_, token) = create_user_and_token(&db, &state, "testuser", false).await;
    let app = create_test_router(state).await;

    let json_value = json!({"columns": ["title", "author"], "width": 200});
    let body = SetPreferenceRequest {
        value: json_value.clone(),
    };
    let request = put_json_request_with_auth(
        "/api/v1/user/preferences/library.grid_config",
        &body,
        &token,
    );
    let (status, response): (StatusCode, Option<UserPreferenceDto>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let pref = response.unwrap();
    assert_eq!(pref.key, "library.grid_config");
    assert_eq!(pref.value, json_value);
    assert_eq!(pref.value_type, "json");
}

#[tokio::test]
async fn test_set_preference_update_existing() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let (_, token) = create_user_and_token(&db, &state, "testuser", false).await;

    // Set initial value
    {
        let app = create_test_router(state.clone()).await;
        let body = SetPreferenceRequest {
            value: json!("light"),
        };
        let request =
            put_json_request_with_auth("/api/v1/user/preferences/ui.theme", &body, &token);
        let (status, _): (StatusCode, Option<UserPreferenceDto>) =
            make_json_request(app, request).await;
        assert_eq!(status, StatusCode::OK);
    }

    // Update value
    {
        let app = create_test_router(state.clone()).await;
        let body = SetPreferenceRequest {
            value: json!("dark"),
        };
        let request =
            put_json_request_with_auth("/api/v1/user/preferences/ui.theme", &body, &token);
        let (status, response): (StatusCode, Option<UserPreferenceDto>) =
            make_json_request(app, request).await;

        assert_eq!(status, StatusCode::OK);
        let pref = response.unwrap();
        assert_eq!(pref.value, json!("dark"));
    }

    // Verify only one preference exists
    {
        let app = create_test_router(state).await;
        let request = get_request_with_auth("/api/v1/user/preferences", &token);
        let (status, response): (StatusCode, Option<UserPreferencesResponse>) =
            make_json_request(app, request).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(response.unwrap().preferences.len(), 1);
    }
}

#[tokio::test]
async fn test_set_preference_null_value() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let (_, token) = create_user_and_token(&db, &state, "testuser", false).await;
    let app = create_test_router(state).await;

    // Null values are accepted and stored as json type
    let body = SetPreferenceRequest { value: json!(null) };
    let request = put_json_request_with_auth("/api/v1/user/preferences/ui.theme", &body, &token);
    let (status, response): (StatusCode, Option<UserPreferenceDto>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let pref = response.unwrap();
    assert_eq!(pref.value, json!(null));
    assert_eq!(pref.value_type, "json");
}

// ============================================================================
// Bulk Set Preferences Tests
// ============================================================================

#[tokio::test]
async fn test_bulk_set_preferences() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let (_, token) = create_user_and_token(&db, &state, "testuser", false).await;
    let app = create_test_router(state).await;

    let mut prefs = HashMap::new();
    prefs.insert("ui.theme".to_string(), json!("dark"));
    prefs.insert("reader.zoom".to_string(), json!(125));
    prefs.insert("ui.sidebar_collapsed".to_string(), json!(true));

    let body = BulkSetPreferencesRequest { preferences: prefs };
    let request = put_json_request_with_auth("/api/v1/user/preferences", &body, &token);
    let (status, response): (StatusCode, Option<SetPreferencesResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let result = response.unwrap();
    assert_eq!(result.updated, 3);
    assert_eq!(result.preferences.len(), 3);
}

#[tokio::test]
async fn test_bulk_set_preferences_empty() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let (_, token) = create_user_and_token(&db, &state, "testuser", false).await;
    let app = create_test_router(state).await;

    let body = BulkSetPreferencesRequest {
        preferences: HashMap::new(),
    };
    let request = put_json_request_with_auth("/api/v1/user/preferences", &body, &token);
    let (status, response): (StatusCode, Option<SetPreferencesResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let result = response.unwrap();
    assert_eq!(result.updated, 0);
}

#[tokio::test]
async fn test_bulk_set_preferences_updates_existing() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let (_, token) = create_user_and_token(&db, &state, "testuser", false).await;

    // Set initial value
    {
        let app = create_test_router(state.clone()).await;
        let body = SetPreferenceRequest {
            value: json!("light"),
        };
        let request =
            put_json_request_with_auth("/api/v1/user/preferences/ui.theme", &body, &token);
        let (status, _): (StatusCode, Option<UserPreferenceDto>) =
            make_json_request(app, request).await;
        assert_eq!(status, StatusCode::OK);
    }

    // Bulk update
    {
        let app = create_test_router(state.clone()).await;
        let mut prefs = HashMap::new();
        prefs.insert("ui.theme".to_string(), json!("dark")); // Update existing
        prefs.insert("reader.zoom".to_string(), json!(150)); // New

        let body = BulkSetPreferencesRequest { preferences: prefs };
        let request = put_json_request_with_auth("/api/v1/user/preferences", &body, &token);
        let (status, response): (StatusCode, Option<SetPreferencesResponse>) =
            make_json_request(app, request).await;

        assert_eq!(status, StatusCode::OK);
        let result = response.unwrap();
        assert_eq!(result.updated, 2);
    }

    // Verify total count is 2 (not 3)
    {
        let app = create_test_router(state).await;
        let request = get_request_with_auth("/api/v1/user/preferences", &token);
        let (status, response): (StatusCode, Option<UserPreferencesResponse>) =
            make_json_request(app, request).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(response.unwrap().preferences.len(), 2);
    }
}

// ============================================================================
// Delete Preference Tests
// ============================================================================

#[tokio::test]
async fn test_delete_preference() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let (_, token) = create_user_and_token(&db, &state, "testuser", false).await;

    // Set preference first
    {
        let app = create_test_router(state.clone()).await;
        let body = SetPreferenceRequest {
            value: json!("dark"),
        };
        let request =
            put_json_request_with_auth("/api/v1/user/preferences/ui.theme", &body, &token);
        let (status, _): (StatusCode, Option<UserPreferenceDto>) =
            make_json_request(app, request).await;
        assert_eq!(status, StatusCode::OK);
    }

    // Delete preference
    {
        let app = create_test_router(state.clone()).await;
        let request = delete_request_with_auth("/api/v1/user/preferences/ui.theme", &token);
        let (status, response): (StatusCode, Option<DeletePreferenceResponse>) =
            make_json_request(app, request).await;

        assert_eq!(status, StatusCode::OK);
        let result = response.unwrap();
        assert!(result.deleted);
    }

    // Verify preference is gone
    {
        let app = create_test_router(state).await;
        let request = get_request_with_auth("/api/v1/user/preferences/ui.theme", &token);
        let (status, _): (StatusCode, Option<ErrorResponse>) =
            make_json_request(app, request).await;

        assert_eq!(status, StatusCode::NOT_FOUND);
    }
}

#[tokio::test]
async fn test_delete_preference_not_found() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let (_, token) = create_user_and_token(&db, &state, "testuser", false).await;
    let app = create_test_router(state).await;

    let request = delete_request_with_auth("/api/v1/user/preferences/nonexistent.key", &token);
    let (status, response): (StatusCode, Option<DeletePreferenceResponse>) =
        make_json_request(app, request).await;

    // Returns OK with deleted=false for idempotency
    assert_eq!(status, StatusCode::OK);
    let result = response.unwrap();
    assert!(!result.deleted);
}

#[tokio::test]
async fn test_delete_preference_unauthorized() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let app = create_test_router(state).await;

    let request = delete_request("/api/v1/user/preferences/ui.theme");
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
}
