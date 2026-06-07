//! Push operations — build entries from local reading progress to push to
//! external services.

use sea_orm::DatabaseConnection;
use std::collections::HashSet;
use tracing::{debug, warn};
use uuid::Uuid;

use codex_db::entities::series;
use codex_db::repositories::{
    BookRepository, ReadProgressRepository, SeriesExternalIdRepository, SeriesRepository,
};
use codex_services::plugin::library::{
    EngagementOptions, SeriesBookProgress, SeriesEngagement, build_series_engagements,
};
use codex_services::plugin::sync::{SyncBookProgress, SyncEntry, SyncProgress, SyncReadingStatus};

use super::settings::CodexSyncSettings;

/// Effective per-field enrichment flags for a push, computed by the caller from
/// plugin capabilities (and, for the metadata fields, the user's `_codex.send*`
/// toggles). Each gates one piece of optional data attached to push entries.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct MetadataFlags {
    pub tags: bool,
    pub genres: bool,
    pub metadata: bool,
    pub custom_metadata: bool,
    /// Attach the per-book reading-progress breakdown (`readBooks`). Gated by the
    /// plugin's `wantsDetailedProgress` capability only (no user toggle). Also
    /// drives fetching per-book detail, which the accurate `maxVolume`/`maxChapter`
    /// fields ride along on.
    pub detailed_progress: bool,
}

impl MetadataFlags {
    /// Whether any taxonomy (genres/tags) must be fetched for this push.
    fn needs_taxonomy(&self) -> bool {
        self.tags || self.genres
    }
}

/// Whether a series in `library_id` is in scope for a plugin allowed to act on
/// `allowed_library_ids`. An empty allowed set means "all libraries".
fn library_in_scope(allowed_library_ids: &[Uuid], library_id: Uuid) -> bool {
    allowed_library_ids.is_empty() || allowed_library_ids.contains(&library_id)
}

/// Fetch the series rows for `series_ids` and drop any outside the plugin's
/// library scope. Degrades to an empty Vec on lookup failure.
async fn scoped_series(
    db: &DatabaseConnection,
    series_ids: &[Uuid],
    allowed_library_ids: &[Uuid],
) -> Vec<series::Model> {
    SeriesRepository::get_by_ids(db, series_ids)
        .await
        .unwrap_or_default()
        .into_iter()
        .filter(|s| library_in_scope(allowed_library_ids, s.library_id))
        .collect()
}

/// Project a [`SeriesEngagement`] into a `SyncEntry`, applying `CodexSyncSettings`.
///
/// Returns `None` when the series should be skipped: no reading progress, or
/// filtered out by `include_completed` / `include_in_progress`. `external_id` and
/// `title` are supplied by the caller (matched entries use the source external ID
/// and a metadata-only title; search-fallback entries use `""` and a required
/// title).
fn project_sync_entry(
    e: &SeriesEngagement,
    external_id: String,
    title: Option<String>,
    settings: &CodexSyncSettings,
    flags: MetadataFlags,
) -> Option<SyncEntry> {
    // Skip series with no progress at all.
    if !e.has_any_progress() {
        return None;
    }

    let completed_count = e.books_read;
    // `has_any_progress` guarantees at least one owned book.
    let all_completed = completed_count == e.books_owned;
    let is_in_progress = !all_completed;

    // Apply Codex sync settings filters.
    if all_completed && !settings.include_completed {
        return None;
    }
    if is_in_progress && !settings.include_in_progress {
        return None;
    }

    let progress_count = if settings.count_partial_progress {
        completed_count + e.in_progress_count
    } else {
        completed_count
    };

    // Completion / progress totals from series metadata.
    let meta = e.metadata.as_ref();
    let total_volume_count = meta
        .and_then(|m| m.total_volume_count)
        .filter(|&total| total > 0);
    let total_chapter_count = meta
        .and_then(|m| m.total_chapter_count)
        .filter(|c| c.is_finite() && *c > 0.0);

    // Mark as Completed only when all local books are read AND the series has a
    // known total_volume_count that we've reached. Otherwise default to Reading —
    // we can't be sure the local library is complete.
    let status = if all_completed {
        let is_truly_complete = total_volume_count.is_some_and(|total| completed_count >= total);
        if is_truly_complete {
            SyncReadingStatus::Completed
        } else {
            SyncReadingStatus::Reading
        }
    } else {
        SyncReadingStatus::Reading
    };

    // Detailed progress, derived from per-book volume/chapter detection. Present
    // only when the engagement was built with per-book detail (i.e. the plugin
    // declares `wantsDetailedProgress`); otherwise `read_books` is empty and these
    // stay `None`.
    //
    // `max_volume`/`max_chapter` are the highest *read* numbers, folded over the
    // same set of books that feeds `volumes`: completed always, plus in-progress
    // when `count_partial_progress` is on. Unlike the `volumes` count, they stay
    // accurate for libraries that don't start at volume 1 or have gaps.
    let counted = |b: &SeriesBookProgress| b.completed || settings.count_partial_progress;
    let max_volume = e
        .read_books
        .iter()
        .filter(|b| counted(b))
        .filter_map(|b| b.volume)
        .max();
    let max_chapter = e
        .read_books
        .iter()
        .filter(|b| counted(b))
        .filter_map(|b| b.chapter)
        .fold(None::<f32>, |acc, c| match acc {
            Some(m) if m >= c => Some(m),
            _ => Some(c),
        });

    // The full per-book breakdown is attached only for plugins that declare the
    // capability. It reflects every book with progress (completed or in-progress),
    // independent of `count_partial_progress` (a raw breakdown the plugin filters).
    let read_books = if flags.detailed_progress {
        Some(
            e.read_books
                .iter()
                .map(|b| SyncBookProgress {
                    volume: b.volume,
                    chapter: b.chapter,
                    completed: b.completed,
                    current_page: b.current_page,
                    progress_percentage: b.progress_percentage,
                })
                .collect(),
        )
    } else {
        None
    };

    // Server always sends books-read as `volumes`. Codex tracks books (each file
    // = 1 volume), not chapters. `chapters` is left `None`; the plugin decides how
    // to map this to service-specific fields.
    let progress = SyncProgress {
        chapters: None,
        volumes: Some(progress_count),
        pages: None,
        total_chapters: total_chapter_count.map(|c| c as i32),
        total_volumes: total_volume_count,
        max_volume,
        max_chapter,
        read_books,
    };

    let (score, notes) = if settings.sync_ratings {
        (e.user_rating.map(|r| r as f64), e.user_notes.clone())
    } else {
        (None, None)
    };

    Some(SyncEntry {
        external_id,
        completed_at: if status == SyncReadingStatus::Completed {
            e.latest_completed_at.map(|dt| dt.to_rfc3339())
        } else {
            None
        },
        status,
        progress: Some(progress),
        score,
        started_at: e.earliest_started.map(|dt| dt.to_rfc3339()),
        notes,
        latest_updated_at: e.latest_read_at.map(|dt| dt.to_rfc3339()),
        title,
        library_id: e.library_id.to_string(),
        library_name: e.library_name.clone(),
        // Per-field enrichment, each gated by its effective flag.
        genres: if flags.genres {
            e.genres.clone()
        } else {
            Vec::new()
        },
        tags: if flags.tags {
            e.tags.clone()
        } else {
            Vec::new()
        },
        metadata: if flags.metadata {
            e.series_metadata_block()
        } else {
            None
        },
        custom_metadata: if flags.custom_metadata {
            e.custom_metadata_value()
        } else {
            None
        },
    })
}

/// Build push entries from a user's Codex reading progress.
///
/// For each series that has an external ID matching the given source,
/// aggregates book-level reading progress into a single `SyncEntry`.
/// Behaviour is controlled by `CodexSyncSettings` (which series to
/// include, whether partial-progress books count, ratings).
///
/// Only series in a library the plugin is scoped to are included.
/// `allowed_library_ids` empty means "all libraries".
pub(crate) async fn build_push_entries(
    db: &DatabaseConnection,
    user_id: Uuid,
    external_id_source: &str,
    task_id: Uuid,
    settings: &CodexSyncSettings,
    allowed_library_ids: &[Uuid],
    flags: MetadataFlags,
) -> Vec<SyncEntry> {
    // 1. Get all series that have external IDs for this source.
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

    if external_ids.is_empty() && !settings.search_fallback {
        return vec![];
    }

    // 2. Resolve series rows for the candidates and drop out-of-scope ones, then
    //    build the shared engagement aggregates in one batched pass.
    let candidate_series_ids: Vec<Uuid> = external_ids.iter().map(|e| e.series_id).collect();
    let series = scoped_series(db, &candidate_series_ids, allowed_library_ids).await;
    let opts = EngagementOptions {
        include_taxonomy: flags.needs_taxonomy(),
        // Always fetch per-book detail for the sync push: the accurate
        // `max_volume`/`max_chapter` (Tier 1) are computed from it for every
        // entry. The capability only gates the heavier `read_books` array.
        include_book_detail: true,
    };
    let engagements = match build_series_engagements(db, user_id, &series, opts).await {
        Ok(map) => map,
        Err(e) => {
            warn!(
                "Task {}: Failed to build series engagements for push: {}",
                task_id, e
            );
            return vec![];
        }
    };

    // Series we matched by external ID (and that are in scope) — used to exclude
    // them from the search-fallback pass below.
    let matched_series_ids: HashSet<Uuid> = engagements.keys().copied().collect();

    // 3. Project each external-ID-bearing, in-scope series into a SyncEntry.
    let mut entries = Vec::new();
    for ext_id in &external_ids {
        let Some(e) = engagements.get(&ext_id.series_id) else {
            continue;
        };
        // Matched entries carry a metadata-only title (no series-name fallback).
        let title = e.metadata.as_ref().map(|m| m.title.clone());
        if let Some(entry) =
            project_sync_entry(e, ext_id.external_id.clone(), title, settings, flags)
        {
            entries.push(entry);
        }
    }

    debug!(
        "Task {}: Built {} push entries from {} in-scope series with external IDs",
        task_id,
        entries.len(),
        matched_series_ids.len()
    );

    // 4. When search_fallback is enabled, also include series that have reading
    //    progress but no external ID for this source. The plugin searches by title.
    if settings.search_fallback {
        let unmatched = build_unmatched_entries(
            db,
            user_id,
            task_id,
            settings,
            &matched_series_ids,
            allowed_library_ids,
            flags,
        )
        .await;

        debug!(
            "Task {}: Built {} unmatched entries for search fallback",
            task_id,
            unmatched.len()
        );

        entries.extend(unmatched);
    }

    entries
}

/// Build push entries for series that have reading progress but no external ID
/// for the given source. These entries have `external_id: ""` and `title` set,
/// so the plugin can search the external service by title.
async fn build_unmatched_entries(
    db: &DatabaseConnection,
    user_id: Uuid,
    task_id: Uuid,
    settings: &CodexSyncSettings,
    matched_series_ids: &HashSet<Uuid>,
    allowed_library_ids: &[Uuid],
    flags: MetadataFlags,
) -> Vec<SyncEntry> {
    // 1. Get all reading progress for this user, then map books → series.
    let all_progress = match ReadProgressRepository::get_by_user(db, user_id).await {
        Ok(p) => p,
        Err(e) => {
            warn!(
                "Task {}: Failed to fetch user reading progress for search fallback: {}",
                task_id, e
            );
            return vec![];
        }
    };

    if all_progress.is_empty() {
        return vec![];
    }

    let book_ids: Vec<Uuid> = all_progress.iter().map(|p| p.book_id).collect();
    let books = match BookRepository::get_by_ids(db, &book_ids).await {
        Ok(b) => b,
        Err(e) => {
            warn!(
                "Task {}: Failed to fetch books for search fallback: {}",
                task_id, e
            );
            return vec![];
        }
    };

    // Collect unmatched series IDs (have progress but no external ID for this source).
    let mut unmatched_series_ids: HashSet<Uuid> = HashSet::new();
    for book in &books {
        if !matched_series_ids.contains(&book.series_id) {
            unmatched_series_ids.insert(book.series_id);
        }
    }

    if unmatched_series_ids.is_empty() {
        return vec![];
    }

    // 2. Resolve series rows, drop out-of-scope ones, build engagements.
    let unmatched_ids_vec: Vec<Uuid> = unmatched_series_ids.into_iter().collect();
    let series = scoped_series(db, &unmatched_ids_vec, allowed_library_ids).await;
    let opts = EngagementOptions {
        include_taxonomy: flags.needs_taxonomy(),
        // Always fetch per-book detail for the sync push: the accurate
        // `max_volume`/`max_chapter` (Tier 1) are computed from it for every
        // entry. The capability only gates the heavier `read_books` array.
        include_book_detail: true,
    };
    let engagements = match build_series_engagements(db, user_id, &series, opts).await {
        Ok(map) => map,
        Err(e) => {
            warn!(
                "Task {}: Failed to build series engagements for unmatched series: {}",
                task_id, e
            );
            return vec![];
        }
    };

    // 3. Project each unmatched, in-scope series with metadata into a SyncEntry.
    let mut entries = Vec::new();
    for s in &series {
        let Some(e) = engagements.get(&s.id) else {
            continue;
        };
        // Need a title to search the external service by; skip series without metadata.
        let title = match e.metadata.as_ref() {
            Some(m) => m.title.clone(),
            None => continue,
        };
        if let Some(entry) = project_sync_entry(e, String::new(), Some(title), settings, flags) {
            entries.push(entry);
        }
    }

    entries
}
