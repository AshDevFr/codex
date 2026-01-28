//! Plugin auto-match task handler
//!
//! This handler processes plugin auto-match tasks, which search for metadata
//! using a plugin and apply the best match to a series.

use anyhow::{Context, Result};
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;
use tracing::{debug, info};
use uuid::Uuid;

use crate::db::entities::tasks;
use crate::db::repositories::{PluginsRepository, SeriesMetadataRepository, SeriesRepository};
use crate::events::{EntityChangeEvent, EntityEvent, EventBroadcaster};
use crate::services::metadata::{ApplyOptions, MetadataApplier, SkippedField};
use crate::services::plugin::protocol::{MetadataGetParams, MetadataSearchParams};
use crate::services::plugin::PluginManager;
use crate::services::settings::SettingsService;
use crate::services::ThumbnailService;
use crate::tasks::handlers::TaskHandler;
use crate::tasks::types::TaskResult;

/// Settings key for the auto-match confidence threshold
const SETTING_AUTO_MATCH_CONFIDENCE_THRESHOLD: &str = "plugins.auto_match_confidence_threshold";
/// Default confidence threshold for auto-match (0.8 = 80%)
const DEFAULT_CONFIDENCE_THRESHOLD: f64 = 0.8;

/// Result of a plugin auto-match operation
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginAutoMatchResult {
    pub matched: bool,
    pub external_id: Option<String>,
    pub external_url: Option<String>,
    pub matched_title: Option<String>,
    pub fields_updated: Vec<String>,
    pub fields_skipped: Vec<SkippedField>,
    pub skipped_reason: Option<String>,
}

/// Handler for plugin auto-match tasks
pub struct PluginAutoMatchHandler {
    plugin_manager: Arc<PluginManager>,
    thumbnail_service: Option<Arc<ThumbnailService>>,
    settings_service: Option<Arc<SettingsService>>,
}

impl PluginAutoMatchHandler {
    pub fn new(plugin_manager: Arc<PluginManager>) -> Self {
        Self {
            plugin_manager,
            thumbnail_service: None,
            settings_service: None,
        }
    }

    pub fn with_thumbnail_service(mut self, thumbnail_service: Arc<ThumbnailService>) -> Self {
        self.thumbnail_service = Some(thumbnail_service);
        self
    }

    pub fn with_settings_service(mut self, settings_service: Arc<SettingsService>) -> Self {
        self.settings_service = Some(settings_service);
        self
    }
}

impl TaskHandler for PluginAutoMatchHandler {
    fn handle<'a>(
        &'a self,
        task: &'a tasks::Model,
        db: &'a DatabaseConnection,
        event_broadcaster: Option<&'a Arc<EventBroadcaster>>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<TaskResult>> + Send + 'a>> {
        Box::pin(async move {
            // Extract task parameters
            let series_id = task
                .series_id
                .ok_or_else(|| anyhow::anyhow!("Missing series_id in task"))?;

            let params = task
                .params
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("Missing params in task"))?;

            let plugin_id: Uuid = params
                .get("plugin_id")
                .and_then(|v| v.as_str())
                .and_then(|s| Uuid::parse_str(s).ok())
                .ok_or_else(|| anyhow::anyhow!("Missing or invalid plugin_id in params"))?;

            let source_scope = params
                .get("source_scope")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            info!(
                "Task {}: Auto-matching series {} with plugin {} (source: {:?})",
                task.id, series_id, plugin_id, source_scope
            );

            // Check if plugin is enabled
            let plugin = match PluginsRepository::get_by_id(db, plugin_id).await? {
                Some(p) => p,
                None => {
                    return Ok(TaskResult::success_with_data(
                        "Plugin not found, skipped",
                        json!(PluginAutoMatchResult {
                            matched: false,
                            external_id: None,
                            external_url: None,
                            matched_title: None,
                            fields_updated: vec![],
                            fields_skipped: vec![],
                            skipped_reason: Some("plugin_not_found".to_string()),
                        }),
                    ));
                }
            };

            if !plugin.enabled {
                return Ok(TaskResult::success_with_data(
                    "Plugin disabled, skipped",
                    json!(PluginAutoMatchResult {
                        matched: false,
                        external_id: None,
                        external_url: None,
                        matched_title: None,
                        fields_updated: vec![],
                        fields_skipped: vec![],
                        skipped_reason: Some("plugin_disabled".to_string()),
                    }),
                ));
            }

            // Get series and its metadata for the search query
            let series = SeriesRepository::get_by_id(db, series_id)
                .await?
                .ok_or_else(|| anyhow::anyhow!("Series not found: {}", series_id))?;

            let series_metadata = SeriesMetadataRepository::get_by_series_id(db, series_id).await?;

            let search_query = series_metadata
                .as_ref()
                .map(|m| m.title.clone())
                .unwrap_or_else(|| series.name.clone());

            debug!(
                "Task {}: Searching for '{}' using plugin {}",
                task.id, search_query, plugin.name
            );

            // Search for metadata
            let search_params = MetadataSearchParams {
                query: search_query.clone(),
                limit: Some(10),
                cursor: None,
            };

            let search_response = self
                .plugin_manager
                .search_series(plugin_id, search_params)
                .await
                .context("Failed to search for metadata")?;

            if search_response.results.is_empty() {
                info!("Task {}: No matches found for '{}'", task.id, search_query);
                return Ok(TaskResult::success_with_data(
                    format!("No matches found for '{}'", search_query),
                    json!(PluginAutoMatchResult {
                        matched: false,
                        external_id: None,
                        external_url: None,
                        matched_title: None,
                        fields_updated: vec![],
                        fields_skipped: vec![],
                        skipped_reason: Some("no_match".to_string()),
                    }),
                ));
            }

            // Pick the best result based on relevance_score
            let best_match = search_response
                .results
                .into_iter()
                .enumerate()
                .max_by(|(i, a), (j, b)| {
                    match (a.relevance_score, b.relevance_score) {
                        (Some(a_score), Some(b_score)) => a_score
                            .partial_cmp(&b_score)
                            .unwrap_or(std::cmp::Ordering::Equal),
                        // If no scores, prefer earlier results (lower index = higher relevance)
                        _ => j.cmp(i),
                    }
                })
                .map(|(_, result)| result)
                .unwrap(); // Safe: we checked results is non-empty

            // Get confidence threshold from settings (fallback to default if not available)
            let min_confidence = if let Some(ref settings) = self.settings_service {
                settings
                    .get_float(
                        SETTING_AUTO_MATCH_CONFIDENCE_THRESHOLD,
                        DEFAULT_CONFIDENCE_THRESHOLD,
                    )
                    .await
                    .unwrap_or(DEFAULT_CONFIDENCE_THRESHOLD)
            } else {
                DEFAULT_CONFIDENCE_THRESHOLD
            };

            // Check confidence threshold
            // Only skip if the plugin provides a relevance score AND it's below the threshold
            // If no relevance score is provided, we proceed with the match (to support plugins
            // that don't return relevance scores)
            if let Some(relevance_score) = best_match.relevance_score {
                if relevance_score < min_confidence {
                    info!(
                        "Task {}: Best match '{}' has low confidence ({:.2} < {:.2}), skipping",
                        task.id, best_match.title, relevance_score, min_confidence
                    );
                    return Ok(TaskResult::success_with_data(
                        format!(
                            "Low confidence match ({:.0}% < {:.0}%), skipped",
                            relevance_score * 100.0,
                            min_confidence * 100.0
                        ),
                        json!(PluginAutoMatchResult {
                            matched: false,
                            external_id: Some(best_match.external_id.clone()),
                            external_url: None, // Not available from search results
                            matched_title: Some(best_match.title.clone()),
                            fields_updated: vec![],
                            fields_skipped: vec![],
                            skipped_reason: Some("low_confidence".to_string()),
                        }),
                    ));
                }
            }

            let external_id = best_match.external_id.clone();
            let matched_title = best_match.title.clone();

            info!(
                "Task {}: Best match: '{}' (id: {})",
                task.id, matched_title, external_id
            );

            // Fetch full metadata
            let get_params = MetadataGetParams {
                external_id: external_id.clone(),
            };

            let plugin_metadata = self
                .plugin_manager
                .get_series_metadata(plugin_id, get_params)
                .await
                .context("Failed to fetch full metadata")?;

            let external_url = plugin_metadata.external_url.clone();

            // Get current metadata for lock checking
            let current_metadata =
                SeriesMetadataRepository::get_by_series_id(db, series_id).await?;

            // Build apply options with thumbnail service and event broadcaster
            let options = ApplyOptions {
                fields_filter: None, // Apply all fields
                thumbnail_service: self.thumbnail_service.clone(),
                event_broadcaster: event_broadcaster.cloned(),
            };

            // Apply metadata using the shared service
            let result = MetadataApplier::apply(
                db,
                series_id,
                series.library_id,
                &plugin,
                &plugin_metadata,
                current_metadata.as_ref(),
                &options,
            )
            .await
            .context("Failed to apply metadata")?;

            let applied_fields = result.applied_fields;
            let skipped_fields = result.skipped_fields;

            info!(
                "Task {}: Applied {} fields, skipped {} fields",
                task.id,
                applied_fields.len(),
                skipped_fields.len()
            );

            // Emit series metadata updated event
            if let Some(broadcaster) = event_broadcaster {
                if !applied_fields.is_empty() {
                    let _ = broadcaster.emit(EntityChangeEvent::new(
                        EntityEvent::SeriesMetadataUpdated {
                            series_id,
                            library_id: series.library_id,
                            plugin_id,
                            fields_updated: applied_fields.clone(),
                        },
                        None,
                    ));
                }
            }

            // Record success with plugin
            if let Err(e) = PluginsRepository::record_success(db, plugin_id).await {
                tracing::warn!("Failed to record plugin success: {}", e);
            }

            let result = PluginAutoMatchResult {
                matched: !applied_fields.is_empty(),
                external_id: Some(external_id),
                external_url: Some(external_url),
                matched_title: Some(matched_title.clone()),
                fields_updated: applied_fields.clone(),
                fields_skipped: skipped_fields,
                skipped_reason: None,
            };

            let message = if applied_fields.is_empty() {
                format!("Matched '{}' but no fields were applied", matched_title)
            } else {
                format!(
                    "Matched '{}' and applied {} field(s)",
                    matched_title,
                    applied_fields.len()
                )
            };

            Ok(TaskResult::success_with_data(message, json!(result)))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_auto_match_result_serialization() {
        let result = PluginAutoMatchResult {
            matched: true,
            external_id: Some("12345".to_string()),
            external_url: Some("https://example.com/series/12345".to_string()),
            matched_title: Some("Test Series".to_string()),
            fields_updated: vec!["title".to_string(), "summary".to_string()],
            fields_skipped: vec![SkippedField {
                field: "genres".to_string(),
                reason: "Plugin does not have permission".to_string(),
            }],
            skipped_reason: None,
        };

        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["matched"], true);
        assert_eq!(json["externalId"], "12345");
        assert_eq!(json["fieldsUpdated"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn test_skipped_result() {
        let result = PluginAutoMatchResult {
            matched: false,
            external_id: None,
            external_url: None,
            matched_title: None,
            fields_updated: vec![],
            fields_skipped: vec![],
            skipped_reason: Some("plugin_disabled".to_string()),
        };

        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["matched"], false);
        assert_eq!(json["skippedReason"], "plugin_disabled");
    }

    #[test]
    fn test_low_confidence_result() {
        // Low confidence result should include the matched info (external_id, title)
        // but not external_url since that's not available from search results
        let result = PluginAutoMatchResult {
            matched: false,
            external_id: Some("12345".to_string()),
            external_url: None, // Not available from search results
            matched_title: Some("Test Series".to_string()),
            fields_updated: vec![],
            fields_skipped: vec![],
            skipped_reason: Some("low_confidence".to_string()),
        };

        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["matched"], false);
        assert_eq!(json["skippedReason"], "low_confidence");
        // Low confidence should still include the matched info
        assert_eq!(json["externalId"], "12345");
        assert_eq!(json["matchedTitle"], "Test Series");
        assert!(json["externalUrl"].is_null());
    }

    #[test]
    fn test_default_confidence_threshold() {
        assert_eq!(DEFAULT_CONFIDENCE_THRESHOLD, 0.8);
    }

    #[test]
    fn test_setting_key() {
        assert_eq!(
            SETTING_AUTO_MATCH_CONFIDENCE_THRESHOLD,
            "plugins.auto_match_confidence_threshold"
        );
    }
}
