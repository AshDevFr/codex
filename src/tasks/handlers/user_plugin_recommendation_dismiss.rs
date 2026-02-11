//! Handler for UserPluginRecommendationDismiss task
//!
//! Notifies the plugin that a recommendation was dismissed by the user.
//! The actual removal from cached data happens synchronously in the API handler;
//! this task only informs the plugin so it can update its internal state.

use anyhow::Result;
use sea_orm::DatabaseConnection;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::db::entities::tasks;
use crate::events::EventBroadcaster;
use crate::services::SettingsService;
use crate::services::plugin::PluginManager;
use crate::services::plugin::protocol::methods;
use crate::services::plugin::recommendations::{
    DismissReason, RecommendationDismissRequest, RecommendationDismissResponse,
};
use crate::tasks::handlers::TaskHandler;
use crate::tasks::types::TaskResult;

/// Default plugin task timeout in seconds (5 minutes)
const DEFAULT_TASK_TIMEOUT_SECS: u64 = 300;

/// Handler for user plugin recommendation dismiss tasks
pub struct UserPluginRecommendationDismissHandler {
    plugin_manager: Arc<PluginManager>,
    settings_service: Option<Arc<SettingsService>>,
}

impl UserPluginRecommendationDismissHandler {
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

impl TaskHandler for UserPluginRecommendationDismissHandler {
    fn handle<'a>(
        &'a self,
        task: &'a tasks::Model,
        _db: &'a DatabaseConnection,
        _event_broadcaster: Option<&'a Arc<EventBroadcaster>>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<TaskResult>> + Send + 'a>> {
        Box::pin(async move {
            let params = task.params.as_ref().ok_or_else(|| {
                anyhow::anyhow!("Missing params in user_plugin_recommendation_dismiss task")
            })?;

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

            let external_id: String = params
                .get("external_id")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .ok_or_else(|| anyhow::anyhow!("Missing external_id in params"))?;

            let reason: Option<DismissReason> = params
                .get("reason")
                .and_then(|v| v.as_str())
                .and_then(|r| match r {
                    "not_interested" => Some(DismissReason::NotInterested),
                    "already_read" => Some(DismissReason::AlreadyRead),
                    "already_owned" => Some(DismissReason::AlreadyOwned),
                    _ => None,
                });

            info!(
                "Task {}: Dismissing recommendation {} for plugin {} / user {}",
                task.id, external_id, plugin_id, user_id
            );

            let request_timeout = self.task_request_timeout().await;

            let (handle, _context) = self
                .plugin_manager
                .get_user_plugin_handle(plugin_id, user_id, request_timeout)
                .await
                .map_err(|e| anyhow::anyhow!("Failed to spawn plugin for dismiss: {}", e))?;

            let dismiss_request = RecommendationDismissRequest {
                external_id: external_id.clone(),
                reason,
            };

            let result = handle
                .call_method::<RecommendationDismissRequest, RecommendationDismissResponse>(
                    methods::RECOMMENDATIONS_DISMISS,
                    dismiss_request,
                )
                .await;

            if let Err(e) = handle.stop().await {
                warn!("Task {}: Failed to stop plugin handle: {}", task.id, e);
            }

            match result {
                Ok(response) => {
                    debug!(
                        "Task {}: Plugin acknowledged dismiss (dismissed={})",
                        task.id, response.dismissed
                    );
                    Ok(TaskResult {
                        success: true,
                        message: Some(format!(
                            "Dismissed recommendation {} (acknowledged={})",
                            external_id, response.dismissed
                        )),
                        data: None,
                    })
                }
                Err(e) => {
                    warn!("Task {}: Plugin failed to process dismiss: {}", task.id, e);
                    // Still consider this a success — the recommendation was already
                    // removed from the cached list. The plugin just couldn't be notified.
                    Ok(TaskResult {
                        success: true,
                        message: Some(format!(
                            "Dismissed recommendation {} (plugin notification failed: {})",
                            external_id, e
                        )),
                        data: None,
                    })
                }
            }
        })
    }
}
