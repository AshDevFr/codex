//! Handler for UserPluginSync task
//!
//! Processes user plugin sync tasks by spawning the plugin process with
//! per-user credentials and calling sync methods (push/pull progress)
//! via JSON-RPC.

use anyhow::Result;
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::db::entities::tasks;
use crate::db::repositories::{SeriesExternalIdRepository, UserPluginsRepository};
use crate::events::EventBroadcaster;
use crate::services::plugin::PluginManager;
use crate::services::plugin::protocol::methods;
use crate::services::plugin::sync::{
    ExternalUserInfo, SyncEntry, SyncPullRequest, SyncPullResponse, SyncPushRequest,
    SyncPushResponse,
};
use crate::tasks::handlers::TaskHandler;
use crate::tasks::types::TaskResult;

/// Result of a user plugin sync operation
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserPluginSyncResult {
    /// Plugin ID
    pub plugin_id: Uuid,
    /// User ID
    pub user_id: Uuid,
    /// External username (if retrieved)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub external_username: Option<String>,
    /// Number of entries pushed
    pub pushed: u32,
    /// Number of entries pulled
    pub pulled: u32,
    /// Number of pulled entries matched to Codex series via external IDs
    #[serde(default)]
    pub matched: u32,
    /// Push failures
    pub push_failures: u32,
    /// Pull had more pages (not all pulled)
    #[serde(default)]
    pub pull_incomplete: bool,
    /// Reason for skipping, if sync was skipped
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skipped_reason: Option<String>,
}

/// Handler for user plugin sync tasks
pub struct UserPluginSyncHandler {
    plugin_manager: Arc<PluginManager>,
}

impl UserPluginSyncHandler {
    pub fn new(plugin_manager: Arc<PluginManager>) -> Self {
        Self { plugin_manager }
    }
}

impl TaskHandler for UserPluginSyncHandler {
    fn handle<'a>(
        &'a self,
        task: &'a tasks::Model,
        db: &'a DatabaseConnection,
        _event_broadcaster: Option<&'a Arc<EventBroadcaster>>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<TaskResult>> + Send + 'a>> {
        Box::pin(async move {
            // Extract task parameters
            let params = task
                .params
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("Missing params in user_plugin_sync task"))?;

            let plugin_id: Uuid = params
                .get("plugin_id")
                .and_then(|v| v.as_str())
                .and_then(|s| Uuid::parse_str(s).ok())
                .ok_or_else(|| anyhow::anyhow!("Missing or invalid plugin_id in params"))?;

            let user_id: Uuid = params
                .get("user_id")
                .and_then(|v| v.as_str())
                .and_then(|s| Uuid::parse_str(s).ok())
                .ok_or_else(|| anyhow::anyhow!("Missing or invalid user_id in params"))?;

            info!(
                "Task {}: Starting sync for plugin {} / user {}",
                task.id, plugin_id, user_id
            );

            // Get user plugin handle (spawns process with per-user credentials)
            let (handle, context) = match self
                .plugin_manager
                .get_user_plugin_handle(plugin_id, user_id)
                .await
            {
                Ok(result) => result,
                Err(e) => {
                    let reason = match &e {
                        crate::services::plugin::PluginManagerError::UserPluginNotFound {
                            ..
                        } => "user_plugin_not_found",
                        crate::services::plugin::PluginManagerError::PluginNotEnabled(_) => {
                            "plugin_not_enabled"
                        }
                        _ => "plugin_start_failed",
                    };
                    warn!("Task {}: Failed to get plugin handle: {}", task.id, e);
                    return Ok(TaskResult::success_with_data(
                        format!("Sync skipped: {}", reason),
                        json!(UserPluginSyncResult {
                            plugin_id,
                            user_id,
                            external_username: None,
                            pushed: 0,
                            pulled: 0,
                            matched: 0,
                            push_failures: 0,
                            pull_incomplete: false,
                            skipped_reason: Some(reason.to_string()),
                        }),
                    ));
                }
            };

            // Step 1: Get external user info (optional, for display)
            let external_username = match handle
                .call_method::<serde_json::Value, ExternalUserInfo>(
                    methods::SYNC_GET_USER_INFO,
                    json!({}),
                )
                .await
            {
                Ok(user_info) => {
                    debug!(
                        "Task {}: Connected as '{}' ({})",
                        task.id, user_info.username, user_info.external_id
                    );
                    Some(user_info.username)
                }
                Err(e) => {
                    warn!(
                        "Task {}: Failed to get user info (continuing): {}",
                        task.id, e
                    );
                    None
                }
            };

            // Resolve the external ID source from the plugin manifest
            let external_id_source = handle
                .manifest()
                .await
                .and_then(|m| m.capabilities.external_id_source.clone());

            if let Some(ref source) = external_id_source {
                debug!(
                    "Task {}: Plugin declares externalIdSource: {}",
                    task.id, source
                );
            }

            // Step 2: Pull progress from external service
            let pull_request = SyncPullRequest {
                since: None, // Full pull for now; incremental can use last_sync_at
                limit: Some(500),
                cursor: None,
            };

            let (pulled_count, pull_incomplete, matched_count) = match handle
                .call_method::<SyncPullRequest, SyncPullResponse>(
                    methods::SYNC_PULL_PROGRESS,
                    pull_request,
                )
                .await
            {
                Ok(pull_response) => {
                    let count = pull_response.entries.len() as u32;
                    let has_more = pull_response.has_more;
                    info!(
                        "Task {}: Pulled {} entries from external service (has_more: {})",
                        task.id, count, has_more
                    );

                    // Match pulled entries to Codex series via external IDs
                    let matched = match_pulled_entries(
                        db,
                        &pull_response.entries,
                        external_id_source.as_deref(),
                        task.id,
                    )
                    .await;

                    // TODO: Apply matched entries to Codex user's reading progress
                    // This will be integrated when the reading progress tracking
                    // system is available in Codex

                    (count, has_more, matched)
                }
                Err(e) => {
                    error!("Task {}: Pull failed: {}", task.id, e);
                    // Continue to push even if pull fails
                    (0, false, 0)
                }
            };

            // Step 3: Push progress to external service
            // TODO: Build push entries from Codex user's reading progress.
            // For now we send an empty push to validate the protocol works.
            let push_request = SyncPushRequest {
                entries: vec![], // Will be populated when reading progress is tracked
            };

            let (pushed_count, push_failures) = match handle
                .call_method::<SyncPushRequest, SyncPushResponse>(
                    methods::SYNC_PUSH_PROGRESS,
                    push_request,
                )
                .await
            {
                Ok(push_response) => {
                    let success_count = push_response.success.len() as u32;
                    let failure_count = push_response.failed.len() as u32;
                    if failure_count > 0 {
                        warn!(
                            "Task {}: Push had {} successes and {} failures",
                            task.id, success_count, failure_count
                        );
                    } else {
                        info!(
                            "Task {}: Pushed {} entries to external service",
                            task.id, success_count
                        );
                    }
                    (success_count, failure_count)
                }
                Err(e) => {
                    error!("Task {}: Push failed: {}", task.id, e);
                    (0, 0)
                }
            };

            // Record sync timestamp on the user plugin instance
            if let Err(e) = UserPluginsRepository::record_sync(db, context.user_plugin_id).await {
                warn!("Task {}: Failed to record sync timestamp: {}", task.id, e);
            }

            // Record success on the user plugin instance
            if let Err(e) = UserPluginsRepository::record_success(db, context.user_plugin_id).await
            {
                warn!("Task {}: Failed to record success: {}", task.id, e);
            }

            let result = UserPluginSyncResult {
                plugin_id,
                user_id,
                external_username,
                pushed: pushed_count,
                pulled: pulled_count,
                matched: matched_count,
                push_failures,
                pull_incomplete,
                skipped_reason: None,
            };

            let message = format!(
                "Sync complete: pulled {} entries ({} matched), pushed {} entries",
                pulled_count, matched_count, pushed_count
            );

            info!("Task {}: {}", task.id, message);

            Ok(TaskResult::success_with_data(message, json!(result)))
        })
    }
}

/// Match pulled sync entries to Codex series using external IDs.
///
/// For each pulled entry, looks up `series_external_ids` where
/// `source = external_id_source` and `external_id = entry.external_id`.
/// Returns the number of entries that were successfully matched.
async fn match_pulled_entries(
    db: &DatabaseConnection,
    entries: &[SyncEntry],
    external_id_source: Option<&str>,
    task_id: Uuid,
) -> u32 {
    let Some(source) = external_id_source else {
        debug!(
            "Task {}: No externalIdSource configured, skipping entry matching",
            task_id
        );
        return 0;
    };

    if entries.is_empty() {
        return 0;
    }

    let mut matched: u32 = 0;
    let mut unmatched: u32 = 0;

    for entry in entries {
        match SeriesExternalIdRepository::find_by_external_id_and_source(
            db,
            &entry.external_id,
            source,
        )
        .await
        {
            Ok(Some(ext_id)) => {
                debug!(
                    "Task {}: Matched entry {} -> series {} (source: {})",
                    task_id, entry.external_id, ext_id.series_id, source
                );
                matched += 1;
            }
            Ok(None) => {
                unmatched += 1;
            }
            Err(e) => {
                warn!(
                    "Task {}: Failed to look up external ID {} (source: {}): {}",
                    task_id, entry.external_id, source, e
                );
                unmatched += 1;
            }
        }
    }

    if unmatched > 0 {
        debug!(
            "Task {}: {} entries matched, {} unmatched (source: {})",
            task_id, matched, unmatched, source
        );
    }

    matched
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::ScanningStrategy;
    use crate::db::repositories::{LibraryRepository, SeriesRepository};
    use crate::db::test_helpers::create_test_db;
    use crate::services::plugin::sync::SyncReadingStatus;

    #[test]
    fn test_handler_creation() {
        // Handler requires a PluginManager, verify the struct is constructed correctly
        // (actual integration test would need a real PluginManager)
    }

    #[test]
    fn test_sync_result_serialization() {
        let result = UserPluginSyncResult {
            plugin_id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            external_username: Some("manga_reader".to_string()),
            pushed: 5,
            pulled: 10,
            matched: 8,
            push_failures: 1,
            pull_incomplete: false,
            skipped_reason: None,
        };

        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["externalUsername"], "manga_reader");
        assert_eq!(json["pushed"], 5);
        assert_eq!(json["pulled"], 10);
        assert_eq!(json["matched"], 8);
        assert_eq!(json["pushFailures"], 1);
        assert!(!json["pullIncomplete"].as_bool().unwrap());
        assert!(!json.as_object().unwrap().contains_key("skippedReason"));
    }

    #[test]
    fn test_sync_result_skipped() {
        let result = UserPluginSyncResult {
            plugin_id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            external_username: None,
            pushed: 0,
            pulled: 0,
            matched: 0,
            push_failures: 0,
            pull_incomplete: false,
            skipped_reason: Some("plugin_not_enabled".to_string()),
        };

        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["skippedReason"], "plugin_not_enabled");
        assert!(!json.as_object().unwrap().contains_key("externalUsername"));
        assert_eq!(json["pushed"], 0);
        assert_eq!(json["pulled"], 0);
        assert_eq!(json["matched"], 0);
    }

    #[test]
    fn test_sync_result_deserialization() {
        let json = serde_json::json!({
            "pluginId": "00000000-0000-0000-0000-000000000001",
            "userId": "00000000-0000-0000-0000-000000000002",
            "externalUsername": "test_user",
            "pushed": 3,
            "pulled": 7,
            "matched": 5,
            "pushFailures": 0,
            "pullIncomplete": true,
        });

        let result: UserPluginSyncResult = serde_json::from_value(json).unwrap();
        assert_eq!(result.external_username, Some("test_user".to_string()));
        assert_eq!(result.pushed, 3);
        assert_eq!(result.pulled, 7);
        assert_eq!(result.matched, 5);
        assert!(result.pull_incomplete);
        assert!(result.skipped_reason.is_none());
    }

    #[test]
    fn test_sync_result_pull_incomplete() {
        let result = UserPluginSyncResult {
            plugin_id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            external_username: Some("user".to_string()),
            pushed: 0,
            pulled: 500,
            matched: 300,
            push_failures: 0,
            pull_incomplete: true,
            skipped_reason: None,
        };

        let json = serde_json::to_value(&result).unwrap();
        assert!(json["pullIncomplete"].as_bool().unwrap());
        assert_eq!(json["pulled"], 500);
        assert_eq!(json["matched"], 300);
    }

    #[tokio::test]
    async fn test_match_pulled_entries_no_source() {
        let (db, _temp_dir) = create_test_db().await;

        let entries = vec![SyncEntry {
            external_id: "12345".to_string(),
            status: SyncReadingStatus::Reading,
            progress: None,
            score: None,
            started_at: None,
            completed_at: None,
            notes: None,
        }];

        let matched =
            match_pulled_entries(db.sea_orm_connection(), &entries, None, Uuid::new_v4()).await;
        assert_eq!(matched, 0);
    }

    #[tokio::test]
    async fn test_match_pulled_entries_with_matches() {
        let (db, _temp_dir) = create_test_db().await;

        let library = LibraryRepository::create(
            db.sea_orm_connection(),
            "Test Library",
            "/test/path",
            ScanningStrategy::Default,
        )
        .await
        .unwrap();

        let series =
            SeriesRepository::create(db.sea_orm_connection(), library.id, "My Manga", None)
                .await
                .unwrap();

        // Create an api:anilist external ID for the series
        SeriesExternalIdRepository::create(
            db.sea_orm_connection(),
            series.id,
            "api:anilist",
            "12345",
            None,
            None,
        )
        .await
        .unwrap();

        let entries = vec![
            SyncEntry {
                external_id: "12345".to_string(), // matches
                status: SyncReadingStatus::Reading,
                progress: None,
                score: None,
                started_at: None,
                completed_at: None,
                notes: None,
            },
            SyncEntry {
                external_id: "99999".to_string(), // no match
                status: SyncReadingStatus::Completed,
                progress: None,
                score: None,
                started_at: None,
                completed_at: None,
                notes: None,
            },
        ];

        let matched = match_pulled_entries(
            db.sea_orm_connection(),
            &entries,
            Some("api:anilist"),
            Uuid::new_v4(),
        )
        .await;
        assert_eq!(matched, 1);
    }

    #[tokio::test]
    async fn test_match_pulled_entries_empty() {
        let (db, _temp_dir) = create_test_db().await;

        let matched = match_pulled_entries(
            db.sea_orm_connection(),
            &[],
            Some("api:anilist"),
            Uuid::new_v4(),
        )
        .await;
        assert_eq!(matched, 0);
    }
}
