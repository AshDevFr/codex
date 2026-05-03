// Allow unused temp_dir - needed to keep TempDir alive but not always referenced
#![allow(unused_variables)]

#[path = "../common/mod.rs"]
mod common;

use codex::api::error::ErrorResponse;
use codex::api::routes::v1::dto::{
    DryRunResponse, FieldGroupDto, MetadataRefreshConfigDto, RunNowResponse,
};
use codex::db::ScanningStrategy;
use codex::db::entities::plugins::PluginPermission;
use codex::db::repositories::{LibraryRepository, PluginsRepository, UserRepository};
use codex::services::plugin::protocol::PluginScope;
use codex::utils::password;
use common::*;
use hyper::StatusCode;
use std::env;
use std::sync::Once;
use uuid::Uuid;

static INIT_ENCRYPTION: Once = Once::new();

fn setup_test_encryption_key() {
    INIT_ENCRYPTION.call_once(|| {
        if env::var("CODEX_ENCRYPTION_KEY").is_err() {
            // SAFETY: tests run with shared env access; first writer wins.
            unsafe {
                env::set_var(
                    "CODEX_ENCRYPTION_KEY",
                    "AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8=",
                );
            }
        }
    });
}

async fn create_admin_and_token(
    db: &sea_orm::DatabaseConnection,
    state: &codex::api::extractors::AppState,
) -> String {
    let password_hash = password::hash_password("admin123").unwrap();
    let user = create_test_user("admin", "admin@example.com", &password_hash, true);
    let created = UserRepository::create(db, &user).await.unwrap();
    state
        .jwt_service
        .generate_token(created.id, created.username.clone(), created.get_role())
        .unwrap()
}

async fn create_readonly_and_token(
    db: &sea_orm::DatabaseConnection,
    state: &codex::api::extractors::AppState,
) -> String {
    let password_hash = password::hash_password("user123").unwrap();
    let user = create_test_user("readonly", "readonly@example.com", &password_hash, false);
    let created = UserRepository::create(db, &user).await.unwrap();
    state
        .jwt_service
        .generate_token(created.id, created.username.clone(), created.get_role())
        .unwrap()
}

async fn create_test_plugin(db: &sea_orm::DatabaseConnection, name: &str, enabled: bool) -> Uuid {
    setup_test_encryption_key();
    PluginsRepository::create(
        db,
        name,
        name,
        None,
        "system",
        "node",
        vec!["dist/index.js".to_string()],
        vec![],
        None,
        vec![PluginPermission::MetadataWriteSummary],
        vec![PluginScope::SeriesDetail],
        vec![],
        None,
        "env",
        None,
        enabled,
        None,
        None,
    )
    .await
    .unwrap()
    .id
}

async fn create_library(db: &sea_orm::DatabaseConnection) -> Uuid {
    LibraryRepository::create(db, "Test Library", "/tmp/test", ScanningStrategy::Default)
        .await
        .unwrap()
        .id
}

// ============================================================================
// GET refresh config
// ============================================================================

#[tokio::test]
async fn test_get_refresh_config_returns_defaults_when_unset() {
    let (db, _tmp) = setup_test_db().await;
    let library_id = create_library(&db).await;

    let state = create_test_app_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_app_state(state);

    let uri = format!("/api/v1/libraries/{library_id}/metadata-refresh");
    let req = get_request_with_auth(&uri, &token);
    let (status, body): (StatusCode, Option<MetadataRefreshConfigDto>) =
        make_json_request(app, req).await;

    assert_eq!(status, StatusCode::OK);
    let cfg = body.unwrap();
    assert!(!cfg.enabled);
    assert!(cfg.existing_source_ids_only);
    assert_eq!(cfg.field_groups, vec!["ratings", "status", "counts"]);
    assert!(cfg.providers.is_empty());
    assert_eq!(cfg.cron_schedule, "0 0 4 * * *");
}

#[tokio::test]
async fn test_get_refresh_config_returns_persisted_values() {
    let (db, _tmp) = setup_test_db().await;
    let library_id = create_library(&db).await;

    // Persist a non-default config directly through the repository.
    let cfg = codex::services::metadata::MetadataRefreshConfig {
        enabled: true,
        cron_schedule: "0 30 2 * * *".to_string(),
        providers: vec!["plugin:none".to_string()],
        ..Default::default()
    };
    LibraryRepository::set_metadata_refresh_config(&db, library_id, &cfg)
        .await
        .unwrap();

    let state = create_test_app_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_app_state(state);

    let uri = format!("/api/v1/libraries/{library_id}/metadata-refresh");
    let req = get_request_with_auth(&uri, &token);
    let (status, body): (StatusCode, Option<MetadataRefreshConfigDto>) =
        make_json_request(app, req).await;

    assert_eq!(status, StatusCode::OK);
    let dto = body.unwrap();
    assert!(dto.enabled);
    assert_eq!(dto.cron_schedule, "0 30 2 * * *");
    assert_eq!(dto.providers, vec!["plugin:none"]);
}

#[tokio::test]
async fn test_get_refresh_config_unknown_library_404() {
    let (db, _tmp) = setup_test_db().await;
    let state = create_test_app_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_app_state(state);

    let bogus = Uuid::new_v4();
    let uri = format!("/api/v1/libraries/{bogus}/metadata-refresh");
    let req = get_request_with_auth(&uri, &token);
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, req).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_get_refresh_config_requires_auth() {
    let (db, _tmp) = setup_test_db().await;
    let library_id = create_library(&db).await;
    let state = create_test_app_state(db.clone()).await;
    let app = create_test_router_with_app_state(state);

    let uri = format!("/api/v1/libraries/{library_id}/metadata-refresh");
    let req = get_request(&uri);
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, req).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

// ============================================================================
// PATCH refresh config
// ============================================================================

#[tokio::test]
async fn test_patch_refresh_config_round_trips() {
    let (db, _tmp) = setup_test_db().await;
    let library_id = create_library(&db).await;
    let plugin_id = create_test_plugin(&db, "mangabaka", true).await;

    let state = create_test_app_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_app_state(state);

    let uri = format!("/api/v1/libraries/{library_id}/metadata-refresh");
    let body = r#"{
        "enabled": true,
        "cronSchedule": "0 0 5 * * *",
        "fieldGroups": ["ratings", "counts"],
        "providers": ["plugin:mangabaka"]
    }"#;
    let req = patch_request_with_auth_json(&uri, &token, body);
    let (status, response): (StatusCode, Option<MetadataRefreshConfigDto>) =
        make_json_request(app, req).await;

    assert_eq!(status, StatusCode::OK);
    let cfg = response.unwrap();
    assert!(cfg.enabled);
    assert_eq!(cfg.cron_schedule, "0 0 5 * * *");
    assert_eq!(cfg.field_groups, vec!["ratings", "counts"]);
    assert_eq!(cfg.providers, vec!["plugin:mangabaka"]);

    // Verify persistence: read back via repo.
    let stored = LibraryRepository::get_metadata_refresh_config(&db, library_id)
        .await
        .unwrap();
    assert!(stored.enabled);
    assert_eq!(stored.cron_schedule, "0 0 5 * * *");
}

#[tokio::test]
async fn test_patch_refresh_config_partial_preserves_other_fields() {
    let (db, _tmp) = setup_test_db().await;
    let library_id = create_library(&db).await;

    // Seed an existing config with field_groups set.
    let cfg = codex::services::metadata::MetadataRefreshConfig {
        field_groups: vec!["status".to_string(), "counts".to_string()],
        ..Default::default()
    };
    LibraryRepository::set_metadata_refresh_config(&db, library_id, &cfg)
        .await
        .unwrap();

    let state = create_test_app_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_app_state(state);

    let uri = format!("/api/v1/libraries/{library_id}/metadata-refresh");
    let body = r#"{"enabled": true}"#;
    let req = patch_request_with_auth_json(&uri, &token, body);
    let (status, response): (StatusCode, Option<MetadataRefreshConfigDto>) =
        make_json_request(app, req).await;

    assert_eq!(status, StatusCode::OK);
    let dto = response.unwrap();
    assert!(dto.enabled);
    // field_groups untouched.
    assert_eq!(dto.field_groups, vec!["status", "counts"]);
}

#[tokio::test]
async fn test_patch_refresh_config_rejects_invalid_cron() {
    let (db, _tmp) = setup_test_db().await;
    let library_id = create_library(&db).await;

    let state = create_test_app_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_app_state(state);

    let uri = format!("/api/v1/libraries/{library_id}/metadata-refresh");
    let body = r#"{"cronSchedule": "not a cron"}"#;
    let req = patch_request_with_auth_json(&uri, &token, body);
    let (status, err): (StatusCode, Option<ErrorResponse>) = make_json_request(app, req).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let err = err.unwrap();
    assert!(err.message.to_lowercase().contains("cron"));
}

#[tokio::test]
async fn test_patch_refresh_config_rejects_unknown_field_group() {
    let (db, _tmp) = setup_test_db().await;
    let library_id = create_library(&db).await;

    let state = create_test_app_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_app_state(state);

    let uri = format!("/api/v1/libraries/{library_id}/metadata-refresh");
    let body = r#"{"fieldGroups": ["ratings", "made_up_group"]}"#;
    let req = patch_request_with_auth_json(&uri, &token, body);
    let (status, err): (StatusCode, Option<ErrorResponse>) = make_json_request(app, req).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(err.unwrap().message.contains("made_up_group"));
}

#[tokio::test]
async fn test_patch_refresh_config_rejects_unknown_provider() {
    let (db, _tmp) = setup_test_db().await;
    let library_id = create_library(&db).await;

    let state = create_test_app_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_app_state(state);

    let uri = format!("/api/v1/libraries/{library_id}/metadata-refresh");
    let body = r#"{"providers": ["plugin:nonexistent"]}"#;
    let req = patch_request_with_auth_json(&uri, &token, body);
    let (status, err): (StatusCode, Option<ErrorResponse>) = make_json_request(app, req).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(err.unwrap().message.contains("nonexistent"));
}

#[tokio::test]
async fn test_patch_refresh_config_rejects_invalid_timezone() {
    let (db, _tmp) = setup_test_db().await;
    let library_id = create_library(&db).await;

    let state = create_test_app_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_app_state(state);

    let uri = format!("/api/v1/libraries/{library_id}/metadata-refresh");
    let body = r#"{"timezone": "Not/Valid"}"#;
    let req = patch_request_with_auth_json(&uri, &token, body);
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, req).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_patch_refresh_config_clears_timezone_on_null() {
    let (db, _tmp) = setup_test_db().await;
    let library_id = create_library(&db).await;
    let cfg = codex::services::metadata::MetadataRefreshConfig {
        timezone: Some("Europe/Paris".to_string()),
        ..Default::default()
    };
    LibraryRepository::set_metadata_refresh_config(&db, library_id, &cfg)
        .await
        .unwrap();

    let state = create_test_app_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_app_state(state);

    let uri = format!("/api/v1/libraries/{library_id}/metadata-refresh");
    let body = r#"{"timezone": null}"#;
    let req = patch_request_with_auth_json(&uri, &token, body);
    let (status, response): (StatusCode, Option<MetadataRefreshConfigDto>) =
        make_json_request(app, req).await;

    assert_eq!(status, StatusCode::OK);
    assert!(response.unwrap().timezone.is_none());
}

#[tokio::test]
async fn test_patch_refresh_config_rejects_zero_max_concurrency() {
    let (db, _tmp) = setup_test_db().await;
    let library_id = create_library(&db).await;

    let state = create_test_app_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_app_state(state);

    let uri = format!("/api/v1/libraries/{library_id}/metadata-refresh");
    let body = r#"{"maxConcurrency": 0}"#;
    let req = patch_request_with_auth_json(&uri, &token, body);
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, req).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_patch_refresh_config_requires_write_permission() {
    let (db, _tmp) = setup_test_db().await;
    let library_id = create_library(&db).await;

    let state = create_test_app_state(db.clone()).await;
    let token = create_readonly_and_token(&db, &state).await;
    let app = create_test_router_with_app_state(state);

    let uri = format!("/api/v1/libraries/{library_id}/metadata-refresh");
    let body = r#"{"enabled": true}"#;
    let req = patch_request_with_auth_json(&uri, &token, body);
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, req).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_patch_refresh_config_round_trips_per_provider_overrides() {
    let (db, _tmp) = setup_test_db().await;
    let library_id = create_library(&db).await;
    let _plugin_id = create_test_plugin(&db, "anilist", true).await;

    let state = create_test_app_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_app_state(state);

    let uri = format!("/api/v1/libraries/{library_id}/metadata-refresh");
    let body = r#"{
        "perProviderOverrides": {
            "plugin:anilist": {
                "fieldGroups": ["ratings"],
                "extraFields": ["coverUrl"]
            }
        }
    }"#;
    let req = patch_request_with_auth_json(&uri, &token, body);
    let (status, response): (StatusCode, Option<MetadataRefreshConfigDto>) =
        make_json_request(app, req).await;

    assert_eq!(status, StatusCode::OK);
    let dto = response.unwrap();
    let overrides = dto
        .per_provider_overrides
        .expect("override map should round-trip");
    let anilist = overrides
        .get("plugin:anilist")
        .expect("anilist override should be present");
    assert_eq!(anilist.field_groups, vec!["ratings"]);
    assert_eq!(anilist.extra_fields, vec!["coverUrl"]);

    // Stored config should also have the override.
    let stored = LibraryRepository::get_metadata_refresh_config(&db, library_id)
        .await
        .unwrap();
    let stored_overrides = stored.per_provider_overrides.unwrap();
    assert!(stored_overrides.contains_key("plugin:anilist"));
}

#[tokio::test]
async fn test_patch_refresh_config_clears_per_provider_overrides_on_null() {
    let (db, _tmp) = setup_test_db().await;
    let library_id = create_library(&db).await;
    let _plugin_id = create_test_plugin(&db, "anilist", true).await;

    // Seed an override.
    let mut overrides = std::collections::BTreeMap::new();
    overrides.insert(
        "plugin:anilist".to_string(),
        codex::services::metadata::ProviderOverride {
            field_groups: vec!["ratings".to_string()],
            extra_fields: vec![],
        },
    );
    let cfg = codex::services::metadata::MetadataRefreshConfig {
        per_provider_overrides: Some(overrides),
        ..Default::default()
    };
    LibraryRepository::set_metadata_refresh_config(&db, library_id, &cfg)
        .await
        .unwrap();

    let state = create_test_app_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_app_state(state);

    let uri = format!("/api/v1/libraries/{library_id}/metadata-refresh");
    let body = r#"{"perProviderOverrides": null}"#;
    let req = patch_request_with_auth_json(&uri, &token, body);
    let (status, response): (StatusCode, Option<MetadataRefreshConfigDto>) =
        make_json_request(app, req).await;

    assert_eq!(status, StatusCode::OK);
    assert!(response.unwrap().per_provider_overrides.is_none());
}

#[tokio::test]
async fn test_patch_refresh_config_rejects_per_provider_unknown_field_group() {
    let (db, _tmp) = setup_test_db().await;
    let library_id = create_library(&db).await;
    let _plugin_id = create_test_plugin(&db, "anilist", true).await;

    let state = create_test_app_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_app_state(state);

    let uri = format!("/api/v1/libraries/{library_id}/metadata-refresh");
    let body = r#"{
        "perProviderOverrides": {
            "plugin:anilist": {
                "fieldGroups": ["made_up_group"]
            }
        }
    }"#;
    let req = patch_request_with_auth_json(&uri, &token, body);
    let (status, err): (StatusCode, Option<ErrorResponse>) = make_json_request(app, req).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let msg = err.unwrap().message;
    assert!(
        msg.contains("plugin:anilist"),
        "error should mention provider, got: {msg}"
    );
    assert!(
        msg.contains("made_up_group"),
        "error should mention bad group, got: {msg}"
    );
}

#[tokio::test]
async fn test_patch_refresh_config_rejects_per_provider_unknown_plugin() {
    let (db, _tmp) = setup_test_db().await;
    let library_id = create_library(&db).await;

    let state = create_test_app_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_app_state(state);

    let uri = format!("/api/v1/libraries/{library_id}/metadata-refresh");
    let body = r#"{
        "perProviderOverrides": {
            "plugin:nonexistent": {
                "fieldGroups": ["ratings"]
            }
        }
    }"#;
    let req = patch_request_with_auth_json(&uri, &token, body);
    let (status, err): (StatusCode, Option<ErrorResponse>) = make_json_request(app, req).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(err.unwrap().message.contains("nonexistent"));
}

// ============================================================================
// run-now
// ============================================================================

#[tokio::test]
async fn test_run_now_enqueues_task() {
    let (db, _tmp) = setup_test_db().await;
    let library_id = create_library(&db).await;

    let state = create_test_app_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_app_state(state);

    let uri = format!("/api/v1/libraries/{library_id}/metadata-refresh/run-now");
    let req = post_request_with_auth(&uri, &token);
    let (status, response): (StatusCode, Option<RunNowResponse>) =
        make_json_request(app, req).await;
    assert_eq!(status, StatusCode::OK);
    let task_id = response.unwrap().task_id;
    assert!(!task_id.is_nil());

    // Verify a task was created in the DB.
    use codex::db::entities::{prelude::Tasks, tasks};
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
    let stored = Tasks::find()
        .filter(tasks::Column::TaskType.eq("refresh_library_metadata"))
        .filter(tasks::Column::LibraryId.eq(library_id))
        .one(&db)
        .await
        .unwrap();
    assert!(stored.is_some(), "task should have been enqueued");
}

#[tokio::test]
async fn test_run_now_conflicts_when_active_task_exists() {
    let (db, _tmp) = setup_test_db().await;
    let library_id = create_library(&db).await;

    // Pre-enqueue a task to simulate an in-flight refresh.
    use codex::db::repositories::TaskRepository;
    use codex::tasks::types::TaskType;
    TaskRepository::enqueue(&db, TaskType::RefreshLibraryMetadata { library_id }, None)
        .await
        .unwrap();

    let state = create_test_app_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_app_state(state);

    let uri = format!("/api/v1/libraries/{library_id}/metadata-refresh/run-now");
    let req = post_request_with_auth(&uri, &token);
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, req).await;
    assert_eq!(status, StatusCode::CONFLICT);
}

#[tokio::test]
async fn test_run_now_unknown_library_404() {
    let (db, _tmp) = setup_test_db().await;
    let state = create_test_app_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_app_state(state);

    let bogus = Uuid::new_v4();
    let uri = format!("/api/v1/libraries/{bogus}/metadata-refresh/run-now");
    let req = post_request_with_auth(&uri, &token);
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, req).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

// ============================================================================
// dry-run
// ============================================================================

#[tokio::test]
async fn test_dry_run_with_no_providers_returns_empty_sample() {
    let (db, _tmp) = setup_test_db().await;
    let library_id = create_library(&db).await;

    let state = create_test_app_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_app_state(state);

    let uri = format!("/api/v1/libraries/{library_id}/metadata-refresh/dry-run");
    let req = post_request_with_auth_json(&uri, &token, "{}");
    let (status, response): (StatusCode, Option<DryRunResponse>) =
        make_json_request(app, req).await;
    assert_eq!(status, StatusCode::OK);
    let dto = response.unwrap();
    assert!(dto.sample.is_empty());
    assert_eq!(dto.total_eligible, 0);
    assert!(dto.unresolved_providers.is_empty());
}

#[tokio::test]
async fn test_dry_run_reports_unresolved_providers() {
    let (db, _tmp) = setup_test_db().await;
    let library_id = create_library(&db).await;

    let cfg = codex::services::metadata::MetadataRefreshConfig {
        providers: vec!["plugin:not_installed".to_string()],
        ..Default::default()
    };
    LibraryRepository::set_metadata_refresh_config(&db, library_id, &cfg)
        .await
        .unwrap();

    let state = create_test_app_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_app_state(state);

    let uri = format!("/api/v1/libraries/{library_id}/metadata-refresh/dry-run");
    let req = post_request_with_auth_json(&uri, &token, "{}");
    let (status, response): (StatusCode, Option<DryRunResponse>) =
        make_json_request(app, req).await;
    assert_eq!(status, StatusCode::OK);
    let dto = response.unwrap();
    assert_eq!(dto.unresolved_providers, vec!["plugin:not_installed"]);
    assert!(dto.sample.is_empty());
}

#[tokio::test]
async fn test_dry_run_validates_config_override() {
    let (db, _tmp) = setup_test_db().await;
    let library_id = create_library(&db).await;

    let state = create_test_app_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_app_state(state);

    let uri = format!("/api/v1/libraries/{library_id}/metadata-refresh/dry-run");
    // Bad cron in the override.
    let body = r#"{"configOverride": {
        "enabled": false,
        "cronSchedule": "garbage",
        "fieldGroups": ["ratings"],
        "extraFields": [],
        "providers": [],
        "existingSourceIdsOnly": true,
        "skipRecentlySyncedWithinS": 0,
        "maxConcurrency": 4
    }}"#;
    let req = post_request_with_auth_json(&uri, &token, body);
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, req).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

// ============================================================================
// field-groups catalog
// ============================================================================

#[tokio::test]
async fn test_list_field_groups_returns_full_catalog() {
    let (db, _tmp) = setup_test_db().await;
    let state = create_test_app_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_app_state(state);

    let req = get_request_with_auth("/api/v1/metadata-refresh/field-groups", &token);
    let (status, response): (StatusCode, Option<Vec<FieldGroupDto>>) =
        make_json_request(app, req).await;
    assert_eq!(status, StatusCode::OK);
    let groups = response.unwrap();
    // 12 groups in FieldGroup::all().
    assert_eq!(groups.len(), 12);

    let ratings = groups.iter().find(|g| g.id == "ratings").unwrap();
    assert!(ratings.fields.contains(&"rating".to_string()));
    assert!(ratings.fields.contains(&"externalRatings".to_string()));
    assert_eq!(ratings.label, "Ratings");

    let counts = groups.iter().find(|g| g.id == "counts").unwrap();
    assert!(counts.fields.contains(&"totalVolumeCount".to_string()));
    assert!(counts.fields.contains(&"totalChapterCount".to_string()));
}

#[tokio::test]
async fn test_list_field_groups_requires_auth() {
    let (db, _tmp) = setup_test_db().await;
    let state = create_test_app_state(db.clone()).await;
    let app = create_test_router_with_app_state(state);

    let req = get_request("/api/v1/metadata-refresh/field-groups");
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, req).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}
