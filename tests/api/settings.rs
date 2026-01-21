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
        .generate_token(
            created_user.id,
            created_user.username.clone(),
            created_user.get_role(),
        )
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
        .generate_token(
            created_user.id,
            created_user.username.clone(),
            created_user.get_role(),
        )
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
    assert!(categories.contains("Authentication"));
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
        "/api/v1/admin/settings/scanner.scan_timeout_minutes",
        &token,
    );
    let (status, response): (StatusCode, Option<SettingDto>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let setting = response.expect("Expected setting response");
    assert_eq!(setting.key, "scanner.scan_timeout_minutes");
    assert_eq!(setting.value, "120");
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
        value: "240".to_string(),
        change_reason: Some("Increase timeout for testing".to_string()),
    };

    let request = put_json_request_with_auth(
        "/api/v1/admin/settings/scanner.scan_timeout_minutes",
        &update,
        &token,
    );
    let (status, response): (StatusCode, Option<SettingDto>) =
        make_json_request(app.clone(), request).await;

    assert_eq!(status, StatusCode::OK);
    let setting = response.expect("Expected setting response");
    assert_eq!(setting.value, "240");

    // Verify the change was persisted
    let persisted_setting = SettingsRepository::get(&db, "scanner.scan_timeout_minutes")
        .await
        .unwrap()
        .expect("Setting should exist");
    assert_eq!(persisted_setting.value, "240");

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
        value: "2000".to_string(), // max is 1440
        change_reason: None,
    };

    let request = put_json_request_with_auth(
        "/api/v1/admin/settings/scanner.scan_timeout_minutes",
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
        "/api/v1/admin/settings/scanner.scan_timeout_minutes",
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
                key: "scanner.scan_timeout_minutes".to_string(),
                value: "180".to_string(),
            },
            BulkSettingUpdate {
                key: "scanner.retry_failed_files".to_string(),
                value: "true".to_string(),
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
    let timeout = settings
        .iter()
        .find(|s| s.key == "scanner.scan_timeout_minutes")
        .expect("Should have scan_timeout_minutes");
    assert_eq!(timeout.value, "180");

    let retry = settings
        .iter()
        .find(|s| s.key == "scanner.retry_failed_files")
        .expect("Should have retry_failed_files");
    assert_eq!(retry.value, "true");
}

#[tokio::test]
async fn test_bulk_update_rolls_back_on_error() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_app_state(db.clone()).await;
    let app = create_test_router_with_app_state(state.clone());

    let token = create_admin_and_token(&db, &state).await;

    // Get original value
    let original = SettingsRepository::get(&db, "scanner.scan_timeout_minutes")
        .await
        .unwrap()
        .expect("Setting should exist");

    let updates = BulkUpdateSettingsRequest {
        updates: vec![
            BulkSettingUpdate {
                key: "scanner.scan_timeout_minutes".to_string(),
                value: "240".to_string(),
            },
            BulkSettingUpdate {
                key: "scanner.retry_failed_files".to_string(),
                value: "99999".to_string(), // Invalid: not a boolean
            },
        ],
        change_reason: None,
    };

    let request = post_json_request_with_auth("/api/v1/admin/settings/bulk", &updates, &token);
    let (status, _response): (StatusCode, Option<Vec<SettingDto>>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::BAD_REQUEST);

    // Verify the first update was rolled back
    let current = SettingsRepository::get(&db, "scanner.scan_timeout_minutes")
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
        value: "240".to_string(),
        change_reason: None,
    };
    let request = put_json_request_with_auth(
        "/api/v1/admin/settings/scanner.scan_timeout_minutes",
        &update,
        &token,
    );
    make_json_request::<SettingDto>(app.clone(), request).await;

    // Now reset it
    let request = post_request_with_auth(
        "/api/v1/admin/settings/scanner.scan_timeout_minutes/reset",
        &token,
    );
    let (status, response): (StatusCode, Option<SettingDto>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let setting = response.expect("Expected setting response");
    assert_eq!(setting.value, setting.default_value);
    assert_eq!(setting.value, "120"); // Default value
}

#[tokio::test]
async fn test_get_setting_history() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_app_state(db.clone()).await;
    let app = create_test_router_with_app_state(state.clone());

    let token = create_admin_and_token(&db, &state).await;

    // Make some updates to create history
    for value in ["180", "240", "300"] {
        let update = UpdateSettingRequest {
            value: value.to_string(),
            change_reason: Some(format!("Update to {}", value)),
        };
        let request = put_json_request_with_auth(
            "/api/v1/admin/settings/scanner.scan_timeout_minutes",
            &update,
            &token,
        );
        make_json_request::<SettingDto>(app.clone(), request).await;
    }

    // Get history
    let request = get_request_with_auth(
        "/api/v1/admin/settings/scanner.scan_timeout_minutes/history",
        &token,
    );
    let (status, response): (StatusCode, Option<Vec<SettingHistoryDto>>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let history = response.expect("Expected history response");
    assert_eq!(history.len(), 3);

    // Verify history is in reverse chronological order (most recent first)
    assert_eq!(history[0].new_value, "300");
    assert_eq!(history[1].new_value, "240");
    assert_eq!(history[2].new_value, "180");

    // Verify old_value tracking
    assert_eq!(history[0].old_value, "240");
    assert_eq!(history[1].old_value, "180");
    assert_eq!(history[2].old_value, "120");
}

#[tokio::test]
async fn test_get_history_empty_for_unchanged_setting() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_app_state(db.clone()).await;
    let app = create_test_router_with_app_state(state.clone());

    let token = create_admin_and_token(&db, &state).await;

    // Get history for a setting that hasn't been changed
    let request = get_request_with_auth(
        "/api/v1/admin/settings/scanner.scan_timeout_minutes/history",
        &token,
    );
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

    // Verify we have the expected scanner settings (only runtime-configurable ones)
    let keys: Vec<&str> = settings.iter().map(|s| s.key.as_str()).collect();
    assert!(keys.contains(&"scanner.scan_timeout_minutes"));
    assert!(keys.contains(&"scanner.retry_failed_files"));
    // Note: scanner.max_concurrent_scans is now in config file (startup-time setting), not in database
}

#[tokio::test]
async fn test_filter_settings_by_task_category() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_app_state(db.clone()).await;
    let app = create_test_router_with_app_state(state.clone());

    let token = create_admin_and_token(&db, &state).await;

    let request = get_request_with_auth("/api/v1/admin/settings?category=Task", &token);
    let (status, response): (StatusCode, Option<Vec<SettingDto>>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let settings = response.expect("Expected settings response");

    // All returned settings should be in Task category
    assert!(!settings.is_empty());
    for setting in &settings {
        assert_eq!(setting.category, "Task");
    }

    // Verify we have the expected task settings
    let keys: Vec<&str> = settings.iter().map(|s| s.key.as_str()).collect();
    assert!(keys.contains(&"task.poll_interval_seconds"));
    assert!(keys.contains(&"task.cleanup_interval_seconds"));
    assert!(keys.contains(&"task.prioritize_scans_over_analysis"));

    // Verify the prioritize_scans setting has correct default
    let prioritize_setting = settings
        .iter()
        .find(|s| s.key == "task.prioritize_scans_over_analysis")
        .expect("Should have prioritize_scans_over_analysis setting");
    assert_eq!(prioritize_setting.value, "true");
    assert_eq!(prioritize_setting.default_value, "true");
    assert_eq!(prioritize_setting.value_type, "Boolean");
    assert_eq!(prioritize_setting.category, "Task");
}

#[tokio::test]
async fn test_get_prioritize_scans_setting() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_app_state(db.clone()).await;
    let app = create_test_router_with_app_state(state.clone());

    let token = create_admin_and_token(&db, &state).await;

    let request = get_request_with_auth(
        "/api/v1/admin/settings/task.prioritize_scans_over_analysis",
        &token,
    );
    let (status, response): (StatusCode, Option<SettingDto>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let setting = response.expect("Expected setting response");
    assert_eq!(setting.key, "task.prioritize_scans_over_analysis");
    assert_eq!(setting.value, "true"); // Default value
    assert_eq!(setting.category, "Task");
    assert_eq!(setting.value_type, "Boolean");
}

#[tokio::test]
async fn test_update_prioritize_scans_setting() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_app_state(db.clone()).await;
    let app = create_test_router_with_app_state(state.clone());

    let token = create_admin_and_token(&db, &state).await;

    // Update to false
    let update = UpdateSettingRequest {
        value: "false".to_string(),
        change_reason: Some("Disable scan prioritization for testing".to_string()),
    };

    let request = put_json_request_with_auth(
        "/api/v1/admin/settings/task.prioritize_scans_over_analysis",
        &update,
        &token,
    );
    let (status, response): (StatusCode, Option<SettingDto>) =
        make_json_request(app.clone(), request).await;

    assert_eq!(status, StatusCode::OK);
    let setting = response.expect("Expected setting response");
    assert_eq!(setting.value, "false");

    // Verify the change was persisted
    let persisted_setting = SettingsRepository::get(&db, "task.prioritize_scans_over_analysis")
        .await
        .unwrap()
        .expect("Setting should exist");
    assert_eq!(persisted_setting.value, "false");

    // Update back to true
    let update = UpdateSettingRequest {
        value: "true".to_string(),
        change_reason: None,
    };

    let request = put_json_request_with_auth(
        "/api/v1/admin/settings/task.prioritize_scans_over_analysis",
        &update,
        &token,
    );
    let (status, response): (StatusCode, Option<SettingDto>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let setting = response.expect("Expected setting response");
    assert_eq!(setting.value, "true");
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
        .get_int("scanner.scan_timeout_minutes", 120)
        .await
        .unwrap();
    assert_eq!(initial_value, 120);

    // Update via API
    let update = UpdateSettingRequest {
        value: "240".to_string(),
        change_reason: None,
    };
    let request = put_json_request_with_auth(
        "/api/v1/admin/settings/scanner.scan_timeout_minutes",
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
        .get_int("scanner.scan_timeout_minutes", 120)
        .await
        .unwrap();
    assert_eq!(updated_value, 240);
}

#[tokio::test]
async fn test_update_setting_captures_ip_address() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_app_state(db.clone()).await;
    let app = create_test_router_with_app_state(state.clone());

    let token = create_admin_and_token(&db, &state).await;

    let update = UpdateSettingRequest {
        value: "240".to_string(),
        change_reason: Some("Test IP capture".to_string()),
    };

    // Send request with X-Forwarded-For header
    let request = put_json_request_with_auth_and_ip(
        "/api/v1/admin/settings/scanner.scan_timeout_minutes",
        &update,
        &token,
        "192.168.1.100, 10.0.0.1",
    );
    let (status, _): (StatusCode, Option<SettingDto>) = make_json_request(app, request).await;
    assert_eq!(status, StatusCode::OK);

    // Verify IP address was stored in history
    let history = SettingsRepository::get_history(&db, "scanner.scan_timeout_minutes", Some(1))
        .await
        .unwrap();

    assert_eq!(history.len(), 1);
    assert_eq!(
        history[0].ip_address,
        Some("192.168.1.100".to_string()),
        "Should capture leftmost IP from X-Forwarded-For"
    );
    assert_eq!(history[0].new_value, "240");
}

#[tokio::test]
async fn test_bulk_update_settings_captures_ip_address() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_app_state(db.clone()).await;
    let app = create_test_router_with_app_state(state.clone());

    let token = create_admin_and_token(&db, &state).await;

    let updates = vec![
        BulkSettingUpdate {
            key: "scanner.scan_timeout_minutes".to_string(),
            value: "240".to_string(),
        },
        BulkSettingUpdate {
            key: "scanner.retry_failed_files".to_string(),
            value: "true".to_string(),
        },
    ];

    let request_body = BulkUpdateSettingsRequest {
        updates,
        change_reason: Some("Bulk update IP test".to_string()),
    };

    // Send request with X-Real-IP header
    let request = post_json_request_with_auth_and_ip(
        "/api/v1/admin/settings/bulk",
        &request_body,
        &token,
        "203.0.113.42",
    );
    let (status, _): (StatusCode, Option<Vec<SettingDto>>) = make_json_request(app, request).await;
    assert_eq!(status, StatusCode::OK);

    // Verify IP address was stored in history for both settings
    let history1 = SettingsRepository::get_history(&db, "scanner.scan_timeout_minutes", Some(1))
        .await
        .unwrap();
    let history2 = SettingsRepository::get_history(&db, "scanner.retry_failed_files", Some(1))
        .await
        .unwrap();

    assert_eq!(history1.len(), 1);
    assert_eq!(
        history1[0].ip_address,
        Some("203.0.113.42".to_string()),
        "Should capture IP from X-Forwarded-For for first setting"
    );

    assert_eq!(history2.len(), 1);
    assert_eq!(
        history2[0].ip_address,
        Some("203.0.113.42".to_string()),
        "Should capture IP from X-Forwarded-For for second setting"
    );
}

#[tokio::test]
async fn test_reset_setting_captures_ip_address() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_app_state(db.clone()).await;
    let app = create_test_router_with_app_state(state.clone());

    let token = create_admin_and_token(&db, &state).await;

    // First, update the setting to a non-default value
    let update = UpdateSettingRequest {
        value: "240".to_string(),
        change_reason: Some("Setup for reset test".to_string()),
    };
    let request = put_json_request_with_auth(
        "/api/v1/admin/settings/scanner.scan_timeout_minutes",
        &update,
        &token,
    );
    let (status, _): (StatusCode, Option<SettingDto>) =
        make_json_request(app.clone(), request).await;
    assert_eq!(status, StatusCode::OK);

    // Now reset it with IP tracking
    let request = hyper::Request::builder()
        .method("POST")
        .uri("/api/v1/admin/settings/scanner.scan_timeout_minutes/reset")
        .header("Authorization", format!("Bearer {}", token))
        .header("X-Forwarded-For", "198.51.100.25")
        .body(String::new())
        .unwrap();

    let (status, _): (StatusCode, Option<SettingDto>) = make_json_request(app, request).await;
    assert_eq!(status, StatusCode::OK);

    // Verify IP address was stored in history (most recent record)
    let history = SettingsRepository::get_history(&db, "scanner.scan_timeout_minutes", Some(1))
        .await
        .unwrap();

    assert_eq!(history.len(), 1);
    assert_eq!(
        history[0].ip_address,
        Some("198.51.100.25".to_string()),
        "Should capture IP from X-Forwarded-For on reset"
    );
    assert_eq!(history[0].new_value, "120"); // Default value
}

#[tokio::test]
async fn test_update_setting_without_ip_header() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_app_state(db.clone()).await;
    let app = create_test_router_with_app_state(state.clone());

    let token = create_admin_and_token(&db, &state).await;

    let update = UpdateSettingRequest {
        value: "240".to_string(),
        change_reason: Some("No IP test".to_string()),
    };

    // Send request without IP headers
    let request = put_json_request_with_auth(
        "/api/v1/admin/settings/scanner.scan_timeout_minutes",
        &update,
        &token,
    );
    let (status, _): (StatusCode, Option<SettingDto>) = make_json_request(app, request).await;
    assert_eq!(status, StatusCode::OK);

    // Verify IP address is None in history
    let history = SettingsRepository::get_history(&db, "scanner.scan_timeout_minutes", Some(1))
        .await
        .unwrap();

    assert_eq!(history.len(), 1);
    assert_eq!(
        history[0].ip_address, None,
        "Should have None when no IP headers present"
    );
}

// ============================================================================
// Branding Endpoint Tests
// ============================================================================

#[tokio::test]
async fn test_branding_endpoint_returns_default_app_name() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_app_state(db.clone()).await;
    let app = create_test_router_with_app_state(state.clone());

    // No authentication needed for branding endpoint
    let request = get_request("/api/v1/settings/branding");
    let (status, response): (StatusCode, Option<codex::api::dto::BrandingSettingsDto>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let branding = response.expect("Expected branding response");
    assert_eq!(branding.application_name, "Codex");
}

#[tokio::test]
async fn test_branding_endpoint_no_auth_required() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_app_state(db.clone()).await;
    let app = create_test_router_with_app_state(state.clone());

    // Explicitly test without any auth headers
    let request = hyper::Request::builder()
        .method("GET")
        .uri("/api/v1/settings/branding")
        .body(String::new())
        .unwrap();

    let (status, _): (StatusCode, Option<codex::api::dto::BrandingSettingsDto>) =
        make_json_request(app, request).await;

    // Should succeed without authentication
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn test_branding_endpoint_returns_custom_app_name() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_app_state(db.clone()).await;
    let app = create_test_router_with_app_state(state.clone());

    // First, update the application.name setting via admin API
    let token = create_admin_and_token(&db, &state).await;
    let update = UpdateSettingRequest {
        value: "My Comic Library".to_string(),
        change_reason: Some("Testing custom app name".to_string()),
    };

    let request =
        put_json_request_with_auth("/api/v1/admin/settings/application.name", &update, &token);
    let (status, _): (StatusCode, Option<SettingDto>) =
        make_json_request(app.clone(), request).await;
    assert_eq!(status, StatusCode::OK);

    // Now test the branding endpoint returns the custom name
    let request = get_request("/api/v1/settings/branding");
    let (status, response): (StatusCode, Option<codex::api::dto::BrandingSettingsDto>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let branding = response.expect("Expected branding response");
    assert_eq!(branding.application_name, "My Comic Library");
}

#[tokio::test]
async fn test_application_name_in_public_settings() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_app_state(db.clone()).await;
    let app = create_test_router_with_app_state(state.clone());

    let token = create_regular_user_and_token(&db, &state).await;

    let request = get_request_with_auth("/api/v1/settings/public", &token);
    let (status, response): (
        StatusCode,
        Option<std::collections::HashMap<String, codex::api::dto::PublicSettingDto>>,
    ) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let settings = response.expect("Expected settings response");

    // application.name should be included in public settings
    assert!(
        settings.contains_key("application.name"),
        "Public settings should include application.name"
    );

    let app_name = settings.get("application.name").unwrap();
    assert_eq!(app_name.key, "application.name");
    assert_eq!(app_name.value, "Codex"); // Default value
}

// ============================================================================
// Settings Repository get_app_name() Tests
// ============================================================================

#[tokio::test]
async fn test_get_app_name_returns_default() {
    let (db, _temp_dir) = setup_test_db().await;

    // Should return default "Codex" since setting hasn't been modified
    let app_name = SettingsRepository::get_app_name(&db).await;
    assert_eq!(app_name, "Codex");
}

#[tokio::test]
async fn test_get_app_name_returns_custom_value() {
    let (db, _temp_dir) = setup_test_db().await;
    let user_id = uuid::Uuid::new_v4();

    // Update the application.name setting
    SettingsRepository::set(
        &db,
        "application.name",
        "Custom App Name".to_string(),
        user_id,
        None,
        None,
    )
    .await
    .expect("Failed to update setting");

    // Should return the custom value
    let app_name = SettingsRepository::get_app_name(&db).await;
    assert_eq!(app_name, "Custom App Name");
}
