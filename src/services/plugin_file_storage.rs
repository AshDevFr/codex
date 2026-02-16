//! Plugin File Storage Service
//!
//! Provides scoped, isolated file storage for plugins on the filesystem.
//! Each plugin gets its own directory under `{plugins_dir}/{plugin_name}/`,
//! with path traversal protection to prevent plugins from escaping their sandbox.
//!
//! This is separate from the database-backed storage (`storage_handler.rs`) which
//! stores small key-value data. This service is for larger file-based storage
//! (e.g., plugin-specific SQLite databases, cached files).

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use tokio::fs;
use tracing::{debug, warn};

/// Statistics about a plugin's file storage usage
#[derive(Debug, Clone, Default)]
pub struct PluginStorageStats {
    /// Name of the plugin
    pub plugin_name: String,
    /// Number of files in the plugin's directory
    pub file_count: u64,
    /// Total size in bytes of all files
    pub total_bytes: u64,
}

/// Statistics from a plugin storage cleanup operation
#[derive(Debug, Clone, Default)]
pub struct PluginCleanupStats {
    /// Number of files deleted
    pub files_deleted: u64,
    /// Total bytes freed
    pub bytes_freed: u64,
    /// Number of files that failed to delete
    pub failures: u64,
    /// Error messages for failed deletions
    pub errors: Vec<String>,
}

/// Service for managing plugin file storage directories
pub struct PluginFileStorage {
    /// Base plugins directory (e.g., "data/plugins")
    plugins_dir: PathBuf,
}

impl PluginFileStorage {
    /// Create a new plugin file storage service
    pub fn new(plugins_dir: impl Into<PathBuf>) -> Self {
        Self {
            plugins_dir: plugins_dir.into(),
        }
    }

    /// Get the base plugins directory
    #[allow(dead_code)]
    pub fn plugins_dir(&self) -> &Path {
        &self.plugins_dir
    }

    /// Validate a plugin name to prevent path traversal attacks.
    ///
    /// Plugin names must:
    /// - Not be empty
    /// - Not contain `..`
    /// - Not contain path separators (`/` or `\`)
    /// - Not start with `.`
    fn validate_plugin_name(name: &str) -> Result<()> {
        if name.is_empty() {
            bail!("Plugin name cannot be empty");
        }
        if name.contains("..") {
            bail!("Plugin name cannot contain '..'");
        }
        if name.contains('/') || name.contains('\\') {
            bail!("Plugin name cannot contain path separators");
        }
        if name.starts_with('.') {
            bail!("Plugin name cannot start with '.'");
        }
        Ok(())
    }

    /// Get the data directory for a specific plugin, creating it if needed.
    ///
    /// Returns `{plugins_dir}/{plugin_name}/`, creating the directory lazily.
    pub async fn get_plugin_dir(&self, plugin_name: &str) -> Result<PathBuf> {
        Self::validate_plugin_name(plugin_name)?;

        let plugin_dir = self.plugins_dir.join(plugin_name);

        if !plugin_dir.exists() {
            fs::create_dir_all(&plugin_dir)
                .await
                .with_context(|| format!("Failed to create plugin directory: {:?}", plugin_dir))?;
            debug!(plugin = plugin_name, path = ?plugin_dir, "Created plugin data directory");
        }

        Ok(plugin_dir)
    }

    /// Get the data directory for a plugin without creating it.
    ///
    /// Returns the path even if it doesn't exist yet.
    #[allow(dead_code)]
    pub fn get_plugin_dir_path(&self, plugin_name: &str) -> Result<PathBuf> {
        Self::validate_plugin_name(plugin_name)?;
        Ok(self.plugins_dir.join(plugin_name))
    }

    /// Scan all plugin directories and compute storage statistics.
    ///
    /// Returns a list of `PluginStorageStats` for each plugin subdirectory.
    pub async fn scan_storage(&self) -> Result<Vec<PluginStorageStats>> {
        let mut results = Vec::new();

        if !self.plugins_dir.exists() {
            return Ok(results);
        }

        let mut entries = fs::read_dir(&self.plugins_dir)
            .await
            .with_context(|| format!("Failed to read plugins directory: {:?}", self.plugins_dir))?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            let plugin_name = match path.file_name().and_then(|n| n.to_str()) {
                Some(name) => name.to_string(),
                None => continue,
            };

            let (file_count, total_bytes) = self.compute_dir_size(&path).await;

            results.push(PluginStorageStats {
                plugin_name,
                file_count,
                total_bytes,
            });
        }

        results.sort_by(|a, b| a.plugin_name.cmp(&b.plugin_name));
        Ok(results)
    }

    /// Compute storage stats for a single plugin.
    pub async fn get_plugin_storage_stats(&self, plugin_name: &str) -> Result<PluginStorageStats> {
        Self::validate_plugin_name(plugin_name)?;
        let plugin_dir = self.plugins_dir.join(plugin_name);

        if !plugin_dir.exists() {
            return Ok(PluginStorageStats {
                plugin_name: plugin_name.to_string(),
                file_count: 0,
                total_bytes: 0,
            });
        }

        let (file_count, total_bytes) = self.compute_dir_size(&plugin_dir).await;

        Ok(PluginStorageStats {
            plugin_name: plugin_name.to_string(),
            file_count,
            total_bytes,
        })
    }

    /// Delete all files for a specific plugin (full directory wipe).
    pub async fn cleanup_plugin(&self, plugin_name: &str) -> Result<PluginCleanupStats> {
        Self::validate_plugin_name(plugin_name)?;
        let plugin_dir = self.plugins_dir.join(plugin_name);

        if !plugin_dir.exists() {
            return Ok(PluginCleanupStats::default());
        }

        let (file_count, total_bytes) = self.compute_dir_size(&plugin_dir).await;

        match fs::remove_dir_all(&plugin_dir).await {
            Ok(_) => {
                debug!(
                    plugin = plugin_name,
                    files = file_count,
                    bytes = total_bytes,
                    "Cleaned up plugin storage"
                );
                Ok(PluginCleanupStats {
                    files_deleted: file_count,
                    bytes_freed: total_bytes,
                    failures: 0,
                    errors: vec![],
                })
            }
            Err(e) => {
                warn!(
                    plugin = plugin_name,
                    error = %e,
                    "Failed to clean up plugin storage"
                );
                Ok(PluginCleanupStats {
                    files_deleted: 0,
                    bytes_freed: 0,
                    failures: 1,
                    errors: vec![format!(
                        "Failed to remove plugin directory {:?}: {}",
                        plugin_dir, e
                    )],
                })
            }
        }
    }

    /// List all files in a plugin's directory (recursively).
    #[allow(dead_code)]
    pub async fn scan_plugin_files(&self, plugin_name: &str) -> Result<Vec<PathBuf>> {
        Self::validate_plugin_name(plugin_name)?;
        let plugin_dir = self.plugins_dir.join(plugin_name);

        if !plugin_dir.exists() {
            return Ok(vec![]);
        }

        let mut files = Vec::new();
        self.collect_files_recursive(&plugin_dir, &mut files)
            .await?;
        Ok(files)
    }

    /// Recursively compute directory size (file_count, total_bytes).
    async fn compute_dir_size(&self, dir: &Path) -> (u64, u64) {
        let mut file_count = 0u64;
        let mut total_bytes = 0u64;

        if let Ok(mut entries) = fs::read_dir(dir).await {
            while let Ok(Some(entry)) = entries.next_entry().await {
                let path = entry.path();
                if path.is_dir() {
                    let (sub_count, sub_bytes) = Box::pin(self.compute_dir_size(&path)).await;
                    file_count += sub_count;
                    total_bytes += sub_bytes;
                } else if let Ok(meta) = fs::metadata(&path).await {
                    file_count += 1;
                    total_bytes += meta.len();
                }
            }
        }

        (file_count, total_bytes)
    }

    /// Recursively collect all file paths in a directory.
    #[allow(dead_code)]
    async fn collect_files_recursive(&self, dir: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
        let mut entries = fs::read_dir(dir)
            .await
            .with_context(|| format!("Failed to read directory: {:?}", dir))?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.is_dir() {
                Box::pin(self.collect_files_recursive(&path, files)).await?;
            } else {
                files.push(path);
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup() -> (TempDir, PluginFileStorage) {
        let temp_dir = TempDir::new().unwrap();
        let plugins_dir = temp_dir.path().join("plugins");
        let service = PluginFileStorage::new(&plugins_dir);
        (temp_dir, service)
    }

    #[test]
    fn test_validate_plugin_name_valid() {
        assert!(PluginFileStorage::validate_plugin_name("metadata-anilist").is_ok());
        assert!(PluginFileStorage::validate_plugin_name("my_plugin").is_ok());
        assert!(PluginFileStorage::validate_plugin_name("plugin123").is_ok());
    }

    #[test]
    fn test_validate_plugin_name_empty() {
        assert!(PluginFileStorage::validate_plugin_name("").is_err());
    }

    #[test]
    fn test_validate_plugin_name_path_traversal() {
        assert!(PluginFileStorage::validate_plugin_name("..").is_err());
        assert!(PluginFileStorage::validate_plugin_name("../etc").is_err());
        assert!(PluginFileStorage::validate_plugin_name("foo/../bar").is_err());
    }

    #[test]
    fn test_validate_plugin_name_path_separators() {
        assert!(PluginFileStorage::validate_plugin_name("foo/bar").is_err());
        assert!(PluginFileStorage::validate_plugin_name("foo\\bar").is_err());
    }

    #[test]
    fn test_validate_plugin_name_hidden() {
        assert!(PluginFileStorage::validate_plugin_name(".hidden").is_err());
    }

    #[test]
    fn test_get_plugin_dir_path() {
        let (_temp, service) = setup();
        let path = service.get_plugin_dir_path("metadata-anilist").unwrap();
        assert!(path.ends_with("plugins/metadata-anilist"));
    }

    #[tokio::test]
    async fn test_get_plugin_dir_creates_directory() {
        let (_temp, service) = setup();
        let dir = service.get_plugin_dir("my-plugin").await.unwrap();
        assert!(dir.exists());
        assert!(dir.is_dir());
        assert!(dir.ends_with("plugins/my-plugin"));
    }

    #[tokio::test]
    async fn test_get_plugin_dir_idempotent() {
        let (_temp, service) = setup();
        let dir1 = service.get_plugin_dir("test-plugin").await.unwrap();
        let dir2 = service.get_plugin_dir("test-plugin").await.unwrap();
        assert_eq!(dir1, dir2);
        assert!(dir1.exists());
    }

    #[tokio::test]
    async fn test_scan_storage_empty() {
        let (_temp, service) = setup();
        let stats = service.scan_storage().await.unwrap();
        assert!(stats.is_empty());
    }

    #[tokio::test]
    async fn test_scan_storage_with_plugins() {
        let (_temp, service) = setup();

        // Create plugin directories with files
        let dir_a = service.get_plugin_dir("plugin-a").await.unwrap();
        let dir_b = service.get_plugin_dir("plugin-b").await.unwrap();

        fs::write(dir_a.join("data.json"), b"hello world")
            .await
            .unwrap();
        fs::write(dir_b.join("cache.db"), b"some database content here")
            .await
            .unwrap();
        fs::write(dir_b.join("config.json"), b"{}").await.unwrap();

        let stats = service.scan_storage().await.unwrap();
        assert_eq!(stats.len(), 2);

        assert_eq!(stats[0].plugin_name, "plugin-a");
        assert_eq!(stats[0].file_count, 1);
        assert_eq!(stats[0].total_bytes, 11); // "hello world"

        assert_eq!(stats[1].plugin_name, "plugin-b");
        assert_eq!(stats[1].file_count, 2);
    }

    #[tokio::test]
    async fn test_get_plugin_storage_stats() {
        let (_temp, service) = setup();

        let dir = service.get_plugin_dir("metadata-anilist").await.unwrap();
        fs::write(dir.join("cache.json"), b"cached data")
            .await
            .unwrap();

        let stats = service
            .get_plugin_storage_stats("metadata-anilist")
            .await
            .unwrap();
        assert_eq!(stats.plugin_name, "metadata-anilist");
        assert_eq!(stats.file_count, 1);
        assert_eq!(stats.total_bytes, 11);
    }

    #[tokio::test]
    async fn test_get_plugin_storage_stats_nonexistent() {
        let (_temp, service) = setup();

        let stats = service
            .get_plugin_storage_stats("nonexistent")
            .await
            .unwrap();
        assert_eq!(stats.file_count, 0);
        assert_eq!(stats.total_bytes, 0);
    }

    #[tokio::test]
    async fn test_cleanup_plugin() {
        let (_temp, service) = setup();

        let dir = service.get_plugin_dir("to-clean").await.unwrap();
        fs::write(dir.join("file1.txt"), b"content1").await.unwrap();
        fs::write(dir.join("file2.txt"), b"content2").await.unwrap();

        let stats = service.cleanup_plugin("to-clean").await.unwrap();
        assert_eq!(stats.files_deleted, 2);
        assert!(stats.bytes_freed > 0);
        assert_eq!(stats.failures, 0);

        // Directory should be gone
        assert!(!dir.exists());
    }

    #[tokio::test]
    async fn test_cleanup_plugin_nonexistent() {
        let (_temp, service) = setup();

        let stats = service.cleanup_plugin("nonexistent").await.unwrap();
        assert_eq!(stats.files_deleted, 0);
        assert_eq!(stats.bytes_freed, 0);
    }

    #[tokio::test]
    async fn test_scan_plugin_files() {
        let (_temp, service) = setup();

        let dir = service.get_plugin_dir("scan-me").await.unwrap();
        let subdir = dir.join("subdir");
        fs::create_dir_all(&subdir).await.unwrap();
        fs::write(dir.join("root.txt"), b"root").await.unwrap();
        fs::write(subdir.join("nested.txt"), b"nested")
            .await
            .unwrap();

        let files = service.scan_plugin_files("scan-me").await.unwrap();
        assert_eq!(files.len(), 2);
    }

    #[tokio::test]
    async fn test_scan_plugin_files_nonexistent() {
        let (_temp, service) = setup();

        let files = service.scan_plugin_files("nonexistent").await.unwrap();
        assert!(files.is_empty());
    }

    #[tokio::test]
    async fn test_path_traversal_rejected() {
        let (_temp, service) = setup();
        assert!(service.get_plugin_dir("../etc").await.is_err());
        assert!(service.cleanup_plugin("..").await.is_err());
        assert!(service.scan_plugin_files("foo/../../etc").await.is_err());
    }
}
