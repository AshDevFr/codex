//! Compares the live environment against the config key registry.
//!
//! Codex v1 names environment variables with a single `_` separating both
//! nesting levels and the words inside a field name, so
//! `CODEX_RATE_LIMIT_ANONYMOUS_RPS` means `rate_limit.anonymous_rps`. That is
//! not mechanically reversible, which is why v2 switches to `__` between
//! levels: `CODEX_RATE_LIMIT__ANONYMOUS_RPS`.
//!
//! This module maps a variable name to the setting it was aiming at. It powers
//! two things: `codex config check`, which reports the v2 name for every
//! variable that changes, and a single advisory line at startup.
//!
//! It also catches plain mistakes. Several variables in the documentation
//! today do nothing at all (`CODEX_DATABASE_POSTGRES_USER` instead of
//! `..._USERNAME`, for instance), and a variable that does not name a real
//! setting is silently ignored by the loader. That failure mode is the reason
//! this stays useful long after the v2 rename is behind us.

use crate::keys::{KeyRegistry, registry};
use crate::types::Config;
use std::collections::{BTreeSet, HashMap};
use std::sync::OnceLock;

/// The prefix every Codex environment variable carries.
pub const ENV_PREFIX: &str = "CODEX_";

/// `CODEX_`-prefixed variables that are deliberately not config keys.
///
/// These are read directly with `std::env::var` at their point of use rather
/// than through [`Config`], so the classifier must not report them as typos.
///
/// `CODEX_BIN_VERSION` is intentionally absent: it is a compile-time value set
/// by `codex-api`'s build script via `cargo:rustc-env` and read with `env!()`,
/// so it never exists in a running process's environment.
pub const NON_CONFIG_VARS: &[&str] = &[
    // Cookie `Secure` attribute override, applied per-response.
    "CODEX_COOKIE_SECURE",
    // Runs `serve` without in-process task workers.
    "CODEX_DISABLE_WORKERS",
    // Credential encryption key, read deep in codex-utils / codex-db.
    "CODEX_ENCRYPTION_KEY",
    // Bound on concurrent image decodes.
    "CODEX_IMAGE_DECODE_CONCURRENCY",
    // How often, and for how long, to poll while waiting on migrations.
    "CODEX_MIGRATION_WAIT_INTERVAL",
    "CODEX_MIGRATION_WAIT_TIMEOUT",
    // Extra executables plugins are permitted to spawn.
    "CODEX_PLUGIN_ALLOWED_COMMANDS",
    // Leaves migrations to an external job and waits for them instead.
    "CODEX_SKIP_MIGRATIONS",
    // Per-invocation endpoints for `codex copy`; also available as CLI flags.
    "CODEX_SOURCE_DATABASE_URL",
    "CODEX_TARGET_DATABASE_URL",
];

/// What the classifier concluded about one environment variable.
///
/// Carries data rather than rendered text so the same findings can be printed
/// as advice in v1.44 and raised as errors in v2.0.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Finding {
    /// A valid v1 name whose v2 spelling differs.
    WillRename {
        var: String,
        /// The v2 spelling, using `__` between nesting levels.
        v2_name: String,
        /// The config path both names resolve to.
        path: String,
    },
    /// A v2-style name, which this version does not read.
    ///
    /// Worth reporting loudly: the setting is being ignored right now, and
    /// renaming ahead of the upgrade is the way operators get here.
    NotYetValid {
        var: String,
        /// The name this version does read.
        v1_name: String,
    },
    /// Not a recognized setting. Never fatal: a sibling tool may legitimately
    /// use the `CODEX_` prefix.
    Unknown {
        var: String,
        /// Closest config path, when one is near enough to be worth showing.
        nearest: Option<String>,
    },
}

impl Finding {
    /// The environment variable this finding is about.
    pub fn var(&self) -> &str {
        match self {
            Finding::WillRename { var, .. }
            | Finding::NotYetValid { var, .. }
            | Finding::Unknown { var, .. } => var,
        }
    }

    /// Whether this finding means a setting is being ignored right now.
    pub fn is_ignored_now(&self) -> bool {
        matches!(self, Finding::NotYetValid { .. } | Finding::Unknown { .. })
    }
}

/// Classify every `CODEX_*` variable in the process environment.
pub fn audit_env() -> Vec<Finding> {
    audit(&env_var_names(), &BTreeSet::new())
}

/// Classify every `CODEX_*` variable, exempting those a provider's
/// `client_secret_env` points at.
///
/// That setting names an arbitrary variable to read a secret from, and the
/// documentation's own example (`CODEX_OIDC_AUTHENTIK_SECRET`) carries the
/// prefix. Without the resolved config those look like typos.
pub fn audit_env_with_config(config: &Config) -> Vec<Finding> {
    audit(&env_var_names(), &secret_env_targets(config))
}

/// Variables named by any provider's `client_secret_env`.
pub fn secret_env_targets(config: &Config) -> BTreeSet<String> {
    config
        .auth
        .oidc
        .providers
        .values()
        .filter_map(|p| p.client_secret_env.clone())
        .collect()
}

fn env_var_names() -> Vec<String> {
    let mut names: Vec<String> = std::env::vars()
        .map(|(k, _)| k)
        .filter(|k| k.starts_with(ENV_PREFIX))
        .collect();
    names.sort();
    names
}

/// Classify a set of variable names. Results are sorted by variable name so
/// output is stable across runs.
pub fn audit(names: &[String], exempt: &BTreeSet<String>) -> Vec<Finding> {
    let registry = registry();
    let mut findings: Vec<Finding> = names
        .iter()
        .filter(|n| !exempt.contains(n.as_str()))
        .filter_map(|n| classify(n, registry))
        .collect();
    findings.sort_by(|a, b| a.var().cmp(b.var()));
    findings
}

/// Classify one variable. `None` means "nothing to report".
pub fn classify(var: &str, registry: &KeyRegistry) -> Option<Finding> {
    let rest = var.strip_prefix(ENV_PREFIX)?;
    if rest.is_empty() || NON_CONFIG_VARS.contains(&var) {
        return None;
    }

    // A `__` anywhere means the operator wrote a v2-style name.
    if rest.contains("__") {
        let path = rest
            .split("__")
            .map(|s| s.to_lowercase())
            .collect::<Vec<_>>()
            .join(".");
        return Some(if registry.contains(&path) {
            Finding::NotYetValid {
                var: var.to_string(),
                v1_name: v1_name_for(&path),
            }
        } else {
            Finding::Unknown {
                var: var.to_string(),
                nearest: nearest_path(&path, registry),
            }
        });
    }

    match resolve_flat(rest, registry) {
        Some(path) => {
            let v2_name = v2_name_for(&path);
            // Single-segment settings such as `data_dir` spell the same in
            // both schemes; there is nothing for the operator to change.
            if v2_name == var {
                None
            } else {
                Some(Finding::WillRename {
                    var: var.to_string(),
                    v2_name,
                    path,
                })
            }
        }
        None => Some(Finding::Unknown {
            var: var.to_string(),
            nearest: nearest_path(rest, registry),
        }),
    }
}

/// Resolve a flat v1 suffix (everything after `CODEX_`) to a config path.
fn resolve_flat(rest: &str, registry: &KeyRegistry) -> Option<String> {
    if let Some(path) = normalized_index(registry).get(&normalize(rest)) {
        return Some(path.clone());
    }
    // Wildcard patterns cover operator-chosen map keys, whose names are not in
    // the registry. Match them against the raw text so the chosen key survives
    // with its original spelling.
    for pattern in registry.wildcard() {
        let segments: Vec<String> = pattern
            .split('.')
            .map(|s| {
                if s == "*" {
                    "*".to_string()
                } else {
                    s.to_uppercase()
                }
            })
            .collect();
        let refs: Vec<&str> = segments.iter().map(String::as_str).collect();
        if let Some(captures) = match_segments(&refs, rest) {
            let mut captures = captures.into_iter();
            let path: Vec<String> = pattern
                .split('.')
                .map(|s| {
                    if s == "*" {
                        captures.next().unwrap_or_default().to_lowercase()
                    } else {
                        s.to_string()
                    }
                })
                .collect();
            return Some(path.join("."));
        }
    }
    None
}

/// Match raw uppercase `segments` against `rest`, returning what each `*`
/// captured.
///
/// A capture may itself contain `_` (a provider named `MY_IDP`, for example),
/// so wildcards are tried longest-first and the literal that follows decides
/// where the capture actually ends.
fn match_segments(segments: &[&str], rest: &str) -> Option<Vec<String>> {
    let Some((first, tail)) = segments.split_first() else {
        return rest.is_empty().then(Vec::new);
    };

    if *first == "*" {
        if tail.is_empty() {
            return (!rest.is_empty()).then(|| vec![rest.to_string()]);
        }
        let mut boundaries: Vec<usize> = rest.match_indices('_').map(|(i, _)| i).collect();
        boundaries.reverse();
        for boundary in boundaries {
            if boundary == 0 {
                continue;
            }
            if let Some(mut rest_captures) = match_segments(tail, &rest[boundary + 1..]) {
                let mut captures = vec![rest[..boundary].to_string()];
                captures.append(&mut rest_captures);
                return Some(captures);
            }
        }
        return None;
    }

    let remainder = rest.strip_prefix(*first)?;
    if tail.is_empty() {
        return remainder.is_empty().then(Vec::new);
    }
    match_segments(tail, remainder.strip_prefix('_')?)
}

/// The v1 spelling of a config path: every separator is a single `_`.
pub fn v1_name_for(path: &str) -> String {
    format!("{ENV_PREFIX}{}", path.replace('.', "_").to_uppercase())
}

/// The v2 spelling of a config path: `__` between nesting levels, `_` inside a
/// field name.
pub fn v2_name_for(path: &str) -> String {
    let segments: Vec<String> = path.split('.').map(str::to_uppercase).collect();
    format!("{ENV_PREFIX}{}", segments.join("__"))
}

/// Lowercased, underscore-free form used to compare a variable name against a
/// config path when the two disagree about where separators go.
fn normalize(value: &str) -> String {
    value
        .chars()
        .filter(|c| *c != '_' && *c != '.')
        .flat_map(char::to_lowercase)
        .collect()
}

/// Normalized form of every exact path, mapped back to the path.
fn normalized_index(registry: &KeyRegistry) -> &'static HashMap<String, String> {
    static INDEX: OnceLock<HashMap<String, String>> = OnceLock::new();
    INDEX.get_or_init(|| {
        registry
            .exact()
            .iter()
            .map(|path| (normalize(path), path.clone()))
            .collect()
    })
}

/// Closest exact config path to `value`, when one is close enough to be a
/// plausible correction.
///
/// The tolerance scales with length so that `application.port` is offered for
/// `APPLICATION_PROT` without a short variable matching half the registry.
///
/// Candidates that extend the typed name (or that it extends) are preferred
/// over equidistant ones that diverge. Plain edit distance cannot separate
/// `..._USER` from `database.postgres.host` and `database.postgres.username`,
/// which are both four edits away, but only one of them is the abbreviation an
/// operator actually typed.
fn nearest_path(value: &str, registry: &KeyRegistry) -> Option<String> {
    let needle = normalize(value);
    if needle.is_empty() {
        return None;
    }
    let tolerance = std::cmp::max(3, needle.len() / 4);

    registry
        .exact()
        .iter()
        .filter_map(|path| {
            let candidate = normalize(path);
            let distance = levenshtein(&needle, &candidate);
            if distance > tolerance {
                return None;
            }
            let diverges =
                u8::from(!candidate.starts_with(&needle) && !needle.starts_with(&candidate));
            Some((diverges, distance, path))
        })
        .min()
        .map(|(_, _, path)| path.clone())
}

fn levenshtein(a: &str, b: &str) -> usize {
    let b_chars: Vec<char> = b.chars().collect();
    let mut previous: Vec<usize> = (0..=b_chars.len()).collect();
    let mut current = vec![0usize; b_chars.len() + 1];

    for (i, a_char) in a.chars().enumerate() {
        current[0] = i + 1;
        for (j, b_char) in b_chars.iter().enumerate() {
            let substitution = previous[j] + usize::from(a_char != *b_char);
            let deletion = previous[j + 1] + 1;
            let insertion = current[j] + 1;
            current[j + 1] = substitution.min(deletion).min(insertion);
        }
        std::mem::swap(&mut previous, &mut current);
    }
    previous[b_chars.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn classify_one(var: &str) -> Option<Finding> {
        classify(var, registry())
    }

    fn rename(var: &str) -> (String, String) {
        match classify_one(var) {
            Some(Finding::WillRename { v2_name, path, .. }) => (v2_name, path),
            other => panic!("expected {var} to be a rename, got {other:?}"),
        }
    }

    #[test]
    fn flat_names_map_to_double_underscore_names() {
        for (var, expected_v2, expected_path) in [
            (
                "CODEX_TASK_WORKER_COUNT",
                "CODEX_TASK__WORKER_COUNT",
                "task.worker_count",
            ),
            (
                "CODEX_RATE_LIMIT_ANONYMOUS_RPS",
                "CODEX_RATE_LIMIT__ANONYMOUS_RPS",
                "rate_limit.anonymous_rps",
            ),
            (
                "CODEX_SCANNER_MAX_CONCURRENT_SCANS",
                "CODEX_SCANNER__MAX_CONCURRENT_SCANS",
                "scanner.max_concurrent_scans",
            ),
            (
                "CODEX_APPLICATION_PORT",
                "CODEX_APPLICATION__PORT",
                "application.port",
            ),
            (
                "CODEX_OBSERVABILITY_METRICS_EXPORT_INTERVAL_MS",
                "CODEX_OBSERVABILITY__METRICS__EXPORT_INTERVAL_MS",
                "observability.metrics.export_interval_ms",
            ),
        ] {
            let (v2, path) = rename(var);
            assert_eq!(v2, expected_v2, "v2 name for {var}");
            assert_eq!(path, expected_path, "path for {var}");
        }
    }

    /// The section name itself contains underscores, so the split has to land
    /// after `PDF_HANDLE_CACHE`, not after `PDF`.
    #[test]
    fn multiword_section_names_split_in_the_right_place() {
        let (v2, path) = rename("CODEX_PDF_HANDLE_CACHE_CAPACITY");
        assert_eq!(v2, "CODEX_PDF_HANDLE_CACHE__CAPACITY");
        assert_eq!(path, "pdf_handle_cache.capacity");
    }

    #[test]
    fn nested_sections_get_one_separator_per_level() {
        let (v2, path) = rename("CODEX_DATABASE_POSTGRES_MAX_CONNECTIONS");
        assert_eq!(v2, "CODEX_DATABASE__POSTGRES__MAX_CONNECTIONS");
        assert_eq!(path, "database.postgres.max_connections");
    }

    /// `database.postgres` is `None` under the default SQLite config, so this
    /// only resolves because the key registry is built from a probe.
    #[test]
    fn postgres_subtree_resolves_despite_the_sqlite_default() {
        let (v2, _) = rename("CODEX_DATABASE_POSTGRES_PASSWORD");
        assert_eq!(v2, "CODEX_DATABASE__POSTGRES__PASSWORD");
    }

    #[test]
    fn oidc_provider_names_survive_the_rename() {
        let (v2, path) = rename("CODEX_AUTH_OIDC_PROVIDERS_AUTHENTIK_ISSUER_URL");
        assert_eq!(v2, "CODEX_AUTH__OIDC__PROVIDERS__AUTHENTIK__ISSUER_URL");
        assert_eq!(path, "auth.oidc.providers.authentik.issuer_url");
    }

    /// A provider whose name contains `_` is the case that makes naive
    /// suffix-stripping wrong.
    #[test]
    fn oidc_provider_names_may_contain_underscores() {
        let (v2, path) = rename("CODEX_AUTH_OIDC_PROVIDERS_MY_IDP_ISSUER_URL");
        assert_eq!(v2, "CODEX_AUTH__OIDC__PROVIDERS__MY_IDP__ISSUER_URL");
        assert_eq!(path, "auth.oidc.providers.my_idp.issuer_url");
    }

    /// `client_secret` is a prefix of `client_secret_env`, so the matcher has
    /// to reject the shorter pattern rather than capture a truncated name.
    #[test]
    fn overlapping_provider_field_names_resolve_exactly() {
        let (_, path) = rename("CODEX_AUTH_OIDC_PROVIDERS_AUTHENTIK_CLIENT_SECRET");
        assert_eq!(path, "auth.oidc.providers.authentik.client_secret");

        let (_, path) = rename("CODEX_AUTH_OIDC_PROVIDERS_AUTHENTIK_CLIENT_SECRET_ENV");
        assert_eq!(path, "auth.oidc.providers.authentik.client_secret_env");
    }

    #[test]
    fn two_wildcards_resolve_together() {
        let (v2, path) = rename("CODEX_AUTH_OIDC_PROVIDERS_AUTHENTIK_ROLE_MAPPING_ADMIN");
        assert_eq!(
            v2,
            "CODEX_AUTH__OIDC__PROVIDERS__AUTHENTIK__ROLE_MAPPING__ADMIN"
        );
        assert_eq!(path, "auth.oidc.providers.authentik.role_mapping.admin");
    }

    #[test]
    fn map_valued_settings_resolve() {
        let (_, path) = rename("CODEX_OBSERVABILITY_OTLP_HEADERS_AUTHORIZATION");
        assert_eq!(path, "observability.otlp.headers.authorization");
    }

    /// A single-segment setting spells the same either way, so there is
    /// nothing to report.
    #[test]
    fn top_level_settings_are_unchanged() {
        assert_eq!(classify_one("CODEX_DATA_DIR"), None);
    }

    #[test]
    fn non_config_vars_are_silent() {
        for var in NON_CONFIG_VARS {
            assert_eq!(classify_one(var), None, "{var} should be allowlisted");
        }
    }

    /// The compile-time build-script value must not be on the allowlist: it
    /// never appears in a running process's environment.
    #[test]
    fn bin_version_is_not_allowlisted() {
        assert!(!NON_CONFIG_VARS.contains(&"CODEX_BIN_VERSION"));
    }

    #[test]
    fn v2_names_are_reported_as_not_yet_valid() {
        match classify_one("CODEX_TASK__WORKER_COUNT") {
            Some(Finding::NotYetValid { v1_name, .. }) => {
                assert_eq!(v1_name, "CODEX_TASK_WORKER_COUNT");
            }
            other => panic!("expected NotYetValid, got {other:?}"),
        }
    }

    #[test]
    fn unrecognized_names_suggest_the_nearest_setting() {
        for (var, expected) in [
            ("CODEX_DATABASE_POSTGRES_USER", "database.postgres.username"),
            (
                "CODEX_DATABASE_POSTGRES_DATABASE",
                "database.postgres.database_name",
            ),
            ("CODEX_APPLICATION_PROT", "application.port"),
        ] {
            match classify_one(var) {
                Some(Finding::Unknown { nearest, .. }) => {
                    assert_eq!(
                        nearest.as_deref(),
                        Some(expected),
                        "wrong suggestion for {var}"
                    );
                }
                other => panic!("expected {var} to be Unknown, got {other:?}"),
            }
        }
    }

    /// Documented today but backed by no field at all.
    #[test]
    fn documented_but_nonexistent_settings_are_flagged() {
        for var in [
            "CODEX_DATABASE_POSTGRES_SSL_MODE",
            "CODEX_PLUGINS_LOG_LEVEL",
            "CODEX_THUMBNAIL_CACHE_DIR",
        ] {
            assert!(
                matches!(classify_one(var), Some(Finding::Unknown { .. })),
                "{var} should be Unknown, got {:?}",
                classify_one(var)
            );
        }
    }

    #[test]
    fn nonsense_names_suggest_nothing() {
        match classify_one("CODEX_TOTALLY_UNRELATED_THING_XYZ") {
            Some(Finding::Unknown { nearest, .. }) => assert_eq!(nearest, None),
            other => panic!("expected Unknown with no suggestion, got {other:?}"),
        }
    }

    #[test]
    fn variables_without_the_prefix_are_ignored() {
        assert_eq!(classify_one("PATH"), None);
        assert_eq!(classify_one("RUST_LOG"), None);
        assert_eq!(classify_one("CODEX_"), None);
    }

    #[test]
    fn findings_are_sorted_by_variable_name() {
        let names = vec![
            "CODEX_TASK_WORKER_COUNT".to_string(),
            "CODEX_APPLICATION_PORT".to_string(),
            "CODEX_RATE_LIMIT_ENABLED".to_string(),
        ];
        let findings = audit(&names, &BTreeSet::new());
        let vars: Vec<&str> = findings.iter().map(Finding::var).collect();
        assert_eq!(
            vars,
            [
                "CODEX_APPLICATION_PORT",
                "CODEX_RATE_LIMIT_ENABLED",
                "CODEX_TASK_WORKER_COUNT"
            ]
        );
    }

    #[test]
    fn exempt_variables_are_skipped() {
        let names = vec!["CODEX_OIDC_AUTHENTIK_SECRET".to_string()];
        assert_eq!(audit(&names, &BTreeSet::new()).len(), 1);

        let exempt = BTreeSet::from(["CODEX_OIDC_AUTHENTIK_SECRET".to_string()]);
        assert!(audit(&names, &exempt).is_empty());
    }

    /// Every variable the docs advertise must classify as either a clean
    /// rename or a flagged mistake, never silently vanish.
    #[test]
    fn every_documented_variable_is_accounted_for() {
        let documented = [
            "CODEX_API_ENABLE_API_DOCS",
            "CODEX_APPLICATION_HOST",
            "CODEX_APPLICATION_PORT",
            "CODEX_AUTH_JWT_SECRET",
            "CODEX_DATABASE_DB_TYPE",
            "CODEX_DATABASE_POSTGRES_DATABASE_NAME",
            "CODEX_DATABASE_POSTGRES_HOST",
            "CODEX_DATABASE_POSTGRES_PASSWORD",
            "CODEX_DATABASE_POSTGRES_PORT",
            "CODEX_DATABASE_POSTGRES_USERNAME",
            "CODEX_FILES_THUMBNAIL_DIR",
            "CODEX_FILES_UPLOADS_DIR",
            "CODEX_KOMGA_API_ENABLED",
            "CODEX_KOMGA_API_PREFIX",
            "CODEX_LOGGING_FILE",
            "CODEX_LOGGING_LEVEL",
            "CODEX_OBSERVABILITY_BROWSER_ENABLED",
            "CODEX_OBSERVABILITY_BROWSER_PROXY_PATH",
            "CODEX_OBSERVABILITY_BROWSER_SAMPLE_RATIO",
            "CODEX_OBSERVABILITY_ENABLED",
            "CODEX_OBSERVABILITY_METRICS_ENABLED",
            "CODEX_OBSERVABILITY_METRICS_EXPORT_INTERVAL_MS",
            "CODEX_OBSERVABILITY_OTLP_ENDPOINT",
            "CODEX_OBSERVABILITY_OTLP_PROTOCOL",
            "CODEX_OBSERVABILITY_OTLP_TIMEOUT_MS",
            "CODEX_OBSERVABILITY_SERVICE_NAME",
            "CODEX_OBSERVABILITY_TRACES_ENABLED",
            "CODEX_OBSERVABILITY_TRACES_SAMPLE_RATIO",
            "CODEX_PDF_CACHE_DIR",
            "CODEX_PDF_CACHE_RENDERED_PAGES",
            "CODEX_PDF_HANDLE_CACHE_CAPACITY",
            "CODEX_PDF_HANDLE_CACHE_ENABLED",
            "CODEX_PDF_HANDLE_CACHE_IDLE_TTL_MINUTES",
            "CODEX_PDF_HANDLE_CACHE_SWEEP_INTERVAL_SECONDS",
            "CODEX_PDF_JPEG_QUALITY",
            "CODEX_PDF_PDFIUM_LIBRARY_PATH",
            "CODEX_PDF_RENDER_DPI",
            "CODEX_RATE_LIMIT_ANONYMOUS_BURST",
            "CODEX_RATE_LIMIT_ANONYMOUS_RPS",
            "CODEX_RATE_LIMIT_AUTHENTICATED_BURST",
            "CODEX_RATE_LIMIT_AUTHENTICATED_RPS",
            "CODEX_RATE_LIMIT_BUCKET_TTL_SECS",
            "CODEX_RATE_LIMIT_CLEANUP_INTERVAL_SECS",
            "CODEX_RATE_LIMIT_ENABLED",
            "CODEX_RATE_LIMIT_EXEMPT_PATHS",
            "CODEX_SCANNER_MAX_CONCURRENT_SCANS",
            "CODEX_SCHEDULER_TIMEZONE",
            "CODEX_TASK_WORKER_COUNT",
        ];
        for var in documented {
            assert!(
                matches!(classify_one(var), Some(Finding::WillRename { .. })),
                "{var} should resolve to a rename, got {:?}",
                classify_one(var)
            );
        }
    }
}
