//! Plugin auto-match task handler
//!
//! This handler processes plugin auto-match tasks, which search for metadata
//! using a plugin and apply the best match to a series.
//!
//! ## Enhanced Features
//!
//! - **Auto-match conditions**: Check library and plugin conditions before processing
//! - **External ID lookup**: Skip search if existing external ID can be used
//! - **Search query preprocessing**: Apply template and preprocessing rules
//! - **External ID storage**: Store/update external ID after successful match

use anyhow::{Context, Result};
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;
use tracing::{debug, info};
use uuid::Uuid;

use crate::db::entities::tasks;
use crate::db::repositories::{
    LibraryRepository, PluginsRepository, SeriesExternalIdRepository, SeriesMetadataRepository,
    SeriesRepository,
};
use crate::events::{EntityChangeEvent, EntityEvent, EventBroadcaster};
use crate::services::metadata::preprocessing::{
    apply_rules, render_template, should_match, AutoMatchConditions, PreprocessingRule,
    SeriesContext, SeriesContextBuilder,
};
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
    /// Reason for skipping the match, if not matched
    ///
    /// Possible values:
    /// - `plugin_not_found`: Plugin was deleted or doesn't exist
    /// - `plugin_disabled`: Plugin is disabled
    /// - `library_conditions_not_met`: Library auto-match conditions not satisfied
    /// - `plugin_conditions_not_met`: Plugin auto-match conditions not satisfied
    /// - `existing_external_id_used`: Used existing external ID for direct lookup (not a skip)
    /// - `no_match`: No search results found
    /// - `low_confidence`: Best match below confidence threshold
    pub skipped_reason: Option<String>,
    /// Whether an existing external ID was used for direct metadata lookup
    #[serde(default)]
    pub used_existing_external_id: bool,
    /// The search query that was used (after preprocessing)
    pub search_query_used: Option<String>,
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

            // Check if plugin exists and is enabled
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
                            used_existing_external_id: false,
                            search_query_used: None,
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
                        used_existing_external_id: false,
                        search_query_used: None,
                    }),
                ));
            }

            // Get series
            let series = SeriesRepository::get_by_id(db, series_id)
                .await?
                .ok_or_else(|| anyhow::anyhow!("Series not found: {}", series_id))?;

            // Get library for preprocessing rules and conditions
            let library = LibraryRepository::get_by_id(db, series.library_id)
                .await?
                .ok_or_else(|| anyhow::anyhow!("Library not found: {}", series.library_id))?;

            // Build series context for condition evaluation using the unified builder
            // This includes metadata, genres, tags, book count, external IDs, and custom metadata
            let series_context = SeriesContextBuilder::new(series_id).build(db).await?;

            // Check library auto-match conditions
            let library_conditions = LibraryRepository::get_auto_match_conditions(&library);
            if let Some(ref conditions) = library_conditions {
                if !check_conditions(conditions, &series_context) {
                    debug!(
                        "Task {}: Library conditions not met for series {}",
                        task.id, series_id
                    );
                    return Ok(TaskResult::success_with_data(
                        "Library auto-match conditions not met, skipped",
                        json!(PluginAutoMatchResult {
                            matched: false,
                            external_id: None,
                            external_url: None,
                            matched_title: None,
                            fields_updated: vec![],
                            fields_skipped: vec![],
                            skipped_reason: Some("library_conditions_not_met".to_string()),
                            used_existing_external_id: false,
                            search_query_used: None,
                        }),
                    ));
                }
            }

            // Check plugin auto-match conditions
            let plugin_conditions = PluginsRepository::get_auto_match_conditions(&plugin);
            if let Some(ref conditions) = plugin_conditions {
                if !check_conditions(conditions, &series_context) {
                    debug!(
                        "Task {}: Plugin conditions not met for series {}",
                        task.id, series_id
                    );
                    return Ok(TaskResult::success_with_data(
                        "Plugin auto-match conditions not met, skipped",
                        json!(PluginAutoMatchResult {
                            matched: false,
                            external_id: None,
                            external_url: None,
                            matched_title: None,
                            fields_updated: vec![],
                            fields_skipped: vec![],
                            skipped_reason: Some("plugin_conditions_not_met".to_string()),
                            used_existing_external_id: false,
                            search_query_used: None,
                        }),
                    ));
                }
            }

            // Check for existing external ID for this plugin
            let mut used_existing_external_id = false;
            let external_id_to_use: Option<String> =
                if PluginsRepository::use_existing_external_id(&plugin) {
                    if let Some(existing) =
                        SeriesExternalIdRepository::get_for_plugin(db, series_id, &plugin.name)
                            .await?
                    {
                        debug!(
                            "Task {}: Found existing external ID '{}' for plugin {}",
                            task.id, existing.external_id, plugin.name
                        );
                        used_existing_external_id = true;
                        Some(existing.external_id)
                    } else {
                        None
                    }
                } else {
                    None
                };

            // Determine the external ID to use (existing or search)
            let (external_id, matched_title, search_query_used) = if let Some(ext_id) =
                external_id_to_use
            {
                // Use existing external ID - fetch metadata directly
                info!(
                    "Task {}: Using existing external ID '{}' for direct lookup",
                    task.id, ext_id
                );
                (ext_id, None, None)
            } else {
                // Build search query with preprocessing
                let base_query = series_context
                    .metadata
                    .title
                    .clone()
                    .unwrap_or_else(|| series.name.clone());

                // Apply plugin search query template if configured
                let templated_query =
                    if let Some(template) = PluginsRepository::get_search_query_template(&plugin) {
                        // Convert series context to JSON for template rendering
                        let context_json = serde_json::to_value(&series_context)
                            .unwrap_or_else(|_| serde_json::json!({"title": base_query}));
                        match render_template(template, &context_json) {
                            Ok(q) => q,
                            Err(e) => {
                                debug!(
                                    "Task {}: Template rendering failed, using base query: {}",
                                    task.id, e
                                );
                                base_query.clone()
                            }
                        }
                    } else {
                        base_query.clone()
                    };

                // Apply preprocessing rules (plugin rules first, then library rules)
                let plugin_rules = PluginsRepository::get_search_preprocessing_rules(&plugin);
                let library_rules = LibraryRepository::get_preprocessing_rules(&library);

                let search_query =
                    apply_preprocessing_rules(&templated_query, &plugin_rules, &library_rules);

                debug!(
                    "Task {}: Search query: '{}' -> '{}' (template: {}, plugin_rules: {}, library_rules: {})",
                    task.id, base_query, search_query,
                    PluginsRepository::get_search_query_template(&plugin).is_some(),
                    plugin_rules.len(),
                    library_rules.len()
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
                            used_existing_external_id: false,
                            search_query_used: Some(search_query),
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

                // Get confidence threshold from settings
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
                                external_url: None,
                                matched_title: Some(best_match.title.clone()),
                                fields_updated: vec![],
                                fields_skipped: vec![],
                                skipped_reason: Some("low_confidence".to_string()),
                                used_existing_external_id: false,
                                search_query_used: Some(search_query),
                            }),
                        ));
                    }
                }

                let ext_id = best_match.external_id.clone();
                let title = best_match.title.clone();

                info!("Task {}: Best match: '{}' (id: {})", task.id, title, ext_id);

                (ext_id, Some(title), Some(search_query))
            };

            // Fetch full metadata using the external ID
            let get_params = MetadataGetParams {
                external_id: external_id.clone(),
            };

            let plugin_metadata = self
                .plugin_manager
                .get_series_metadata(plugin_id, get_params)
                .await
                .context("Failed to fetch full metadata")?;

            let external_url = plugin_metadata.external_url.clone();
            let final_matched_title = matched_title.unwrap_or_else(|| {
                plugin_metadata
                    .title
                    .clone()
                    .unwrap_or_else(|| "Unknown".to_string())
            });

            // Get current metadata for lock checking
            let current_metadata =
                SeriesMetadataRepository::get_by_series_id(db, series_id).await?;

            // Build apply options
            let options = ApplyOptions {
                fields_filter: None,
                thumbnail_service: self.thumbnail_service.clone(),
                event_broadcaster: event_broadcaster.cloned(),
            };

            // Apply metadata
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

            // Store/update external ID for future lookups
            if let Err(e) = SeriesExternalIdRepository::upsert_for_plugin(
                db,
                series_id,
                &plugin.name,
                &external_id,
                Some(&external_url),
                None, // metadata_hash - not calculated here
            )
            .await
            {
                tracing::warn!("Task {}: Failed to store external ID: {}", task.id, e);
            }

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
                matched_title: Some(final_matched_title.clone()),
                fields_updated: applied_fields.clone(),
                fields_skipped: skipped_fields,
                skipped_reason: None,
                used_existing_external_id,
                search_query_used,
            };

            let message = if applied_fields.is_empty() {
                format!(
                    "Matched '{}' but no fields were applied",
                    final_matched_title
                )
            } else {
                let method = if used_existing_external_id {
                    "via existing ID"
                } else {
                    "via search"
                };
                format!(
                    "Matched '{}' ({}) and applied {} field(s)",
                    final_matched_title,
                    method,
                    applied_fields.len()
                )
            };

            Ok(TaskResult::success_with_data(message, json!(result)))
        })
    }
}

/// Check if auto-match conditions are satisfied for a series
fn check_conditions(conditions: &AutoMatchConditions, context: &SeriesContext) -> bool {
    should_match(conditions, context)
}

/// Apply preprocessing rules to a search query
///
/// Plugin rules are applied first, then library rules.
fn apply_preprocessing_rules(
    query: &str,
    plugin_rules: &[PreprocessingRule],
    library_rules: &[PreprocessingRule],
) -> String {
    let mut result = query.to_string();

    // Apply plugin rules first
    if !plugin_rules.is_empty() {
        result = apply_rules(&result, plugin_rules);
    }

    // Then apply library rules
    if !library_rules.is_empty() {
        result = apply_rules(&result, library_rules);
    }

    result
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
            used_existing_external_id: false,
            search_query_used: Some("Test Series".to_string()),
        };

        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["matched"], true);
        assert_eq!(json["externalId"], "12345");
        assert_eq!(json["fieldsUpdated"].as_array().unwrap().len(), 2);
        assert_eq!(json["usedExistingExternalId"], false);
        assert_eq!(json["searchQueryUsed"], "Test Series");
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
            used_existing_external_id: false,
            search_query_used: None,
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
            used_existing_external_id: false,
            search_query_used: Some("Test Series".to_string()),
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
    fn test_existing_external_id_result() {
        // Result when an existing external ID was used for direct lookup
        let result = PluginAutoMatchResult {
            matched: true,
            external_id: Some("mangadex-12345".to_string()),
            external_url: Some("https://mangadex.org/title/12345".to_string()),
            matched_title: Some("One Piece".to_string()),
            fields_updated: vec!["summary".to_string()],
            fields_skipped: vec![],
            skipped_reason: None,
            used_existing_external_id: true, // Key difference
            search_query_used: None,         // No search was performed
        };

        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["matched"], true);
        assert_eq!(json["usedExistingExternalId"], true);
        assert!(json["searchQueryUsed"].is_null());
    }

    #[test]
    fn test_condition_not_met_result() {
        let result = PluginAutoMatchResult {
            matched: false,
            external_id: None,
            external_url: None,
            matched_title: None,
            fields_updated: vec![],
            fields_skipped: vec![],
            skipped_reason: Some("library_conditions_not_met".to_string()),
            used_existing_external_id: false,
            search_query_used: None,
        };

        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["matched"], false);
        assert_eq!(json["skippedReason"], "library_conditions_not_met");
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

    #[test]
    fn test_apply_preprocessing_rules() {
        use crate::services::metadata::preprocessing::PreprocessingRule;

        // Test with empty rules
        let result = apply_preprocessing_rules("One Piece (Digital)", &[], &[]);
        assert_eq!(result, "One Piece (Digital)");

        // Test with library rules only
        let library_rules = vec![PreprocessingRule::with_description(
            r"\s*\(Digital\)$",
            "",
            "Remove Digital suffix",
        )];
        let result = apply_preprocessing_rules("One Piece (Digital)", &[], &library_rules);
        assert_eq!(result, "One Piece");

        // Test with plugin rules only
        let plugin_rules = vec![PreprocessingRule::with_description(
            r"^Vol\.\s*\d+\s*-\s*",
            "",
            "Remove volume prefix",
        )];
        let result = apply_preprocessing_rules("Vol. 1 - One Piece", &plugin_rules, &[]);
        assert_eq!(result, "One Piece");

        // Test with both - plugin rules applied first
        let result = apply_preprocessing_rules(
            "Vol. 1 - One Piece (Digital)",
            &plugin_rules,
            &library_rules,
        );
        assert_eq!(result, "One Piece");
    }

    #[test]
    fn test_check_conditions() {
        use crate::services::metadata::preprocessing::{
            AutoMatchConditions, ConditionMode, ConditionOperator, ConditionRule, MetadataContext,
        };

        // Build a context with 100 books
        let metadata = MetadataContext {
            title: Some("One Piece".to_string()),
            year: Some(1999),
            status: Some("ongoing".to_string()),
            ..Default::default()
        };
        let context = SeriesContext::new().book_count(100).metadata(metadata);

        // Test condition that should pass
        let conditions = AutoMatchConditions::new(ConditionMode::All).with_rule(
            ConditionRule::with_value("book_count", ConditionOperator::Gte, serde_json::json!(1)),
        );
        assert!(check_conditions(&conditions, &context));

        // Test condition that should fail
        let conditions = AutoMatchConditions::new(ConditionMode::All).with_rule(
            ConditionRule::with_value("book_count", ConditionOperator::Lt, serde_json::json!(1)),
        );
        assert!(!check_conditions(&conditions, &context));
    }
}
