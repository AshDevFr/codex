//! Cross-replica entity change bridge (publisher side).
//!
//! Entity events emitted by an API handler reach only that process's
//! broadcaster. With a single `codex serve` that is the whole world, but with
//! several replicas behind a load balancer it is not: a metadata edit served by
//! one replica leaves every sibling's SSE subscribers unaware of it and every
//! sibling's fuzzy search index describing a row that no longer looks like
//! that. Worker-originated changes already cross the gap, because task
//! completion replays recorded events over `task_completion`; direct API writes
//! had no such path.
//!
//! This publisher drains the broadcaster's entity sink and re-publishes each
//! event with `pg_notify` on [`ENTITY_EVENTS_CHANNEL`], reusing the transport
//! that already carries task completion and progress.
//!
//! Each payload carries the id of the process that produced it. PostgreSQL
//! delivers a notification to every listener including the one that sent it, so
//! without that marker the emitting replica would apply its own event twice:
//! once locally and once off the wire, duplicating it for its SSE subscribers.
//! The listener drops anything bearing its own origin.

use codex_events::EntityChangeEvent;
use sea_orm::{ConnectionTrait, DatabaseBackend, DatabaseConnection, Statement};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tracing::{debug, info, warn};
use uuid::Uuid;

/// PostgreSQL channel name for entity change notifications.
pub const ENTITY_EVENTS_CHANNEL: &str = "entity_events";

/// PostgreSQL caps a notification payload at 8000 bytes. Entity events are
/// small, so exceeding this means something unexpected rather than something to
/// handle gracefully; the event is dropped and logged rather than killing the
/// publisher with a database error.
const MAX_NOTIFY_PAYLOAD: usize = 7900;

/// What actually travels over the wire.
///
/// The event is nested rather than flattened so the SSE payload shape stays
/// exactly what it was: `origin` is transport bookkeeping and must never reach
/// a browser.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityEventEnvelope {
    /// Process that emitted the event.
    pub origin: Uuid,
    pub event: EntityChangeEvent,
}

/// Spawn the publisher. The returned handle completes when `rx` closes.
pub fn spawn(
    db: DatabaseConnection,
    origin: Uuid,
    rx: mpsc::Receiver<EntityChangeEvent>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(run(db, origin, rx))
}

async fn run(db: DatabaseConnection, origin: Uuid, mut rx: mpsc::Receiver<EntityChangeEvent>) {
    info!(
        "Entity event publisher started (channel '{}', origin {})",
        ENTITY_EVENTS_CHANNEL, origin
    );

    while let Some(event) = rx.recv().await {
        let envelope = EntityEventEnvelope { origin, event };

        let payload = match serde_json::to_string(&envelope) {
            Ok(payload) => payload,
            Err(e) => {
                warn!("Failed to serialize entity event for publication: {e}");
                continue;
            }
        };

        if payload.len() > MAX_NOTIFY_PAYLOAD {
            warn!(
                bytes = payload.len(),
                "Entity event exceeds the PostgreSQL notify payload limit, dropping"
            );
            continue;
        }

        // `pg_notify` runs in autocommit here, so the notification is delivered
        // immediately.
        let stmt = Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            "SELECT pg_notify($1, $2)",
            [ENTITY_EVENTS_CHANNEL.into(), payload.into()],
        );
        if let Err(e) = db.execute(stmt).await {
            debug!("Failed to publish entity event notify: {e}");
        }
    }

    info!("Entity event publisher stopped");
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_events::EntityEvent;

    fn sample_event() -> EntityChangeEvent {
        EntityChangeEvent::new(
            EntityEvent::SeriesUpdated {
                series_id: Uuid::new_v4(),
                library_id: Uuid::new_v4(),
                fields: Some(vec!["title".to_string()]),
            },
            None,
        )
    }

    /// The envelope must survive the round trip, and `origin` must not leak
    /// into the event the browser sees.
    #[test]
    fn envelope_round_trips_and_keeps_the_event_shape() {
        let origin = Uuid::new_v4();
        let event = sample_event();
        let envelope = EntityEventEnvelope {
            origin,
            event: event.clone(),
        };

        let json = serde_json::to_string(&envelope).unwrap();
        let decoded: EntityEventEnvelope = serde_json::from_str(&json).unwrap();

        assert_eq!(decoded.origin, origin);
        assert_eq!(
            serde_json::to_value(&decoded.event).unwrap(),
            serde_json::to_value(&event).unwrap(),
            "the wrapped event must deserialize back to exactly what was sent"
        );

        // `origin` lives on the envelope, not on the event.
        let event_json = serde_json::to_value(&decoded.event).unwrap();
        assert!(event_json.get("origin").is_none());
    }
}
