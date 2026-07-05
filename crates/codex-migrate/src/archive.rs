//! Portable export archive: a gzip-compressed tar bundling the database
//! (as NDJSON) plus the on-disk artifacts the database only references by path.
//!
//! Layout:
//! ```text
//! manifest.json
//! db/<table>.ndjson
//! thumbnails/   (files.thumbnail_dir, when bundled)
//! uploads/      (files.uploads_dir,   when bundled)
//! plugins/      (files.plugins_dir,   when bundled)
//! cache/        (pdf.cache_dir,       only with --include-cache)
//! ```

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use sea_orm::{ConnectionTrait, DatabaseConnection};

use crate::manifest::{
    ARCHIVE_FORMAT_VERSION, ArtifactEntry, ArtifactGroup, Manifest, TableCount, backend_name,
    schema_version,
};
use crate::progress::Progress;
use crate::reroot::{self, RerootStats};
use crate::{TransferReport, registry};

/// An artifact tree to bundle on export: which group, and the source
/// instance's directory that currently holds those files.
#[derive(Clone, Debug)]
pub struct ArtifactSource {
    pub group: ArtifactGroup,
    pub source_dir: PathBuf,
}

/// Where an artifact group's files should be written on import, for the target
/// instance.
#[derive(Clone, Debug)]
pub struct ArtifactTarget {
    pub group: ArtifactGroup,
    pub target_dir: PathBuf,
}

/// Outcome of [`import_archive`].
#[derive(Debug)]
pub struct ImportOutcome {
    pub manifest: Manifest,
    pub report: TransferReport,
    pub reroot: RerootStats,
    /// Per-table canonical-content mismatches, when full verification was
    /// requested; empty otherwise.
    pub full_verify: Vec<crate::full_verify::FullMismatch>,
}

/// Export `conn` and the given artifact trees into a `.tar.gz` at `out_path`.
/// Returns the manifest that was written. Artifact sources whose directory does
/// not exist are silently skipped (and omitted from the manifest).
pub async fn export_archive(
    conn: &DatabaseConnection,
    out_path: &Path,
    artifacts: &[ArtifactSource],
    progress: Progress,
) -> Result<Manifest> {
    let staging = tempfile::tempdir().context("failed to create export staging dir")?;
    let db_dir = staging.path().join("db");
    std::fs::create_dir_all(&db_dir)?;

    let counts = registry::dump_all_to_dir(conn, &db_dir, progress)
        .await
        .context("failed to dump database tables")?;
    let total_rows = counts.iter().map(|c| c.rows).sum();

    let present: Vec<&ArtifactSource> =
        artifacts.iter().filter(|a| a.source_dir.exists()).collect();

    let manifest = Manifest {
        format_version: ARCHIVE_FORMAT_VERSION,
        source_backend: backend_name(conn.get_database_backend()).to_string(),
        schema_version: schema_version(conn).await?,
        tables: counts
            .into_iter()
            .map(|c| TableCount {
                table: c.table,
                rows: c.rows,
            })
            .collect(),
        total_rows,
        artifacts: present
            .iter()
            .map(|a| ArtifactEntry {
                group: a.group,
                source_base_dir: a.source_dir.to_string_lossy().into_owned(),
            })
            .collect(),
        created_at: chrono::Utc::now().to_rfc3339(),
    };

    std::fs::write(
        staging.path().join("manifest.json"),
        serde_json::to_vec_pretty(&manifest)?,
    )
    .context("failed to write manifest.json")?;

    write_archive(out_path, staging.path(), &present)
        .with_context(|| format!("failed to write archive {}", out_path.display()))?;

    Ok(manifest)
}

/// Import a `.tar.gz` produced by [`export_archive`] into `conn`, unpacking
/// bundled artifacts to the given targets and re-rooting DB paths accordingly.
///
/// This performs the load and re-root only. The schema-version and
/// fresh-target safety checks belong to the CLI layer, which should validate
/// the returned/[`extract`]ed manifest before committing to an import.
pub async fn import_archive(
    conn: &DatabaseConnection,
    in_path: &Path,
    targets: &[ArtifactTarget],
    progress: Progress,
    full_verify: bool,
) -> Result<ImportOutcome> {
    let staging = tempfile::tempdir().context("failed to create import staging dir")?;
    let manifest = extract(in_path, staging.path())?;
    let db_dir = staging.path().join("db");

    let report = crate::load_from_dir(conn, &db_dir, progress)
        .await
        .context("failed to load database from archive")?;

    // Unpack each bundled artifact group to its target dir and record the
    // base-dir remapping for path re-rooting.
    let mut thumbnail_remap: Option<(String, String)> = None;
    let mut uploads_remap: Option<(String, String)> = None;

    for entry in &manifest.artifacts {
        let Some(target) = targets.iter().find(|t| t.group == entry.group) else {
            continue;
        };
        let src = staging.path().join(entry.group.archive_dir());
        if src.exists() {
            copy_dir_all(&src, &target.target_dir).with_context(|| {
                format!(
                    "failed to unpack {} into {}",
                    entry.group.archive_dir(),
                    target.target_dir.display()
                )
            })?;
        }
        let to = target.target_dir.to_string_lossy().into_owned();
        match entry.group {
            ArtifactGroup::Thumbnails => {
                thumbnail_remap = Some((entry.source_base_dir.clone(), to))
            }
            ArtifactGroup::Uploads => uploads_remap = Some((entry.source_base_dir.clone(), to)),
            ArtifactGroup::Plugins | ArtifactGroup::Cache => {}
        }
    }

    let reroot = reroot::reroot_all(
        conn,
        thumbnail_remap
            .as_ref()
            .map(|(f, t)| (f.as_str(), t.as_str())),
        uploads_remap
            .as_ref()
            .map(|(f, t)| (f.as_str(), t.as_str())),
    )
    .await
    .context("failed to re-root artifact paths")?;

    // Optional deep check: compare the canonical content of every row in the
    // archive against the loaded target.
    let full_verify_mismatches = if full_verify {
        let source = registry::digest_all_from_ndjson_dir(&db_dir)
            .await
            .context("failed to digest archive contents")?;
        let target = registry::digest_all_from_conn(conn)
            .await
            .context("failed to digest imported data")?;
        crate::full_verify::compare_digests(&source, &target)
    } else {
        Vec::new()
    };

    Ok(ImportOutcome {
        manifest,
        report,
        reroot,
        full_verify: full_verify_mismatches,
    })
}

/// Extract an archive into `dest` and return its (validated-format) manifest.
/// Exposed so the CLI can inspect the manifest before importing.
pub fn extract(in_path: &Path, dest: &Path) -> Result<Manifest> {
    let file = std::fs::File::open(in_path)
        .with_context(|| format!("failed to open archive {}", in_path.display()))?;
    let decoder = flate2::read::GzDecoder::new(file);
    let mut archive = tar::Archive::new(decoder);
    archive
        .unpack(dest)
        .context("failed to extract archive (corrupt or not a codex export?)")?;

    let manifest_file = std::fs::File::open(dest.join("manifest.json"))
        .context("archive is missing manifest.json")?;
    let manifest: Manifest = serde_json::from_reader(std::io::BufReader::new(manifest_file))
        .context("failed to parse manifest.json")?;

    check_format_version(&manifest)?;
    Ok(manifest)
}

/// Read only the `manifest.json` from an archive without unpacking it, for
/// pre-flight checks (schema version, artifact list) before a heavy import.
/// `manifest.json` is written first, so this stops after the first entry.
pub fn read_manifest(in_path: &Path) -> Result<Manifest> {
    let file = std::fs::File::open(in_path)
        .with_context(|| format!("failed to open archive {}", in_path.display()))?;
    let decoder = flate2::read::GzDecoder::new(file);
    let mut archive = tar::Archive::new(decoder);

    for entry in archive.entries().context("archive is not a valid tar")? {
        let mut entry = entry?;
        if entry.path()?.as_ref() == Path::new("manifest.json") {
            let manifest: Manifest =
                serde_json::from_reader(&mut entry).context("failed to parse manifest.json")?;
            check_format_version(&manifest)?;
            return Ok(manifest);
        }
    }
    anyhow::bail!("archive is missing manifest.json");
}

fn check_format_version(manifest: &Manifest) -> Result<()> {
    if manifest.format_version != ARCHIVE_FORMAT_VERSION {
        anyhow::bail!(
            "unsupported archive format version {} (this build reads version {})",
            manifest.format_version,
            ARCHIVE_FORMAT_VERSION
        );
    }
    Ok(())
}

/// Pack the staged manifest + `db/` tree and the artifact source dirs into a
/// gzip tar. Artifacts are streamed directly from their source locations (no
/// intermediate copy).
fn write_archive(out: &Path, staging: &Path, artifacts: &[&ArtifactSource]) -> Result<()> {
    if let Some(parent) = out.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent).ok();
    }
    let file = std::fs::File::create(out)?;
    let encoder = flate2::write::GzEncoder::new(file, flate2::Compression::default());
    let mut builder = tar::Builder::new(encoder);

    builder.append_path_with_name(staging.join("manifest.json"), "manifest.json")?;
    builder.append_dir_all("db", staging.join("db"))?;
    for a in artifacts {
        builder.append_dir_all(a.group.archive_dir(), &a.source_dir)?;
    }

    builder.into_inner()?.finish()?;
    Ok(())
}

/// Recursively copy `src` into `dst`, creating `dst` and intermediate dirs.
fn copy_dir_all(src: &Path, dst: &Path) -> Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let target = dst.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir_all(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}
