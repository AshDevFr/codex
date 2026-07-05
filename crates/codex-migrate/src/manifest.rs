//! Archive manifest: the `manifest.json` at the root of an export bundle.
//!
//! Records enough to validate an import (format + schema version), to verify
//! it (per-table row counts), and to re-root on-disk artifact paths (which
//! artifact groups are present and the source base directory each came from).

use anyhow::{Context, Result};
use migration::{Migrator, MigratorTrait};
use sea_orm::{ConnectionTrait, DatabaseBackend};
use serde::{Deserialize, Serialize};

/// Bumped when the on-disk archive layout or manifest shape changes
/// incompatibly. Import refuses a mismatched major format.
pub const ARCHIVE_FORMAT_VERSION: u32 = 1;

/// A bundleable tree of on-disk files that the database only references by
/// path. Each maps to a top-level directory inside the archive.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactGroup {
    /// Generated book/series thumbnails (`files.thumbnail_dir`).
    Thumbnails,
    /// Uploaded / extracted / plugin covers (`files.uploads_dir`).
    Uploads,
    /// Plugin data and credentials on disk (`files.plugins_dir`).
    Plugins,
    /// Rendered PDF page cache (`pdf.cache_dir`) — reproducible, opt-in.
    Cache,
}

impl ArtifactGroup {
    /// The archive-relative top-level directory for this group.
    pub fn archive_dir(self) -> &'static str {
        match self {
            ArtifactGroup::Thumbnails => "thumbnails",
            ArtifactGroup::Uploads => "uploads",
            ArtifactGroup::Plugins => "plugins",
            ArtifactGroup::Cache => "cache",
        }
    }
}

/// Which artifact group is present in the archive and the source instance's
/// base directory it was captured from (needed to re-root DB paths on import).
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ArtifactEntry {
    pub group: ArtifactGroup,
    /// The source instance's configured base dir (e.g. `data/thumbnails`).
    pub source_base_dir: String,
}

/// Row count for one table at export time.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct TableCount {
    pub table: String,
    pub rows: u64,
}

/// The archive's `manifest.json`.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Manifest {
    pub format_version: u32,
    /// `"sqlite"` or `"postgres"` — the backend the data was exported from.
    pub source_backend: String,
    /// Name of the last applied migration at export time, or `None` if the
    /// source had no migration bookkeeping.
    pub schema_version: Option<String>,
    pub tables: Vec<TableCount>,
    pub total_rows: u64,
    pub artifacts: Vec<ArtifactEntry>,
    /// RFC 3339 timestamp of the export.
    pub created_at: String,
}

/// Render a backend as its manifest string.
pub fn backend_name(backend: DatabaseBackend) -> &'static str {
    match backend {
        DatabaseBackend::Sqlite => "sqlite",
        DatabaseBackend::Postgres => "postgres",
        DatabaseBackend::MySql => "mysql",
    }
}

/// The current schema version of `conn`: the lexicographically-last applied
/// migration name (migration names are timestamp-prefixed, so this is the most
/// recent). Returns `None` if no migrations are applied.
pub async fn schema_version<C: ConnectionTrait>(conn: &C) -> Result<Option<String>> {
    let applied = Migrator::get_applied_migrations(conn)
        .await
        .context("failed to read applied migrations")?;
    Ok(applied.into_iter().map(|m| m.name().to_string()).max())
}
