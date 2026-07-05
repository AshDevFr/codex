use anyhow::{Context, Result, bail};
use codex_cli_common::{init_tracing, load_config};
use codex_config::Config;
use codex_db::Database;
use codex_migrate::archive::{ArtifactTarget, import_archive, read_manifest};
use codex_migrate::manifest::ArtifactGroup;
use codex_migrate::{guard, manifest};
use std::path::PathBuf;
use tracing::{info, warn};

/// Import a `.tar.gz` produced by `export` into the current instance. Runs
/// migrations on the target, verifies the archive matches this schema, refuses
/// a target that already holds user data (unless `--replace`), then loads the
/// data, unpacks artifacts, and re-roots stored file paths.
pub async fn import_command(
    config_path: PathBuf,
    input: PathBuf,
    replace: bool,
    progress: bool,
    no_verify: bool,
    full_verification: bool,
) -> Result<()> {
    let (config, _created) = load_config(config_path.clone())?;
    let _tracing = init_tracing(&config)?;
    info!("Loading configuration from {:?}", config_path);

    if !input.exists() {
        bail!("archive not found: {}", input.display());
    }

    let db = Database::new(&config.database)
        .await
        .context("Failed to connect to database")?;

    // The target must have the schema before we can load into it.
    db.run_migrations()
        .await
        .context("Failed to run migrations on the target database")?;
    let conn = db.sea_orm_connection();

    // Pre-flight guards, before any destructive change.
    let archive_manifest = read_manifest(&input).context("Failed to read archive manifest")?;
    let target_version = manifest::schema_version(conn).await?;
    if archive_manifest.schema_version != target_version {
        bail!(
            "schema version mismatch: archive was exported at {:?}, this instance is at {:?}. \
             Import with a Codex build whose schema matches the archive.",
            archive_manifest.schema_version,
            target_version
        );
    }

    if !replace && guard::has_user_data(conn).await? {
        bail!(
            "target database already contains data (libraries/series/books/users). \
             Refusing to overwrite. Re-run with --replace to replace it with the archive."
        );
    }

    info!("Importing {} ...", input.display());
    let outcome = import_archive(
        conn,
        &input,
        &artifact_targets(&config),
        codex_migrate::Progress::from_flag(progress),
        full_verification,
    )
    .await
    .context("Import failed")?;

    if full_verification {
        super::report_full_verify(&outcome.full_verify, codex_migrate::table_names().len());
    }

    if !no_verify {
        // The manifest records the source's per-table counts at export time.
        let source_counts: Vec<codex_migrate::TableRows> = outcome
            .manifest
            .tables
            .iter()
            .map(|t| codex_migrate::TableRows {
                table: t.table.clone(),
                rows: t.rows,
            })
            .collect();
        super::verify_row_counts(&source_counts, conn).await?;
    }

    info!("========================================");
    info!(
        "✓ Import complete: {} rows across {} tables",
        outcome.report.total_rows,
        outcome.report.tables.len()
    );
    info!(
        "  re-rooted {} thumbnail path(s), {} cover path(s)",
        outcome.reroot.thumbnails, outcome.reroot.covers
    );
    warn!(
        "Reminder: encrypted values (e.g. plugin credentials) were copied as ciphertext. \
         This instance must be configured with the SAME encryption key as the source, or they \
         cannot be decrypted."
    );
    info!("========================================");
    Ok(())
}

/// Where each artifact group should be unpacked on this instance. `import`
/// only writes the groups actually present in the archive.
fn artifact_targets(config: &Config) -> Vec<ArtifactTarget> {
    vec![
        ArtifactTarget {
            group: ArtifactGroup::Thumbnails,
            target_dir: config.files.thumbnail_dir.clone().into(),
        },
        ArtifactTarget {
            group: ArtifactGroup::Uploads,
            target_dir: config.files.uploads_dir.clone().into(),
        },
        ArtifactTarget {
            group: ArtifactGroup::Plugins,
            target_dir: config.files.plugins_dir.clone().into(),
        },
        ArtifactTarget {
            group: ArtifactGroup::Cache,
            target_dir: config.pdf.cache_dir.clone().into(),
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::export::export_command;
    use codex_db::Database;
    use std::path::Path;
    use tempfile::TempDir;

    /// Write a minimal SQLite config with artifact dirs under `dir`.
    fn write_config(dir: &Path, name: &str) -> PathBuf {
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

    async fn seed_library(config_path: &Path, name: &str) {
        let (config, _) = load_config(config_path.to_path_buf()).unwrap();
        let db = Database::new(&config.database).await.unwrap();
        db.run_migrations().await.unwrap();
        db.create_library(name, "/lib", codex_db::ScanningStrategy::Default)
            .await
            .unwrap();
    }

    async fn library_names(config_path: &Path) -> Vec<String> {
        let (config, _) = load_config(config_path.to_path_buf()).unwrap();
        let db = Database::new(&config.database).await.unwrap();
        db.list_libraries()
            .await
            .unwrap()
            .into_iter()
            .map(|l| l.name)
            .collect()
    }

    #[tokio::test]
    async fn export_then_import_roundtrips_and_guards_nonfresh_target() {
        let dir = TempDir::new().unwrap();
        let src_cfg = write_config(dir.path(), "src");
        let tgt_cfg = write_config(dir.path(), "tgt");
        let archive = dir.path().join("export.tar.gz");

        seed_library(&src_cfg, "Comics").await;

        export_command(
            src_cfg.clone(),
            archive.clone(),
            false,
            false,
            false,
            false,
            false,
            false,
        )
        .await
        .expect("export should succeed");
        assert!(archive.exists(), "archive written");

        // Import into a fresh target.
        import_command(tgt_cfg.clone(), archive.clone(), false, false, false, false)
            .await
            .expect("import into fresh target should succeed");
        assert_eq!(library_names(&tgt_cfg).await, vec!["Comics".to_string()]);

        // A second import without --replace is refused (target now has data).
        let err = import_command(tgt_cfg.clone(), archive.clone(), false, false, false, false)
            .await
            .expect_err("import into non-fresh target should be refused");
        assert!(
            err.to_string().contains("already contains data"),
            "unexpected error: {err}"
        );

        // With --replace it succeeds and still mirrors the source.
        import_command(tgt_cfg.clone(), archive.clone(), true, false, false, false)
            .await
            .expect("import --replace should succeed");
        assert_eq!(library_names(&tgt_cfg).await, vec!["Comics".to_string()]);
    }

    #[tokio::test]
    async fn import_rejects_missing_archive() {
        let dir = TempDir::new().unwrap();
        let cfg = write_config(dir.path(), "tgt");
        let err = import_command(
            cfg,
            dir.path().join("nope.tar.gz"),
            false,
            false,
            false,
            false,
        )
        .await
        .expect_err("missing archive should error");
        assert!(err.to_string().contains("archive not found"));
    }
}
