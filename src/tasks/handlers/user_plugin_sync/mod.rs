//! Handler for UserPluginSync task
//!
//! Processes user plugin sync tasks by spawning the plugin process with
//! per-user credentials and calling sync methods (push/pull progress)
//! via JSON-RPC.
//!
//! Module structure:
//! - `settings` — CodexSyncSettings parsing from user config
//! - `push` — Build entries from local reading progress to push
//! - `pull` — Match external entries and apply reading progress

mod pull;
mod push;
pub(crate) mod settings;

#[cfg(test)]
mod tests;

use anyhow::Result;
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::db::entities::tasks;
use crate::db::repositories::{UserPluginDataRepository, UserPluginsRepository};
use crate::events::EventBroadcaster;
use crate::services::SettingsService;
use crate::services::plugin::PluginManager;
use crate::services::plugin::protocol::methods;
use crate::services::plugin::sync::{
    ExternalUserInfo, SyncPullRequest, SyncPullResponse, SyncPushRequest, SyncPushResponse,
};
use crate::tasks::handlers::TaskHandler;
use crate::tasks::types::TaskResult;

pub(crate) use settings::CodexSyncSettings;

/// Storage key under which the last sync result is persisted in `user_plugin_data`.
/// Used by the sync handler to write the result and by API handlers to read it.
pub const LAST_SYNC_RESULT_KEY: &str = "last_sync_result";

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
    /// Number of books whose reading progress was applied from pulled entries
    #[serde(default)]
    pub applied: u32,
    /// Push failures
    pub push_failures: u32,
    /// Pull had more pages (not all pulled)
    #[serde(default)]
    pub pull_incomplete: bool,
    /// Error message if pull failed entirely
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pull_error: Option<String>,
    /// Error message if push failed entirely
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub push_error: Option<String>,
    /// Reason for skipping, if sync was skipped
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skipped_reason: Option<String>,
}

/// Default plugin task timeout in seconds (5 minutes)
const DEFAULT_TASK_TIMEOUT_SECS: u64 = 300;

/// Handler for user plugin sync tasks
pub struct UserPluginSyncHandler {
    plugin_manager: Arc<PluginManager>,
    settings_service: Option<Arc<SettingsService>>,
}

impl UserPluginSyncHandler {
    pub fn new(plugin_manager: Arc<PluginManager>) -> Self {
        Self {
            plugin_manager,
            settings_service: None,
        }
    }

    pub fn with_settings_service(mut self, settings_service: Arc<SettingsService>) -> Self {
        self.settings_service = Some(settings_service);
        self
    }

    /// Read the configured plugin task timeout from settings
    async fn task_request_timeout(&self) -> Option<Duration> {
        if let Some(ref settings) = self.settings_service {
            let secs = settings
                .get_uint(
                    "plugin.task_request_timeout_seconds",
                    DEFAULT_TASK_TIMEOUT_SECS,
                )
                .await
                .unwrap_or(DEFAULT_TASK_TIMEOUT_SECS);
            Some(Duration::from_secs(secs))
        } else {
            None
        }
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

            // Read user plugin config
            let user_config =
                match UserPluginsRepository::get_by_user_and_plugin(db, user_id, plugin_id).await {
                    Ok(Some(instance)) => instance.config.clone(),
                    _ => serde_json::json!({}),
                };
            let sync_mode = user_config
                .get("syncMode")
                .and_then(|v| v.as_str())
                .unwrap_or("both")
                .to_string();
            let do_pull = sync_mode == "both" || sync_mode == "pull";
            let do_push = sync_mode == "both" || sync_mode == "push";
            let codex_settings = CodexSyncSettings::from_user_config(&user_config);

            debug!(
                "Task {}: syncMode={} (pull={}, push={})",
                task.id, sync_mode, do_pull, do_push
            );

            // Read configured task timeout from settings
            let request_timeout = self.task_request_timeout().await;

            // Get user plugin handle (spawns process with per-user credentials)
            let (handle, context) = match self
                .plugin_manager
                .get_user_plugin_handle(plugin_id, user_id, request_timeout)
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
                            applied: 0,
                            push_failures: 0,
                            pull_incomplete: false,
                            pull_error: None,
                            push_error: None,
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
            let (pulled_count, pull_incomplete, matched_count, applied_count, pull_error) =
                if do_pull {
                    let pull_request = SyncPullRequest {
                        since: None, // Full pull for now; incremental can use last_sync_at
                        limit: Some(500),
                        cursor: None,
                    };

                    match handle
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

                            // Match pulled entries to Codex series and apply to reading progress
                            let (matched, applied) = pull::match_and_apply_pulled_entries(
                                db,
                                &pull_response.entries,
                                external_id_source.as_deref(),
                                user_id,
                                task.id,
                                codex_settings.sync_ratings,
                            )
                            .await;

                            if applied > 0 {
                                info!(
                                    "Task {}: Applied reading progress for {} books",
                                    task.id, applied
                                );
                            }

                            (count, has_more, matched, applied, None)
                        }
                        Err(e) => {
                            error!("Task {}: Pull failed: {}", task.id, e);
                            // Continue to push even if pull fails
                            (0, false, 0, 0, Some(e.to_string()))
                        }
                    }
                } else {
                    info!("Task {}: Skipping pull (syncMode={})", task.id, sync_mode);
                    (0, false, 0, 0, None)
                };

            // Step 3: Push progress to external service
            let (pushed_count, push_failures, push_error) = if do_push {
                let entries = if let Some(ref source) = external_id_source {
                    push::build_push_entries(db, user_id, source, task.id, &codex_settings).await
                } else {
                    warn!(
                        "Task {}: Plugin has no externalIdSource in manifest — cannot build push entries",
                        task.id
                    );
                    vec![]
                };
                info!(
                    "Task {}: Built {} push entries from reading progress",
                    task.id,
                    entries.len()
                );
                let push_request = SyncPushRequest { entries };

                match handle
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
                        (success_count, failure_count, None)
                    }
                    Err(e) => {
                        error!("Task {}: Push failed: {}", task.id, e);
                        (0, 0, Some(e.to_string()))
                    }
                }
            } else {
                info!("Task {}: Skipping push (syncMode={})", task.id, sync_mode);
                (0, 0, None)
            };

            // Stop the user plugin handle to clean up the spawned process
            if let Err(e) = handle.stop().await {
                warn!("Task {}: Failed to stop plugin handle: {}", task.id, e);
            }

            let had_errors = pull_error.is_some() || push_error.is_some();

            // Record sync timestamp on the user plugin instance
            if let Err(e) = UserPluginsRepository::record_sync(db, context.user_plugin_id).await {
                warn!("Task {}: Failed to record sync timestamp: {}", task.id, e);
            }

            // Record success or failure on the user plugin instance
            if had_errors {
                if let Err(e) =
                    UserPluginsRepository::record_failure(db, context.user_plugin_id).await
                {
                    warn!("Task {}: Failed to record failure: {}", task.id, e);
                }
            } else if let Err(e) =
                UserPluginsRepository::record_success(db, context.user_plugin_id).await
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
                applied: applied_count,
                push_failures,
                pull_incomplete,
                pull_error,
                push_error,
                skipped_reason: None,
            };

            // Store sync result in user_plugin_data for display on the card
            if let Err(e) = UserPluginDataRepository::set(
                db,
                context.user_plugin_id,
                LAST_SYNC_RESULT_KEY,
                json!(result),
                None,
            )
            .await
            {
                warn!("Task {}: Failed to store sync result: {}", task.id, e);
            }

            let message = format!(
                "Sync complete: pulled {} entries ({} matched, {} applied), pushed {} entries",
                pulled_count, matched_count, applied_count, pushed_count
            );

            info!("Task {}: {}", task.id, message);

            Ok(TaskResult::success_with_data(message, json!(result)))
        })
    }
}
