//! Registry of every settable configuration path, derived from [`Config`]
//! itself rather than hand-maintained.
//!
//! A hand-written table of key names drifts the first time somebody adds a
//! field. Instead this module serializes a *probe* config and walks the
//! resulting tree, so the registry is a function of the struct definition.
//!
//! Two kinds of entry come out of the walk:
//!
//! - **Exact** paths such as `task.worker_count`, one per scalar field.
//! - **Wildcard** paths such as `auth.oidc.providers.*.issuer_url`, produced
//!   wherever the config holds a map whose keys the operator chooses.
//!
//! The probe exists because a plain [`Config::default`] cannot describe the
//! whole surface: `database.postgres` is `None` under the default SQLite
//! setup, and every map is empty, so both subtrees would be invisible. The
//! probe populates every `Option` and drops a sentinel key into every map.

use crate::types::{Config, OidcProviderConfig, PostgresConfig, SQLiteConfig};
use serde_json::Value;
use std::collections::{BTreeSet, HashMap};
use std::sync::OnceLock;

/// Sentinel map key planted by [`key_probe`]. Seeing it as a field name during
/// the walk is how the walker tells "this node is a map the operator fills in"
/// apart from "this node is a struct with known fields", without a hardcoded
/// list of map-valued paths that could go stale.
const MAP_PROBE_KEY: &str = "__codex_map_probe__";

/// Every settable configuration path.
#[derive(Debug, Default)]
pub struct KeyRegistry {
    /// Fully static dotted paths, e.g. `rate_limit.anonymous_rps`.
    exact: BTreeSet<String>,
    /// Paths with one or more `*` segments standing for operator-chosen map
    /// keys, e.g. `auth.oidc.providers.*.issuer_url`.
    wildcard: BTreeSet<String>,
}

impl KeyRegistry {
    /// Exact paths, sorted.
    pub fn exact(&self) -> &BTreeSet<String> {
        &self.exact
    }

    /// Wildcard patterns, sorted.
    pub fn wildcard(&self) -> &BTreeSet<String> {
        &self.wildcard
    }

    /// Every entry, exact and wildcard, sorted.
    pub fn all(&self) -> impl Iterator<Item = &String> {
        self.exact.iter().chain(self.wildcard.iter())
    }

    /// Whether `path` names a real setting. Wildcard segments match any single
    /// non-empty path segment.
    pub fn contains(&self, path: &str) -> bool {
        if self.exact.contains(path) {
            return true;
        }
        self.wildcard.iter().any(|pat| wildcard_matches(pat, path))
    }
}

/// Does `path` fit `pattern`, where `*` segments match any one segment?
fn wildcard_matches(pattern: &str, path: &str) -> bool {
    let pat: Vec<&str> = pattern.split('.').collect();
    let got: Vec<&str> = path.split('.').collect();
    if pat.len() != got.len() {
        return false;
    }
    pat.iter().zip(got.iter()).all(|(p, g)| *p == "*" || p == g)
}

/// The process-wide registry, computed once.
pub fn registry() -> &'static KeyRegistry {
    static REGISTRY: OnceLock<KeyRegistry> = OnceLock::new();
    REGISTRY.get_or_init(build_registry)
}

fn build_registry() -> KeyRegistry {
    let value = serde_json::to_value(key_probe())
        .expect("Config is a plain serde struct and must always serialize");
    let mut registry = KeyRegistry::default();
    walk(&value, "", &mut registry);
    registry
}

/// Depth-first walk emitting one entry per leaf.
///
/// Anything that is not a JSON object is a leaf, including arrays and nulls:
/// a `Vec<String>` field is set as a whole, not per element.
fn walk(value: &Value, path: &str, out: &mut KeyRegistry) {
    match value {
        Value::Object(fields) => {
            // A map is settable as a whole, not only key by key:
            // `CODEX_OBSERVABILITY_OTLP_HEADERS` takes the entire header set as
            // `k1=v1,k2=v2`. Record the container alongside the per-key
            // wildcard so that form is recognized too.
            if fields.contains_key(MAP_PROBE_KEY) && !path.is_empty() {
                out.exact.insert(path.to_string());
            }
            for (name, child) in fields {
                let segment = if name == MAP_PROBE_KEY { "*" } else { name };
                let child_path = if path.is_empty() {
                    segment.to_string()
                } else {
                    format!("{path}.{segment}")
                };
                walk(child, &child_path, out);
            }
        }
        _ => {
            if path.is_empty() {
                return;
            }
            if path.split('.').any(|s| s == "*") {
                out.wildcard.insert(path.to_string());
            } else {
                out.exact.insert(path.to_string());
            }
        }
    }
}

/// A [`Config`] shaped for key discovery, not for use.
///
/// Every `Option` is `Some` and every map holds one [`MAP_PROBE_KEY`] entry, so
/// that no part of the tree is invisible to [`walk`]. Values are meaningless;
/// only the shape matters.
///
/// Two tests guard this against drift: the serialized probe must contain no
/// `null` (which would mean an `Option` was left unpopulated) and no empty
/// object (which would mean a map was left empty).
fn key_probe() -> Config {
    let mut config = Config::default();

    config.database.postgres = Some(PostgresConfig::default());
    config.database.sqlite = Some(SQLiteConfig {
        pragmas: Some(probe_string_map()),
        ..SQLiteConfig::default()
    });

    config.application.base_url = Some(String::new());
    config.logging.file = Some(String::new());
    config.email.verification_url_base = Some(String::new());
    config.pdf.pdfium_library_path = Some(String::new());

    config.observability.otlp.proxy_endpoint = Some(String::new());
    config.observability.otlp.headers = probe_string_map();

    config.auth.oidc.redirect_uri_base = Some(String::new());
    config.auth.oidc.providers = HashMap::from([(MAP_PROBE_KEY.to_string(), probe_provider())]);

    config
}

fn probe_string_map() -> HashMap<String, String> {
    HashMap::from([(MAP_PROBE_KEY.to_string(), String::new())])
}

fn probe_provider() -> OidcProviderConfig {
    OidcProviderConfig {
        display_name: String::new(),
        issuer_url: String::new(),
        client_id: String::new(),
        client_secret: Some(String::new()),
        client_secret_env: Some(String::new()),
        scopes: Vec::new(),
        role_mapping: HashMap::from([(MAP_PROBE_KEY.to_string(), Vec::new())]),
        groups_claim: String::new(),
        username_claim: String::new(),
        email_claim: String::new(),
        accepted_audiences: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every top-level section must contribute at least one key. A new section
    /// added to `Config` without reaching the registry fails here.
    #[test]
    fn every_config_section_is_represented() {
        let registry = registry();
        for section in [
            "data_dir",
            "database.",
            "application.",
            "logging.",
            "auth.",
            "api.",
            "email.",
            "task.",
            "scanner.",
            "scheduler.",
            "files.",
            "pdf.",
            "pdf_handle_cache.",
            "komga_api.",
            "koreader_api.",
            "rate_limit.",
            "observability.",
        ] {
            assert!(
                registry.all().any(|k| k.starts_with(section)),
                "no key found for section `{section}`"
            );
        }
    }

    #[test]
    fn known_scalar_paths_are_present() {
        let registry = registry();
        for path in [
            "task.worker_count",
            "rate_limit.anonymous_rps",
            "pdf_handle_cache.capacity",
            "observability.otlp.endpoint",
            "observability.browser.sample_ratio",
            "scheduler.timezone",
            "koreader_api.enabled",
            "email.smtp_password",
        ] {
            assert!(registry.contains(path), "missing `{path}`");
        }
    }

    /// `database.postgres` is `None` in `Config::default()` because the default
    /// database is SQLite. Without the probe the entire postgres subtree would
    /// be missing from the registry.
    #[test]
    fn optional_subtrees_are_present() {
        let registry = registry();
        for path in [
            "database.postgres.host",
            "database.postgres.max_connections",
            "database.sqlite.path",
            "database.sqlite.background_max_connections",
        ] {
            assert!(registry.contains(path), "missing `{path}`");
        }
    }

    /// Guards the `Option<String>` leaves that serialize away when `None`.
    #[test]
    fn optional_leaves_are_present() {
        let registry = registry();
        for path in [
            "application.base_url",
            "logging.file",
            "email.verification_url_base",
            "pdf.pdfium_library_path",
            "observability.otlp.proxy_endpoint",
            "auth.oidc.redirect_uri_base",
        ] {
            assert!(registry.contains(path), "missing `{path}`");
        }
    }

    #[test]
    fn map_valued_nodes_become_wildcards() {
        let registry = registry();
        for path in [
            "auth.oidc.providers.*.issuer_url",
            "auth.oidc.providers.*.client_secret_env",
            "auth.oidc.providers.*.role_mapping.*",
            "database.sqlite.pragmas.*",
            "observability.otlp.headers.*",
        ] {
            assert!(
                registry.wildcard().contains(path),
                "missing wildcard `{path}`, have {:?}",
                registry.wildcard()
            );
        }
    }

    /// `CODEX_OBSERVABILITY_OTLP_HEADERS` sets every header at once as
    /// `k1=v1,k2=v2`, so the map container is a setting in its own right and
    /// not merely a prefix for per-key variables.
    #[test]
    fn map_containers_are_settable_as_a_whole() {
        let registry = registry();
        for path in [
            "observability.otlp.headers",
            "database.sqlite.pragmas",
            "auth.oidc.providers",
        ] {
            assert!(
                registry.exact().contains(path),
                "map container `{path}` should be settable on its own"
            );
        }
    }

    #[test]
    fn wildcards_match_concrete_paths() {
        let registry = registry();
        assert!(registry.contains("auth.oidc.providers.authentik.issuer_url"));
        assert!(registry.contains("auth.oidc.providers.my-idp.client_id"));
        assert!(registry.contains("observability.otlp.headers.authorization"));
        assert!(!registry.contains("auth.oidc.providers.authentik.nope"));
        assert!(!registry.contains("auth.oidc.providers.issuer_url"));
    }

    #[test]
    fn unknown_paths_are_rejected() {
        let registry = registry();
        assert!(!registry.contains("database.postgres.ssl_mode"));
        assert!(!registry.contains("plugins.log_level"));
        assert!(!registry.contains("task.worker.count"));
    }

    /// A `null` in the probe means an `Option` was left unpopulated, which
    /// would silently hide that key (or an entire subtree) from the registry.
    #[test]
    fn probe_leaves_no_option_unpopulated() {
        let value = serde_json::to_value(key_probe()).unwrap();
        let mut nulls = Vec::new();
        find_nulls(&value, "", &mut nulls);
        assert!(
            nulls.is_empty(),
            "key_probe() must populate every Option; these serialized as null: {nulls:?}"
        );
    }

    /// An empty object in the probe means a map was left empty, which would
    /// hide every key under it.
    #[test]
    fn probe_leaves_no_map_empty() {
        let value = serde_json::to_value(key_probe()).unwrap();
        let mut empties = Vec::new();
        find_empty_objects(&value, "", &mut empties);
        assert!(
            empties.is_empty(),
            "key_probe() must plant MAP_PROBE_KEY in every map; these were empty: {empties:?}"
        );
    }

    fn find_nulls(value: &Value, path: &str, out: &mut Vec<String>) {
        match value {
            Value::Null => out.push(path.to_string()),
            Value::Object(fields) => {
                for (name, child) in fields {
                    find_nulls(child, &join(path, name), out);
                }
            }
            _ => {}
        }
    }

    fn find_empty_objects(value: &Value, path: &str, out: &mut Vec<String>) {
        if let Value::Object(fields) = value {
            if fields.is_empty() {
                out.push(path.to_string());
                return;
            }
            for (name, child) in fields {
                find_empty_objects(child, &join(path, name), out);
            }
        }
    }

    fn join(path: &str, name: &str) -> String {
        if path.is_empty() {
            name.to_string()
        } else {
            format!("{path}.{name}")
        }
    }
}
