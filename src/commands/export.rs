use anyhow::{Context, Result};
use codex_cli_common::{init_tracing, load_config};
use codex_db::Database;
use codex_migrate::archive::{ArtifactSource, export_archive};
use codex_migrate::manifest::ArtifactGroup;
use std::path::PathBuf;
use tracing::info;

/// Export the current instance's database and on-disk artifacts to a portable
/// `.tar.gz` archive. The database is written as one NDJSON file per table; the
/// default artifact bundle is thumbnails + uploads + plugin data.
#[allow(clippy::too_many_arguments)]
pub async fn export_command(
    config_path: PathBuf,
    output: PathBuf,
    include_cache: bool,
    db_only: bool,
    no_thumbnails: bool,
    no_uploads: bool,
    no_plugins: bool,
) -> Result<()> {
    let (config, _created) = load_config(config_path.clone())?;
    let _tracing = init_tracing(&config)?;
    info!("Loading configuration from {:?}", config_path);

    let db = Database::new(&config.database)
        .await
        .context("Failed to connect to database")?;

    let mut artifacts = Vec::new();
    if !db_only {
        if !no_thumbnails {
            artifacts.push(ArtifactSource {
                group: ArtifactGroup::Thumbnails,
                source_dir: config.files.thumbnail_dir.clone().into(),
            });
        }
        if !no_uploads {
            artifacts.push(ArtifactSource {
                group: ArtifactGroup::Uploads,
                source_dir: config.files.uploads_dir.clone().into(),
            });
        }
        if !no_plugins {
            artifacts.push(ArtifactSource {
                group: ArtifactGroup::Plugins,
                source_dir: config.files.plugins_dir.clone().into(),
            });
        }
        if include_cache {
            artifacts.push(ArtifactSource {
                group: ArtifactGroup::Cache,
                source_dir: config.pdf.cache_dir.clone().into(),
            });
        }
    }

    info!("Exporting database to {}", output.display());
    let manifest = export_archive(db.sea_orm_connection(), &output, &artifacts)
        .await
        .context("Export failed")?;

    info!("========================================");
    info!(
        "✓ Export complete: {} rows across {} tables",
        manifest.total_rows,
        manifest.tables.len()
    );
    if manifest.artifacts.is_empty() {
        info!("  (database only — no artifacts bundled)");
    } else {
        for entry in &manifest.artifacts {
            info!(
                "  bundled {} from {}",
                entry.group.archive_dir(),
                entry.source_base_dir
            );
        }
    }
    info!("Archive written to {}", output.display());
    info!("========================================");
    Ok(())
}
