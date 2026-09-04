//! Cross-replica delivery of entity change events.
//!
//! Entity events emitted by an API handler only ever reached the process that
//! emitted them. Task-originated changes crossed process boundaries already,
//! replayed on task completion, but a direct API write belongs to no task and
//! had no such path. With more than one web replica that left every sibling's
//! SSE subscribers unaware of the change and every sibling's fuzzy index
//! describing a stale row.
//!
//! PostgreSQL only: the bridge is LISTEN/NOTIFY, and a SQLite deployment is a
//! single process with nobody to tell.
//!
//! Every assertion here matches on the series id the test itself generated.
//! `entity_events` is a database-wide channel and the PostgreSQL tests share
//! one database, so a test that merely counted arrivals would see whatever a
//! concurrently running test published and fail for reasons of its own making.

#[path = "../common/mod.rs"]
mod common;

use codex::events::{EntityChangeEvent, EntityEvent, EventBroadcaster};
use codex_services::TaskListener;
use common::*;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::broadcast;
use tokio::sync::mpsc;
use uuid::Uuid;

/// One web replica: its own broadcaster, publisher and listener, sharing only
/// the database with its siblings.
struct Replica {
    broadcaster: Arc<EventBroadcaster>,
}

impl Replica {
    async fn start(db: &sea_orm::DatabaseConnection) -> Self {
        let id = Uuid::new_v4();
        let (tx, rx) = mpsc::channel(100);
        let broadcaster = Arc::new(EventBroadcaster::new(100).with_entity_notifier(tx));

        codex_services::entity_event_publisher::spawn(db.clone(), id, rx);

        let listener = TaskListener::from_sea_orm(db, broadcaster.clone(), id)
            .expect("listener connects to PostgreSQL");
        tokio::spawn(async move {
            let _ = listener.start().await;
        });

        Self { broadcaster }
    }

    fn emit(&self, series_id: Uuid) {
        let _ = self.broadcaster.emit(EntityChangeEvent::new(
            EntityEvent::SeriesUpdated {
                series_id,
                library_id: Uuid::new_v4(),
                fields: Some(vec!["title".to_string()]),
            },
            None,
        ));
    }
}

fn series_id_of(event: &EntityChangeEvent) -> Option<Uuid> {
    match event.event {
        EntityEvent::SeriesUpdated { series_id, .. } => Some(series_id),
        _ => None,
    }
}

/// Wait for an event carrying `wanted`, ignoring anything another test put on
/// the shared channel. Returns false if the deadline passes first.
async fn saw(
    rx: &mut broadcast::Receiver<EntityChangeEvent>,
    wanted: Uuid,
    within: Duration,
) -> bool {
    let deadline = Instant::now() + within;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return false;
        }
        match tokio::time::timeout(remaining, rx.recv()).await {
            Ok(Ok(event)) if series_id_of(&event) == Some(wanted) => return true,
            // Another test's event, or a lag notice. Keep waiting.
            Ok(Ok(_)) | Ok(Err(broadcast::error::RecvError::Lagged(_))) => continue,
            Ok(Err(broadcast::error::RecvError::Closed)) | Err(_) => return false,
        }
    }
}

/// Count how many times `wanted` arrives over the whole window.
async fn count_arrivals(
    rx: &mut broadcast::Receiver<EntityChangeEvent>,
    wanted: Uuid,
    window: Duration,
) -> usize {
    let deadline = Instant::now() + window;
    let mut seen = 0;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return seen;
        }
        match tokio::time::timeout(remaining, rx.recv()).await {
            Ok(Ok(event)) => {
                if series_id_of(&event) == Some(wanted) {
                    seen += 1;
                }
            }
            Ok(Err(broadcast::error::RecvError::Lagged(_))) => continue,
            Ok(Err(broadcast::error::RecvError::Closed)) | Err(_) => return seen,
        }
    }
}

// The `_postgres` name suffix is load-bearing: nextest serialises tests
// matching `test(~postgres)` into one group because `setup_test_db_postgres`
// truncates a database shared by the whole run. A PostgreSQL test without the
// suffix runs in parallel and deletes other tests' fixtures mid-flight.
#[tokio::test]
#[ignore] // Requires PostgreSQL test database
async fn entity_event_reaches_a_sibling_replica_postgres() {
    let Some(db) = setup_test_db_postgres().await else {
        eprintln!("PostgreSQL test database not available, skipping");
        return;
    };

    let author = Replica::start(&db).await;
    let sibling = Replica::start(&db).await;
    let mut watching = sibling.broadcaster.subscribe();

    // LISTEN registration is asynchronous, so republish until it lands rather
    // than guessing at a sleep. Re-emitting is harmless: the assertion is that
    // the sibling sees this series id at all.
    let series_id = Uuid::new_v4();
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut delivered = false;
    while Instant::now() < deadline && !delivered {
        author.emit(series_id);
        delivered = saw(&mut watching, series_id, Duration::from_millis(250)).await;
    }

    assert!(
        delivered,
        "a change served by one replica must reach its siblings"
    );
}

/// The emitting replica delivers its own event exactly once.
///
/// PostgreSQL sends a notification to every listener, the sender included, so
/// the origin marker on the payload is the only thing stopping the author from
/// re-applying its own event and showing every change twice to its own SSE
/// subscribers.
#[tokio::test]
#[ignore] // Requires PostgreSQL test database
async fn author_replica_does_not_receive_its_own_event_twice_postgres() {
    let Some(db) = setup_test_db_postgres().await else {
        eprintln!("PostgreSQL test database not available, skipping");
        return;
    };

    let author = Replica::start(&db).await;
    let prober = Replica::start(&db).await;
    let mut watching = author.broadcaster.subscribe();

    // Establish that the author's LISTEN is live before asserting that nothing
    // comes back on it. Without this the test would pass just as happily
    // against a bridge that was never connected.
    let probe_id = Uuid::new_v4();
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut bridged = false;
    while Instant::now() < deadline && !bridged {
        prober.emit(probe_id);
        bridged = saw(&mut watching, probe_id, Duration::from_millis(250)).await;
    }
    assert!(bridged, "the author's listener must be receiving");

    // Now the real assertion. The window is comfortably longer than the round
    // trip just measured, so a duplicate would have arrived within it.
    let own_id = Uuid::new_v4();
    author.emit(own_id);
    let arrivals = count_arrivals(&mut watching, own_id, Duration::from_secs(2)).await;

    assert_eq!(
        arrivals, 1,
        "the author must see its own event once, not again off the wire"
    );
}
