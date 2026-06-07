//! User Library Builder
//!
//! Builds the data sent to user plugins from a user's Codex library.
//!
//! [`build_series_engagements`] is the shared, batched data-gathering layer: for
//! a set of series it fetches metadata, reading progress, ratings, and
//! (optionally) taxonomy in one pass and folds book-level progress into a
//! per-series [`SeriesEngagement`]. Callers project that aggregate into their own
//! wire DTO:
//! - [`build_user_library`] → `Vec<UserLibraryEntry>` for recommendation plugins.
//! - the sync push builders → `Vec<SyncEntry>` for sync plugins.

use anyhow::Result;
use chrono::{DateTime, Utc};
use sea_orm::DatabaseConnection;
use std::collections::HashMap;
use tracing::{debug, warn};
use uuid::Uuid;

use crate::plugin::protocol::{UserLibraryEntry, UserLibraryExternalId, UserReadingStatus};
use codex_db::entities::{SeriesStatus, series, series_metadata};
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

/// Controls how much library data [`build_series_engagements`] fetches.
///
/// The progress aggregate, series metadata, ratings, and library context are
/// always fetched (every caller needs them). Taxonomy is optional: recommendation
/// library building always wants it, while sync push only needs it when it is
/// going to send full metadata — so the sync path can leave it off to avoid four
/// extra batch queries per run.
#[derive(Debug, Clone, Copy, Default)]
pub struct EngagementOptions {
    /// Also fetch genres, tags, alternate titles, and external IDs.
    pub include_taxonomy: bool,
}

/// Per-series aggregate of a user's engagement, plus the library data needed to
/// project it into a protocol DTO. Built in batch by [`build_series_engagements`].
///
/// The reading-progress fields are folded from the user's owned books in the
/// series. `metadata`/taxonomy carry the source data callers map into their DTO.
#[derive(Debug, Clone)]
pub struct SeriesEngagement {
    pub series_id: Uuid,
    pub library_id: Uuid,
    pub library_name: String,
    /// Title with the series-name fallback applied (`metadata.title` or
    /// `series.name`). Callers that want a metadata-only title (no fallback)
    /// should read [`Self::metadata`] directly instead.
    pub title: String,
    /// Series metadata row, when present.
    pub metadata: Option<series_metadata::Model>,

    /// Number of books the user owns in this series.
    pub books_owned: i32,
    /// Books the user has completed.
    pub books_read: i32,
    /// Books with reading progress that are not yet complete.
    pub in_progress_count: i32,
    /// Earliest `started_at` across books with progress.
    pub earliest_started: Option<DateTime<Utc>>,
    /// Latest `updated_at` across books with progress.
    pub latest_read_at: Option<DateTime<Utc>>,
    /// Latest `completed_at` across completed books.
    pub latest_completed_at: Option<DateTime<Utc>>,

    /// Genres — populated only when [`EngagementOptions::include_taxonomy`].
    pub genres: Vec<String>,
    /// Tags — populated only when [`EngagementOptions::include_taxonomy`].
    pub tags: Vec<String>,
    /// Alternate titles — populated only when [`EngagementOptions::include_taxonomy`].
    pub alternate_titles: Vec<String>,
    /// External IDs — populated only when [`EngagementOptions::include_taxonomy`].
    pub external_ids: Vec<UserLibraryExternalId>,

    /// User's personal rating (1-100 scale), when set.
    pub user_rating: Option<i32>,
    /// User's personal notes, when set.
    pub user_notes: Option<String>,
}

impl SeriesEngagement {
    /// Whether the user has any reading progress (complete or in-progress) for
    /// this series.
    pub fn has_any_progress(&self) -> bool {
        self.books_read > 0 || self.in_progress_count > 0
    }
}

/// Build per-series [`SeriesEngagement`] aggregates for the given series in one
/// batched pass.
///
/// Fetches series metadata, books, the user's reading progress and ratings, and
/// library names; optionally genres/tags/alternate-titles/external-IDs (see
/// [`EngagementOptions`]). Book-level progress is folded into per-series counts
/// and timestamps. The caller decides which series to pass and how to project
/// the result.
pub async fn build_series_engagements(
    db: &DatabaseConnection,
    user_id: Uuid,
    series: &[series::Model],
    opts: EngagementOptions,
) -> Result<HashMap<Uuid, SeriesEngagement>> {
    if series.is_empty() {
        return Ok(HashMap::new());
    }

    let series_ids: Vec<Uuid> = series.iter().map(|s| s.id).collect();

    // Resolve library names so each engagement can carry its library context.
    let library_ids: Vec<Uuid> = {
        let mut ids: Vec<Uuid> = series.iter().map(|s| s.library_id).collect();
        ids.sort_unstable();
        ids.dedup();
        ids
    };
    let lib_names = library_names(db, &library_ids).await;

    // Always needed: metadata (titles/totals/summary), books, progress, ratings.
    let metadata_map = SeriesMetadataRepository::get_by_series_ids(db, &series_ids).await?;
    let books_map = BookRepository::get_by_series_ids(db, &series_ids).await?;
    let all_book_ids: Vec<Uuid> = books_map.values().flatten().map(|b| b.id).collect();
    let progress_map =
        ReadProgressRepository::get_for_user_books(db, user_id, &all_book_ids).await?;
    let ratings_map: HashMap<Uuid, _> =
        match UserSeriesRatingRepository::get_all_for_user(db, user_id).await {
            Ok(ratings) => ratings.into_iter().map(|r| (r.series_id, r)).collect(),
            Err(e) => {
                warn!("Failed to fetch user ratings: {}", e);
                HashMap::new()
            }
        };

    // Optional taxonomy — only fetched when the caller will use it.
    let (genres_map, tags_map, alt_titles_map, ext_ids_map) = if opts.include_taxonomy {
        (
            GenreRepository::get_genres_for_series_ids(db, &series_ids).await?,
            TagRepository::get_tags_for_series_ids(db, &series_ids).await?,
            AlternateTitleRepository::get_for_series_ids(db, &series_ids).await?,
            SeriesExternalIdRepository::get_for_series_ids(db, &series_ids).await?,
        )
    } else {
        Default::default()
    };

    let mut engagements = HashMap::with_capacity(series.len());
    for s in series {
        let meta = metadata_map.get(&s.id);
        let title = meta
            .map(|m| m.title.clone())
            .unwrap_or_else(|| s.name.clone());

        let books = books_map.get(&s.id);
        let books_owned = books.map(|b| b.len() as i32).unwrap_or(0);

        let mut books_read = 0i32;
        let mut in_progress_count = 0i32;
        let mut earliest_started: Option<DateTime<Utc>> = None;
        let mut latest_read_at: Option<DateTime<Utc>> = None;
        let mut latest_completed_at: Option<DateTime<Utc>> = None;

        if let Some(books) = books {
            for book in books {
                if let Some(progress) = progress_map.get(&book.id) {
                    if progress.completed {
                        books_read += 1;
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
                    latest_read_at = Some(match latest_read_at {
                        Some(existing) if progress.updated_at > existing => progress.updated_at,
                        Some(existing) => existing,
                        None => progress.updated_at,
                    });
                }
            }
        }

        let genres = genres_map
            .get(&s.id)
            .map(|gs| gs.iter().map(|g| g.name.clone()).collect())
            .unwrap_or_default();
        let tags = tags_map
            .get(&s.id)
            .map(|ts| ts.iter().map(|t| t.name.clone()).collect())
            .unwrap_or_default();
        let alternate_titles = alt_titles_map
            .get(&s.id)
            .map(|alts| alts.iter().map(|a| a.title.clone()).collect())
            .unwrap_or_default();
        let external_ids = ext_ids_map
            .get(&s.id)
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

        let (user_rating, user_notes) = match ratings_map.get(&s.id) {
            Some(r) => (Some(r.rating), r.notes.clone()),
            None => (None, None),
        };

        engagements.insert(
            s.id,
            SeriesEngagement {
                series_id: s.id,
                library_id: s.library_id,
                library_name: lib_names.get(&s.library_id).cloned().unwrap_or_default(),
                title,
                metadata: meta.cloned(),
                books_owned,
                books_read,
                in_progress_count,
                earliest_started,
                latest_read_at,
                latest_completed_at,
                genres,
                tags,
                alternate_titles,
                external_ids,
                user_rating,
                user_notes,
            },
        );
    }

    Ok(engagements)
}

/// Build the user library as `Vec<UserLibraryEntry>` for recommendation plugins.
///
/// Assembles every series in scope (with full taxonomy) via
/// [`build_series_engagements`] and projects each into a `UserLibraryEntry`.
///
/// Only series in a library the plugin is scoped to are included.
/// `allowed_library_ids` empty means "all libraries".
pub async fn build_user_library(
    db: &DatabaseConnection,
    user_id: Uuid,
    allowed_library_ids: &[Uuid],
) -> Result<Vec<UserLibraryEntry>> {
    // Get all series, then drop any outside the plugin's library scope.
    let mut all_series = SeriesRepository::list_all(db, None).await?;
    if !allowed_library_ids.is_empty() {
        all_series.retain(|s| allowed_library_ids.contains(&s.library_id));
    }
    if all_series.is_empty() {
        return Ok(vec![]);
    }

    let engagements = build_series_engagements(
        db,
        user_id,
        &all_series,
        EngagementOptions {
            include_taxonomy: true,
        },
    )
    .await?;

    let mut entries = Vec::with_capacity(all_series.len());
    for series in &all_series {
        let Some(e) = engagements.get(&series.id) else {
            continue;
        };

        // Derive reading status from the aggregate.
        let reading_status = if e.books_read == 0 {
            Some(UserReadingStatus::Unread)
        } else if e.books_read >= e.books_owned && e.books_owned > 0 {
            Some(UserReadingStatus::Completed)
        } else {
            Some(UserReadingStatus::Reading)
        };

        let meta = e.metadata.as_ref();
        entries.push(UserLibraryEntry {
            series_id: e.series_id.to_string(),
            library_id: e.library_id.to_string(),
            library_name: e.library_name.clone(),
            title: e.title.clone(),
            alternate_titles: e.alternate_titles.clone(),
            year: meta.and_then(|m| m.year),
            status: meta.and_then(|m| {
                m.status
                    .as_deref()
                    .and_then(|s| s.parse::<SeriesStatus>().ok())
            }),
            genres: e.genres.clone(),
            tags: e.tags.clone(),
            total_volume_count: meta.and_then(|m| m.total_volume_count),
            total_chapter_count: meta.and_then(|m| m.total_chapter_count),
            external_ids: e.external_ids.clone(),
            reading_status,
            books_read: e.books_read,
            books_owned: e.books_owned,
            user_rating: e.user_rating,
            user_notes: e.user_notes.clone(),
            started_at: e.earliest_started.map(|dt| dt.to_rfc3339()),
            last_read_at: e.latest_read_at.map(|dt| dt.to_rfc3339()),
            completed_at: e.latest_completed_at.map(|dt| dt.to_rfc3339()),
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
    use codex_db::entities::{books, users};
    use codex_db::repositories::{
        BookRepository, LibraryRepository, ReadProgressRepository, SeriesRepository, UserRepository,
    };
    use codex_db::test_helpers::create_test_db;

    /// Insert a minimal user row so reading-progress FKs are satisfied.
    async fn create_user(db: &DatabaseConnection) -> Uuid {
        let user = users::Model {
            id: Uuid::new_v4(),
            username: format!("u_{}", Uuid::new_v4()),
            email: format!("{}@example.com", Uuid::new_v4()),
            password_hash: "x".to_string(),
            role: "user".to_string(),
            is_active: true,
            email_verified: false,
            permissions: serde_json::json!([]),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            last_login_at: None,
        };
        UserRepository::create(db, &user).await.unwrap().id
    }

    /// Insert a minimal book row in `series` for tests.
    async fn create_book(
        db: &DatabaseConnection,
        series_id: Uuid,
        library_id: Uuid,
    ) -> books::Model {
        let book = books::Model {
            id: Uuid::new_v4(),
            series_id,
            library_id,
            path: format!("/test/book_{}.cbz", Uuid::new_v4()),
            file_name: "book.cbz".to_string(),
            file_size: 1024,
            file_hash: format!("hash_{}", Uuid::new_v4()),
            partial_hash: String::new(),
            format: "cbz".to_string(),
            page_count: 50,
            deleted: false,
            analyzed: false,
            analysis_error: None,
            analysis_errors: None,
            modified_at: Utc::now(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            thumbnail_path: None,
            thumbnail_generated_at: None,
            koreader_hash: None,
            epub_positions: None,
            epub_spine_items: None,
        };
        BookRepository::create(db, &book, None).await.unwrap()
    }

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

    #[tokio::test]
    async fn test_build_series_engagements_aggregates_progress() {
        let (db, _temp_dir) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let user_id = create_user(conn).await;

        let lib = LibraryRepository::create(conn, "Lib", "/l", ScanningStrategy::Default)
            .await
            .unwrap();
        let series = SeriesRepository::create(conn, lib.id, "Engaged Series", None)
            .await
            .unwrap();

        // Three books: one completed, one in progress, one untouched.
        let done = create_book(conn, series.id, lib.id).await;
        let reading = create_book(conn, series.id, lib.id).await;
        let _untouched = create_book(conn, series.id, lib.id).await;
        ReadProgressRepository::upsert(conn, user_id, done.id, 50, true)
            .await
            .unwrap();
        ReadProgressRepository::upsert(conn, user_id, reading.id, 10, false)
            .await
            .unwrap();

        let engagements = build_series_engagements(
            conn,
            user_id,
            &[series.clone()],
            EngagementOptions::default(),
        )
        .await
        .unwrap();

        let e = engagements.get(&series.id).expect("engagement present");
        assert_eq!(e.books_owned, 3);
        assert_eq!(e.books_read, 1);
        assert_eq!(e.in_progress_count, 1);
        assert!(e.has_any_progress());
        assert!(e.earliest_started.is_some());
        assert!(e.latest_read_at.is_some());
        assert!(e.latest_completed_at.is_some());
        assert_eq!(e.library_name, "Lib");
        // Taxonomy not requested → empty.
        assert!(e.genres.is_empty());
        assert!(e.external_ids.is_empty());
    }

    #[tokio::test]
    async fn test_build_series_engagements_empty_input() {
        let (db, _temp_dir) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let engagements =
            build_series_engagements(conn, Uuid::new_v4(), &[], EngagementOptions::default())
                .await
                .unwrap();
        assert!(engagements.is_empty());
    }
}
