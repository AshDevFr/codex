use anyhow::{Context, Result, bail};
use codex_cli_common::{init_tracing, load_config};
use codex_config::DatabaseConfig;
use codex_db::Database;
use codex_migrate::{database_config_from_url, guard, manifest, transfer};
use std::path::{Path, PathBuf};
use tracing::{info, warn};

/// Copy all database rows directly from one instance to another.
///
/// Each side resolves independently, in precedence order: explicit URL flag →
/// `CODEX_SOURCE_/TARGET_DATABASE_URL` env → `--from-config`/`--to-config`
/// file → the local instance config (`--config`) when that side is omitted.
/// At least one side must be non-local.
///
/// `copy` moves rows only; on-disk files are not transferred.
#[allow(clippy::too_many_arguments)]
pub async fn copy_command(
    config_path: PathBuf,
    from: Option<String>,
    to: Option<String>,
    from_config: Option<PathBuf>,
    to_config: Option<PathBuf>,
    replace: bool,
    progress: bool,
    no_verify: bool,
    full_verification: bool,
) -> Result<()> {
    // Local config: used for tracing and as the fallback for an omitted side.
    let (local_config, _created) = load_config(config_path.clone())?;
    let _tracing = init_tracing(&local_config)?;

    let source_explicit = resolve_side(
        from.as_deref(),
        from_config.as_deref(),
        "CODEX_SOURCE_DATABASE_URL",
    )?;
    let target_explicit = resolve_side(
        to.as_deref(),
        to_config.as_deref(),
        "CODEX_TARGET_DATABASE_URL",
    )?;

    if source_explicit.is_none() && target_explicit.is_none() {
        bail!(
            "copy needs at least one explicit side: pass --from/--to (or --from-config/--to-config, \
             or set CODEX_SOURCE_DATABASE_URL / CODEX_TARGET_DATABASE_URL). With neither, both sides \
             would be the local config — a copy onto itself."
        );
    }

    let source_cfg = source_explicit.unwrap_or_else(|| local_config.database.clone());
    let target_cfg = target_explicit.unwrap_or_else(|| local_config.database.clone());

    info!("Connecting to source and target databases...");
    let source = Database::new(&source_cfg)
        .await
        .context("Failed to connect to source database")?;
    let target = Database::new(&target_cfg)
        .await
        .context("Failed to connect to target database")?;

    // Ensure the target has the schema, then verify source and target agree.
    target
        .run_migrations()
        .await
        .context("Failed to run migrations on the target database")?;

    let source_version = manifest::schema_version(source.sea_orm_connection()).await?;
    let target_version = manifest::schema_version(target.sea_orm_connection()).await?;
    if source_version != target_version {
        bail!(
            "schema version mismatch: source is at {:?}, target is at {:?}. \
             Both instances must be on the same Codex schema.",
            source_version,
            target_version
        );
    }

    if !replace && guard::has_user_data(target.sea_orm_connection()).await? {
        bail!(
            "target database already contains data (libraries/series/books/users). \
             Refusing to overwrite. Re-run with --replace to replace it."
        );
    }

    info!("Copying database rows from source to target...");
    let report = transfer(
        source.sea_orm_connection(),
        target.sea_orm_connection(),
        codex_migrate::Progress::from_flag(progress),
    )
    .await
    .context("Copy failed")?;

    if !no_verify {
        let source_counts = codex_migrate::registry::count_all(source.sea_orm_connection()).await?;
        super::verify_row_counts(&source_counts, target.sea_orm_connection()).await?;
    }

    if full_verification {
        info!("Running full per-record verification...");
        let src_digests =
            codex_migrate::registry::digest_all_from_conn(source.sea_orm_connection()).await?;
        let dst_digests =
            codex_migrate::registry::digest_all_from_conn(target.sea_orm_connection()).await?;
        let mismatches = codex_migrate::full_verify::compare_digests(&src_digests, &dst_digests);
        super::report_full_verify(&mismatches, src_digests.len());
    }

    info!("========================================");
    info!(
        "✓ Copy complete: {} rows across {} tables",
        report.total_rows,
        report.tables.len()
    );
    warn!(
        "copy transfers database rows only — on-disk files (thumbnails, covers, plugin data) are \
         NOT moved. Sync those separately (rsync / volume copy), and ensure the target uses the \
         same encryption key as the source."
    );
    info!("========================================");
    Ok(())
}

/// Resolve one side of the copy. Returns `Some(config)` when an explicit source
/// is given (flag → env → config file), or `None` to signal "use local".
fn resolve_side(
    url: Option<&str>,
    config_file: Option<&Path>,
    env_key: &str,
) -> Result<Option<DatabaseConfig>> {
    if let Some(u) = url {
        return Ok(Some(database_config_from_url(u)?));
    }
    if let Ok(u) = std::env::var(env_key)
        && !u.is_empty()
    {
        return Ok(Some(database_config_from_url(&u)?));
    }
    if let Some(path) = config_file {
        let (config, _created) = load_config(path.to_path_buf())?;
        return Ok(Some(config.database));
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_config::DatabaseType;
    use codex_db::Database;
    use tempfile::TempDir;

    fn write_config(dir: &std::path::Path, name: &str) -> PathBuf {
        let cfg = dir.join(format!("{name}.yaml"));
        let content = format!(
            r#"
application:
  host: "127.0.0.1"
  port: 8080
database:
  db_type: sqlite
  sqlite:
    path: "{db}"
files:
  thumbnail_dir: "{base}/{name}-thumbnails"
  uploads_dir: "{base}/{name}-uploads"
  plugins_dir: "{base}/{name}-plugins"
"#,
            db = dir.join(format!("{name}.db")).display(),
            base = dir.display(),
            name = name,
        );
        std::fs::write(&cfg, content).unwrap();
        cfg
    }

    #[test]
    fn resolve_side_prefers_explicit_url_and_defaults_to_local() {
        let from_url = resolve_side(Some("sqlite:///tmp/x.db"), None, "CODEX_UNSET_XYZ")
            .unwrap()
            .expect("explicit url resolves to Some");
        assert_eq!(from_url.db_type, DatabaseType::SQLite);

        // No url, no env, no config file → None (meaning "use local").
        let none = resolve_side(None, None, "CODEX_UNSET_XYZ").unwrap();
        assert!(none.is_none());
    }

    #[tokio::test]
    async fn copy_pulls_source_url_into_local_target() {
        let dir = TempDir::new().unwrap();
        let src_cfg = write_config(dir.path(), "src");
        let tgt_cfg = write_config(dir.path(), "tgt");
        let src_db_path = dir.path().join("src.db");

        // Seed the source.
        {
            let (config, _) = load_config(src_cfg.clone()).unwrap();
            let db = Database::new(&config.database).await.unwrap();
            db.run_migrations().await.unwrap();
            db.create_library("Manga", "/lib", codex_db::ScanningStrategy::Default)
                .await
                .unwrap();
        }

        // copy --from sqlite://src into the local (tgt) config.
        copy_command(
            tgt_cfg.clone(),
            Some(format!("sqlite://{}", src_db_path.display())),
            None,
            None,
            None,
            false,
            false,
            false,
            false,
        )
        .await
        .expect("copy should succeed");

        let (config, _) = load_config(tgt_cfg).unwrap();
        let db = Database::new(&config.database).await.unwrap();
        let libs = db.list_libraries().await.unwrap();
        assert_eq!(libs.len(), 1);
        assert_eq!(libs[0].name, "Manga");
    }

    #[tokio::test]
    async fn copy_requires_at_least_one_explicit_side() {
        let dir = TempDir::new().unwrap();
        let cfg = write_config(dir.path(), "only");
        let err = copy_command(cfg, None, None, None, None, false, false, false, false)
            .await
            .expect_err("copy with no explicit side must error");
        assert!(err.to_string().contains("at least one explicit side"));
    }
}
