//! Layered configuration loading: struct defaults -> config file -> local
//! overlay -> environment.
//!
//! The config file may be YAML or TOML; the provider is chosen from the file
//! extension. A sibling `<stem>.local.<ext>` file (for example
//! `codex.yaml` -> `codex.local.yaml`) is merged on top of the base file when
//! present, so an operator can pin secrets and per-host overrides without
//! editing the committed config.
//!
//! Environment variables use the `CODEX_` prefix with `__` between nesting
//! levels: `CODEX_RATE_LIMIT__ANONYMOUS_RPS` sets `rate_limit.anonymous_rps`.
//! A single `_` separates words *within* one key, which is why the separator
//! has to be doubled: Codex v1 used a single `_` for both jobs, making
//! `CODEX_RATE_LIMIT_ANONYMOUS_RPS` impossible to split correctly without a
//! hand-maintained table of section names.

use super::types::Config;
use anyhow::{Context, Result};
use figment::providers::{Env, Format, Serialized, Toml, Yaml};
use figment::{Figment, Profile};
use std::fs;
use std::path::{Path, PathBuf};

/// Prefix on every Codex environment variable.
pub const ENV_PREFIX: &str = "CODEX_";

/// Separator between nesting levels in an environment variable name.
pub const ENV_NESTING_SEPARATOR: &str = "__";

impl Config {
    /// Resolve configuration from `path`, its local overlay, and the
    /// environment.
    ///
    /// A missing config file is not an error: defaults plus the environment
    /// are a complete configuration on their own, which is what a container
    /// with no mounted file relies on.
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();

        // The same layers twice: once over the defaults to produce the config,
        // and once on their own to answer "did anyone actually set this key?".
        // Without the second chain a value equal to its default is
        // indistinguishable from an unset one, which is what path rooting
        // needs to know.
        let overrides = layers(path, Figment::new());
        let figment = layers(path, Figment::from(Serialized::defaults(Config::default())));

        let mut config: Config = figment
            .extract()
            .with_context(|| format!("failed to parse configuration from {}", path.display()))?;

        config.root_paths_at_data_dir(&|key| overrides.find_value(key).is_ok());

        Ok(config)
    }

    /// Read a single config file, ignoring the overlay and the environment.
    ///
    /// Only for callers that want the file's literal contents, such as reading
    /// the other side's config during `codex copy`. Everything that configures
    /// a running process wants [`Config::load`].
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        Figment::from(Serialized::defaults(Config::default()))
            .merge(file_provider(path))
            .extract()
            .with_context(|| format!("failed to parse configuration from {}", path.display()))
    }

    pub fn to_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let yaml = serde_yaml::to_string(self)?;
        fs::write(path, yaml)?;
        Ok(())
    }
}

/// Stack the file, overlay and environment layers onto `base`.
fn layers(path: &Path, base: Figment) -> Figment {
    let mut figment = base;

    if path.exists() {
        figment = figment.merge(file_provider(path));
    }

    if let Some(local) = local_overlay_path(path)
        && local.exists()
    {
        figment = figment.merge(file_provider(&local));
    }

    figment.merge(Env::prefixed(ENV_PREFIX).split(ENV_NESTING_SEPARATOR))
}

/// Pick a provider by file extension. Anything that is not `.toml` is read as
/// YAML, which keeps `codex.yaml`, `codex.yml` and extensionless paths working.
fn file_provider(path: &Path) -> Figment {
    match path.extension().and_then(|ext| ext.to_str()) {
        Some(ext) if ext.eq_ignore_ascii_case("toml") => {
            Figment::from(Toml::file(path).profile(Profile::Default))
        }
        _ => Figment::from(Yaml::file(path).profile(Profile::Default)),
    }
}

/// `config/codex.yaml` -> `config/codex.local.yaml`.
///
/// Returns `None` for a path with no extension, where there is no sensible
/// place to put the `.local` infix.
fn local_overlay_path(path: &Path) -> Option<PathBuf> {
    let extension = path.extension()?.to_str()?;
    let stem = path.file_stem()?.to_str()?;
    let mut local = path.to_path_buf();
    local.set_file_name(format!("{stem}.local.{extension}"));
    Some(local)
}

#[cfg(test)]
// `figment::Error` is a large type and every `Jail` closure returns it, which
// is the shape figment's test harness requires.
#[allow(clippy::result_large_err)]
mod tests {
    use super::*;
    use crate::DatabaseType;
    use crate::types::LogLevel;
    use figment::Jail;

    #[test]
    fn defaults_apply_when_no_file_exists() {
        Jail::expect_with(|jail| {
            let config = Config::load(jail.directory().join("absent.yaml")).unwrap();
            assert_eq!(config.application.host, "0.0.0.0");
            assert_eq!(config.application.port, 8080);
            assert_eq!(config.task.worker_count, 2);
            Ok(())
        });
    }

    #[test]
    fn the_config_file_overrides_defaults() {
        Jail::expect_with(|jail| {
            jail.create_file("codex.yaml", "application:\n  port: 9000\n")?;
            let config = Config::load(jail.directory().join("codex.yaml")).unwrap();
            assert_eq!(config.application.port, 9000);
            assert_eq!(
                config.application.host, "0.0.0.0",
                "untouched key keeps its default"
            );
            Ok(())
        });
    }

    #[test]
    fn a_toml_file_is_read_as_toml() {
        Jail::expect_with(|jail| {
            jail.create_file("codex.toml", "[application]\nport = 9100\n")?;
            let config = Config::load(jail.directory().join("codex.toml")).unwrap();
            assert_eq!(config.application.port, 9100);
            Ok(())
        });
    }

    #[test]
    fn the_local_overlay_beats_the_base_file() {
        Jail::expect_with(|jail| {
            jail.create_file(
                "codex.yaml",
                "application:\n  port: 9000\n  host: 1.1.1.1\n",
            )?;
            jail.create_file("codex.local.yaml", "application:\n  port: 9999\n")?;

            let config = Config::load(jail.directory().join("codex.yaml")).unwrap();

            assert_eq!(config.application.port, 9999);
            assert_eq!(
                config.application.host, "1.1.1.1",
                "the overlay merges field by field rather than replacing the section"
            );
            Ok(())
        });
    }

    #[test]
    fn the_local_overlay_is_optional() {
        Jail::expect_with(|jail| {
            jail.create_file("codex.yaml", "application:\n  port: 9001\n")?;
            let config = Config::load(jail.directory().join("codex.yaml")).unwrap();
            assert_eq!(config.application.port, 9001);
            Ok(())
        });
    }

    /// The whole precedence chain in one test, one layer per level.
    #[test]
    fn the_environment_wins_over_every_file_layer() {
        Jail::expect_with(|jail| {
            jail.create_file("codex.yaml", "application:\n  port: 2\n")?;
            jail.create_file("codex.local.yaml", "application:\n  port: 3\n")?;
            jail.set_env("CODEX_APPLICATION__PORT", "4");

            let config = Config::load(jail.directory().join("codex.yaml")).unwrap();

            assert_eq!(config.application.port, 4);
            Ok(())
        });
    }

    #[test]
    fn nested_sections_use_one_separator_per_level() {
        Jail::expect_with(|jail| {
            jail.set_env("CODEX_RATE_LIMIT__ANONYMOUS_RPS", "77");
            jail.set_env("CODEX_OBSERVABILITY__OTLP__TIMEOUT_MS", "1234");
            jail.set_env("CODEX_PDF_HANDLE_CACHE__CAPACITY", "999");

            let config = Config::load(jail.directory().join("absent.yaml")).unwrap();

            assert_eq!(config.rate_limit.anonymous_rps, 77);
            assert_eq!(config.observability.otlp.timeout_ms, 1234);
            assert_eq!(config.pdf_handle_cache.capacity, 999);
            Ok(())
        });
    }

    /// v1 spelling must NOT be honoured. It is reported by the validator with
    /// its replacement instead of quietly half-working.
    #[test]
    fn the_old_flat_names_are_not_read() {
        Jail::expect_with(|jail| {
            jail.set_env("CODEX_APPLICATION_PORT", "4321");
            let config = Config::load(jail.directory().join("absent.yaml")).unwrap();
            assert_eq!(config.application.port, 8080);
            Ok(())
        });
    }

    /// The env layer can create a subtree the file never mentioned. In v1 this
    /// silently did nothing, because the override was only applied when
    /// `database.postgres` was already `Some`, and `display_database_config`
    /// then unwrapped the `None`.
    #[test]
    fn the_environment_can_introduce_the_postgres_subtree() {
        Jail::expect_with(|jail| {
            jail.create_file("codex.yaml", "database:\n  db_type: sqlite\n")?;
            jail.set_env("CODEX_DATABASE__DB_TYPE", "postgres");
            jail.set_env("CODEX_DATABASE__POSTGRES__HOST", "db.internal");
            jail.set_env("CODEX_DATABASE__POSTGRES__PASSWORD", "hunter2");

            let config = Config::load(jail.directory().join("codex.yaml")).unwrap();

            assert_eq!(config.database.db_type, DatabaseType::Postgres);
            let postgres = config
                .database
                .postgres
                .expect("env should have created the postgres section");
            assert_eq!(postgres.host, "db.internal");
            assert_eq!(postgres.password, "hunter2");
            assert_eq!(
                postgres.port, 5432,
                "unset keys still fall back to defaults"
            );
            Ok(())
        });
    }

    #[test]
    fn maps_round_trip_through_the_file_layer() {
        Jail::expect_with(|jail| {
            jail.create_file(
                "codex.yaml",
                r#"
database:
  sqlite:
    pragmas:
      journal_mode: DELETE
auth:
  oidc:
    providers:
      authentik:
        display_name: Authentik
        issuer_url: https://idp.example.com
        client_id: codex
"#,
            )?;

            let config = Config::load(jail.directory().join("codex.yaml")).unwrap();

            let pragmas = config.database.sqlite.unwrap().pragmas.unwrap();
            assert_eq!(
                pragmas.get("journal_mode").map(String::as_str),
                Some("DELETE")
            );

            let provider = &config.auth.oidc.providers["authentik"];
            assert_eq!(provider.issuer_url, "https://idp.example.com");
            assert_eq!(
                provider.groups_claim, "groups",
                "provider fields absent from the file keep their serde defaults"
            );
            Ok(())
        });
    }

    #[test]
    fn enums_parse_from_both_the_file_and_the_environment() {
        Jail::expect_with(|jail| {
            jail.create_file("codex.yaml", "logging:\n  level: debug\n")?;
            let config = Config::load(jail.directory().join("codex.yaml")).unwrap();
            assert!(matches!(config.logging.level, LogLevel::Debug));

            jail.set_env("CODEX_LOGGING__LEVEL", "warn");
            let config = Config::load(jail.directory().join("codex.yaml")).unwrap();
            assert!(matches!(config.logging.level, LogLevel::Warn));
            Ok(())
        });
    }

    #[test]
    fn a_malformed_file_is_an_error_naming_the_path() {
        Jail::expect_with(|jail| {
            jail.create_file("codex.yaml", "this is not valid yaml: {{{}")?;
            let error = Config::load(jail.directory().join("codex.yaml")).unwrap_err();
            assert!(
                error.to_string().contains("codex.yaml"),
                "error should name the file: {error}"
            );
            Ok(())
        });
    }

    // ---- path rooting under data_dir (replaces the old sentinel logic) ----

    #[test]
    fn absent_paths_are_rooted_at_data_dir() {
        Jail::expect_with(|jail| {
            jail.create_file("codex.yaml", "data_dir: /var/lib/codex\n")?;

            let config = Config::load(jail.directory().join("codex.yaml")).unwrap();

            assert_eq!(config.files.thumbnail_dir, "/var/lib/codex/thumbnails");
            assert_eq!(config.files.uploads_dir, "/var/lib/codex/uploads");
            assert_eq!(config.files.plugins_dir, "/var/lib/codex/plugins");
            assert_eq!(config.pdf.cache_dir, "/var/lib/codex/cache");
            assert_eq!(
                config.database.sqlite.unwrap().path,
                "/var/lib/codex/codex.db"
            );
            Ok(())
        });
    }

    #[test]
    fn an_explicit_path_survives_a_different_data_dir() {
        Jail::expect_with(|jail| {
            jail.create_file(
                "codex.yaml",
                "data_dir: /var/lib/codex\nfiles:\n  thumbnail_dir: /mnt/thumbs\n",
            )?;

            let config = Config::load(jail.directory().join("codex.yaml")).unwrap();

            assert_eq!(config.files.thumbnail_dir, "/mnt/thumbs");
            assert_eq!(config.files.uploads_dir, "/var/lib/codex/uploads");
            Ok(())
        });
    }

    /// The case the old sentinel got wrong: writing the literal default is an
    /// explicit choice, and used to be silently rewritten.
    #[test]
    fn writing_the_literal_default_path_is_respected() {
        Jail::expect_with(|jail| {
            jail.create_file(
                "codex.yaml",
                "data_dir: /var/lib/codex\nfiles:\n  thumbnail_dir: data/thumbnails\n",
            )?;

            let config = Config::load(jail.directory().join("codex.yaml")).unwrap();

            assert_eq!(
                config.files.thumbnail_dir, "data/thumbnails",
                "an explicitly written path must not be rewritten, even when it \
                 happens to equal the old hardcoded default"
            );
            Ok(())
        });
    }

    #[test]
    fn the_environment_can_set_a_path_directly() {
        Jail::expect_with(|jail| {
            jail.create_file("codex.yaml", "data_dir: /var/lib/codex\n")?;
            jail.set_env("CODEX_FILES__THUMBNAIL_DIR", "/mnt/env-thumbs");

            let config = Config::load(jail.directory().join("codex.yaml")).unwrap();

            assert_eq!(config.files.thumbnail_dir, "/mnt/env-thumbs");
            Ok(())
        });
    }

    #[test]
    fn the_default_data_dir_produces_the_historical_layout() {
        Jail::expect_with(|jail| {
            let config = Config::load(jail.directory().join("absent.yaml")).unwrap();
            assert_eq!(config.files.thumbnail_dir, "data/thumbnails");
            assert_eq!(config.pdf.cache_dir, "data/cache");
            assert_eq!(config.database.sqlite.unwrap().path, "data/codex.db");
            Ok(())
        });
    }

    /// Every scalar setting must be reachable from the environment.
    ///
    /// This replaces the per-key tests that lived in the deleted
    /// `env_override.rs`. Rather than a hand-written list that goes stale, it
    /// walks the key registry, sets each key through its `__` name with a
    /// value of the right shape, and checks the loaded config actually
    /// changed at that path.
    #[test]
    fn every_scalar_setting_is_reachable_from_the_environment() {
        use serde_json::Value;

        /// Enum-valued keys need a valid variant, not an arbitrary string.
        /// The third column is the canonical spelling the value serializes
        /// back to, which differs from the input wherever an alias is used.
        const ENUM_VALUES: &[(&str, &str, &str)] = &[
            ("database.db_type", "postgresql", "postgres"),
            ("logging.level", "warn", "warn"),
            ("observability.otlp.protocol", "http/json", "http-json"),
            ("auth.oidc.default_role", "admin", "admin"),
        ];

        fn at<'v>(value: &'v Value, path: &str) -> Option<&'v Value> {
            path.split('.').try_fold(value, |node, seg| node.get(seg))
        }

        let defaults = serde_json::to_value(Config::default()).unwrap();
        let mut checked = 0usize;

        for key in crate::registry().exact() {
            // Only keys present in the default tree can be shape-inferred.
            // Subtrees that default to absent (postgres) are covered by
            // `the_environment_can_introduce_the_postgres_subtree`.
            let Some(current) = at(&defaults, key) else {
                continue;
            };

            let (raw, expected): (String, Value) = match ENUM_VALUES
                .iter()
                .find(|(k, _, _)| k == key)
            {
                Some((_, input, canonical)) => ((*input).to_string(), Value::from(*canonical)),
                None => match current {
                    Value::Bool(b) => ((!b).to_string(), Value::Bool(!b)),
                    Value::Number(n) if n.is_f64() => ("0.25".to_string(), Value::from(0.25f64)),
                    Value::Number(n) => {
                        let next = n.as_u64().unwrap_or(0) + 7;
                        (next.to_string(), Value::from(next))
                    }
                    Value::String(_) | Value::Null => {
                        ("env-probe".to_string(), Value::from("env-probe"))
                    }
                    // Arrays and maps need the lenient env forms, which are
                    // covered by their own tests.
                    _ => continue,
                },
            };

            let var = crate::v2_name_for(key);
            Jail::expect_with(|jail| {
                jail.set_env(&var, &raw);
                let config = Config::load(jail.directory().join("absent.yaml"))
                    .unwrap_or_else(|e| panic!("{var}={raw} should load: {e:#}"));
                let loaded = serde_json::to_value(config).unwrap();
                assert_eq!(
                    at(&loaded, key),
                    Some(&expected),
                    "setting {var} did not reach `{key}`"
                );
                Ok(())
            });
            checked += 1;
        }

        assert!(
            checked > 60,
            "expected broad coverage of the settable surface, only checked {checked}"
        );
    }

    // ---- input shapes the v1 override layer accepted ----

    #[test]
    fn bools_accept_one_and_zero_from_the_environment() {
        Jail::expect_with(|jail| {
            jail.set_env("CODEX_KOMGA_API__ENABLED", "1");
            jail.set_env("CODEX_API__CORS_ENABLED", "0");

            let config = Config::load(jail.directory().join("absent.yaml")).unwrap();

            assert!(config.komga_api.enabled);
            assert!(!config.api.cors_enabled);
            Ok(())
        });
    }

    /// v1 read `eq_ignore_ascii_case("true") || == "1"`, so a typo meant
    /// `false` and the operator never found out. It is now an error.
    #[test]
    fn a_misspelled_bool_stops_startup() {
        Jail::expect_with(|jail| {
            jail.set_env("CODEX_KOMGA_API__ENABLED", "ture");
            let error = Config::load(jail.directory().join("absent.yaml")).unwrap_err();
            assert!(
                format!("{error:#}").contains("ture"),
                "error should quote the bad value: {error:#}"
            );
            Ok(())
        });
    }

    #[test]
    fn comma_separated_lists_come_through_the_environment() {
        Jail::expect_with(|jail| {
            jail.set_env(
                "CODEX_API__CORS_ORIGINS",
                "https://a.example, https://b.example",
            );
            jail.set_env("CODEX_RATE_LIMIT__EXEMPT_PATHS", "/health,/metrics");

            let config = Config::load(jail.directory().join("absent.yaml")).unwrap();

            assert_eq!(
                config.api.cors_origins,
                vec!["https://a.example", "https://b.example"]
            );
            assert_eq!(config.rate_limit.exempt_paths, vec!["/health", "/metrics"]);
            Ok(())
        });
    }

    #[test]
    fn a_yaml_sequence_still_works_for_the_same_field() {
        Jail::expect_with(|jail| {
            jail.create_file(
                "codex.yaml",
                "api:\n  cors_origins:\n    - https://a.example\n    - https://b.example\n",
            )?;
            let config = Config::load(jail.directory().join("codex.yaml")).unwrap();
            assert_eq!(
                config.api.cors_origins,
                vec!["https://a.example", "https://b.example"]
            );
            Ok(())
        });
    }

    #[test]
    fn otlp_headers_accept_key_value_pairs() {
        Jail::expect_with(|jail| {
            jail.set_env(
                "CODEX_OBSERVABILITY__OTLP__HEADERS",
                "authorization=Bearer tok,x-tenant=acme",
            );

            let config = Config::load(jail.directory().join("absent.yaml")).unwrap();

            assert_eq!(
                config
                    .observability
                    .otlp
                    .headers
                    .get("authorization")
                    .map(String::as_str),
                Some("Bearer tok")
            );
            assert_eq!(
                config
                    .observability
                    .otlp
                    .headers
                    .get("x-tenant")
                    .map(String::as_str),
                Some("acme")
            );
            Ok(())
        });
    }

    /// Blanking a variable and unsetting it are the same gesture in most
    /// deployment tooling, and v1's `env_string_opt` filtered empties.
    #[test]
    fn an_empty_optional_string_means_unset() {
        Jail::expect_with(|jail| {
            jail.create_file("codex.yaml", "logging:\n  file: /var/log/codex.log\n")?;
            jail.set_env("CODEX_LOGGING__FILE", "");

            let config = Config::load(jail.directory().join("codex.yaml")).unwrap();

            assert_eq!(config.logging.file, None);
            Ok(())
        });
    }

    /// v1 set role mappings one role at a time; each value is a group list.
    #[test]
    fn oidc_role_mapping_is_set_per_role() {
        Jail::expect_with(|jail| {
            jail.set_env(
                "CODEX_AUTH__OIDC__PROVIDERS__AUTHENTIK__ISSUER_URL",
                "https://idp.example.com",
            );
            jail.set_env(
                "CODEX_AUTH__OIDC__PROVIDERS__AUTHENTIK__ROLE_MAPPING__ADMIN",
                "codex-admins, platform",
            );

            let config = Config::load(jail.directory().join("absent.yaml")).unwrap();

            let provider = &config.auth.oidc.providers["authentik"];
            assert_eq!(provider.issuer_url, "https://idp.example.com");
            assert_eq!(
                provider.role_mapping.get("admin"),
                Some(&vec!["codex-admins".to_string(), "platform".to_string()])
            );
            Ok(())
        });
    }

    #[test]
    fn database_type_accepts_the_longer_spelling() {
        Jail::expect_with(|jail| {
            jail.set_env("CODEX_DATABASE__DB_TYPE", "postgresql");
            let config = Config::load(jail.directory().join("absent.yaml")).unwrap();
            assert_eq!(config.database.db_type, DatabaseType::Postgres);
            Ok(())
        });
    }

    #[test]
    fn overlay_path_gets_a_local_infix() {
        assert_eq!(
            local_overlay_path(Path::new("config/codex.yaml")),
            Some(PathBuf::from("config/codex.local.yaml"))
        );
        assert_eq!(
            local_overlay_path(Path::new("/etc/codex/config.docker.toml")),
            Some(PathBuf::from("/etc/codex/config.docker.local.toml"))
        );
        assert_eq!(local_overlay_path(Path::new("codex")), None);
    }

    /// Every shipped example must still load. These are what operators copy.
    #[test]
    fn the_bundled_example_configs_load() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap();
        for name in [
            "config.docker.yaml",
            "config.kubernetes.yaml",
            "config.sqlite.yaml",
            "config.screenshots.yaml",
        ] {
            let path = root.join("config").join(name);
            assert!(path.exists(), "missing example config {}", path.display());
            Config::from_file(&path).unwrap_or_else(|e| panic!("{name} should load: {e:#}"));
        }
    }
}
