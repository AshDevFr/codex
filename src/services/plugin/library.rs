//! User Library Builder
//!
//! Builds `Vec<UserLibraryEntry>` from a user's Codex library data for
//! sending to recommendation plugins. Uses batch queries for efficiency.

use anyhow::Result;
use sea_orm::DatabaseConnection;
use std::collections::HashMap;
use tracing::{debug, warn};
use uuid::Uuid;

use crate::db::entities::SeriesStatus;
use crate::db::repositories::{
    AlternateTitleRepository, BookRepository, GenreRepository, ReadProgressRepository,
    SeriesExternalIdRepository, SeriesMetadataRepository, SeriesRepository, TagRepository,
    UserSeriesRatingRepository,
};
use crate::services::plugin::protocol::{
    UserLibraryEntry, UserLibraryExternalId, UserReadingStatus,
};

/// Build the full user library as `Vec<UserLibraryEntry>` for recommendation plugins.
///
/// Fetches all series, metadata, genres, tags, external IDs, reading progress,
/// and user ratings in batch, then assembles them into library entries.
pub async fn build_user_library(
    db: &DatabaseConnection,
    user_id: Uuid,
) -> Result<Vec<UserLibraryEntry>> {
    // 1. Get all series
    let all_series = SeriesRepository::list_all(db).await?;
    if all_series.is_empty() {
        return Ok(vec![]);
    }

    let series_ids: Vec<Uuid> = all_series.iter().map(|s| s.id).collect();

    // 2. Batch-fetch all related data
    let metadata_map = SeriesMetadataRepository::get_by_series_ids(db, &series_ids).await?;
    let genres_map = GenreRepository::get_genres_for_series_ids(db, &series_ids).await?;
    let tags_map = TagRepository::get_tags_for_series_ids(db, &series_ids).await?;
    let ext_ids_map = SeriesExternalIdRepository::get_for_series_ids(db, &series_ids).await?;
    let alt_titles_map = AlternateTitleRepository::get_for_series_ids(db, &series_ids).await?;

    // 3. Batch-fetch all books and reading progress
    let all_books = BookRepository::list_by_series_ids(db, &series_ids).await?;
    let mut books_by_series: HashMap<Uuid, Vec<Uuid>> = HashMap::new();
    for book in &all_books {
        books_by_series
            .entry(book.series_id)
            .or_default()
            .push(book.id);
    }

    let all_progress = ReadProgressRepository::get_by_user(db, user_id).await?;
    let progress_by_book: HashMap<Uuid, _> =
        all_progress.into_iter().map(|p| (p.book_id, p)).collect();

    // 4. Batch-fetch user ratings
    let ratings_map: HashMap<Uuid, _> =
        match UserSeriesRatingRepository::get_all_for_user(db, user_id).await {
            Ok(ratings) => ratings.into_iter().map(|r| (r.series_id, r)).collect(),
            Err(e) => {
                warn!("Failed to fetch user ratings: {}", e);
                HashMap::new()
            }
        };

    // 5. Build entries
    let mut entries = Vec::new();

    for series in &all_series {
        let meta = metadata_map.get(&series.id);
        let title = meta
            .map(|m| m.title.clone())
            .unwrap_or_else(|| series.name.clone());

        let book_ids = books_by_series.get(&series.id);
        let books_owned = book_ids.map(|b| b.len() as i32).unwrap_or(0);

        // Aggregate reading progress
        let mut books_read = 0i32;
        let mut earliest_started: Option<chrono::DateTime<chrono::Utc>> = None;
        let mut latest_read_at: Option<chrono::DateTime<chrono::Utc>> = None;
        let mut latest_completed_at: Option<chrono::DateTime<chrono::Utc>> = None;

        if let Some(book_ids) = book_ids {
            for book_id in book_ids {
                if let Some(progress) = progress_by_book.get(book_id) {
                    if progress.completed {
                        books_read += 1;
                        if let Some(cat) = progress.completed_at {
                            latest_completed_at = Some(match latest_completed_at {
                                Some(existing) if cat > existing => cat,
                                Some(existing) => existing,
                                None => cat,
                            });
                        }
                    }
                    earliest_started = Some(match earliest_started {
                        Some(existing) if progress.started_at < existing => progress.started_at,
                        Some(existing) => existing,
                        None => progress.started_at,
                    });
                    latest_read_at = Some(match latest_read_at {
                        Some(existing) if progress.updated_at > existing => progress.updated_at,
                        Some(existing) => existing,
                        None => progress.updated_at,
                    });
                }
            }
        }

        // Derive reading status
        let reading_status = if books_read == 0 {
            Some(UserReadingStatus::Unread)
        } else if books_read >= books_owned && books_owned > 0 {
            Some(UserReadingStatus::Completed)
        } else {
            Some(UserReadingStatus::Reading)
        };

        // Genres and tags as string names
        let genres = genres_map
            .get(&series.id)
            .map(|gs| gs.iter().map(|g| g.name.clone()).collect())
            .unwrap_or_default();

        let tags = tags_map
            .get(&series.id)
            .map(|ts| ts.iter().map(|t| t.name.clone()).collect())
            .unwrap_or_default();

        // External IDs
        let external_ids = ext_ids_map
            .get(&series.id)
            .map(|eids| {
                eids.iter()
                    .map(|e| UserLibraryExternalId {
                        source: e.source.clone(),
                        external_id: e.external_id.clone(),
                        external_url: e.external_url.clone(),
                    })
                    .collect()
            })
            .unwrap_or_default();

        // Alternate titles
        let alternate_titles = alt_titles_map
            .get(&series.id)
            .map(|alts| alts.iter().map(|a| a.title.clone()).collect())
            .unwrap_or_default();

        // User rating/notes
        let (user_rating, user_notes) = match ratings_map.get(&series.id) {
            Some(r) => (Some(r.rating), r.notes.clone()),
            None => (None, None),
        };

        entries.push(UserLibraryEntry {
            series_id: series.id.to_string(),
            title,
            alternate_titles,
            year: meta.and_then(|m| m.year),
            status: meta.and_then(|m| {
                m.status
                    .as_deref()
                    .and_then(|s| s.parse::<SeriesStatus>().ok())
            }),
            genres,
            tags,
            total_book_count: meta.and_then(|m| m.total_book_count),
            total_volume_count: meta.and_then(|m| m.total_volume_count),
            total_chapter_count: meta.and_then(|m| m.total_chapter_count),
            external_ids,
            reading_status,
            books_read,
            books_owned,
            user_rating,
            user_notes,
            started_at: earliest_started.map(|dt| dt.to_rfc3339()),
            last_read_at: latest_read_at.map(|dt| dt.to_rfc3339()),
            completed_at: latest_completed_at.map(|dt| dt.to_rfc3339()),
        });
    }

    debug!(
        "Built {} user library entries for user {}",
        entries.len(),
        user_id
    );

    Ok(entries)
}
