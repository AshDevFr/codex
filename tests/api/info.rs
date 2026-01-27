#[path = "../common/mod.rs"]
mod common;

use codex::api::routes::v1::dto::AppInfoDto;
use common::*;
use hyper::StatusCode;

// ============================================================================
// App Info Tests
// ============================================================================

#[tokio::test]
async fn test_get_app_info() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db).await;
    let app = create_test_router(state).await;

    // Info endpoint is public (no authentication required)
    let request = get_request("/api/v1/info");
    let (status, response): (StatusCode, Option<AppInfoDto>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let info = response.unwrap();

    // Version should match CARGO_PKG_VERSION
    assert_eq!(info.version, env!("CARGO_PKG_VERSION"));

    // Name should match CARGO_PKG_NAME
    assert_eq!(info.name, env!("CARGO_PKG_NAME"));
}

#[tokio::test]
async fn test_get_app_info_version_format() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db).await;
    let app = create_test_router(state).await;

    let request = get_request("/api/v1/info");
    let (status, response): (StatusCode, Option<AppInfoDto>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let info = response.unwrap();

    // Version should be in semver format (e.g., "1.0.0")
    let version_parts: Vec<&str> = info.version.split('.').collect();
    assert!(
        version_parts.len() >= 2,
        "Version should have at least major.minor format"
    );

    // First part should be a number
    assert!(
        version_parts[0].parse::<u32>().is_ok(),
        "Major version should be a number"
    );
}
