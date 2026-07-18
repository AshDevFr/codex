//! Integration tests for `GET /api/v1/plugins/web-links`.

#[path = "../common/mod.rs"]
mod common;

use codex::db::repositories::{PluginsRepository, UserRepository};
use codex::utils::password;
use common::*;
use hyper::StatusCode;
use sea_orm::DatabaseConnection;
use serde_json::json;

async fn create_admin_and_token(
    db: &DatabaseConnection,
    state: &codex::api::extractors::AuthState,
) -> String {
    let password_hash = password::hash_password("admin123").unwrap();
    let user = create_test_user("admin", "admin@example.com", &password_hash, true);
    let created = UserRepository::create(db, &user).await.unwrap();
    state
        .jwt_service
        .generate_token(created.id, created.username.clone(), created.get_role())
        .unwrap()
}

/// Insert a plugin row with the given config, manifest capabilities, and
/// enabled flag. `config_schema`, when provided, is attached to the cached
/// manifest (its field defaults back-fill unset config keys).
async fn make_plugin_with_schema(
    db: &DatabaseConnection,
    name: &str,
    display_name: &str,
    config: Option<serde_json::Value>,
    capabilities: serde_json::Value,
    config_schema: Option<serde_json::Value>,
    enabled: bool,
) {
    let plugin = PluginsRepository::create(
        db,
        name,
        display_name,
        Some("test plugin"),
        "system",
        "echo",
        vec!["ok".to_string()],
        vec![],
        None,
        vec![],
        vec![],
        vec![],
        None,
        "none",
        config,
        enabled,
        None,
        None,
        None,
        None,
    )
    .await
    .unwrap();

    let mut manifest = json!({
        "name": name,
        "displayName": display_name,
        "version": "1.0.0",
        "protocolVersion": "1.1",
        "capabilities": capabilities
    });
    if let Some(schema) = config_schema {
        manifest["configSchema"] = schema;
    }
    PluginsRepository::update_manifest(db, plugin.id, Some(manifest))
        .await
        .unwrap();
}

/// Insert a plugin row without a manifest config schema.
async fn make_plugin(
    db: &DatabaseConnection,
    name: &str,
    display_name: &str,
    config: Option<serde_json::Value>,
    capabilities: serde_json::Value,
    enabled: bool,
) {
    make_plugin_with_schema(db, name, display_name, config, capabilities, None, enabled).await;
}

fn tsundoku_web_links_capabilities() -> serde_json::Value {
    json!({
        "webLinks": {
            "searchUrlTemplate": "{config.baseUrl}/search?q={title}",
            "seriesLinks": [
                {
                    "source": "mangabaka",
                    "urlTemplate": "{config.baseUrl}/series/lookup?source=mangabaka&id={externalId}"
                },
                {
                    "source": "myanimelist",
                    "urlTemplate": "{config.baseUrl}/series/lookup?source=mal&id={externalId}"
                }
            ]
        }
    })
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct SeriesLinkDto {
    source: String,
    url_template: String,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProviderDto {
    plugin_name: String,
    display_name: String,
    search_url_template: String,
    series_links: Vec<SeriesLinkDto>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct WebLinksResponseDto {
    providers: Vec<ProviderDto>,
}

#[tokio::test]
async fn web_links_returns_resolved_templates_in_declared_order() {
    let (db, _temp) = setup_test_db().await;
    make_plugin(
        &db,
        "release-tsundoku",
        "Tsundoku",
        Some(json!({ "baseUrl": "https://tsundoku.example.com/" })),
        tsundoku_web_links_capabilities(),
        true,
    )
    .await;
    // A plugin without the capability must not be listed.
    make_plugin(
        &db,
        "metadata-only",
        "Metadata Only",
        None,
        json!({ "metadataProvider": ["series"] }),
        true,
    )
    .await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let req = get_request_with_auth("/api/v1/plugins/web-links", &token);
    let (status, body): (StatusCode, Option<WebLinksResponseDto>) =
        make_json_request(app, req).await;
    assert_eq!(status, StatusCode::OK);
    let body = body.unwrap();
    assert_eq!(body.providers.len(), 1);

    let provider = &body.providers[0];
    assert_eq!(provider.plugin_name, "release-tsundoku");
    assert_eq!(provider.display_name, "Tsundoku");
    // Trailing slash on the configured baseUrl is trimmed.
    assert_eq!(
        provider.search_url_template,
        "https://tsundoku.example.com/search?q={title}"
    );
    // Declaration order is preserved: it doubles as match priority.
    assert_eq!(provider.series_links.len(), 2);
    assert_eq!(provider.series_links[0].source, "mangabaka");
    assert_eq!(
        provider.series_links[0].url_template,
        "https://tsundoku.example.com/series/lookup?source=mangabaka&id={externalId}"
    );
    assert_eq!(provider.series_links[1].source, "myanimelist");
    assert_eq!(
        provider.series_links[1].url_template,
        "https://tsundoku.example.com/series/lookup?source=mal&id={externalId}"
    );
}

#[tokio::test]
async fn web_links_omits_disabled_plugin() {
    let (db, _temp) = setup_test_db().await;
    make_plugin(
        &db,
        "release-tsundoku",
        "Tsundoku",
        Some(json!({ "baseUrl": "https://tsundoku.example.com" })),
        tsundoku_web_links_capabilities(),
        false,
    )
    .await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let req = get_request_with_auth("/api/v1/plugins/web-links", &token);
    let (status, body): (StatusCode, Option<WebLinksResponseDto>) =
        make_json_request(app, req).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.unwrap().providers.is_empty());
}

#[tokio::test]
async fn web_links_omits_provider_when_search_config_unresolved() {
    let (db, _temp) = setup_test_db().await;
    // No baseUrl configured: the search template can't resolve, so the
    // whole provider is dropped.
    make_plugin(
        &db,
        "release-tsundoku",
        "Tsundoku",
        None,
        tsundoku_web_links_capabilities(),
        true,
    )
    .await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let req = get_request_with_auth("/api/v1/plugins/web-links", &token);
    let (status, body): (StatusCode, Option<WebLinksResponseDto>) =
        make_json_request(app, req).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.unwrap().providers.is_empty());
}

#[tokio::test]
async fn web_links_drops_only_unresolvable_series_link_entries() {
    let (db, _temp) = setup_test_db().await;
    make_plugin(
        &db,
        "mixed-links",
        "Mixed",
        Some(json!({ "baseUrl": "https://site.example.com" })),
        json!({
            "webLinks": {
                "searchUrlTemplate": "{config.baseUrl}/search?q={title}",
                "seriesLinks": [
                    // References a config field that is not set: dropped.
                    {
                        "source": "anilist",
                        "urlTemplate": "{config.mirrorUrl}/anilist/{externalId}"
                    },
                    // Resolvable: kept.
                    {
                        "source": "mangaupdates",
                        "urlTemplate": "{config.baseUrl}/mu/{externalId}"
                    }
                ]
            }
        }),
        true,
    )
    .await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let req = get_request_with_auth("/api/v1/plugins/web-links", &token);
    let (status, body): (StatusCode, Option<WebLinksResponseDto>) =
        make_json_request(app, req).await;
    assert_eq!(status, StatusCode::OK);
    let body = body.unwrap();
    assert_eq!(body.providers.len(), 1);
    let provider = &body.providers[0];
    assert_eq!(provider.series_links.len(), 1);
    assert_eq!(provider.series_links[0].source, "mangaupdates");
    assert_eq!(
        provider.series_links[0].url_template,
        "https://site.example.com/mu/{externalId}"
    );
}

#[tokio::test]
async fn web_links_falls_back_to_config_schema_defaults() {
    let (db, _temp) = setup_test_db().await;
    // Nyaa-style plugin: baseUrl has a schema default and no stored value.
    make_plugin_with_schema(
        &db,
        "release-nyaa",
        "Nyaa",
        None,
        json!({
            "webLinks": { "searchUrlTemplate": "{config.baseUrl}/?q={title}" }
        }),
        Some(json!({
            "fields": [
                { "key": "baseUrl", "label": "Base URL", "type": "string", "default": "https://nyaa.si" }
            ]
        })),
        true,
    )
    .await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let req = get_request_with_auth("/api/v1/plugins/web-links", &token);
    let (status, body): (StatusCode, Option<WebLinksResponseDto>) =
        make_json_request(app, req).await;
    assert_eq!(status, StatusCode::OK);
    let body = body.unwrap();
    assert_eq!(body.providers.len(), 1);
    assert_eq!(
        body.providers[0].search_url_template,
        "https://nyaa.si/?q={title}"
    );
}

#[tokio::test]
async fn web_links_requires_authentication() {
    let (db, _temp) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let app = create_test_router(state).await;

    let req = get_request("/api/v1/plugins/web-links");
    let (status, _body): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, req).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}
