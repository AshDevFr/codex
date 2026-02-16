//! DTOs for plugin file storage endpoints

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Storage statistics for a single plugin
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PluginStorageStatsDto {
    /// Name of the plugin
    #[schema(example = "metadata-anilist")]
    pub plugin_name: String,

    /// Number of files in the plugin's storage directory
    #[schema(example = 5)]
    pub file_count: u64,

    /// Total size of all files in bytes
    #[schema(example = 1048576)]
    pub total_bytes: u64,
}

impl From<crate::services::PluginStorageStats> for PluginStorageStatsDto {
    fn from(stats: crate::services::PluginStorageStats) -> Self {
        Self {
            plugin_name: stats.plugin_name,
            file_count: stats.file_count,
            total_bytes: stats.total_bytes,
        }
    }
}

/// Overall storage statistics for all plugins
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AllPluginStorageStatsDto {
    /// Storage statistics per plugin
    pub plugins: Vec<PluginStorageStatsDto>,

    /// Total file count across all plugins
    #[schema(example = 15)]
    pub total_file_count: u64,

    /// Total bytes across all plugins
    #[schema(example = 5242880)]
    pub total_bytes: u64,
}

/// Result of a plugin storage cleanup operation
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PluginCleanupResultDto {
    /// Number of files deleted
    #[schema(example = 5)]
    pub files_deleted: u64,

    /// Total bytes freed by deletion
    #[schema(example = 1048576)]
    pub bytes_freed: u64,

    /// Number of files that failed to delete
    #[schema(example = 0)]
    pub failures: u64,

    /// Error messages for any failed deletions
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub errors: Vec<String>,
}

impl From<crate::services::PluginCleanupStats> for PluginCleanupResultDto {
    fn from(stats: crate::services::PluginCleanupStats) -> Self {
        Self {
            files_deleted: stats.files_deleted,
            bytes_freed: stats.bytes_freed,
            failures: stats.failures,
            errors: stats.errors,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_storage_stats_serialization() {
        let stats = PluginStorageStatsDto {
            plugin_name: "metadata-anilist".to_string(),
            file_count: 3,
            total_bytes: 1024,
        };

        let json = serde_json::to_string(&stats).unwrap();
        assert!(json.contains("\"pluginName\":\"metadata-anilist\""));
        assert!(json.contains("\"fileCount\":3"));
        assert!(json.contains("\"totalBytes\":1024"));
    }

    #[test]
    fn test_all_plugin_storage_stats_serialization() {
        let stats = AllPluginStorageStatsDto {
            plugins: vec![PluginStorageStatsDto {
                plugin_name: "test".to_string(),
                file_count: 1,
                total_bytes: 100,
            }],
            total_file_count: 1,
            total_bytes: 100,
        };

        let json = serde_json::to_string(&stats).unwrap();
        assert!(json.contains("\"plugins\""));
        assert!(json.contains("\"totalFileCount\":1"));
        assert!(json.contains("\"totalBytes\":100"));
    }

    #[test]
    fn test_plugin_cleanup_result_empty_errors_skipped() {
        let dto = PluginCleanupResultDto {
            files_deleted: 5,
            bytes_freed: 1000,
            failures: 0,
            errors: vec![],
        };

        let json = serde_json::to_string(&dto).unwrap();
        assert!(!json.contains("\"errors\""));
    }
}
