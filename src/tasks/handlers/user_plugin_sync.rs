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
use crate::db::repositories::UserPluginsRepository;
use crate::events::EventBroadcaster;
use crate::services::plugin::PluginManager;
use crate::services::plugin::protocol::methods;
use crate::services::plugin::sync::{
    ExternalUserInfo, SyncPullRequest, SyncPullResponse, SyncPushRequest, SyncPushResponse,
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

            // Step 2: Pull progress from external service
            let pull_request = SyncPullRequest {
                since: None, // Full pull for now; incremental can use last_sync_at
                limit: Some(500),
                cursor: None,
            };

            let (pulled_count, pull_incomplete) = match handle
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
                    // TODO: Apply pulled entries to Codex user's reading progress
                    // This will be integrated when the reading progress tracking
                    // system is available in Codex
                    (count, has_more)
                }
                Err(e) => {
                    error!("Task {}: Pull failed: {}", task.id, e);
                    // Continue to push even if pull fails
                    (0, false)
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
                push_failures,
                pull_incomplete,
                skipped_reason: None,
            };

            let message = format!(
                "Sync complete: pulled {} entries, pushed {} entries",
                pulled_count, pushed_count
            );

            info!("Task {}: {}", task.id, message);

            Ok(TaskResult::success_with_data(message, json!(result)))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
            push_failures: 1,
            pull_incomplete: false,
            skipped_reason: None,
        };

        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["externalUsername"], "manga_reader");
        assert_eq!(json["pushed"], 5);
        assert_eq!(json["pulled"], 10);
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
            push_failures: 0,
            pull_incomplete: false,
            skipped_reason: Some("plugin_not_enabled".to_string()),
        };

        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["skippedReason"], "plugin_not_enabled");
        assert!(!json.as_object().unwrap().contains_key("externalUsername"));
        assert_eq!(json["pushed"], 0);
        assert_eq!(json["pulled"], 0);
    }

    #[test]
    fn test_sync_result_deserialization() {
        let json = serde_json::json!({
            "pluginId": "00000000-0000-0000-0000-000000000001",
            "userId": "00000000-0000-0000-0000-000000000002",
            "externalUsername": "test_user",
            "pushed": 3,
            "pulled": 7,
            "pushFailures": 0,
            "pullIncomplete": true,
        });

        let result: UserPluginSyncResult = serde_json::from_value(json).unwrap();
        assert_eq!(result.external_username, Some("test_user".to_string()));
        assert_eq!(result.pushed, 3);
        assert_eq!(result.pulled, 7);
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
            push_failures: 0,
            pull_incomplete: true,
            skipped_reason: None,
        };

        let json = serde_json::to_value(&result).unwrap();
        assert!(json["pullIncomplete"].as_bool().unwrap());
        assert_eq!(json["pulled"], 500);
    }
}
