#[path = "../common/mod.rs"]
mod common;

use codex::api::dto::settings::{
    BulkSettingUpdate, BulkUpdateSettingsRequest, SettingDto, SettingHistoryDto,
    UpdateSettingRequest,
};
use codex::db::repositories::{SettingsRepository, UserRepository};
use codex::utils::password;
use common::*;
use hyper::StatusCode;

/// Helper to create an admin user and return their JWT token
async fn create_admin_and_token(
    db: &sea_orm::DatabaseConnection,
    state: &std::sync::Arc<codex::api::extractors::AppState>,
) -> String {
    let hashed_password = password::hash_password("admin123").unwrap();
    let user = create_test_user("admin", "admin@test.com", &hashed_password, true);

    let created_user = UserRepository::create(db, &user).await.unwrap();
    state
        .jwt_service
        .generate_token(created_user.id, created_user.email, true)
        .unwrap()
}

/// Helper to create a regular user and return their JWT token
async fn create_regular_user_and_token(
    db: &sea_orm::DatabaseConnection,
    state: &std::sync::Arc<codex::api::extractors::AppState>,
) -> String {
    let hashed_password = password::hash_password("user123").unwrap();
    let user = create_test_user("user", "user@test.com", &hashed_password, false);

    let created_user = UserRepository::create(db, &user).await.unwrap();
    state
        .jwt_service
        .generate_token(created_user.id, created_user.email, false)
        .unwrap()
}

#[tokio::test]
async fn test_list_all_settings() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_app_state(db.clone()).await;
    let app = create_test_router_with_app_state(state.clone());

    let token = create_admin_and_token(&db, &state).await;

    let request = get_request_with_auth("/api/v1/admin/settings", &token);
    let (status, response): (StatusCode, Option<Vec<SettingDto>>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let settings = response.expect("Expected settings response");
    assert!(!settings.is_empty(), "Should have seeded settings");

    // Verify we have the expected categories
    let categories: std::collections::HashSet<String> =
        settings.iter().map(|s| s.category.clone()).collect();
    assert!(categories.contains("Scanner"));
    assert!(categories.contains("Application"));
    assert!(categories.contains("Logging"));
    assert!(categories.contains("Task"));
}

#[tokio::test]
async fn test_list_settings_requires_admin() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_app_state(db.clone()).await;
    let app = create_test_router_with_app_state(state.clone());

    let token = create_regular_user_and_token(&db, &state).await;

    let request = get_request_with_auth("/api/v1/admin/settings", &token);
    let (status, _response): (StatusCode, Option<Vec<SettingDto>>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_get_single_setting() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_app_state(db.clone()).await;
    let app = create_test_router_with_app_state(state.clone());

    let token = create_admin_and_token(&db, &state).await;

    let request = get_request_with_auth(
        "/api/v1/admin/settings/scanner.max_concurrent_scans",
        &token,
    );
    let (status, response): (StatusCode, Option<SettingDto>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let setting = response.expect("Expected setting response");
    assert_eq!(setting.key, "scanner.max_concurrent_scans");
    assert_eq!(setting.value, "2");
    assert_eq!(setting.category, "Scanner");
}

#[tokio::test]
async fn test_get_nonexistent_setting() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_app_state(db.clone()).await;
    let app = create_test_router_with_app_state(state.clone());

    let token = create_admin_and_token(&db, &state).await;

    let request = get_request_with_auth("/api/v1/admin/settings/nonexistent.setting", &token);
    let (status, _response): (StatusCode, Option<SettingDto>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_update_setting() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_app_state(db.clone()).await;
    let app = create_test_router_with_app_state(state.clone());

    let token = create_admin_and_token(&db, &state).await;

    let update = UpdateSettingRequest {
        value: "4".to_string(),
        change_reason: Some("Increase concurrency for testing".to_string()),
    };

    let request = put_json_request_with_auth(
        "/api/v1/admin/settings/scanner.max_concurrent_scans",
        &update,
        &token,
    );
    let (status, response): (StatusCode, Option<SettingDto>) =
        make_json_request(app.clone(), request).await;

    assert_eq!(status, StatusCode::OK);
    let setting = response.expect("Expected setting response");
    assert_eq!(setting.value, "4");

    // Verify the change was persisted
    let persisted_setting = SettingsRepository::get(&db, "scanner.max_concurrent_scans")
        .await
        .unwrap()
        .expect("Setting should exist");
    assert_eq!(persisted_setting.value, "4");

    // Verify version was incremented
    assert_eq!(setting.version, 2);
}

#[tokio::test]
async fn test_update_setting_with_invalid_value() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_app_state(db.clone()).await;
    let app = create_test_router_with_app_state(state.clone());

    let token = create_admin_and_token(&db, &state).await;

    // Try to set an invalid value (out of range)
    let update = UpdateSettingRequest {
        value: "100".to_string(), // max is 10
        change_reason: None,
    };

    let request = put_json_request_with_auth(
        "/api/v1/admin/settings/scanner.max_concurrent_scans",
        &update,
        &token,
    );
    let (status, _response): (StatusCode, Option<SettingDto>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_update_setting_requires_admin() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_app_state(db.clone()).await;
    let app = create_test_router_with_app_state(state.clone());

    let token = create_regular_user_and_token(&db, &state).await;

    let update = UpdateSettingRequest {
        value: "4".to_string(),
        change_reason: None,
    };

    let request = put_json_request_with_auth(
        "/api/v1/admin/settings/scanner.max_concurrent_scans",
        &update,
        &token,
    );
    let (status, _response): (StatusCode, Option<SettingDto>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_bulk_update_settings() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_app_state(db.clone()).await;
    let app = create_test_router_with_app_state(state.clone());

    let token = create_admin_and_token(&db, &state).await;

    let updates = BulkUpdateSettingsRequest {
        updates: vec![
            BulkSettingUpdate {
                key: "scanner.max_concurrent_scans".to_string(),
                value: "4".to_string(),
            },
            BulkSettingUpdate {
                key: "scanner.scan_timeout_minutes".to_string(),
                value: "180".to_string(),
            },
        ],
        change_reason: Some("Bulk update for testing".to_string()),
    };

    let request = post_json_request_with_auth("/api/v1/admin/settings/bulk", &updates, &token);
    let (status, response): (StatusCode, Option<Vec<SettingDto>>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let settings = response.expect("Expected settings response");
    assert_eq!(settings.len(), 2);

    // Verify both updates were applied
    let max_scans = settings
        .iter()
        .find(|s| s.key == "scanner.max_concurrent_scans")
        .expect("Should have max_concurrent_scans");
    assert_eq!(max_scans.value, "4");

    let timeout = settings
        .iter()
        .find(|s| s.key == "scanner.scan_timeout_minutes")
        .expect("Should have scan_timeout_minutes");
    assert_eq!(timeout.value, "180");
}

#[tokio::test]
async fn test_bulk_update_rolls_back_on_error() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_app_state(db.clone()).await;
    let app = create_test_router_with_app_state(state.clone());

    let token = create_admin_and_token(&db, &state).await;

    // Get original value
    let original = SettingsRepository::get(&db, "scanner.max_concurrent_scans")
        .await
        .unwrap()
        .expect("Setting should exist");

    let updates = BulkUpdateSettingsRequest {
        updates: vec![
            BulkSettingUpdate {
                key: "scanner.max_concurrent_scans".to_string(),
                value: "4".to_string(),
            },
            BulkSettingUpdate {
                key: "scanner.scan_timeout_minutes".to_string(),
                value: "99999".to_string(), // Invalid: exceeds max value
            },
        ],
        change_reason: None,
    };

    let request = post_json_request_with_auth("/api/v1/admin/settings/bulk", &updates, &token);
    let (status, _response): (StatusCode, Option<Vec<SettingDto>>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::BAD_REQUEST);

    // Verify the first update was rolled back
    let current = SettingsRepository::get(&db, "scanner.max_concurrent_scans")
        .await
        .unwrap()
        .expect("Setting should exist");
    assert_eq!(current.value, original.value);
}

#[tokio::test]
async fn test_reset_setting_to_default() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_app_state(db.clone()).await;
    let app = create_test_router_with_app_state(state.clone());

    let token = create_admin_and_token(&db, &state).await;

    // First, update the setting
    let update = UpdateSettingRequest {
        value: "8".to_string(),
        change_reason: None,
    };
    let request = put_json_request_with_auth(
        "/api/v1/admin/settings/scanner.max_concurrent_scans",
        &update,
        &token,
    );
    make_json_request::<SettingDto>(app.clone(), request).await;

    // Now reset it
    let request = post_request_with_auth(
        "/api/v1/admin/settings/scanner.max_concurrent_scans/reset",
        &token,
    );
    let (status, response): (StatusCode, Option<SettingDto>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let setting = response.expect("Expected setting response");
    assert_eq!(setting.value, setting.default_value);
    assert_eq!(setting.value, "2"); // Default value
}

#[tokio::test]
async fn test_get_setting_history() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_app_state(db.clone()).await;
    let app = create_test_router_with_app_state(state.clone());

    let token = create_admin_and_token(&db, &state).await;

    // Make some updates to create history
    for value in ["3", "4", "5"] {
        let update = UpdateSettingRequest {
            value: value.to_string(),
            change_reason: Some(format!("Update to {}", value)),
        };
        let request = put_json_request_with_auth(
            "/api/v1/admin/settings/scanner.max_concurrent_scans",
            &update,
            &token,
        );
        make_json_request::<SettingDto>(app.clone(), request).await;
    }

    // Get history
    let request = get_request_with_auth(
        "/api/v1/admin/settings/scanner.max_concurrent_scans/history",
        &token,
    );
    let (status, response): (StatusCode, Option<Vec<SettingHistoryDto>>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let history = response.expect("Expected history response");
    assert_eq!(history.len(), 3);

    // Verify history is in reverse chronological order (most recent first)
    assert_eq!(history[0].new_value, "5");
    assert_eq!(history[1].new_value, "4");
    assert_eq!(history[2].new_value, "3");

    // Verify old_value tracking
    assert_eq!(history[0].old_value, "4");
    assert_eq!(history[1].old_value, "3");
    assert_eq!(history[2].old_value, "2");
}

#[tokio::test]
async fn test_get_history_empty_for_unchanged_setting() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_app_state(db.clone()).await;
    let app = create_test_router_with_app_state(state.clone());

    let token = create_admin_and_token(&db, &state).await;

    // Get history for a setting that hasn't been changed
    let request = get_request_with_auth("/api/v1/admin/settings/logging.console/history", &token);
    let (status, response): (StatusCode, Option<Vec<SettingHistoryDto>>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let history = response.expect("Expected history response");
    assert_eq!(history.len(), 0);
}

#[tokio::test]
async fn test_filter_settings_by_category() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_app_state(db.clone()).await;
    let app = create_test_router_with_app_state(state.clone());

    let token = create_admin_and_token(&db, &state).await;

    let request = get_request_with_auth("/api/v1/admin/settings?category=Scanner", &token);
    let (status, response): (StatusCode, Option<Vec<SettingDto>>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let settings = response.expect("Expected settings response");

    // All returned settings should be in Scanner category
    assert!(!settings.is_empty());
    for setting in &settings {
        assert_eq!(setting.category, "Scanner");
    }

    // Verify we have the expected scanner settings
    let keys: Vec<&str> = settings.iter().map(|s| s.key.as_str()).collect();
    assert!(keys.contains(&"scanner.max_concurrent_scans"));
    assert!(keys.contains(&"scanner.scan_timeout_minutes"));
    assert!(keys.contains(&"scanner.retry_failed_files"));
    assert!(keys.contains(&"scanner.auto_analyze_concurrency"));
}

#[tokio::test]
async fn test_hot_reload_mechanism() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_app_state(db.clone()).await;
    let app = create_test_router_with_app_state(state.clone());

    let token = create_admin_and_token(&db, &state).await;

    // Get initial value from service
    let initial_value = state
        .settings_service
        .get_int("scanner.max_concurrent_scans", 2)
        .await
        .unwrap();
    assert_eq!(initial_value, 2);

    // Update via API
    let update = UpdateSettingRequest {
        value: "6".to_string(),
        change_reason: None,
    };
    let request = put_json_request_with_auth(
        "/api/v1/admin/settings/scanner.max_concurrent_scans",
        &update,
        &token,
    );
    let (status, _): (StatusCode, Option<SettingDto>) = make_json_request(app, request).await;
    assert_eq!(status, StatusCode::OK);

    // Manually reload cache (in production this happens automatically)
    state.settings_service.reload().await.unwrap();

    // Verify the new value is available from service
    let updated_value = state
        .settings_service
        .get_int("scanner.max_concurrent_scans", 2)
        .await
        .unwrap();
    assert_eq!(updated_value, 6);
}
