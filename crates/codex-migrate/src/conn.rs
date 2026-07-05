//! Build a [`DatabaseConfig`] from a connection URL, for the `copy` command's
//! `--from` / `--to` flags and the `CODEX_SOURCE_/TARGET_DATABASE_URL` env vars.
//!
//! Supported forms:
//! - `sqlite://<path>` (also `sqlite:<path>`) — e.g. `sqlite:///abs/codex.db`
//!   or `sqlite://relative/codex.db`
//! - `postgres://user:pass@host:port/dbname` (also `postgresql://`)
//!
//! Passwords with characters that need URL-escaping are fiddly to pass on a
//! command line; prefer `--from-config` / `--to-config` (or env) for those.

use anyhow::{Context, Result, bail};
use codex_config::{DatabaseConfig, DatabaseType, PostgresConfig, SQLiteConfig};

/// Parse a database connection URL into a [`DatabaseConfig`]. Pool settings and
/// other fields use their defaults.
pub fn database_config_from_url(raw: &str) -> Result<DatabaseConfig> {
    if let Some(rest) = raw
        .strip_prefix("sqlite://")
        .or_else(|| raw.strip_prefix("sqlite:"))
    {
        let path = rest.split('?').next().unwrap_or(rest);
        if path.is_empty() {
            bail!("sqlite URL is missing a file path: {raw}");
        }
        return Ok(DatabaseConfig {
            db_type: DatabaseType::SQLite,
            postgres: None,
            sqlite: Some(SQLiteConfig {
                path: path.to_string(),
                ..SQLiteConfig::default()
            }),
        });
    }

    if raw.starts_with("postgres://") || raw.starts_with("postgresql://") {
        let parsed =
            url::Url::parse(raw).with_context(|| format!("invalid postgres URL: {raw}"))?;
        let host = parsed.host_str().unwrap_or("localhost").to_string();
        let port = parsed.port().unwrap_or(5432);
        let username = parsed.username().to_string();
        let password = parsed.password().unwrap_or("").to_string();
        let database_name = parsed.path().trim_start_matches('/').to_string();
        if database_name.is_empty() {
            bail!("postgres URL is missing a database name: {raw}");
        }
        return Ok(DatabaseConfig {
            db_type: DatabaseType::Postgres,
            postgres: Some(PostgresConfig {
                host,
                port,
                username,
                password,
                database_name,
                ..PostgresConfig::default()
            }),
            sqlite: None,
        });
    }

    bail!("unsupported database URL scheme (expected sqlite:// or postgres://): {raw}");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_absolute_sqlite() {
        let cfg = database_config_from_url("sqlite:///var/lib/codex/codex.db").unwrap();
        assert_eq!(cfg.db_type, DatabaseType::SQLite);
        assert_eq!(cfg.sqlite.unwrap().path, "/var/lib/codex/codex.db");
    }

    #[test]
    fn parses_relative_sqlite_and_strips_query() {
        let cfg = database_config_from_url("sqlite://data/codex.db?mode=rwc").unwrap();
        assert_eq!(cfg.sqlite.unwrap().path, "data/codex.db");
    }

    #[test]
    fn parses_postgres() {
        let cfg =
            database_config_from_url("postgres://codex:secret@db.internal:5433/codexdb").unwrap();
        assert_eq!(cfg.db_type, DatabaseType::Postgres);
        let pg = cfg.postgres.unwrap();
        assert_eq!(pg.host, "db.internal");
        assert_eq!(pg.port, 5433);
        assert_eq!(pg.username, "codex");
        assert_eq!(pg.password, "secret");
        assert_eq!(pg.database_name, "codexdb");
    }

    #[test]
    fn postgres_defaults_port_when_absent() {
        let cfg = database_config_from_url("postgresql://u@localhost/db").unwrap();
        assert_eq!(cfg.postgres.unwrap().port, 5432);
    }

    #[test]
    fn rejects_unknown_scheme() {
        assert!(database_config_from_url("mysql://x/y").is_err());
        assert!(database_config_from_url("postgres://u@h/").is_err());
        assert!(database_config_from_url("sqlite://").is_err());
    }
}
