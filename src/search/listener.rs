//! Event-driven incremental updates for the fuzzy search index.
//!
//! Subscribes to the global [`EventBroadcaster`] and translates each entity
//! event into a single-row upsert or remove on the in-memory index, so the
//! index stays consistent with the database without periodic rebuilds.
//!
//! On a `RecvError::Lagged` the channel buffer overflowed (slow listener,
//! event burst, etc.) and we may have missed some events; the listener
//! responds by triggering a full rebuild from the DB to re-converge.
//!
//! The task lives for the life of the server. Shutdown is signalled via the
//! same [`CancellationToken`] used by other long-lived background jobs.

use std::sync::Arc;

use sea_orm::DatabaseConnection;
use tokio::sync::broadcast::error::RecvError;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

use crate::events::{EntityEvent, EventBroadcaster};

use super::FuzzyIndex;
use super::builder::{fetch_book_entry, fetch_series_entry, rebuild_into};

/// Spawn the listener task. Runs until `cancel` is triggered or the
/// broadcaster's sender side is dropped.
pub fn spawn_listener(
    index: Arc<FuzzyIndex>,
    broadcaster: Arc<EventBroadcaster>,
    db: DatabaseConnection,
    cancel: CancellationToken,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        run_listener(index, broadcaster, db, cancel).await;
    })
}

async fn run_listener(
    index: Arc<FuzzyIndex>,
    broadcaster: Arc<EventBroadcaster>,
    db: DatabaseConnection,
    cancel: CancellationToken,
) {
    let mut rx = broadcaster.subscribe();
    info!(target: "search::fuzzy", "fuzzy search event listener started");

    loop {
        tokio::select! {
            biased;
            _ = cancel.cancelled() => {
                info!(target: "search::fuzzy", "fuzzy search event listener stopping (cancelled)");
                return;
            }
            recv = rx.recv() => match recv {
                Ok(event) => {
                    if event.is_shutdown() {
                        info!(target: "search::fuzzy", "fuzzy search event listener stopping (shutdown signal)");
                        return;
                    }
                    apply_event(&index, &db, &event.event).await;
                }
                Err(RecvError::Lagged(n)) => {
                    warn!(
                        target: "search::fuzzy",
                        missed = n,
                        "fuzzy index event subscriber lagged; triggering full rebuild"
                    );
                    if let Err(err) = rebuild_into(&index, &db).await {
                        warn!(
                            target: "search::fuzzy",
                            error = %err,
                            "rebuild after lag failed; index may be stale until next rebuild"
                        );
                    }
                }
                Err(RecvError::Closed) => {
                    info!(target: "search::fuzzy", "fuzzy search event listener stopping (channel closed)");
                    return;
                }
            }
        }
    }
}

/// Apply one event to the index. Public for direct testing.
pub async fn apply_event(index: &FuzzyIndex, db: &DatabaseConnection, event: &EntityEvent) {
    match event {
        EntityEvent::SeriesCreated { series_id, .. }
        | EntityEvent::SeriesUpdated { series_id, .. }
        | EntityEvent::SeriesMetadataUpdated { series_id, .. } => {
            upsert_series(index, db, *series_id).await;
        }
        EntityEvent::SeriesDeleted { series_id, .. } => {
            index.remove_series(*series_id);
            debug!(target: "search::fuzzy", series_id = %series_id, "removed series from fuzzy index");
        }
        EntityEvent::SeriesBulkPurged { series_id, .. } => {
            // The event fires after deleted books were purged; the parent
            // series row still exists (or has its own SeriesDeleted event).
            // Drop the books and refresh the series in case the surviving
            // book set affects anything we surface.
            let dropped = index.remove_books_for_series(*series_id);
            debug!(
                target: "search::fuzzy",
                series_id = %series_id,
                dropped_books = dropped,
                "purged books for series in fuzzy index"
            );
            // The series might also have been deleted — refresh covers both
            // "still exists" (re-upsert) and "now gone" (remove). We can't
            // tell from the event alone.
            upsert_or_remove_series(index, db, *series_id).await;
        }
        EntityEvent::BookCreated { book_id, .. } | EntityEvent::BookUpdated { book_id, .. } => {
            upsert_book(index, db, *book_id).await;
        }
        EntityEvent::BookDeleted { book_id, .. } => {
            index.remove_book(*book_id);
            debug!(target: "search::fuzzy", book_id = %book_id, "removed book from fuzzy index");
        }
        // Events that don't touch series/book search surface.
        EntityEvent::CoverUpdated { .. }
        | EntityEvent::LibraryUpdated { .. }
        | EntityEvent::LibraryDeleted { .. }
        | EntityEvent::PluginCreated { .. }
        | EntityEvent::PluginUpdated { .. }
        | EntityEvent::PluginEnabled { .. }
        | EntityEvent::PluginDisabled { .. }
        | EntityEvent::PluginDeleted { .. }
        | EntityEvent::ReleaseAnnounced { .. }
        | EntityEvent::ReleaseSourcePolled { .. }
        | EntityEvent::Shutdown => {}
    }
}

async fn upsert_series(index: &FuzzyIndex, db: &DatabaseConnection, series_id: uuid::Uuid) {
    match fetch_series_entry(db, series_id).await {
        Ok(Some(entry)) => {
            index.upsert_series(entry);
            debug!(target: "search::fuzzy", series_id = %series_id, "upserted series in fuzzy index");
        }
        Ok(None) => {
            // The event raced with a delete; treat as removal.
            index.remove_series(series_id);
            debug!(
                target: "search::fuzzy",
                series_id = %series_id,
                "series no longer exists at refetch; removed from fuzzy index"
            );
        }
        Err(err) => {
            warn!(
                target: "search::fuzzy",
                series_id = %series_id,
                error = %err,
                "failed to refetch series for fuzzy index; entry may be stale"
            );
        }
    }
}

async fn upsert_or_remove_series(
    index: &FuzzyIndex,
    db: &DatabaseConnection,
    series_id: uuid::Uuid,
) {
    match fetch_series_entry(db, series_id).await {
        Ok(Some(entry)) => {
            index.upsert_series(entry);
        }
        Ok(None) => {
            index.remove_series(series_id);
        }
        Err(err) => {
            warn!(
                target: "search::fuzzy",
                series_id = %series_id,
                error = %err,
                "failed to refetch series after bulk purge; index may be stale"
            );
        }
    }
}

async fn upsert_book(index: &FuzzyIndex, db: &DatabaseConnection, book_id: uuid::Uuid) {
    match fetch_book_entry(db, book_id).await {
        Ok(Some(entry)) => {
            index.upsert_book(entry);
            debug!(target: "search::fuzzy", book_id = %book_id, "upserted book in fuzzy index");
        }
        Ok(None) => {
            // Either missing or soft-deleted; either way it should not be in
            // the index.
            index.remove_book(book_id);
            debug!(
                target: "search::fuzzy",
                book_id = %book_id,
                "book missing or soft-deleted at refetch; removed from fuzzy index"
            );
        }
        Err(err) => {
            warn!(
                target: "search::fuzzy",
                book_id = %book_id,
                error = %err,
                "failed to refetch book for fuzzy index; entry may be stale"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::ScanningStrategy;
    use crate::db::entities::books;
    use crate::db::repositories::{
        AlternateTitleRepository, BookRepository, LibraryRepository, SeriesMetadataRepository,
        SeriesRepository,
    };
    use crate::db::test_helpers::create_test_db;
    use crate::events::EntityChangeEvent;
    use crate::search::builder::build_from_db;
    use chrono::Utc;
    use std::time::Duration;
    use uuid::Uuid;

    fn book_model(series_id: Uuid, library_id: Uuid, path: &str, name: &str) -> books::Model {
        let now = Utc::now();
        books::Model {
            id: Uuid::new_v4(),
            series_id,
            library_id,
            file_path: path.to_string(),
            file_name: name.to_string(),
            file_size: 1024,
            file_hash: format!("hash_{}", Uuid::new_v4()),
            partial_hash: String::new(),
            format: "cbz".to_string(),
            page_count: 10,
            deleted: false,
            analyzed: false,
            analysis_error: None,
            analysis_errors: None,
            modified_at: now,
            created_at: now,
            updated_at: now,
            thumbnail_path: None,
            thumbnail_generated_at: None,
            koreader_hash: None,
            epub_positions: None,
            epub_spine_items: None,
        }
    }

    async fn setup() -> (
        crate::db::Database,
        Arc<FuzzyIndex>,
        Arc<EventBroadcaster>,
        Uuid,
        tempfile::TempDir,
    ) {
        let (db, temp) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let library = LibraryRepository::create(
            conn,
            "Test Library",
            "/test/path",
            ScanningStrategy::Default,
        )
        .await
        .unwrap();
        let index = Arc::new(FuzzyIndex::empty());
        let broadcaster = Arc::new(EventBroadcaster::new(64));
        (db, index, broadcaster, library.id, temp)
    }

    #[tokio::test]
    async fn series_created_event_inserts_into_index() {
        let (db, index, _broadcaster, library_id, _temp) = setup().await;
        let conn = db.sea_orm_connection();
        let series = SeriesRepository::create(conn, library_id, "Berserk", None)
            .await
            .unwrap();

        apply_event(
            &index,
            conn,
            &EntityEvent::SeriesCreated {
                series_id: series.id,
                library_id,
            },
        )
        .await;

        assert_eq!(index.series_count(), 1);
        let hits = index.search_series("berserk", 10, None);
        assert_eq!(hits.first().map(|h| h.0), Some(series.id));
    }

    #[tokio::test]
    async fn series_metadata_updated_event_refreshes_haystack() {
        let (db, index, _broadcaster, library_id, _temp) = setup().await;
        let conn = db.sea_orm_connection();
        // The series's filesystem name lives in `series.name` and is kept in
        // the haystack as a fallback — so to test "metadata title changed"
        // in isolation, we use a directory name that won't fuzzy-match the
        // new title.
        let series = SeriesRepository::create(conn, library_id, "xqz-dir", None)
            .await
            .unwrap();
        apply_event(
            &index,
            conn,
            &EntityEvent::SeriesCreated {
                series_id: series.id,
                library_id,
            },
        )
        .await;
        assert!(
            index.search_series("vagabond", 10, None).is_empty(),
            "haystack should not contain 'vagabond' before metadata update"
        );

        // Rename via metadata; emit the corresponding event.
        SeriesMetadataRepository::update_title(conn, series.id, "Vagabond".to_string(), None, None)
            .await
            .unwrap();
        apply_event(
            &index,
            conn,
            &EntityEvent::SeriesMetadataUpdated {
                series_id: series.id,
                library_id,
                plugin_id: None,
                fields_updated: vec!["title".to_string()],
            },
        )
        .await;

        let hits = index.search_series("vagabond", 10, None);
        assert_eq!(hits.first().map(|h| h.0), Some(series.id));
    }

    #[tokio::test]
    async fn alt_title_changes_are_seen_through_metadata_event() {
        let (db, index, _broadcaster, library_id, _temp) = setup().await;
        let conn = db.sea_orm_connection();
        let series = SeriesRepository::create(conn, library_id, "Shingeki no Kyojin", None)
            .await
            .unwrap();
        apply_event(
            &index,
            conn,
            &EntityEvent::SeriesCreated {
                series_id: series.id,
                library_id,
            },
        )
        .await;
        assert!(index.search_series("進撃", 10, None).is_empty());

        AlternateTitleRepository::create(conn, series.id, "Japanese", "進撃の巨人", None)
            .await
            .unwrap();
        apply_event(
            &index,
            conn,
            &EntityEvent::SeriesMetadataUpdated {
                series_id: series.id,
                library_id,
                plugin_id: None,
                fields_updated: vec!["alternate_titles".to_string()],
            },
        )
        .await;
        let hits = index.search_series("進撃", 10, None);
        assert_eq!(hits.first().map(|h| h.0), Some(series.id));
    }

    #[tokio::test]
    async fn series_deleted_event_removes_from_index() {
        let (db, index, _broadcaster, library_id, _temp) = setup().await;
        let conn = db.sea_orm_connection();
        let series = SeriesRepository::create(conn, library_id, "Berserk", None)
            .await
            .unwrap();
        index.upsert_series(fetch_series_entry(conn, series.id).await.unwrap().unwrap());
        assert_eq!(index.series_count(), 1);

        apply_event(
            &index,
            conn,
            &EntityEvent::SeriesDeleted {
                series_id: series.id,
                library_id,
            },
        )
        .await;
        assert_eq!(index.series_count(), 0);
    }

    #[tokio::test]
    async fn book_created_then_deleted_event_round_trip() {
        let (db, index, _broadcaster, library_id, _temp) = setup().await;
        let conn = db.sea_orm_connection();
        let series = SeriesRepository::create(conn, library_id, "Berserk", None)
            .await
            .unwrap();
        let book = BookRepository::create(
            conn,
            &book_model(
                series.id,
                library_id,
                "/test/berserk/vol01.cbz",
                "vol01.cbz",
            ),
            None,
        )
        .await
        .unwrap();

        apply_event(
            &index,
            conn,
            &EntityEvent::BookCreated {
                book_id: book.id,
                series_id: series.id,
                library_id,
            },
        )
        .await;
        assert_eq!(index.book_count(), 1);

        apply_event(
            &index,
            conn,
            &EntityEvent::BookDeleted {
                book_id: book.id,
                series_id: series.id,
                library_id,
            },
        )
        .await;
        assert_eq!(index.book_count(), 0);
    }

    #[tokio::test]
    async fn book_updated_for_soft_deleted_row_removes_from_index() {
        let (db, index, _broadcaster, library_id, _temp) = setup().await;
        let conn = db.sea_orm_connection();
        let series = SeriesRepository::create(conn, library_id, "Berserk", None)
            .await
            .unwrap();
        let book = BookRepository::create(
            conn,
            &book_model(
                series.id,
                library_id,
                "/test/berserk/vol01.cbz",
                "vol01.cbz",
            ),
            None,
        )
        .await
        .unwrap();
        index.upsert_book(fetch_book_entry(conn, book.id).await.unwrap().unwrap());
        assert_eq!(index.book_count(), 1);

        // Mark deleted via BookRepository::mark_deleted, then apply a
        // BookUpdated event: the listener should refetch, see deleted=true,
        // and prune.
        BookRepository::mark_deleted(conn, book.id, true, None)
            .await
            .unwrap();
        apply_event(
            &index,
            conn,
            &EntityEvent::BookUpdated {
                book_id: book.id,
                series_id: series.id,
                library_id,
                fields: Some(vec!["deleted".to_string()]),
            },
        )
        .await;
        assert_eq!(
            index.book_count(),
            0,
            "soft-deleted book should be removed from the index"
        );
    }

    #[tokio::test]
    async fn ignored_events_are_no_ops() {
        let (db, index, _broadcaster, library_id, _temp) = setup().await;
        let conn = db.sea_orm_connection();
        let series = SeriesRepository::create(conn, library_id, "Berserk", None)
            .await
            .unwrap();
        index.upsert_series(fetch_series_entry(conn, series.id).await.unwrap().unwrap());
        let before = index.series_count();

        apply_event(&index, conn, &EntityEvent::LibraryUpdated { library_id }).await;
        apply_event(
            &index,
            conn,
            &EntityEvent::PluginEnabled {
                plugin_id: Uuid::new_v4(),
            },
        )
        .await;
        assert_eq!(index.series_count(), before);
    }

    #[tokio::test]
    async fn spawn_listener_applies_emitted_events_end_to_end() {
        let (db, index, broadcaster, library_id, _temp) = setup().await;
        let conn = db.sea_orm_connection();
        let cancel = CancellationToken::new();
        let handle = spawn_listener(
            index.clone(),
            broadcaster.clone(),
            conn.clone(),
            cancel.clone(),
        );

        // Race avoidance: create the series first, then emit. The listener
        // subscribes at task start, but the broadcast channel buffers events
        // emitted before recv() is awaited.
        let series = SeriesRepository::create(conn, library_id, "Berserk", None)
            .await
            .unwrap();
        broadcaster
            .emit(EntityChangeEvent::new(
                EntityEvent::SeriesCreated {
                    series_id: series.id,
                    library_id,
                },
                None,
            ))
            .ok();

        // Wait until the listener has processed the event. Bounded poll: we
        // don't want a flaky deadline-style sleep but also don't want to hang
        // forever if something regresses.
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        loop {
            if index.series_count() == 1 {
                break;
            }
            if std::time::Instant::now() > deadline {
                panic!(
                    "listener never applied SeriesCreated event (series_count = {})",
                    index.series_count()
                );
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }

        cancel.cancel();
        let _ = handle.await;
    }

    #[tokio::test]
    async fn lagged_subscriber_triggers_full_rebuild() {
        let (db, index, _broadcaster, library_id, _temp) = setup().await;
        let conn = db.sea_orm_connection();

        // Seed two series into the DB but only one into the index, simulating
        // the state right after a lag (some events missed).
        let s1 = SeriesRepository::create(conn, library_id, "Berserk", None)
            .await
            .unwrap();
        let _s2 = SeriesRepository::create(conn, library_id, "Vagabond", None)
            .await
            .unwrap();
        index.upsert_series(fetch_series_entry(conn, s1.id).await.unwrap().unwrap());
        assert_eq!(index.series_count(), 1);

        // Use a tiny channel so we can easily produce a Lagged error.
        let broadcaster = Arc::new(EventBroadcaster::new(1));
        let cancel = CancellationToken::new();
        let handle = spawn_listener(
            index.clone(),
            broadcaster.clone(),
            conn.clone(),
            cancel.clone(),
        );

        // Give the spawned listener a moment to subscribe.
        tokio::time::sleep(Duration::from_millis(20)).await;

        // Burst more events than the buffer holds; the second send pushes the
        // first out of the buffer and the subscriber observes RecvError::Lagged
        // on its next recv().
        for _ in 0..4 {
            broadcaster
                .emit(EntityChangeEvent::new(
                    EntityEvent::SeriesCreated {
                        series_id: Uuid::new_v4(),
                        library_id,
                    },
                    None,
                ))
                .ok();
        }

        // Wait until the rebuild has brought series_count up to 2.
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        loop {
            if index.series_count() == 2 {
                break;
            }
            if std::time::Instant::now() > deadline {
                panic!(
                    "rebuild after lag did not converge (series_count = {})",
                    index.series_count()
                );
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }

        cancel.cancel();
        let _ = handle.await;
    }

    #[tokio::test]
    async fn full_rebuild_helper_resets_state() {
        // Sanity check that build_from_db replaces everything (used as the
        // lag-recovery primitive).
        let (db, _index, _broadcaster, library_id, _temp) = setup().await;
        let conn = db.sea_orm_connection();
        SeriesRepository::create(conn, library_id, "Berserk", None)
            .await
            .unwrap();
        let idx = build_from_db(conn).await.unwrap();
        assert_eq!(idx.series_count(), 1);
    }
}
