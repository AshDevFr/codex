//! Handler for UserPluginSync task
//!
//! Processes user plugin sync tasks by spawning the plugin process with
//! per-user credentials and calling sync methods (push/pull progress)
//! via JSON-RPC.

use anyhow::Result;
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::db::entities::tasks;
use crate::db::repositories::{
    BookRepository, ReadProgressRepository, SeriesExternalIdRepository, SeriesMetadataRepository,
    UserPluginDataRepository, UserPluginsRepository, UserSeriesRatingRepository,
};
use crate::events::EventBroadcaster;
use crate::services::plugin::PluginManager;
use crate::services::plugin::protocol::methods;
use crate::services::plugin::sync::{
    ExternalUserInfo, SyncEntry, SyncProgress, SyncPullRequest, SyncPullResponse, SyncPushRequest,
    SyncPushResponse, SyncReadingStatus,
};
use crate::tasks::handlers::TaskHandler;
use crate::tasks::types::TaskResult;

/// Configuration for push behavior, parsed from user plugin config.
#[derive(Debug, Clone)]
struct PushConfig {
    /// Whether each Codex book counts as a "volume" or a "chapter" in the
    /// external service. Defaults to `"volumes"`.
    progress_unit: String,
    /// Push series where all local books are marked as read. Default: true.
    push_completed_series: bool,
    /// Push series where at least one book has been started. Default: true.
    push_in_progress_series: bool,
    /// Count partially-read books in the progress count. Default: false.
    push_in_progress_volumes: bool,
    /// Include scores and notes in push/pull. Default: false.
    sync_ratings: bool,
}

impl PushConfig {
    /// Parse push configuration from the user plugin config JSON.
    fn from_user_config(config: &serde_json::Value) -> Self {
        Self {
            progress_unit: config
                .get("progressUnit")
                .and_then(|v| v.as_str())
                .unwrap_or("volumes")
                .to_string(),
            push_completed_series: config
                .get("pushCompletedSeries")
                .and_then(|v| v.as_bool())
                .unwrap_or(true),
            push_in_progress_series: config
                .get("pushInProgressSeries")
                .and_then(|v| v.as_bool())
                .unwrap_or(true),
            push_in_progress_volumes: config
                .get("pushInProgressVolumes")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            sync_ratings: config
                .get("syncRatings")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
        }
    }
}

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
            let sync_ratings = user_config
                .get("syncRatings")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            debug!(
                "Task {}: syncMode={} (pull={}, push={})",
                task.id, sync_mode, do_pull, do_push
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
                            let (matched, applied) = match_and_apply_pulled_entries(
                                db,
                                &pull_response.entries,
                                external_id_source.as_deref(),
                                user_id,
                                task.id,
                                sync_ratings,
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
                let push_config = PushConfig::from_user_config(&user_config);
                let entries = if let Some(ref source) = external_id_source {
                    build_push_entries(db, user_id, source, task.id, &push_config).await
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
                "last_sync_result",
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

/// Build push entries from a user's Codex reading progress.
///
/// For each series that has an external ID matching the given source,
/// aggregates book-level reading progress into a single `SyncEntry`.
/// Behaviour is controlled by `PushConfig` (progress unit, which series
/// to include, whether in-progress books count).
async fn build_push_entries(
    db: &DatabaseConnection,
    user_id: Uuid,
    external_id_source: &str,
    task_id: Uuid,
    config: &PushConfig,
) -> Vec<SyncEntry> {
    // Get all series that have external IDs for this source
    let external_ids =
        match SeriesExternalIdRepository::find_by_source(db, external_id_source).await {
            Ok(ids) => ids,
            Err(e) => {
                warn!(
                    "Task {}: Failed to fetch external IDs for source {}: {}",
                    task_id, external_id_source, e
                );
                return vec![];
            }
        };

    debug!(
        "Task {}: Found {} series with external IDs for source {}",
        task_id,
        external_ids.len(),
        external_id_source
    );

    if external_ids.is_empty() {
        return vec![];
    }

    // Batch-fetch all user ratings for efficient O(1) lookup per series
    let ratings_map: HashMap<Uuid, crate::db::entities::user_series_ratings::Model> =
        if config.sync_ratings {
            match UserSeriesRatingRepository::get_all_for_user(db, user_id).await {
                Ok(ratings) => ratings.into_iter().map(|r| (r.series_id, r)).collect(),
                Err(e) => {
                    warn!(
                        "Task {}: Failed to fetch user ratings for push: {}",
                        task_id, e
                    );
                    HashMap::new()
                }
            }
        } else {
            HashMap::new()
        };

    let mut entries = Vec::new();

    for ext_id in &external_ids {
        // Get ordered books for this series
        let books = match BookRepository::list_by_series(db, ext_id.series_id, false).await {
            Ok(b) => b,
            Err(e) => {
                warn!(
                    "Task {}: Failed to list books for series {}: {}",
                    task_id, ext_id.series_id, e
                );
                continue;
            }
        };

        if books.is_empty() {
            continue;
        }

        // Check reading progress for each book
        let mut completed_count: i32 = 0;
        let mut in_progress_count: i32 = 0;
        let mut has_any_progress = false;
        let mut earliest_started: Option<chrono::DateTime<chrono::Utc>> = None;
        let mut latest_completed_at: Option<chrono::DateTime<chrono::Utc>> = None;

        for book in &books {
            match ReadProgressRepository::get_by_user_and_book(db, user_id, book.id).await {
                Ok(Some(progress)) => {
                    has_any_progress = true;
                    if progress.completed {
                        completed_count += 1;
                        if let Some(cat) = progress.completed_at {
                            latest_completed_at = Some(match latest_completed_at {
                                Some(existing) if cat > existing => cat,
                                Some(existing) => existing,
                                None => cat,
                            });
                        }
                    } else {
                        in_progress_count += 1;
                    }
                    earliest_started = Some(match earliest_started {
                        Some(existing) if progress.started_at < existing => progress.started_at,
                        Some(existing) => existing,
                        None => progress.started_at,
                    });
                }
                Ok(None) => {}
                Err(e) => {
                    warn!(
                        "Task {}: Failed to get progress for book {}: {}",
                        task_id, book.id, e
                    );
                }
            }
        }

        // Skip series with no progress at all
        if !has_any_progress {
            debug!(
                "Task {}: Skipping series {} (ext_id={}) — no reading progress",
                task_id, ext_id.series_id, ext_id.external_id
            );
            continue;
        }

        let all_completed = completed_count == books.len() as i32;
        let is_in_progress = !all_completed;

        // Apply push config filters
        if all_completed && !config.push_completed_series {
            debug!(
                "Task {}: Skipping series {} (ext_id={}) — completed but pushCompletedSeries=false",
                task_id, ext_id.series_id, ext_id.external_id
            );
            continue;
        }
        if is_in_progress && !config.push_in_progress_series {
            debug!(
                "Task {}: Skipping series {} (ext_id={}) — in-progress but pushInProgressSeries=false",
                task_id, ext_id.series_id, ext_id.external_id
            );
            continue;
        }

        // Calculate progress count based on config
        let progress_count = if config.push_in_progress_volumes {
            completed_count + in_progress_count
        } else {
            completed_count
        };

        debug!(
            "Task {}: Series {} (ext_id={}): {}/{} books completed, {} in-progress, progress_count={}",
            task_id,
            ext_id.series_id,
            ext_id.external_id,
            completed_count,
            books.len(),
            in_progress_count,
            progress_count,
        );

        // Mark as Completed only when:
        // 1. All local books are read, AND
        // 2. The series has a known total_book_count in metadata, AND
        // 3. completed_count >= total_book_count
        // Otherwise default to Reading — we can't be sure the library is complete.
        let status = if all_completed {
            let is_truly_complete =
                match SeriesMetadataRepository::get_by_series_id(db, ext_id.series_id).await {
                    Ok(Some(meta)) => meta
                        .total_book_count
                        .is_some_and(|total| total > 0 && completed_count >= total),
                    _ => false,
                };
            if is_truly_complete {
                SyncReadingStatus::Completed
            } else {
                SyncReadingStatus::Reading
            }
        } else {
            SyncReadingStatus::Reading
        };

        // Use the configured progress unit (volumes or chapters).
        // When using "volumes", we only send `progressVolumes` — sending
        // `progress` (chapters) would be incorrect because AniList treats
        // that as chapter count, causing misleading "Read chapter X" activity.
        let progress = if config.progress_unit == "chapters" {
            SyncProgress {
                chapters: Some(progress_count),
                volumes: None,
                pages: None,
            }
        } else {
            SyncProgress {
                chapters: None,
                volumes: Some(progress_count),
                pages: None,
            }
        };

        // Look up rating/notes if sync_ratings is enabled
        let (score, notes) = if config.sync_ratings {
            match ratings_map.get(&ext_id.series_id) {
                Some(r) => (Some(r.rating as f64), r.notes.clone()),
                None => (None, None),
            }
        } else {
            (None, None)
        };

        entries.push(SyncEntry {
            external_id: ext_id.external_id.clone(),
            status: status.clone(),
            progress: Some(progress),
            score,
            started_at: earliest_started.map(|dt| dt.to_rfc3339()),
            completed_at: if status == SyncReadingStatus::Completed {
                latest_completed_at.map(|dt| dt.to_rfc3339())
            } else {
                None
            },
            notes,
        });
    }

    debug!(
        "Task {}: Built {} push entries from {} series with external IDs",
        task_id,
        entries.len(),
        external_ids.len()
    );

    entries
}

/// Match pulled sync entries to Codex series using external IDs and apply
/// reading progress.
///
/// For each pulled entry, looks up `series_external_ids` where
/// `source = external_id_source` and `external_id = entry.external_id`.
/// When a match is found, applies the pulled reading progress to the user's
/// Codex books (each book = 1 chapter).
///
/// Returns `(matched, applied)` — matched entries count and books updated.
async fn match_and_apply_pulled_entries(
    db: &DatabaseConnection,
    entries: &[SyncEntry],
    external_id_source: Option<&str>,
    user_id: Uuid,
    task_id: Uuid,
    sync_ratings: bool,
) -> (u32, u32) {
    let Some(source) = external_id_source else {
        debug!(
            "Task {}: No externalIdSource configured, skipping entry matching",
            task_id
        );
        return (0, 0);
    };

    if entries.is_empty() {
        return (0, 0);
    }

    let mut matched: u32 = 0;
    let mut unmatched: u32 = 0;
    let mut applied: u32 = 0;

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

                // Apply reading progress to the matched series
                let books_applied =
                    apply_pulled_entry(db, user_id, ext_id.series_id, entry, task_id).await;
                applied += books_applied;

                // Apply pulled rating/notes if enabled and Codex has no existing rating
                if sync_ratings && let Some(pulled_score) = entry.score {
                    let score_i32 = (pulled_score.round() as i32).clamp(1, 100);
                    match UserSeriesRatingRepository::get_by_user_and_series(
                        db,
                        user_id,
                        ext_id.series_id,
                    )
                    .await
                    {
                        Ok(None) => {
                            if let Err(e) = UserSeriesRatingRepository::upsert(
                                db,
                                user_id,
                                ext_id.series_id,
                                score_i32,
                                entry.notes.clone(),
                            )
                            .await
                            {
                                warn!(
                                    "Task {}: Failed to apply pulled rating for series {}: {}",
                                    task_id, ext_id.series_id, e
                                );
                            }
                        }
                        Ok(Some(_)) => {
                            debug!(
                                "Task {}: Skipping pulled rating for series {} — Codex already has a rating",
                                task_id, ext_id.series_id
                            );
                        }
                        Err(e) => {
                            warn!(
                                "Task {}: Failed to check existing rating for series {}: {}",
                                task_id, ext_id.series_id, e
                            );
                        }
                    }
                }
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

    (matched, applied)
}

/// Apply a single pulled entry's reading progress to a Codex series.
///
/// Maps chapters_read from the external service to books in the series:
/// - If status is Completed → mark ALL books as read
/// - Otherwise → mark the first `chapters_read` books as read
///
/// Only marks books that aren't already completed. Returns the number of
/// books newly marked as read.
async fn apply_pulled_entry(
    db: &DatabaseConnection,
    user_id: Uuid,
    series_id: Uuid,
    entry: &SyncEntry,
    task_id: Uuid,
) -> u32 {
    // Get ordered books for this series
    let books = match BookRepository::list_by_series(db, series_id, false).await {
        Ok(b) => b,
        Err(e) => {
            warn!(
                "Task {}: Failed to list books for series {} during pull apply: {}",
                task_id, series_id, e
            );
            return 0;
        }
    };

    if books.is_empty() {
        return 0;
    }

    // Use volumes if available, fall back to chapters
    let units_read = entry
        .progress
        .as_ref()
        .and_then(|p| p.volumes.or(p.chapters))
        .unwrap_or(0);

    // Determine which books to mark as read
    let books_to_mark = if entry.status == SyncReadingStatus::Completed {
        // Mark all books as read
        &books[..]
    } else if units_read > 0 {
        // Mark first N books as read (each book = 1 volume/chapter)
        let n = (units_read as usize).min(books.len());
        &books[..n]
    } else {
        // No progress units and not completed — nothing to apply
        return 0;
    };

    let mut newly_applied: u32 = 0;

    for book in books_to_mark {
        // Check if already completed — skip if so
        match ReadProgressRepository::get_by_user_and_book(db, user_id, book.id).await {
            Ok(Some(progress)) if progress.completed => {
                continue; // Already read, skip
            }
            Ok(_) => {} // Not completed or no record — will mark
            Err(e) => {
                warn!(
                    "Task {}: Failed to check progress for book {}: {}",
                    task_id, book.id, e
                );
                continue;
            }
        }

        // Mark as read
        match ReadProgressRepository::mark_as_read(db, user_id, book.id, book.page_count).await {
            Ok(_) => {
                newly_applied += 1;
            }
            Err(e) => {
                warn!(
                    "Task {}: Failed to mark book {} as read: {}",
                    task_id, book.id, e
                );
            }
        }
    }

    newly_applied
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::ScanningStrategy;
    use crate::db::entities::{books, users};
    use crate::db::repositories::{
        BookRepository, LibraryRepository, SeriesRepository, UserRepository,
        UserSeriesRatingRepository,
    };
    use crate::db::test_helpers::create_test_db;
    use crate::services::plugin::sync::{SyncProgress, SyncReadingStatus};
    use chrono::Utc;

    /// Helper to create a test user in the database
    async fn create_test_user(db: &DatabaseConnection) -> users::Model {
        let user = users::Model {
            id: Uuid::new_v4(),
            username: format!("syncuser_{}", Uuid::new_v4()),
            email: format!("sync_{}@example.com", Uuid::new_v4()),
            password_hash: "hash123".to_string(),
            role: "reader".to_string(),
            is_active: true,
            email_verified: false,
            permissions: serde_json::json!([]),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            last_login_at: None,
        };
        UserRepository::create(db, &user).await.unwrap()
    }

    /// Helper to create a book in a series with a given page count
    async fn create_test_book(
        db: &DatabaseConnection,
        series_id: Uuid,
        library_id: Uuid,
        index: usize,
        page_count: i32,
    ) -> books::Model {
        let book = books::Model {
            id: Uuid::new_v4(),
            series_id,
            library_id,
            file_path: format!("/test/book_{}_{}.cbz", index, Uuid::new_v4()),
            file_name: format!("book_{}.cbz", index),
            file_size: 1024,
            file_hash: format!("hash_{}_{}", index, Uuid::new_v4()),
            partial_hash: String::new(),
            format: "cbz".to_string(),
            page_count,
            deleted: false,
            analyzed: false,
            analysis_error: None,
            analysis_errors: None,
            modified_at: Utc::now(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            thumbnail_path: None,
            thumbnail_generated_at: None,
        };
        BookRepository::create(db, &book, None).await.unwrap()
    }

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
            applied: 6,
            push_failures: 1,
            pull_incomplete: false,
            pull_error: None,
            push_error: None,
            skipped_reason: None,
        };

        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["externalUsername"], "manga_reader");
        assert_eq!(json["pushed"], 5);
        assert_eq!(json["pulled"], 10);
        assert_eq!(json["matched"], 8);
        assert_eq!(json["applied"], 6);
        assert_eq!(json["pushFailures"], 1);
        assert!(!json["pullIncomplete"].as_bool().unwrap());
        assert!(!json.as_object().unwrap().contains_key("skippedReason"));
        assert!(!json.as_object().unwrap().contains_key("pullError"));
        assert!(!json.as_object().unwrap().contains_key("pushError"));
    }

    #[test]
    fn test_sync_result_with_errors() {
        let result = UserPluginSyncResult {
            plugin_id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            external_username: Some("user".to_string()),
            pushed: 3,
            pulled: 0,
            matched: 0,
            applied: 0,
            push_failures: 0,
            pull_incomplete: false,
            pull_error: Some("AniList API error: 400 Bad Request".to_string()),
            push_error: None,
            skipped_reason: None,
        };

        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["pullError"], "AniList API error: 400 Bad Request");
        assert!(!json.as_object().unwrap().contains_key("pushError"));
        assert_eq!(json["pushed"], 3);
        assert_eq!(json["pulled"], 0);

        // Round-trip
        let deserialized: UserPluginSyncResult = serde_json::from_value(json).unwrap();
        assert_eq!(
            deserialized.pull_error,
            Some("AniList API error: 400 Bad Request".to_string())
        );
        assert!(deserialized.push_error.is_none());
    }

    #[test]
    fn test_sync_result_with_both_errors() {
        let result = UserPluginSyncResult {
            plugin_id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            external_username: None,
            pushed: 0,
            pulled: 0,
            matched: 0,
            applied: 0,
            push_failures: 0,
            pull_incomplete: false,
            pull_error: Some("Pull failed".to_string()),
            push_error: Some("Push failed".to_string()),
            skipped_reason: None,
        };

        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["pullError"], "Pull failed");
        assert_eq!(json["pushError"], "Push failed");
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
            applied: 0,
            push_failures: 0,
            pull_incomplete: false,
            pull_error: None,
            push_error: None,
            skipped_reason: Some("plugin_not_enabled".to_string()),
        };

        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["skippedReason"], "plugin_not_enabled");
        assert!(!json.as_object().unwrap().contains_key("externalUsername"));
        assert_eq!(json["pushed"], 0);
        assert_eq!(json["pulled"], 0);
        assert_eq!(json["matched"], 0);
        assert_eq!(json["applied"], 0);
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
            "applied": 4,
            "pushFailures": 0,
            "pullIncomplete": true,
        });

        let result: UserPluginSyncResult = serde_json::from_value(json).unwrap();
        assert_eq!(result.external_username, Some("test_user".to_string()));
        assert_eq!(result.pushed, 3);
        assert_eq!(result.pulled, 7);
        assert_eq!(result.matched, 5);
        assert_eq!(result.applied, 4);
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
            applied: 250,
            push_failures: 0,
            pull_incomplete: true,
            pull_error: None,
            push_error: None,
            skipped_reason: None,
        };

        let json = serde_json::to_value(&result).unwrap();
        assert!(json["pullIncomplete"].as_bool().unwrap());
        assert_eq!(json["pulled"], 500);
        assert_eq!(json["matched"], 300);
        assert_eq!(json["applied"], 250);
    }

    #[test]
    fn test_sync_result_applied_field() {
        let result = UserPluginSyncResult {
            plugin_id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            external_username: None,
            pushed: 0,
            pulled: 10,
            matched: 5,
            applied: 3,
            push_failures: 0,
            pull_incomplete: false,
            pull_error: None,
            push_error: None,
            skipped_reason: None,
        };

        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["applied"], 3);

        // Verify round-trip
        let deserialized: UserPluginSyncResult = serde_json::from_value(json).unwrap();
        assert_eq!(deserialized.applied, 3);
    }

    #[tokio::test]
    async fn test_match_and_apply_no_source() {
        let (db, _temp_dir) = create_test_db().await;
        let user_id = Uuid::new_v4();

        let entries = vec![SyncEntry {
            external_id: "12345".to_string(),
            status: SyncReadingStatus::Reading,
            progress: None,
            score: None,
            started_at: None,
            completed_at: None,
            notes: None,
        }];

        let (matched, applied) = match_and_apply_pulled_entries(
            db.sea_orm_connection(),
            &entries,
            None,
            user_id,
            Uuid::new_v4(),
            false,
        )
        .await;
        assert_eq!(matched, 0);
        assert_eq!(applied, 0);
    }

    #[tokio::test]
    async fn test_match_and_apply_with_matches() {
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

        let user_id = Uuid::new_v4();

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

        let (matched, _applied) = match_and_apply_pulled_entries(
            db.sea_orm_connection(),
            &entries,
            Some("api:anilist"),
            user_id,
            Uuid::new_v4(),
            false,
        )
        .await;
        assert_eq!(matched, 1);
    }

    #[tokio::test]
    async fn test_match_and_apply_pulled_entries_applies_progress() {
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
            SeriesRepository::create(db.sea_orm_connection(), library.id, "Test Manga", None)
                .await
                .unwrap();

        // Create 5 books in the series
        for i in 1..=5 {
            create_test_book(db.sea_orm_connection(), series.id, library.id, i, 100).await;
        }

        SeriesExternalIdRepository::create(
            db.sea_orm_connection(),
            series.id,
            "api:anilist",
            "42",
            None,
            None,
        )
        .await
        .unwrap();

        let user = create_test_user(db.sea_orm_connection()).await;
        let user_id = user.id;

        // Pull entry says 3 chapters read
        let entries = vec![SyncEntry {
            external_id: "42".to_string(),
            status: SyncReadingStatus::Reading,
            progress: Some(SyncProgress {
                chapters: Some(3),
                volumes: None,
                pages: None,
            }),
            score: None,
            started_at: None,
            completed_at: None,
            notes: None,
        }];

        let (matched, applied) = match_and_apply_pulled_entries(
            db.sea_orm_connection(),
            &entries,
            Some("api:anilist"),
            user_id,
            Uuid::new_v4(),
            false,
        )
        .await;
        assert_eq!(matched, 1);
        assert_eq!(applied, 3);

        // Verify: first 3 books should be marked as read
        let books_list = BookRepository::list_by_series(db.sea_orm_connection(), series.id, false)
            .await
            .unwrap();
        for (i, book) in books_list.iter().enumerate() {
            let progress = ReadProgressRepository::get_by_user_and_book(
                db.sea_orm_connection(),
                user_id,
                book.id,
            )
            .await
            .unwrap();
            if i < 3 {
                assert!(progress.is_some(), "Book {} should have progress", i);
                assert!(
                    progress.unwrap().completed,
                    "Book {} should be completed",
                    i
                );
            } else {
                assert!(progress.is_none(), "Book {} should have no progress", i);
            }
        }
    }

    #[tokio::test]
    async fn test_match_and_apply_skips_already_read() {
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
            SeriesRepository::create(db.sea_orm_connection(), library.id, "Test Manga", None)
                .await
                .unwrap();

        // Create 3 books
        let mut book_ids = Vec::new();
        for i in 1..=3 {
            let book =
                create_test_book(db.sea_orm_connection(), series.id, library.id, i, 50).await;
            book_ids.push(book.id);
        }

        let user = create_test_user(db.sea_orm_connection()).await;
        let user_id = user.id;

        // Pre-mark book 1 as read
        ReadProgressRepository::mark_as_read(db.sea_orm_connection(), user_id, book_ids[0], 50)
            .await
            .unwrap();

        SeriesExternalIdRepository::create(
            db.sea_orm_connection(),
            series.id,
            "api:anilist",
            "99",
            None,
            None,
        )
        .await
        .unwrap();

        // Pull says completed (all 3 chapters)
        let entries = vec![SyncEntry {
            external_id: "99".to_string(),
            status: SyncReadingStatus::Completed,
            progress: Some(SyncProgress {
                chapters: Some(3),
                volumes: None,
                pages: None,
            }),
            score: None,
            started_at: None,
            completed_at: None,
            notes: None,
        }];

        let (matched, applied) = match_and_apply_pulled_entries(
            db.sea_orm_connection(),
            &entries,
            Some("api:anilist"),
            user_id,
            Uuid::new_v4(),
            false,
        )
        .await;
        assert_eq!(matched, 1);
        // Only 2 books newly applied (book 1 was already read)
        assert_eq!(applied, 2);
    }

    /// Default push config for tests (matches production defaults)
    fn default_push_config() -> PushConfig {
        PushConfig {
            progress_unit: "volumes".to_string(),
            push_completed_series: true,
            push_in_progress_series: true,
            push_in_progress_volumes: false,
            sync_ratings: false,
        }
    }

    #[tokio::test]
    async fn test_build_push_entries_with_progress() {
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
            SeriesRepository::create(db.sea_orm_connection(), library.id, "Push Manga", None)
                .await
                .unwrap();

        // Create 4 books
        let mut test_books = Vec::new();
        for i in 1..=4 {
            let book =
                create_test_book(db.sea_orm_connection(), series.id, library.id, i, 100).await;
            test_books.push(book);
        }

        let user = create_test_user(db.sea_orm_connection()).await;
        let user_id = user.id;

        // Mark first 2 books as read
        ReadProgressRepository::mark_as_read(
            db.sea_orm_connection(),
            user_id,
            test_books[0].id,
            100,
        )
        .await
        .unwrap();
        ReadProgressRepository::mark_as_read(
            db.sea_orm_connection(),
            user_id,
            test_books[1].id,
            100,
        )
        .await
        .unwrap();

        SeriesExternalIdRepository::create(
            db.sea_orm_connection(),
            series.id,
            "api:anilist",
            "777",
            None,
            None,
        )
        .await
        .unwrap();

        let entries = build_push_entries(
            db.sea_orm_connection(),
            user_id,
            "api:anilist",
            Uuid::new_v4(),
            &default_push_config(),
        )
        .await;

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].external_id, "777");
        assert_eq!(entries[0].status, SyncReadingStatus::Reading);
        // "volumes" mode sends only volumes (not chapters, to avoid misleading activity)
        assert_eq!(entries[0].progress.as_ref().unwrap().volumes, Some(2));
        assert!(entries[0].progress.as_ref().unwrap().chapters.is_none());
    }

    #[tokio::test]
    async fn test_build_push_entries_all_completed() {
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
            SeriesRepository::create(db.sea_orm_connection(), library.id, "Done Manga", None)
                .await
                .unwrap();

        // Create 2 books
        let mut test_books = Vec::new();
        for i in 1..=2 {
            let book =
                create_test_book(db.sea_orm_connection(), series.id, library.id, i, 50).await;
            test_books.push(book);
        }

        let user = create_test_user(db.sea_orm_connection()).await;
        let user_id = user.id;

        // Mark all books as read
        for book in &test_books {
            ReadProgressRepository::mark_as_read(db.sea_orm_connection(), user_id, book.id, 50)
                .await
                .unwrap();
        }

        SeriesExternalIdRepository::create(
            db.sea_orm_connection(),
            series.id,
            "api:anilist",
            "888",
            None,
            None,
        )
        .await
        .unwrap();

        let entries = build_push_entries(
            db.sea_orm_connection(),
            user_id,
            "api:anilist",
            Uuid::new_v4(),
            &default_push_config(),
        )
        .await;

        assert_eq!(entries.len(), 1);
        // Always push as Reading — we can't know total chapter count from external service
        assert_eq!(entries[0].status, SyncReadingStatus::Reading);
        // "volumes" mode sends only volumes (not chapters, to avoid misleading activity)
        assert_eq!(entries[0].progress.as_ref().unwrap().volumes, Some(2));
        assert!(entries[0].progress.as_ref().unwrap().chapters.is_none());
        assert!(entries[0].completed_at.is_none());
    }

    #[tokio::test]
    async fn test_build_push_entries_skips_no_progress() {
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
            SeriesRepository::create(db.sea_orm_connection(), library.id, "Unread Manga", None)
                .await
                .unwrap();

        // Create a book with no progress
        create_test_book(db.sea_orm_connection(), series.id, library.id, 1, 100).await;

        SeriesExternalIdRepository::create(
            db.sea_orm_connection(),
            series.id,
            "api:anilist",
            "999",
            None,
            None,
        )
        .await
        .unwrap();

        let user_id = Uuid::new_v4();

        let entries = build_push_entries(
            db.sea_orm_connection(),
            user_id,
            "api:anilist",
            Uuid::new_v4(),
            &default_push_config(),
        )
        .await;

        // No progress → should skip
        assert!(entries.is_empty());
    }

    #[test]
    fn test_sync_mode_parsing_default_is_both() {
        // When config has no syncMode key, default to "both"
        let config = serde_json::json!({});
        let sync_mode = config
            .get("syncMode")
            .and_then(|v| v.as_str())
            .unwrap_or("both");
        assert_eq!(sync_mode, "both");
        let do_pull = sync_mode == "both" || sync_mode == "pull";
        let do_push = sync_mode == "both" || sync_mode == "push";
        assert!(do_pull);
        assert!(do_push);
    }

    #[test]
    fn test_sync_mode_parsing_pull_only() {
        let config = serde_json::json!({"syncMode": "pull"});
        let sync_mode = config
            .get("syncMode")
            .and_then(|v| v.as_str())
            .unwrap_or("both");
        assert_eq!(sync_mode, "pull");
        let do_pull = sync_mode == "both" || sync_mode == "pull";
        let do_push = sync_mode == "both" || sync_mode == "push";
        assert!(do_pull);
        assert!(!do_push);
    }

    #[test]
    fn test_sync_mode_parsing_push_only() {
        let config = serde_json::json!({"syncMode": "push"});
        let sync_mode = config
            .get("syncMode")
            .and_then(|v| v.as_str())
            .unwrap_or("both");
        assert_eq!(sync_mode, "push");
        let do_pull = sync_mode == "both" || sync_mode == "pull";
        let do_push = sync_mode == "both" || sync_mode == "push";
        assert!(!do_pull);
        assert!(do_push);
    }

    #[test]
    fn test_sync_mode_parsing_both_explicit() {
        let config = serde_json::json!({"syncMode": "both"});
        let sync_mode = config
            .get("syncMode")
            .and_then(|v| v.as_str())
            .unwrap_or("both");
        assert_eq!(sync_mode, "both");
        let do_pull = sync_mode == "both" || sync_mode == "pull";
        let do_push = sync_mode == "both" || sync_mode == "push";
        assert!(do_pull);
        assert!(do_push);
    }

    #[test]
    fn test_sync_mode_parsing_invalid_value_disables_both() {
        // An unrecognized syncMode value should disable both pull and push
        let config = serde_json::json!({"syncMode": "invalid"});
        let sync_mode = config
            .get("syncMode")
            .and_then(|v| v.as_str())
            .unwrap_or("both");
        assert_eq!(sync_mode, "invalid");
        let do_pull = sync_mode == "both" || sync_mode == "pull";
        let do_push = sync_mode == "both" || sync_mode == "push";
        assert!(!do_pull);
        assert!(!do_push);
    }

    #[test]
    fn test_sync_mode_parsing_non_string_falls_back_to_both() {
        // If syncMode is a non-string value, as_str() returns None → default "both"
        let config = serde_json::json!({"syncMode": 123});
        let sync_mode = config
            .get("syncMode")
            .and_then(|v| v.as_str())
            .unwrap_or("both");
        assert_eq!(sync_mode, "both");
    }

    #[tokio::test]
    async fn test_match_and_apply_empty() {
        let (db, _temp_dir) = create_test_db().await;

        let (matched, applied) = match_and_apply_pulled_entries(
            db.sea_orm_connection(),
            &[],
            Some("api:anilist"),
            Uuid::new_v4(),
            Uuid::new_v4(),
            false,
        )
        .await;
        assert_eq!(matched, 0);
        assert_eq!(applied, 0);
    }

    #[test]
    fn test_push_config_defaults() {
        let config = serde_json::json!({});
        let push_config = PushConfig::from_user_config(&config);
        assert_eq!(push_config.progress_unit, "volumes");
        assert!(push_config.push_completed_series);
        assert!(push_config.push_in_progress_series);
        assert!(!push_config.push_in_progress_volumes);
    }

    #[test]
    fn test_push_config_from_user_config() {
        let config = serde_json::json!({
            "progressUnit": "chapters",
            "pushCompletedSeries": false,
            "pushInProgressSeries": true,
            "pushInProgressVolumes": true,
        });
        let push_config = PushConfig::from_user_config(&config);
        assert_eq!(push_config.progress_unit, "chapters");
        assert!(!push_config.push_completed_series);
        assert!(push_config.push_in_progress_series);
        assert!(push_config.push_in_progress_volumes);
    }

    #[tokio::test]
    async fn test_build_push_entries_chapters_unit() {
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
            SeriesRepository::create(db.sea_orm_connection(), library.id, "Chapter Manga", None)
                .await
                .unwrap();

        let book = create_test_book(db.sea_orm_connection(), series.id, library.id, 1, 100).await;

        let user = create_test_user(db.sea_orm_connection()).await;

        ReadProgressRepository::mark_as_read(db.sea_orm_connection(), user.id, book.id, 100)
            .await
            .unwrap();

        SeriesExternalIdRepository::create(
            db.sea_orm_connection(),
            series.id,
            "api:anilist",
            "111",
            None,
            None,
        )
        .await
        .unwrap();

        let config = PushConfig {
            progress_unit: "chapters".to_string(),
            ..default_push_config()
        };

        let entries = build_push_entries(
            db.sea_orm_connection(),
            user.id,
            "api:anilist",
            Uuid::new_v4(),
            &config,
        )
        .await;

        assert_eq!(entries.len(), 1);
        // Should use chapters, not volumes
        assert_eq!(entries[0].progress.as_ref().unwrap().chapters, Some(1));
        assert!(entries[0].progress.as_ref().unwrap().volumes.is_none());
    }

    #[tokio::test]
    async fn test_build_push_entries_skip_completed_series() {
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
            SeriesRepository::create(db.sea_orm_connection(), library.id, "Done Manga 2", None)
                .await
                .unwrap();

        // Create 2 books, mark both as read (= completed)
        let mut test_books = Vec::new();
        for i in 1..=2 {
            let book =
                create_test_book(db.sea_orm_connection(), series.id, library.id, i, 50).await;
            test_books.push(book);
        }

        let user = create_test_user(db.sea_orm_connection()).await;
        for book in &test_books {
            ReadProgressRepository::mark_as_read(db.sea_orm_connection(), user.id, book.id, 50)
                .await
                .unwrap();
        }

        SeriesExternalIdRepository::create(
            db.sea_orm_connection(),
            series.id,
            "api:anilist",
            "222",
            None,
            None,
        )
        .await
        .unwrap();

        // Disable pushing completed series
        let config = PushConfig {
            push_completed_series: false,
            ..default_push_config()
        };

        let entries = build_push_entries(
            db.sea_orm_connection(),
            user.id,
            "api:anilist",
            Uuid::new_v4(),
            &config,
        )
        .await;

        assert!(entries.is_empty(), "Completed series should be skipped");
    }

    #[tokio::test]
    async fn test_build_push_entries_skip_in_progress_series() {
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
            SeriesRepository::create(db.sea_orm_connection(), library.id, "WIP Manga", None)
                .await
                .unwrap();

        // Create 3 books, mark only 1 as read (= in-progress)
        let mut test_books = Vec::new();
        for i in 1..=3 {
            let book =
                create_test_book(db.sea_orm_connection(), series.id, library.id, i, 50).await;
            test_books.push(book);
        }

        let user = create_test_user(db.sea_orm_connection()).await;
        ReadProgressRepository::mark_as_read(
            db.sea_orm_connection(),
            user.id,
            test_books[0].id,
            50,
        )
        .await
        .unwrap();

        SeriesExternalIdRepository::create(
            db.sea_orm_connection(),
            series.id,
            "api:anilist",
            "333",
            None,
            None,
        )
        .await
        .unwrap();

        // Disable pushing in-progress series
        let config = PushConfig {
            push_in_progress_series: false,
            ..default_push_config()
        };

        let entries = build_push_entries(
            db.sea_orm_connection(),
            user.id,
            "api:anilist",
            Uuid::new_v4(),
            &config,
        )
        .await;

        assert!(entries.is_empty(), "In-progress series should be skipped");
    }

    #[tokio::test]
    async fn test_build_push_entries_count_in_progress_volumes() {
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
            SeriesRepository::create(db.sea_orm_connection(), library.id, "IP Manga", None)
                .await
                .unwrap();

        // Create 4 books
        let mut test_books = Vec::new();
        for i in 1..=4 {
            let book =
                create_test_book(db.sea_orm_connection(), series.id, library.id, i, 100).await;
            test_books.push(book);
        }

        let user = create_test_user(db.sea_orm_connection()).await;

        // Mark book 1 as fully read
        ReadProgressRepository::mark_as_read(
            db.sea_orm_connection(),
            user.id,
            test_books[0].id,
            100,
        )
        .await
        .unwrap();

        // Mark book 2 as partially read (in-progress)
        ReadProgressRepository::upsert(
            db.sea_orm_connection(),
            user.id,
            test_books[1].id,
            50,    // current_page
            false, // not completed
        )
        .await
        .unwrap();

        SeriesExternalIdRepository::create(
            db.sea_orm_connection(),
            series.id,
            "api:anilist",
            "444",
            None,
            None,
        )
        .await
        .unwrap();

        // Without in-progress volumes: should count only completed (1)
        let config_no_ip = default_push_config();
        let entries = build_push_entries(
            db.sea_orm_connection(),
            user.id,
            "api:anilist",
            Uuid::new_v4(),
            &config_no_ip,
        )
        .await;
        assert_eq!(entries.len(), 1);
        // "volumes" mode sends only volumes (not chapters, to avoid misleading activity)
        assert_eq!(entries[0].progress.as_ref().unwrap().volumes, Some(1));
        assert!(entries[0].progress.as_ref().unwrap().chapters.is_none());

        // With in-progress volumes: should count completed + in-progress (2)
        let config_with_ip = PushConfig {
            push_in_progress_volumes: true,
            ..default_push_config()
        };
        let entries = build_push_entries(
            db.sea_orm_connection(),
            user.id,
            "api:anilist",
            Uuid::new_v4(),
            &config_with_ip,
        )
        .await;
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].progress.as_ref().unwrap().volumes, Some(2));
        assert!(entries[0].progress.as_ref().unwrap().chapters.is_none());
    }

    #[tokio::test]
    async fn test_apply_pulled_entry_uses_volumes() {
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
            SeriesRepository::create(db.sea_orm_connection(), library.id, "Vol Manga", None)
                .await
                .unwrap();

        // Create 5 books
        for i in 1..=5 {
            create_test_book(db.sea_orm_connection(), series.id, library.id, i, 100).await;
        }

        let user = create_test_user(db.sea_orm_connection()).await;

        // Pull entry with volumes=2 (no chapters)
        let entry = SyncEntry {
            external_id: "55".to_string(),
            status: SyncReadingStatus::Reading,
            progress: Some(SyncProgress {
                chapters: None,
                volumes: Some(2),
                pages: None,
            }),
            score: None,
            started_at: None,
            completed_at: None,
            notes: None,
        };

        let applied = apply_pulled_entry(
            db.sea_orm_connection(),
            user.id,
            series.id,
            &entry,
            Uuid::new_v4(),
        )
        .await;
        assert_eq!(applied, 2);

        // Verify first 2 books are marked as read
        let books = BookRepository::list_by_series(db.sea_orm_connection(), series.id, false)
            .await
            .unwrap();
        for (i, book) in books.iter().enumerate() {
            let progress = ReadProgressRepository::get_by_user_and_book(
                db.sea_orm_connection(),
                user.id,
                book.id,
            )
            .await
            .unwrap();
            if i < 2 {
                assert!(progress.is_some(), "Book {} should have progress", i);
                assert!(
                    progress.unwrap().completed,
                    "Book {} should be completed",
                    i
                );
            } else {
                assert!(progress.is_none(), "Book {} should have no progress", i);
            }
        }
    }

    // =========================================================================
    // Rating sync tests
    // =========================================================================

    #[test]
    fn test_push_config_sync_ratings_default() {
        let config = serde_json::json!({});
        let push_config = PushConfig::from_user_config(&config);
        assert!(!push_config.sync_ratings);
    }

    #[test]
    fn test_push_config_sync_ratings_enabled() {
        let config = serde_json::json!({"syncRatings": true});
        let push_config = PushConfig::from_user_config(&config);
        assert!(push_config.sync_ratings);
    }

    #[tokio::test]
    async fn test_build_push_entries_includes_rating() {
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
            SeriesRepository::create(db.sea_orm_connection(), library.id, "Rated Manga", None)
                .await
                .unwrap();

        let book = create_test_book(db.sea_orm_connection(), series.id, library.id, 1, 100).await;

        let user = create_test_user(db.sea_orm_connection()).await;

        ReadProgressRepository::mark_as_read(db.sea_orm_connection(), user.id, book.id, 100)
            .await
            .unwrap();

        // Create a rating for this series
        UserSeriesRatingRepository::create(
            db.sea_orm_connection(),
            user.id,
            series.id,
            85,
            Some("Excellent manga!".to_string()),
        )
        .await
        .unwrap();

        SeriesExternalIdRepository::create(
            db.sea_orm_connection(),
            series.id,
            "api:anilist",
            "555",
            None,
            None,
        )
        .await
        .unwrap();

        let config = PushConfig {
            sync_ratings: true,
            ..default_push_config()
        };

        let entries = build_push_entries(
            db.sea_orm_connection(),
            user.id,
            "api:anilist",
            Uuid::new_v4(),
            &config,
        )
        .await;

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].score, Some(85.0));
        assert_eq!(entries[0].notes, Some("Excellent manga!".to_string()));
    }

    #[tokio::test]
    async fn test_build_push_entries_no_rating_when_disabled() {
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
            SeriesRepository::create(db.sea_orm_connection(), library.id, "Rated Manga 2", None)
                .await
                .unwrap();

        let book = create_test_book(db.sea_orm_connection(), series.id, library.id, 1, 100).await;

        let user = create_test_user(db.sea_orm_connection()).await;

        ReadProgressRepository::mark_as_read(db.sea_orm_connection(), user.id, book.id, 100)
            .await
            .unwrap();

        // Create a rating, but sync_ratings is false
        UserSeriesRatingRepository::create(db.sea_orm_connection(), user.id, series.id, 85, None)
            .await
            .unwrap();

        SeriesExternalIdRepository::create(
            db.sea_orm_connection(),
            series.id,
            "api:anilist",
            "556",
            None,
            None,
        )
        .await
        .unwrap();

        let entries = build_push_entries(
            db.sea_orm_connection(),
            user.id,
            "api:anilist",
            Uuid::new_v4(),
            &default_push_config(), // sync_ratings=false
        )
        .await;

        assert_eq!(entries.len(), 1);
        assert!(entries[0].score.is_none());
        assert!(entries[0].notes.is_none());
    }

    #[tokio::test]
    async fn test_build_push_entries_no_rating_for_unrated() {
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
            SeriesRepository::create(db.sea_orm_connection(), library.id, "Unrated Manga", None)
                .await
                .unwrap();

        let book = create_test_book(db.sea_orm_connection(), series.id, library.id, 1, 100).await;

        let user = create_test_user(db.sea_orm_connection()).await;

        ReadProgressRepository::mark_as_read(db.sea_orm_connection(), user.id, book.id, 100)
            .await
            .unwrap();

        // No rating created

        SeriesExternalIdRepository::create(
            db.sea_orm_connection(),
            series.id,
            "api:anilist",
            "557",
            None,
            None,
        )
        .await
        .unwrap();

        let config = PushConfig {
            sync_ratings: true,
            ..default_push_config()
        };

        let entries = build_push_entries(
            db.sea_orm_connection(),
            user.id,
            "api:anilist",
            Uuid::new_v4(),
            &config,
        )
        .await;

        assert_eq!(entries.len(), 1);
        assert!(entries[0].score.is_none());
        assert!(entries[0].notes.is_none());
    }

    #[tokio::test]
    async fn test_apply_pulled_rating_no_existing() {
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
            SeriesRepository::create(db.sea_orm_connection(), library.id, "Pull Manga", None)
                .await
                .unwrap();

        create_test_book(db.sea_orm_connection(), series.id, library.id, 1, 100).await;

        let user = create_test_user(db.sea_orm_connection()).await;

        SeriesExternalIdRepository::create(
            db.sea_orm_connection(),
            series.id,
            "api:anilist",
            "600",
            None,
            None,
        )
        .await
        .unwrap();

        let entries = vec![SyncEntry {
            external_id: "600".to_string(),
            status: SyncReadingStatus::Reading,
            progress: Some(SyncProgress {
                chapters: Some(1),
                volumes: None,
                pages: None,
            }),
            score: Some(75.0),
            started_at: None,
            completed_at: None,
            notes: Some("Good so far".to_string()),
        }];

        let (matched, _applied) = match_and_apply_pulled_entries(
            db.sea_orm_connection(),
            &entries,
            Some("api:anilist"),
            user.id,
            Uuid::new_v4(),
            true, // sync_ratings=true
        )
        .await;

        assert_eq!(matched, 1);

        // Verify rating was created
        let rating = UserSeriesRatingRepository::get_by_user_and_series(
            db.sea_orm_connection(),
            user.id,
            series.id,
        )
        .await
        .unwrap();
        assert!(rating.is_some());
        let rating = rating.unwrap();
        assert_eq!(rating.rating, 75);
        assert_eq!(rating.notes, Some("Good so far".to_string()));
    }

    #[tokio::test]
    async fn test_apply_pulled_rating_existing_not_overwritten() {
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
            SeriesRepository::create(db.sea_orm_connection(), library.id, "Rated Manga 3", None)
                .await
                .unwrap();

        create_test_book(db.sea_orm_connection(), series.id, library.id, 1, 100).await;

        let user = create_test_user(db.sea_orm_connection()).await;

        // Pre-create a Codex rating
        UserSeriesRatingRepository::create(
            db.sea_orm_connection(),
            user.id,
            series.id,
            90,
            Some("My notes".to_string()),
        )
        .await
        .unwrap();

        SeriesExternalIdRepository::create(
            db.sea_orm_connection(),
            series.id,
            "api:anilist",
            "601",
            None,
            None,
        )
        .await
        .unwrap();

        // Pull entry with different score
        let entries = vec![SyncEntry {
            external_id: "601".to_string(),
            status: SyncReadingStatus::Reading,
            progress: Some(SyncProgress {
                chapters: Some(1),
                volumes: None,
                pages: None,
            }),
            score: Some(60.0),
            started_at: None,
            completed_at: None,
            notes: Some("AniList notes".to_string()),
        }];

        let (_matched, _applied) = match_and_apply_pulled_entries(
            db.sea_orm_connection(),
            &entries,
            Some("api:anilist"),
            user.id,
            Uuid::new_v4(),
            true,
        )
        .await;

        // Verify Codex rating was NOT overwritten
        let rating = UserSeriesRatingRepository::get_by_user_and_series(
            db.sea_orm_connection(),
            user.id,
            series.id,
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(rating.rating, 90); // Original Codex rating preserved
        assert_eq!(rating.notes, Some("My notes".to_string()));
    }

    #[tokio::test]
    async fn test_apply_pulled_rating_disabled() {
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
            SeriesRepository::create(db.sea_orm_connection(), library.id, "No Sync Manga", None)
                .await
                .unwrap();

        create_test_book(db.sea_orm_connection(), series.id, library.id, 1, 100).await;

        let user = create_test_user(db.sea_orm_connection()).await;

        SeriesExternalIdRepository::create(
            db.sea_orm_connection(),
            series.id,
            "api:anilist",
            "602",
            None,
            None,
        )
        .await
        .unwrap();

        let entries = vec![SyncEntry {
            external_id: "602".to_string(),
            status: SyncReadingStatus::Reading,
            progress: Some(SyncProgress {
                chapters: Some(1),
                volumes: None,
                pages: None,
            }),
            score: Some(80.0),
            started_at: None,
            completed_at: None,
            notes: Some("Should not be stored".to_string()),
        }];

        let (_matched, _applied) = match_and_apply_pulled_entries(
            db.sea_orm_connection(),
            &entries,
            Some("api:anilist"),
            user.id,
            Uuid::new_v4(),
            false, // sync_ratings=false
        )
        .await;

        // Verify no rating was created
        let rating = UserSeriesRatingRepository::get_by_user_and_series(
            db.sea_orm_connection(),
            user.id,
            series.id,
        )
        .await
        .unwrap();
        assert!(rating.is_none());
    }
}
