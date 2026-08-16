//! Compares the live environment against the config key registry.
//!
//! Codex v1 names environment variables with a single `_` separating both
//! nesting levels and the words inside a field name, so
//! `CODEX_RATE_LIMIT_ANONYMOUS_RPS` means `rate_limit.anonymous_rps`. That is
//! not mechanically reversible, which is why v2 switches to `__` between
//! levels: `CODEX_RATE_LIMIT__ANONYMOUS_RPS`.
//!
//! This module maps a variable name to the setting it was aiming at. A name
//! in the old flat form is no longer read, so it is reported as an error with
//! its replacement rather than ignored: a deployment that keeps
//! `CODEX_RATE_LIMIT_ANONYMOUS_RPS` would otherwise silently run with default
//! rate limits.
//!
//! It also catches plain mistakes. Several variables in the documentation
//! today do nothing at all (`CODEX_DATABASE_POSTGRES_USER` instead of
//! `..._USERNAME`, for instance), and a variable that does not name a real
//! setting is silently ignored by the loader. That failure mode is the reason
//! this stays useful long after the v2 rename is behind us.

use crate::keys::{KeyRegistry, registry};
use crate::loader::ENV_PREFIX;
use crate::types::{Config, ConfigError};
use std::collections::{BTreeSet, HashMap};
use std::sync::OnceLock;

/// `CODEX_`-prefixed variables that are deliberately not config keys.
///
/// These are read directly with `std::env::var` at their point of use rather
/// than through [`Config`], so the classifier must not report them as typos.
///
/// `CODEX_BIN_VERSION` is intentionally absent: it is a compile-time value set
/// by `codex-api`'s build script via `cargo:rustc-env` and read with `env!()`,
/// so it never exists in a running process's environment.
pub const NON_CONFIG_VARS: &[&str] = &[
    // Credential encryption key. Read at its point of use deep in
    // codex-utils / codex-db, which have no configuration in scope; bringing
    // it into `Config` means threading config through those crates.
    "CODEX_ENCRYPTION_KEY",
    // Per-invocation endpoints for `codex copy`, also available as CLI flags.
    "CODEX_SOURCE_DATABASE_URL",
    "CODEX_TARGET_DATABASE_URL",
];

/// Variables that were removed in favour of a real config key.
///
/// These cannot be derived by re-spelling, either because the setting was
/// renamed outright or because its sense was inverted, so they need saying
/// explicitly. Getting one of them wrong is not cosmetic: a deployment that
/// keeps `CODEX_DISABLE_WORKERS=true` and is not told about it starts task
/// workers in a pod meant to serve web traffic only.
pub const REMOVED_VARS: &[(&str, &str, &str)] = &[
    (
        "CODEX_COOKIE_SECURE",
        "CODEX_AUTH__COOKIE_SECURE",
        "same meaning",
    ),
    (
        "CODEX_DISABLE_WORKERS",
        "CODEX_TASK__RUN_IN_PROCESS",
        "INVERTED: `DISABLE_WORKERS=true` becomes `RUN_IN_PROCESS=false`",
    ),
    (
        "CODEX_IMAGE_DECODE_CONCURRENCY",
        "CODEX_IMAGES__DECODE_CONCURRENCY",
        "same meaning",
    ),
    (
        "CODEX_MIGRATION_WAIT_INTERVAL",
        "CODEX_DATABASE__MIGRATION_WAIT_INTERVAL_SECS",
        "same meaning",
    ),
    (
        "CODEX_MIGRATION_WAIT_TIMEOUT",
        "CODEX_DATABASE__MIGRATION_WAIT_TIMEOUT_SECS",
        "same meaning",
    ),
    (
        "CODEX_PLUGIN_ALLOWED_COMMANDS",
        "CODEX_PLUGINS__ALLOWED_COMMANDS",
        "same meaning",
    ),
    (
        "CODEX_SKIP_MIGRATIONS",
        "CODEX_DATABASE__RUN_MIGRATIONS",
        "INVERTED: `SKIP_MIGRATIONS=true` becomes `RUN_MIGRATIONS=false`",
    ),
];

/// What the classifier concluded about one environment variable.
///
/// Carries data rather than rendered text so the same findings can be printed
/// as advice in v1.44 and raised as errors in v2.0.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Finding {
    /// The old flat spelling. No longer read.
    Legacy {
        var: String,
        /// The nested spelling that replaces it.
        replacement: String,
        /// The config path both forms aimed at.
        path: String,
    },
    /// Removed in favour of a config key that cannot be derived by
    /// re-spelling the old name.
    Removed {
        var: String,
        /// The variable to set instead.
        replacement: String,
        /// How the meaning maps over, including any inversion.
        note: String,
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
            Finding::Legacy { var, .. }
            | Finding::Removed { var, .. }
            | Finding::Unknown { var, .. } => var,
        }
    }

    /// Whether this must stop startup.
    ///
    /// True where the operator named a real setting in a form that is no
    /// longer read, so continuing would run with a value they did not choose.
    /// An unrecognized name is only a warning: another tool may legitimately
    /// use the `CODEX_` prefix, and guessing wrong should not take a
    /// deployment down.
    pub fn is_fatal(&self) -> bool {
        matches!(self, Finding::Legacy { .. } | Finding::Removed { .. })
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

    // Checked before any spelling heuristic: these moved to a differently
    // named key, sometimes with the sense flipped, so guessing would be worse
    // than saying nothing.
    if let Some((_, replacement, note)) = REMOVED_VARS.iter().find(|(old, _, _)| *old == var) {
        return Some(Finding::Removed {
            var: var.to_string(),
            replacement: (*replacement).to_string(),
            note: (*note).to_string(),
        });
    }

    // A `__` anywhere means the operator wrote the current form. If it names a
    // real setting there is nothing to say; the loader has already read it.
    if rest.contains("__") {
        let path = rest
            .split("__")
            .map(|s| s.to_lowercase())
            .collect::<Vec<_>>()
            .join(".");
        return if registry.contains(&path) {
            None
        } else {
            Some(Finding::Unknown {
                var: var.to_string(),
                nearest: nearest_path(&path, registry),
            })
        };
    }

    match resolve_flat(rest, registry) {
        Some(path) => {
            let replacement = v2_name_for(&path);
            // Single-segment settings such as `data_dir` spell the same either
            // way, so they are still read and there is nothing to change.
            if replacement == var {
                None
            } else {
                Some(Finding::Legacy {
                    var: var.to_string(),
                    replacement,
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

/// Fail when the environment names a setting in a form that is no longer read.
///
/// Every offending variable is reported in one message. An operator with a
/// dozen of them should fix all twelve in one pass, not discover them one
/// restart at a time.
pub fn enforce_env(config: &Config) -> Result<Vec<Finding>, ConfigError> {
    let findings = audit_env_with_config(config);
    let (fatal, warnings): (Vec<_>, Vec<_>) = findings.iter().partition(|f| f.is_fatal());

    if fatal.is_empty() {
        return Ok(warnings.into_iter().cloned().collect());
    }

    let mut message = String::from("environment variables that are no longer read:\n");
    for finding in &fatal {
        match finding {
            Finding::Legacy {
                var, replacement, ..
            } => {
                message.push_str(&format!("  {var}\n      renamed to {replacement}\n"));
            }
            Finding::Removed {
                var,
                replacement,
                note,
            } => {
                message.push_str(&format!(
                    "  {var}\n      replaced by {replacement} ({note})\n"
                ));
            }
            _ => {}
        }
    }
    message.push_str(
        "\nNesting levels are separated by `__` since Codex 2.0. \
         Run `codex config check` to see this list without starting the server.",
    );

    Err(ConfigError::new(message))
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

/// The pre-2.0 flat spelling of a config path, where every separator was a
/// single `_`. Kept for describing what an operator must change.
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
            Some(Finding::Legacy {
                replacement, path, ..
            }) => (replacement, path),
            other => panic!("expected {var} to be a legacy name, got {other:?}"),
        }
    }

    #[test]
    fn legacy_flat_names_map_to_double_underscore_names() {
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
    /// Three entries, not ten: the operator-tunable settings moved into
    /// `Config`, leaving only a secret read too deep to reach and two
    /// per-invocation arguments.
    #[test]
    fn the_allowlist_is_only_what_cannot_be_config() {
        assert_eq!(
            NON_CONFIG_VARS,
            [
                "CODEX_ENCRYPTION_KEY",
                "CODEX_SOURCE_DATABASE_URL",
                "CODEX_TARGET_DATABASE_URL"
            ]
        );
    }

    /// Every removed variable must report its replacement. Silence here means
    /// a deployment keeps a setting that no longer does anything.
    #[test]
    fn removed_variables_name_their_replacement() {
        for (old, replacement, _) in REMOVED_VARS {
            match classify_one(old) {
                Some(Finding::Removed {
                    replacement: got, ..
                }) => assert_eq!(&got, replacement, "wrong replacement for {old}"),
                other => panic!("{old} should be Removed, got {other:?}"),
            }
        }
    }

    /// The two inverted ones are the dangerous pair: keeping the old variable
    /// and ignoring it silently flips behaviour.
    #[test]
    fn inverted_replacements_say_so() {
        for var in ["CODEX_DISABLE_WORKERS", "CODEX_SKIP_MIGRATIONS"] {
            match classify_one(var) {
                Some(Finding::Removed { note, .. }) => assert!(
                    note.contains("INVERTED"),
                    "{var} flips sense and must say so, got: {note}"
                ),
                other => panic!("{var} should be Removed, got {other:?}"),
            }
        }
    }

    /// Their new names must resolve as ordinary settings.
    #[test]
    fn the_replacements_are_real_settings() {
        for (_, replacement, _) in REMOVED_VARS {
            let path = replacement
                .strip_prefix(ENV_PREFIX)
                .unwrap()
                .split("__")
                .map(str::to_lowercase)
                .collect::<Vec<_>>()
                .join(".");
            assert!(
                registry().contains(&path),
                "{replacement} maps to `{path}`, which is not a config key"
            );
        }
    }

    #[test]
    fn bin_version_is_not_allowlisted() {
        assert!(!NON_CONFIG_VARS.contains(&"CODEX_BIN_VERSION"));
    }

    /// The current spelling is simply read; there is nothing to report.
    #[test]
    fn nested_names_are_accepted_silently() {
        assert_eq!(classify_one("CODEX_TASK__WORKER_COUNT"), None);
        assert_eq!(classify_one("CODEX_DATABASE__POSTGRES__HOST"), None);
    }

    /// A nested name that does not resolve is still just a warning.
    #[test]
    fn a_nested_name_for_no_setting_is_unknown() {
        assert!(matches!(
            classify_one("CODEX_TASK__NOPE"),
            Some(Finding::Unknown { .. })
        ));
    }

    #[test]
    fn legacy_and_removed_are_fatal_but_unknown_is_not() {
        assert!(classify_one("CODEX_TASK_WORKER_COUNT").unwrap().is_fatal());
        assert!(classify_one("CODEX_DISABLE_WORKERS").unwrap().is_fatal());
        assert!(!classify_one("CODEX_NOT_A_THING_AT_ALL").unwrap().is_fatal());
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
                matches!(classify_one(var), Some(Finding::Legacy { .. })),
                "{var} should be reported as a legacy spelling, got {:?}",
                classify_one(var)
            );
        }
    }
}
