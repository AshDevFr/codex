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
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::db::entities::tasks;
use crate::events::EventBroadcaster;
use crate::services::plugin::PluginManager;
use crate::services::plugin::library::build_user_library;
use crate::services::plugin::protocol::methods;
use crate::services::plugin::recommendations::{
    RecommendationClearResponse, RecommendationRequest, RecommendationResponse,
};
use crate::tasks::handlers::TaskHandler;
use crate::tasks::types::TaskResult;

/// Handler for user plugin recommendation refresh tasks
pub struct UserPluginRecommendationsHandler {
    plugin_manager: Arc<PluginManager>,
}

impl UserPluginRecommendationsHandler {
    pub fn new(plugin_manager: Arc<PluginManager>) -> Self {
        Self { plugin_manager }
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

            // Get user plugin handle (spawns process with per-user credentials)
            let (handle, _context) = self
                .plugin_manager
                .get_user_plugin_handle(plugin_id, user_id)
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
