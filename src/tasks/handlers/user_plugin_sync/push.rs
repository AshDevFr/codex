//! Push operations — build entries from local reading progress to push to
//! external services.

use sea_orm::DatabaseConnection;
use std::collections::HashMap;
use tracing::{debug, warn};
use uuid::Uuid;

use crate::db::repositories::{
    BookRepository, ReadProgressRepository, SeriesExternalIdRepository, SeriesMetadataRepository,
    UserSeriesRatingRepository,
};
use crate::services::plugin::sync::{SyncEntry, SyncProgress, SyncReadingStatus};

use super::settings::CodexSyncSettings;

/// Build push entries from a user's Codex reading progress.
///
/// For each series that has an external ID matching the given source,
/// aggregates book-level reading progress into a single `SyncEntry`.
/// Behaviour is controlled by `CodexSyncSettings` (which series to
/// include, whether partial-progress books count, ratings).
pub(crate) async fn build_push_entries(
    db: &DatabaseConnection,
    user_id: Uuid,
    external_id_source: &str,
    task_id: Uuid,
    settings: &CodexSyncSettings,
) -> Vec<SyncEntry> {
    // 1. Get all series that have external IDs for this source (1 query)
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

    // Collect all series IDs for batch queries
    let series_ids: Vec<Uuid> = external_ids.iter().map(|e| e.series_id).collect();

    // 2. Batch-fetch all books grouped by series (1 query instead of N)
    let books_map = match BookRepository::get_by_series_ids(db, &series_ids).await {
        Ok(map) => map,
        Err(e) => {
            warn!(
                "Task {}: Failed to batch-fetch books for {} series: {}",
                task_id,
                series_ids.len(),
                e
            );
            return vec![];
        }
    };

    // Collect all book IDs for batch progress lookup
    let all_book_ids: Vec<Uuid> = books_map.values().flatten().map(|b| b.id).collect();

    // 3. Batch-fetch all reading progress for these books (1 query instead of N*M)
    let progress_map =
        match ReadProgressRepository::get_for_user_books(db, user_id, &all_book_ids).await {
            Ok(map) => map,
            Err(e) => {
                warn!(
                    "Task {}: Failed to batch-fetch reading progress: {}",
                    task_id, e
                );
                HashMap::new()
            }
        };

    // 4. Batch-fetch all series metadata (1 query instead of N)
    let metadata_map = match SeriesMetadataRepository::get_by_series_ids(db, &series_ids).await {
        Ok(map) => map,
        Err(e) => {
            warn!(
                "Task {}: Failed to batch-fetch series metadata: {}",
                task_id, e
            );
            HashMap::new()
        }
    };

    // 5. Batch-fetch all user ratings (1 query — already batched)
    let ratings_map: HashMap<Uuid, crate::db::entities::user_series_ratings::Model> =
        if settings.sync_ratings {
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

    // Now iterate using in-memory lookups only — zero additional queries
    let mut entries = Vec::new();

    for ext_id in &external_ids {
        let books = match books_map.get(&ext_id.series_id) {
            Some(b) if !b.is_empty() => b,
            _ => continue,
        };

        // Check reading progress for each book using the pre-fetched map
        let mut completed_count: i32 = 0;
        let mut in_progress_count: i32 = 0;
        let mut has_any_progress = false;
        let mut earliest_started: Option<chrono::DateTime<chrono::Utc>> = None;
        let mut latest_completed_at: Option<chrono::DateTime<chrono::Utc>> = None;
        let mut latest_updated_at: Option<chrono::DateTime<chrono::Utc>> = None;

        for book in books {
            if let Some(progress) = progress_map.get(&book.id) {
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
                latest_updated_at = Some(match latest_updated_at {
                    Some(existing) if progress.updated_at > existing => progress.updated_at,
                    Some(existing) => existing,
                    None => progress.updated_at,
                });
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

        // Apply Codex sync settings filters
        if all_completed && !settings.include_completed {
            debug!(
                "Task {}: Skipping series {} (ext_id={}) — completed but includeCompleted=false",
                task_id, ext_id.series_id, ext_id.external_id
            );
            continue;
        }
        if is_in_progress && !settings.include_in_progress {
            debug!(
                "Task {}: Skipping series {} (ext_id={}) — in-progress but includeInProgress=false",
                task_id, ext_id.series_id, ext_id.external_id
            );
            continue;
        }

        // Calculate progress count based on settings
        let progress_count = if settings.count_partial_progress {
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

        // Use pre-fetched series metadata (for total_book_count)
        let total_book_count = metadata_map
            .get(&ext_id.series_id)
            .and_then(|m| m.total_book_count)
            .filter(|&total| total > 0);

        // Mark as Completed only when:
        // 1. All local books are read, AND
        // 2. The series has a known total_book_count in metadata, AND
        // 3. completed_count >= total_book_count
        // Otherwise default to Reading — we can't be sure the library is complete.
        let status = if all_completed {
            let is_truly_complete = total_book_count.is_some_and(|total| completed_count >= total);
            if is_truly_complete {
                SyncReadingStatus::Completed
            } else {
                SyncReadingStatus::Reading
            }
        } else {
            SyncReadingStatus::Reading
        };

        // Server always sends books-read as `volumes`. Codex tracks books
        // (each file = 1 volume), not chapters. `chapters` is left `None`.
        // The plugin decides how to map this to service-specific fields
        // (e.g. AniList's `progress` vs `progressVolumes` based on its own
        // `progressUnit` config).
        let progress = SyncProgress {
            chapters: None,
            volumes: Some(progress_count),
            pages: None,
            total_chapters: None,
            total_volumes: total_book_count,
        };

        // Look up rating/notes if sync_ratings is enabled
        let (score, notes) = if settings.sync_ratings {
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
            latest_updated_at: latest_updated_at.map(|dt| dt.to_rfc3339()),
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
