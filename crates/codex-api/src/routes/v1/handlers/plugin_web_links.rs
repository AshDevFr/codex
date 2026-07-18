//! Plugin web links: user-facing "open on <site>" navigation targets.
//!
//! Plugins declare a `webLinks` capability in their manifest (a search page
//! template and optional per-source direct series-link templates). This
//! handler resolves the `{config.<field>}` placeholders from each plugin's
//! stored admin config and exposes the resulting templates; the frontend
//! fills the runtime placeholders (`{title}`, `{externalId}`) per series.

use axum::{Json, extract::State};
use std::sync::Arc;

use crate::{
    error::ApiError,
    extractors::{AuthContext, AuthState},
    permissions::Permission,
};
use codex_db::repositories::PluginsRepository;

/// One resolved per-source direct-link template.
#[derive(Debug, serde::Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct WebLinkSeriesLinkDto {
    /// Bare Codex external-ID source name, e.g. `mangabaka`, `myanimelist`.
    pub source: String,
    /// URL template with config placeholders already substituted. The only
    /// remaining placeholder is `{externalId}`, filled client-side.
    pub url_template: String,
}

/// A plugin exposing web links, with config placeholders resolved.
#[derive(Debug, serde::Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct WebLinkProviderDto {
    /// Plugin `name` (stable identifier).
    pub plugin_name: String,
    /// Human-readable label for the button; falls back to the plugin name
    /// when no display name is configured.
    pub display_name: String,
    /// Search page template. The only remaining placeholder is `{title}`,
    /// filled client-side with the URL-encoded series title.
    pub search_url_template: String,
    /// Ordered direct-link templates; the first entry whose `source` the
    /// series has an external ID for wins. When none match, the frontend
    /// falls back to `search_url_template`.
    pub series_links: Vec<WebLinkSeriesLinkDto>,
}

/// Response shape for `GET /api/v1/plugins/web-links`.
#[derive(Debug, serde::Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PluginWebLinksResponse {
    /// One entry per enabled plugin declaring the `webLinks` capability
    /// whose search template resolved against its config.
    pub providers: Vec<WebLinkProviderDto>,
}

/// Substitute `{config.<field>}` placeholders in `template` from a plugin's
/// stored config object.
///
/// Strings substitute verbatim, numbers and booleans are stringified, and a
/// trailing `/` is trimmed from every substituted value so templates can
/// safely write `{config.baseUrl}/path` regardless of how the admin typed the
/// URL. Returns `None` when any referenced field is missing, empty, or not a
/// scalar: an unresolvable template must hide the link, not render a broken
/// URL. Runtime placeholders (`{title}`, `{externalId}`) pass through
/// untouched.
fn resolve_config_placeholders(template: &str, config: &serde_json::Value) -> Option<String> {
    const PREFIX: &str = "{config.";
    let mut result = String::with_capacity(template.len());
    let mut rest = template;

    while let Some(start) = rest.find(PREFIX) {
        result.push_str(&rest[..start]);
        let after_prefix = &rest[start + PREFIX.len()..];
        let end = after_prefix.find('}')?;
        let key = &after_prefix[..end];

        let value = match config.get(key) {
            Some(serde_json::Value::String(s)) => s.clone(),
            Some(serde_json::Value::Number(n)) => n.to_string(),
            Some(serde_json::Value::Bool(b)) => b.to_string(),
            _ => return None,
        };
        let value = value.trim().trim_end_matches('/');
        if value.is_empty() {
            return None;
        }
        result.push_str(value);
        rest = &after_prefix[end + 1..];
    }
    result.push_str(rest);
    Some(result)
}

/// Web-link providers for the series detail page.
///
/// Read-only, requires only `SeriesRead`: the response carries resolved URL
/// templates (config values like the instance base URL are embedded, which
/// users could already discover by clicking the resulting links) but no
/// credentials, plugin IDs, or other admin-sensitive data. The response only
/// changes when an admin edits plugin config, so the frontend caches it per
/// session.
#[utoipa::path(
    get,
    path = "/api/v1/plugins/web-links",
    responses(
        (status = 200, description = "Resolved web-link providers", body = PluginWebLinksResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "SeriesRead permission required"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Plugins"
)]
pub async fn get_plugin_web_links(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
) -> Result<Json<PluginWebLinksResponse>, ApiError> {
    auth.require_permission(&Permission::SeriesRead)?;

    let plugins = PluginsRepository::get_enabled(&state.db)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to load plugins: {}", e)))?;

    let mut providers: Vec<WebLinkProviderDto> = Vec::new();
    for plugin in plugins {
        // Capability check via the cached manifest, deserialized through the
        // canonical struct so a malformed manifest can't claim the capability.
        let Some(manifest_json) = plugin.manifest.as_ref() else {
            continue;
        };
        let Ok(manifest) = serde_json::from_value::<codex_services::plugin::protocol::PluginManifest>(
            manifest_json.clone(),
        ) else {
            continue;
        };
        let Some(web_links) = manifest.capabilities.web_links else {
            continue;
        };

        // An unresolvable search template (e.g. baseUrl not configured yet)
        // drops the whole provider: search is the universal fallback, so
        // without it no button can ever render.
        let Some(search_url_template) =
            resolve_config_placeholders(&web_links.search_url_template, &plugin.config)
        else {
            continue;
        };

        // An unresolvable series-link entry only drops that entry.
        let series_links = web_links
            .series_links
            .iter()
            .filter_map(|link| {
                resolve_config_placeholders(&link.url_template, &plugin.config).map(
                    |url_template| WebLinkSeriesLinkDto {
                        source: link.source.clone(),
                        url_template,
                    },
                )
            })
            .collect();

        let display_name = if plugin.display_name.trim().is_empty() {
            plugin.name.clone()
        } else {
            plugin.display_name.clone()
        };
        providers.push(WebLinkProviderDto {
            plugin_name: plugin.name,
            display_name,
            search_url_template,
            series_links,
        });
    }

    Ok(Json(PluginWebLinksResponse { providers }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_resolve_substitutes_string_config_value() {
        let config = json!({ "baseUrl": "https://tsundoku.example.com" });
        assert_eq!(
            resolve_config_placeholders("{config.baseUrl}/search?q={title}", &config).as_deref(),
            Some("https://tsundoku.example.com/search?q={title}")
        );
    }

    #[test]
    fn test_resolve_trims_trailing_slash_and_whitespace() {
        let config = json!({ "baseUrl": " https://tsundoku.example.com/ " });
        assert_eq!(
            resolve_config_placeholders("{config.baseUrl}/search", &config).as_deref(),
            Some("https://tsundoku.example.com/search")
        );
    }

    #[test]
    fn test_resolve_stringifies_numbers_and_booleans() {
        let config = json!({ "port": 8080, "secure": true });
        assert_eq!(
            resolve_config_placeholders("http://host:{config.port}/?s={config.secure}", &config)
                .as_deref(),
            Some("http://host:8080/?s=true")
        );
    }

    #[test]
    fn test_resolve_missing_field_returns_none() {
        let config = json!({});
        assert!(resolve_config_placeholders("{config.baseUrl}/search", &config).is_none());
    }

    #[test]
    fn test_resolve_empty_or_non_scalar_field_returns_none() {
        assert!(
            resolve_config_placeholders("{config.baseUrl}/x", &json!({ "baseUrl": "" })).is_none()
        );
        assert!(
            resolve_config_placeholders("{config.baseUrl}/x", &json!({ "baseUrl": null }))
                .is_none()
        );
        assert!(
            resolve_config_placeholders("{config.baseUrl}/x", &json!({ "baseUrl": ["a"] }))
                .is_none()
        );
    }

    #[test]
    fn test_resolve_leaves_runtime_placeholders_untouched() {
        let config = json!({ "baseUrl": "https://x.io" });
        assert_eq!(
            resolve_config_placeholders(
                "{config.baseUrl}/lookup?source=mal&id={externalId}&t={title}",
                &config
            )
            .as_deref(),
            Some("https://x.io/lookup?source=mal&id={externalId}&t={title}")
        );
    }

    #[test]
    fn test_resolve_template_without_config_placeholders_passes_through() {
        assert_eq!(
            resolve_config_placeholders("https://nyaa.si/?q={title}", &json!({})).as_deref(),
            Some("https://nyaa.si/?q={title}")
        );
    }

    #[test]
    fn test_resolve_unterminated_placeholder_returns_none() {
        let config = json!({ "baseUrl": "https://x.io" });
        assert!(resolve_config_placeholders("{config.baseUrl", &config).is_none());
    }

    #[test]
    fn test_resolve_multiple_occurrences_of_same_field() {
        let config = json!({ "baseUrl": "https://x.io" });
        assert_eq!(
            resolve_config_placeholders("{config.baseUrl}/a?home={config.baseUrl}", &config)
                .as_deref(),
            Some("https://x.io/a?home=https://x.io")
        );
    }
}
