//! Handler for UserPluginRecommendations task
//!
//! Processes recommendation refresh tasks by spawning the plugin process
//! with per-user credentials. Optionally calls `recommendations/clear` to
//! invalidate cached recommendations (if supported), then calls
//! `recommendations/get` to pre-generate fresh results.

use anyhow::Result;
use chrono::Utc;
use sea_orm::DatabaseConnection;
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::db::entities::tasks;
use crate::db::repositories::{PluginsRepository, UserPluginDataRepository, UserPluginsRepository};
use crate::events::EventBroadcaster;
use crate::services::SettingsService;
use crate::services::plugin::PluginManager;
use crate::services::plugin::library::build_user_library;
use crate::services::plugin::protocol::{
    PluginManifest, UserLibraryEntry, UserReadingStatus, methods,
};
use crate::services::plugin::recommendations::{
    RecommendationClearResponse, RecommendationRequest, RecommendationResponse,
};
use crate::tasks::handlers::TaskHandler;
use crate::tasks::types::TaskResult;

/// Default plugin task timeout in seconds (5 minutes)
const DEFAULT_TASK_TIMEOUT_SECS: u64 = 300;

// =============================================================================
// Codex Recommendation Settings
// =============================================================================

/// JSON key for the Codex-reserved namespace in user plugin config.
const CODEX_CONFIG_NAMESPACE: &str = "_codex";

/// Codex recommendation settings — server-interpreted preferences that control
/// seed curation and result limits. Stored in `config._codex` on the user plugin.
/// The plugin never reads these; they control server-side behavior.
#[derive(Debug, Clone)]
struct CodexRecommendationSettings {
    /// Maximum number of recommendations to request from the plugin (1-100). Default: 20.
    max_recommendations: u32,
    /// Maximum number of seed titles to send to the plugin (1-25). Default: 10.
    max_seeds: u32,
    /// Minimum user rating (0-100 internal scale) for a series to be used as seed.
    /// Stored in config as 0-10 (display scale), converted by multiplying by 10.
    /// Default: 0 (no threshold).
    drop_threshold: i32,
}

impl CodexRecommendationSettings {
    /// Parse recommendation settings from the `_codex` namespace in user plugin config.
    fn from_user_config(config: &serde_json::Value) -> Self {
        let codex = config
            .get(CODEX_CONFIG_NAMESPACE)
            .unwrap_or(&serde_json::Value::Null);

        let max_recommendations = codex
            .get("maxRecommendations")
            .and_then(|v| v.as_u64())
            .map(|v| (v as u32).clamp(1, 100))
            .unwrap_or(20);

        let max_seeds = codex
            .get("maxSeeds")
            .and_then(|v| v.as_u64())
            .map(|v| (v as u32).clamp(1, 25))
            .unwrap_or(10);

        // dropThreshold is stored as 0-10 (display scale with 0.5 steps).
        // Convert to 0-100 internal scale by multiplying by 10.
        let drop_threshold = codex
            .get("dropThreshold")
            .and_then(|v| v.as_f64())
            .map(|v| (v * 10.0).round() as i32)
            .map(|v| v.clamp(0, 100))
            .unwrap_or(0);

        Self {
            max_recommendations,
            max_seeds,
            drop_threshold,
        }
    }
}

/// Curate seed entries from the user's library for recommendation generation.
///
/// 1. Prefer Reading/Completed entries (user has engaged with these)
/// 2. Split into rated (above threshold) and unrated
/// 3. Sort rated by user_rating desc, then last_read_at desc
/// 4. Sort unrated by last_read_at desc
/// 5. Take top max_seeds: rated first, fill remaining with unrated
/// 6. If no engaged entries, fall back to all entries (sorted by title)
///    so users with only unread series still get recommendations
fn curate_seeds(
    library: &[UserLibraryEntry],
    settings: &CodexRecommendationSettings,
) -> Vec<UserLibraryEntry> {
    // Prefer entries the user has actually engaged with
    let engaged: Vec<&UserLibraryEntry> = library
        .iter()
        .filter(|e| {
            matches!(
                e.reading_status,
                Some(UserReadingStatus::Reading) | Some(UserReadingStatus::Completed)
            )
        })
        .collect();

    // If no engaged entries exist, fall back to the full library so users
    // with only unread series can still get recommendations.
    let candidates = if engaged.is_empty() {
        library.iter().collect::<Vec<_>>()
    } else {
        engaged
    };

    // Split by rating threshold
    let (mut rated, mut unrated): (Vec<&UserLibraryEntry>, Vec<&UserLibraryEntry>) = candidates
        .into_iter()
        .partition(|e| e.user_rating.is_some_and(|r| r >= settings.drop_threshold));

    // Sort rated by rating desc, then last_read_at desc
    rated.sort_by(|a, b| {
        b.user_rating
            .cmp(&a.user_rating)
            .then_with(|| b.last_read_at.cmp(&a.last_read_at))
    });

    // Sort unrated by last_read_at desc (most recent first)
    unrated.sort_by(|a, b| b.last_read_at.cmp(&a.last_read_at));

    // Take top max_seeds: rated first, fill with unrated
    rated
        .into_iter()
        .chain(unrated)
        .take(settings.max_seeds as usize)
        .cloned()
        .collect()
}

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
            let (handle, context) = self
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

            // Read user plugin config for _codex recommendation settings
            let rec_settings =
                match UserPluginsRepository::get_by_user_and_plugin(db, user_id, plugin_id).await {
                    Ok(Some(user_plugin)) => {
                        CodexRecommendationSettings::from_user_config(&user_plugin.config)
                    }
                    _ => CodexRecommendationSettings::from_user_config(&serde_json::Value::Null),
                };

            debug!(
                "Task {}: Recommendation settings: max_recommendations={}, max_seeds={}, drop_threshold={}",
                task.id,
                rec_settings.max_recommendations,
                rec_settings.max_seeds,
                rec_settings.drop_threshold
            );

            // Build user library data
            let library = build_user_library(db, user_id).await.unwrap_or_else(|e| {
                warn!(
                    "Task {}: Failed to build user library, using empty: {}",
                    task.id, e
                );
                vec![]
            });

            // Resolve the plugin's external_id_source so we can populate exclude_ids
            // with external IDs from series the user has already read (Reading or Completed).
            // Unread series are NOT excluded — the user may want recommendations for titles
            // they own but haven't started yet.
            let exclude_ids = match PluginsRepository::get_by_id(db, plugin_id).await {
                Ok(Some(plugin_model)) => {
                    let source = plugin_model
                        .manifest
                        .as_ref()
                        .and_then(|m| serde_json::from_value::<PluginManifest>(m.clone()).ok())
                        .and_then(|m| m.capabilities.external_id_source);

                    if let Some(source) = source {
                        library
                            .iter()
                            .filter(|entry| {
                                matches!(
                                    entry.reading_status,
                                    Some(UserReadingStatus::Reading)
                                        | Some(UserReadingStatus::Completed)
                                )
                            })
                            .flat_map(|entry| {
                                entry
                                    .external_ids
                                    .iter()
                                    .filter(|eid| eid.source == source)
                                    .map(|eid| eid.external_id.clone())
                            })
                            .collect::<Vec<_>>()
                    } else {
                        vec![]
                    }
                }
                _ => vec![],
            };

            debug!(
                "Task {}: Excluding {} external IDs from recommendations",
                task.id,
                exclude_ids.len()
            );

            // Curate seeds from library: rated entries first, then recent reads
            let seeds = curate_seeds(&library, &rec_settings);

            debug!(
                "Task {}: Curated {} seeds from {} library entries (threshold={}, max_seeds={})",
                task.id,
                seeds.len(),
                library.len(),
                rec_settings.drop_threshold,
                rec_settings.max_seeds
            );

            // Call recommendations/get with curated seeds (not the full library)
            let request = RecommendationRequest {
                library: seeds,
                limit: Some(rec_settings.max_recommendations),
                exclude_ids,
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

            let mut response = result.map_err(|e| {
                warn!(
                    "Task {}: Failed to generate recommendations: {}",
                    task.id, e
                );
                anyhow::anyhow!("Failed to generate recommendations: {}", e)
            })?;

            let count = response.recommendations.len();

            // Stamp generation time and persist to user_plugin_data for the GET endpoint
            response.generated_at = Some(Utc::now().to_rfc3339());
            let cached_data = serde_json::to_value(&response)
                .map_err(|e| anyhow::anyhow!("Failed to serialize recommendations: {}", e))?;
            UserPluginDataRepository::set(
                db,
                context.user_plugin_id,
                "recommendations",
                cached_data,
                None,
            )
            .await
            .map_err(|e| anyhow::anyhow!("Failed to persist recommendations: {}", e))?;

            info!(
                "Task {}: Generated and persisted {} recommendations for plugin {} / user {}",
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
