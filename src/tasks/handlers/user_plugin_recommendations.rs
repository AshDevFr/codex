//! Handler for UserPluginRecommendations task
//!
//! Processes recommendation refresh tasks by spawning the plugin process
//! with per-user credentials. Optionally calls `recommendations/clear` to
//! invalidate cached recommendations (if supported), then calls
//! `recommendations/get` to pre-generate fresh results.

use anyhow::Result;
use sea_orm::DatabaseConnection;
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::db::entities::tasks;
use crate::events::EventBroadcaster;
use crate::services::SettingsService;
use crate::services::plugin::PluginManager;
use crate::services::plugin::library::build_user_library;
use crate::services::plugin::protocol::methods;
use crate::services::plugin::recommendations::{
    RecommendationClearResponse, RecommendationRequest, RecommendationResponse,
};
use crate::tasks::handlers::TaskHandler;
use crate::tasks::types::TaskResult;

/// Default plugin task timeout in seconds (5 minutes)
const DEFAULT_TASK_TIMEOUT_SECS: u64 = 300;

/// Handler for user plugin recommendation refresh tasks
pub struct UserPluginRecommendationsHandler {
    plugin_manager: Arc<PluginManager>,
    settings_service: Option<Arc<SettingsService>>,
}

impl UserPluginRecommendationsHandler {
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

impl TaskHandler for UserPluginRecommendationsHandler {
    fn handle<'a>(
        &'a self,
        task: &'a tasks::Model,
        db: &'a DatabaseConnection,
        _event_broadcaster: Option<&'a Arc<EventBroadcaster>>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<TaskResult>> + Send + 'a>> {
        Box::pin(async move {
            // Extract task parameters
            let params = task.params.as_ref().ok_or_else(|| {
                anyhow::anyhow!("Missing params in user_plugin_recommendations task")
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

            info!(
                "Task {}: Refreshing recommendations for plugin {} / user {}",
                task.id, plugin_id, user_id
            );

            // Read configured task timeout from settings
            let request_timeout = self.task_request_timeout().await;

            // Get user plugin handle (spawns process with per-user credentials)
            let (handle, _context) = self
                .plugin_manager
                .get_user_plugin_handle(plugin_id, user_id, request_timeout)
                .await
                .map_err(|e| anyhow::anyhow!("Failed to spawn recommendation plugin: {}", e))?;

            // Try to clear cached recommendations (optional — not all plugins support this)
            match handle
                .call_method::<serde_json::Value, RecommendationClearResponse>(
                    methods::RECOMMENDATIONS_CLEAR,
                    json!({}),
                )
                .await
            {
                Ok(response) => {
                    info!(
                        "Task {}: Recommendations cache cleared (cleared={})",
                        task.id, response.cleared
                    );
                }
                Err(e) => {
                    debug!(
                        "Task {}: recommendations/clear not supported, skipping: {}",
                        task.id, e
                    );
                }
            }

            // Build user library data to seed recommendations
            let library = build_user_library(db, user_id).await.unwrap_or_else(|e| {
                warn!(
                    "Task {}: Failed to build user library, using empty: {}",
                    task.id, e
                );
                vec![]
            });

            debug!(
                "Task {}: Sending {} library entries to recommendation plugin",
                task.id,
                library.len()
            );

            // Call recommendations/get to generate fresh results
            let request = RecommendationRequest {
                library,
                limit: Some(20),
                exclude_ids: vec![],
            };

            let result = handle
                .call_method::<RecommendationRequest, RecommendationResponse>(
                    methods::RECOMMENDATIONS_GET,
                    request,
                )
                .await;

            // Always stop the user plugin handle to clean up the spawned process
            if let Err(e) = handle.stop().await {
                warn!("Task {}: Failed to stop plugin handle: {}", task.id, e);
            }

            let response = result.map_err(|e| {
                warn!(
                    "Task {}: Failed to generate recommendations: {}",
                    task.id, e
                );
                anyhow::anyhow!("Failed to generate recommendations: {}", e)
            })?;

            let count = response.recommendations.len();
            info!(
                "Task {}: Generated {} fresh recommendations for plugin {} / user {}",
                task.id, count, plugin_id, user_id
            );

            Ok(TaskResult {
                success: true,
                message: Some(format!("Generated {} recommendations", count)),
                data: Some(json!({
                    "plugin_id": plugin_id.to_string(),
                    "user_id": user_id.to_string(),
                    "recommendation_count": count,
                })),
            })
        })
    }
}
