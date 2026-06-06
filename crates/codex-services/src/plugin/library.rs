//! User Library Builder
//!
//! Builds `Vec<UserLibraryEntry>` from a user's Codex library data for
//! sending to recommendation plugins. Uses batch queries for efficiency.

use anyhow::Result;
use sea_orm::DatabaseConnection;
use std::collections::HashMap;
use tracing::{debug, warn};
use uuid::Uuid;

use crate::plugin::protocol::{UserLibraryEntry, UserLibraryExternalId, UserReadingStatus};
use codex_db::entities::SeriesStatus;
use codex_db::repositories::{
    AlternateTitleRepository, BookRepository, GenreRepository, LibraryRepository,
    ReadProgressRepository, SeriesExternalIdRepository, SeriesMetadataRepository, SeriesRepository,
    TagRepository, UserSeriesRatingRepository,
};

/// Resolve a set of library IDs to a `library_id -> library_name` map in a
/// single query. Used to stamp `library_name` onto entries sent to plugins.
/// Returns an empty map (and logs) on failure so callers can degrade gracefully.
pub async fn library_names(db: &DatabaseConnection, library_ids: &[Uuid]) -> HashMap<Uuid, String> {
    match LibraryRepository::get_by_ids(db, library_ids).await {
        Ok(libs) => libs.into_iter().map(|(id, lib)| (id, lib.name)).collect(),
        Err(e) => {
            warn!("Failed to fetch library names for {library_ids:?}: {e}");
            HashMap::new()
        }
    }
}

/// Build the user library as `Vec<UserLibraryEntry>` for recommendation plugins.
///
/// Fetches series, metadata, genres, tags, external IDs, reading progress,
/// and user ratings in batch, then assembles them into library entries.
///
/// Only series in a library the plugin is scoped to are included.
/// `allowed_library_ids` empty means "all libraries".
pub async fn build_user_library(
    db: &DatabaseConnection,
    user_id: Uuid,
    allowed_library_ids: &[Uuid],
) -> Result<Vec<UserLibraryEntry>> {
    // 1. Get all series, then drop any outside the plugin's library scope.
    let mut all_series = SeriesRepository::list_all(db, None).await?;
    if !allowed_library_ids.is_empty() {
        all_series.retain(|s| allowed_library_ids.contains(&s.library_id));
    }
    if all_series.is_empty() {
        return Ok(vec![]);
    }

    let series_ids: Vec<Uuid> = all_series.iter().map(|s| s.id).collect();

    // Resolve library names so each entry can carry its library context.
    let library_ids: Vec<Uuid> = {
        let mut ids: Vec<Uuid> = all_series.iter().map(|s| s.library_id).collect();
        ids.sort_unstable();
        ids.dedup();
        ids
    };
    let lib_names = library_names(db, &library_ids).await;

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
            library_id: series.library_id.to_string(),
            library_name: lib_names
                .get(&series.library_id)
                .cloned()
                .unwrap_or_default(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use codex_db::ScanningStrategy;
    use codex_db::repositories::{LibraryRepository, SeriesRepository};
    use codex_db::test_helpers::create_test_db;

    #[tokio::test]
    async fn test_build_user_library_respects_library_scope_and_stamps_info() {
        let (db, _temp_dir) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let user_id = Uuid::new_v4();

        let lib_a = LibraryRepository::create(conn, "Library A", "/a", ScanningStrategy::Default)
            .await
            .unwrap();
        let lib_b = LibraryRepository::create(conn, "Library B", "/b", ScanningStrategy::Default)
            .await
            .unwrap();
        SeriesRepository::create(conn, lib_a.id, "Series A", None)
            .await
            .unwrap();
        SeriesRepository::create(conn, lib_b.id, "Series B", None)
            .await
            .unwrap();

        // Scoped to library A: only its series, stamped with library context.
        let entries = build_user_library(conn, user_id, &[lib_a.id])
            .await
            .unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].title, "Series A");
        assert_eq!(entries[0].library_id, lib_a.id.to_string());
        assert_eq!(entries[0].library_name, "Library A");

        // Empty scope = all libraries.
        let all = build_user_library(conn, user_id, &[]).await.unwrap();
        assert_eq!(all.len(), 2);
    }
}
