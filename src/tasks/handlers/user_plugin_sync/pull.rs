//! Pull operations — match external entries to local series and apply
//! reading progress.

use sea_orm::DatabaseConnection;
use std::collections::HashMap;
use tracing::{debug, warn};
use uuid::Uuid;

use crate::db::repositories::{
    BookRepository, ReadProgressRepository, SeriesExternalIdRepository, UserSeriesRatingRepository,
};
use crate::services::plugin::sync::{SyncEntry, SyncReadingStatus};

/// Match pulled sync entries to Codex series using external IDs and apply
/// reading progress.
///
/// For each pulled entry, looks up `series_external_ids` where
/// `source = external_id_source` and `external_id = entry.external_id`.
/// When a match is found, applies the pulled reading progress to the user's
/// Codex books (each book = 1 chapter).
///
/// Returns `(matched, applied)` — matched entries count and books updated.
pub(crate) async fn match_and_apply_pulled_entries(
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

    // 1. Batch-fetch all external ID → series mappings (1 query instead of N)
    let entry_external_ids: Vec<String> = entries.iter().map(|e| e.external_id.clone()).collect();
    let ext_id_map = match SeriesExternalIdRepository::find_by_external_ids_and_source(
        db,
        &entry_external_ids,
        source,
    )
    .await
    {
        Ok(map) => map,
        Err(e) => {
            warn!(
                "Task {}: Failed to batch-fetch external IDs for source {}: {}",
                task_id, source, e
            );
            return (0, 0);
        }
    };

    // 2. Batch-fetch books for all matched series (1 query instead of N)
    let matched_series_ids: Vec<Uuid> = ext_id_map.values().map(|e| e.series_id).collect();
    let books_map = match BookRepository::get_by_series_ids(db, &matched_series_ids).await {
        Ok(map) => map,
        Err(e) => {
            warn!(
                "Task {}: Failed to batch-fetch books for pull apply: {}",
                task_id, e
            );
            return (0, 0);
        }
    };

    // 3. Batch-fetch reading progress for all books in matched series (1 query instead of N*M)
    let all_book_ids: Vec<Uuid> = books_map.values().flatten().map(|b| b.id).collect();
    let progress_map =
        match ReadProgressRepository::get_for_user_books(db, user_id, &all_book_ids).await {
            Ok(map) => map,
            Err(e) => {
                warn!(
                    "Task {}: Failed to batch-fetch reading progress for pull: {}",
                    task_id, e
                );
                HashMap::new()
            }
        };

    // 4. Batch-fetch existing ratings if sync_ratings is enabled (1 query instead of N)
    let existing_ratings: HashMap<Uuid, crate::db::entities::user_series_ratings::Model> =
        if sync_ratings {
            match UserSeriesRatingRepository::get_all_for_user(db, user_id).await {
                Ok(ratings) => ratings.into_iter().map(|r| (r.series_id, r)).collect(),
                Err(e) => {
                    warn!(
                        "Task {}: Failed to batch-fetch existing ratings: {}",
                        task_id, e
                    );
                    HashMap::new()
                }
            }
        } else {
            HashMap::new()
        };

    let mut matched: u32 = 0;
    let mut unmatched: u32 = 0;
    let mut applied: u32 = 0;

    for entry in entries {
        match ext_id_map.get(&entry.external_id) {
            Some(ext_id) => {
                debug!(
                    "Task {}: Matched entry {} -> series {} (source: {})",
                    task_id, entry.external_id, ext_id.series_id, source
                );
                matched += 1;

                // Apply reading progress using pre-fetched data
                let books_applied = apply_pulled_entry(
                    db,
                    user_id,
                    ext_id.series_id,
                    entry,
                    task_id,
                    &books_map,
                    &progress_map,
                )
                .await;
                applied += books_applied;

                // Apply pulled rating/notes if enabled and Codex has no existing rating
                if sync_ratings && let Some(pulled_score) = entry.score {
                    if !existing_ratings.contains_key(&ext_id.series_id) {
                        let score_i32 = (pulled_score.round() as i32).clamp(1, 100);
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
                    } else {
                        debug!(
                            "Task {}: Skipping pulled rating for series {} — Codex already has a rating",
                            task_id, ext_id.series_id
                        );
                    }
                }
            }
            None => {
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
///
/// Uses pre-fetched `books_map` and `progress_map` to avoid N+1 queries.
/// Only issues write queries (`mark_as_read`) for books that actually need updating.
async fn apply_pulled_entry(
    db: &DatabaseConnection,
    user_id: Uuid,
    series_id: Uuid,
    entry: &SyncEntry,
    task_id: Uuid,
    books_map: &HashMap<Uuid, Vec<crate::db::entities::books::Model>>,
    progress_map: &HashMap<Uuid, crate::db::entities::read_progress::Model>,
) -> u32 {
    let books = match books_map.get(&series_id) {
        Some(b) if !b.is_empty() => b,
        _ => return 0,
    };

    // Use volumes if available, fall back to chapters
    let units_read = entry
        .progress
        .as_ref()
        .and_then(|p| p.volumes.or(p.chapters))
        .unwrap_or(0);

    // Determine which books to mark as read
    let books_to_mark: &[crate::db::entities::books::Model] =
        if entry.status == SyncReadingStatus::Completed {
            // Mark all books as read
            books
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
        // Check if already completed using pre-fetched progress — skip if so
        if let Some(progress) = progress_map.get(&book.id)
            && progress.completed
        {
            continue; // Already read, skip
        }

        // Mark as read (this is a write — must be a real query)
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
