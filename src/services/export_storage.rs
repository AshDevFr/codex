//! Export file storage service
//!
//! Manages on-disk files for series exports. Provides path computation,
//! atomic writes (tmp + rename), and deletion. The root directory is
//! read from DB settings (`exports.dir`) at construction time.
//!
//! Directory layout:
//! ```text
//! {root}/{user_id}/{export_id}.{json|csv}
//! ```

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use tokio::fs;
use uuid::Uuid;

/// Default exports directory (relative to working dir / data_dir).
pub const DEFAULT_EXPORTS_DIR: &str = "data/exports";

pub struct ExportStorage {
    root: PathBuf,
}

impl ExportStorage {
    /// Create an ExportStorage rooted at the given directory.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// Root directory for all exports.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Directory for a specific user's exports.
    pub fn user_dir(&self, user_id: Uuid) -> PathBuf {
        self.root.join(user_id.to_string())
    }

    /// Full path for an export file.
    pub fn path_for(&self, user_id: Uuid, export_id: Uuid, format: &str) -> PathBuf {
        let ext = match format {
            "csv" => "csv",
            _ => "json",
        };
        self.user_dir(user_id)
            .join(format!("{}.{}", export_id, ext))
    }

    /// Path for the temporary file used during atomic writes.
    fn tmp_path_for(&self, user_id: Uuid, export_id: Uuid, format: &str) -> PathBuf {
        let ext = match format {
            "csv" => "csv",
            _ => "json",
        };
        self.user_dir(user_id)
            .join(format!("{}.{}.tmp", export_id, ext))
    }

    /// Ensure the user's export directory exists.
    pub async fn ensure_user_dir(&self, user_id: Uuid) -> Result<()> {
        let dir = self.user_dir(user_id);
        fs::create_dir_all(&dir)
            .await
            .with_context(|| format!("Failed to create export directory: {}", dir.display()))?;
        Ok(())
    }

    /// Write a file atomically using a caller-provided async writer function.
    ///
    /// 1. Writes to `{export_id}.{ext}.tmp`
    /// 2. On success, renames to the final path
    /// 3. On failure, removes the tmp file
    ///
    /// Returns `(final_path, file_size_bytes)`.
    pub async fn write_atomic<F, Fut>(
        &self,
        user_id: Uuid,
        export_id: Uuid,
        format: &str,
        writer_fn: F,
    ) -> Result<(PathBuf, u64)>
    where
        F: FnOnce(PathBuf) -> Fut,
        Fut: std::future::Future<Output = Result<()>>,
    {
        self.ensure_user_dir(user_id).await?;

        let tmp_path = self.tmp_path_for(user_id, export_id, format);
        let final_path = self.path_for(user_id, export_id, format);

        // Run the writer; on failure, clean up the tmp file
        match writer_fn(tmp_path.clone()).await {
            Ok(()) => {}
            Err(e) => {
                // Best-effort cleanup
                let _ = fs::remove_file(&tmp_path).await;
                return Err(e).context("Export writer failed");
            }
        }

        // Rename tmp → final
        fs::rename(&tmp_path, &final_path).await.with_context(|| {
            format!(
                "Failed to rename {} → {}",
                tmp_path.display(),
                final_path.display()
            )
        })?;

        // Measure file size
        let metadata = fs::metadata(&final_path)
            .await
            .with_context(|| format!("Failed to read metadata for {}", final_path.display()))?;

        Ok((final_path, metadata.len()))
    }

    /// Delete an export file. Returns Ok(true) if the file existed.
    pub async fn delete(&self, user_id: Uuid, export_id: Uuid, format: &str) -> Result<bool> {
        let path = self.path_for(user_id, export_id, format);
        match fs::remove_file(&path).await {
            Ok(()) => Ok(true),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(e) => Err(e).with_context(|| format!("Failed to delete {}", path.display())),
        }
    }

    /// Get the size of an export file. Returns None if the file doesn't exist.
    pub async fn size(&self, user_id: Uuid, export_id: Uuid, format: &str) -> Result<Option<u64>> {
        let path = self.path_for(user_id, export_id, format);
        match fs::metadata(&path).await {
            Ok(m) => Ok(Some(m.len())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e).with_context(|| format!("Failed to stat {}", path.display())),
        }
    }

    /// Check if an export file exists on disk.
    pub async fn exists(&self, user_id: Uuid, export_id: Uuid, format: &str) -> bool {
        let path = self.path_for(user_id, export_id, format);
        fs::metadata(&path).await.is_ok()
    }

    /// List stale `.tmp` files across all user directories.
    /// Returns paths of tmp files older than `max_age`.
    pub async fn list_stale_tmp_files(&self, max_age: std::time::Duration) -> Result<Vec<PathBuf>> {
        let mut stale = Vec::new();
        let now = std::time::SystemTime::now();

        let mut root_entries = match fs::read_dir(&self.root).await {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(stale),
            Err(e) => return Err(e).context("Failed to read exports root directory"),
        };

        while let Some(user_entry) = root_entries.next_entry().await? {
            let user_path = user_entry.path();
            if !user_path.is_dir() {
                continue;
            }

            let mut files = fs::read_dir(&user_path).await?;
            while let Some(file_entry) = files.next_entry().await? {
                let path = file_entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("tmp")
                    && let Ok(meta) = fs::metadata(&path).await
                    && let Ok(modified) = meta.modified()
                    && let Ok(age) = now.duration_since(modified)
                    && age > max_age
                {
                    stale.push(path);
                }
            }
        }

        Ok(stale)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup() -> (ExportStorage, TempDir) {
        let tmp = TempDir::new().unwrap();
        let storage = ExportStorage::new(tmp.path().to_path_buf());
        (storage, tmp)
    }

    #[test]
    fn test_path_computation() {
        let (storage, _tmp) = setup();
        let user_id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let export_id = Uuid::parse_str("660e8400-e29b-41d4-a716-446655440000").unwrap();

        let json_path = storage.path_for(user_id, export_id, "json");
        assert!(json_path.to_str().unwrap().ends_with(
            "550e8400-e29b-41d4-a716-446655440000/660e8400-e29b-41d4-a716-446655440000.json"
        ));

        let csv_path = storage.path_for(user_id, export_id, "csv");
        assert!(csv_path.to_str().unwrap().ends_with(
            "550e8400-e29b-41d4-a716-446655440000/660e8400-e29b-41d4-a716-446655440000.csv"
        ));
    }

    #[test]
    fn test_user_dir() {
        let (storage, _tmp) = setup();
        let user_id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();

        let dir = storage.user_dir(user_id);
        assert!(
            dir.to_str()
                .unwrap()
                .ends_with("550e8400-e29b-41d4-a716-446655440000")
        );
    }

    #[tokio::test]
    async fn test_ensure_user_dir() {
        let (storage, _tmp) = setup();
        let user_id = Uuid::new_v4();

        let dir = storage.user_dir(user_id);
        assert!(!dir.exists());

        storage.ensure_user_dir(user_id).await.unwrap();
        assert!(dir.exists());
        assert!(dir.is_dir());

        // Idempotent
        storage.ensure_user_dir(user_id).await.unwrap();
    }

    #[tokio::test]
    async fn test_write_atomic_success() {
        let (storage, _tmp) = setup();
        let user_id = Uuid::new_v4();
        let export_id = Uuid::new_v4();

        let (final_path, size) = storage
            .write_atomic(user_id, export_id, "json", |tmp_path| async move {
                fs::write(&tmp_path, b"[{\"hello\":\"world\"}]").await?;
                Ok(())
            })
            .await
            .unwrap();

        assert!(final_path.exists());
        assert_eq!(size, 19);
        assert!(final_path.to_str().unwrap().ends_with(".json"));

        // tmp file should not exist
        let tmp = storage.tmp_path_for(user_id, export_id, "json");
        assert!(!tmp.exists());
    }

    #[tokio::test]
    async fn test_write_atomic_failure_cleans_up() {
        let (storage, _tmp) = setup();
        let user_id = Uuid::new_v4();
        let export_id = Uuid::new_v4();

        let result = storage
            .write_atomic(user_id, export_id, "csv", |tmp_path| async move {
                // Write something then fail
                fs::write(&tmp_path, b"partial data").await?;
                anyhow::bail!("simulated write failure");
            })
            .await;

        assert!(result.is_err());

        // Neither tmp nor final should exist
        let tmp = storage.tmp_path_for(user_id, export_id, "csv");
        let final_path = storage.path_for(user_id, export_id, "csv");
        assert!(!tmp.exists());
        assert!(!final_path.exists());
    }

    #[tokio::test]
    async fn test_delete() {
        let (storage, _tmp) = setup();
        let user_id = Uuid::new_v4();
        let export_id = Uuid::new_v4();

        // Write a file first
        storage
            .write_atomic(user_id, export_id, "json", |tmp_path| async move {
                fs::write(&tmp_path, b"[]").await?;
                Ok(())
            })
            .await
            .unwrap();

        let deleted = storage.delete(user_id, export_id, "json").await.unwrap();
        assert!(deleted);

        // Deleting non-existent returns false
        let deleted_again = storage.delete(user_id, export_id, "json").await.unwrap();
        assert!(!deleted_again);
    }

    #[tokio::test]
    async fn test_size_and_exists() {
        let (storage, _tmp) = setup();
        let user_id = Uuid::new_v4();
        let export_id = Uuid::new_v4();

        // Before writing
        assert!(!storage.exists(user_id, export_id, "json").await);
        assert_eq!(
            storage.size(user_id, export_id, "json").await.unwrap(),
            None
        );

        // After writing
        storage
            .write_atomic(user_id, export_id, "json", |tmp_path| async move {
                fs::write(&tmp_path, b"test content").await?;
                Ok(())
            })
            .await
            .unwrap();

        assert!(storage.exists(user_id, export_id, "json").await);
        assert_eq!(
            storage.size(user_id, export_id, "json").await.unwrap(),
            Some(12)
        );
    }

    #[tokio::test]
    async fn test_list_stale_tmp_files() {
        let (storage, _tmp) = setup();
        let user_id = Uuid::new_v4();

        storage.ensure_user_dir(user_id).await.unwrap();

        // Create a .tmp file
        let tmp_path = storage.user_dir(user_id).join("old_export.json.tmp");
        fs::write(&tmp_path, b"stale").await.unwrap();

        // Create a non-tmp file (should be ignored)
        let json_path = storage.user_dir(user_id).join("good_export.json");
        fs::write(&json_path, b"good").await.unwrap();

        // With a 0-second max age, the tmp file is stale immediately
        let stale = storage
            .list_stale_tmp_files(std::time::Duration::from_secs(0))
            .await
            .unwrap();
        assert_eq!(stale.len(), 1);
        assert_eq!(stale[0], tmp_path);

        // With a very long max age, nothing is stale
        let stale = storage
            .list_stale_tmp_files(std::time::Duration::from_secs(999_999))
            .await
            .unwrap();
        assert!(stale.is_empty());
    }

    #[tokio::test]
    async fn test_list_stale_tmp_no_root_dir() {
        // Root doesn't exist yet - should return empty, not error
        let storage = ExportStorage::new(PathBuf::from("/tmp/nonexistent_codex_test_dir"));
        let stale = storage
            .list_stale_tmp_files(std::time::Duration::from_secs(0))
            .await
            .unwrap();
        assert!(stale.is_empty());
    }
}
