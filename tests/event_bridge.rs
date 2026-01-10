//! Tests for the generic event bridge in distributed deployments
//!
//! These tests verify that events emitted during task execution are properly
//! recorded and can be replayed on a different broadcaster (simulating the
//! web server replaying events from worker processes).

mod common;

use chrono::Utc;
use codex::events::{EntityChangeEvent, EntityEvent, EntityType, EventBroadcaster, RecordedEvent};
use serde_json::json;
use uuid::Uuid;

/// Test that event recording captures events correctly
#[tokio::test]
async fn test_event_recording_captures_all_events() {
    // Create a recording broadcaster (simulating distributed worker)
    let broadcaster = EventBroadcaster::new_with_recording(100, true);

    let library_id = Uuid::new_v4();
    let series_id = Uuid::new_v4();
    let book_id = Uuid::new_v4();

    // Emit a BookCreated event
    let _ = broadcaster.emit(EntityChangeEvent::new(
        EntityEvent::BookCreated {
            book_id,
            series_id,
            library_id,
        },
        None,
    ));

    // Emit a CoverUpdated event
    let _ = broadcaster.emit(EntityChangeEvent::new(
        EntityEvent::CoverUpdated {
            entity_type: EntityType::Book,
            entity_id: book_id,
            library_id: Some(library_id),
        },
        None,
    ));

    // Take recorded events
    let recorded = broadcaster.take_recorded_events();

    assert_eq!(recorded.len(), 2, "Should have recorded 2 events");

    // Verify first event is BookCreated
    match &recorded[0].event {
        EntityEvent::BookCreated {
            book_id: b,
            series_id: s,
            library_id: l,
        } => {
            assert_eq!(*b, book_id);
            assert_eq!(*s, series_id);
            assert_eq!(*l, library_id);
        }
        _ => panic!("Expected BookCreated event"),
    }

    // Verify second event is CoverUpdated
    match &recorded[1].event {
        EntityEvent::CoverUpdated {
            entity_type,
            entity_id,
            library_id: l,
        } => {
            assert_eq!(*entity_type, EntityType::Book);
            assert_eq!(*entity_id, book_id);
            assert_eq!(*l, Some(library_id));
        }
        _ => panic!("Expected CoverUpdated event"),
    }
}

/// Test that recorded events can be serialized to JSON and deserialized back
#[test]
fn test_recorded_events_serialization_roundtrip() {
    let book_id = Uuid::new_v4();
    let series_id = Uuid::new_v4();
    let library_id = Uuid::new_v4();
    let timestamp = Utc::now();

    let events = vec![
        RecordedEvent {
            event: EntityEvent::BookCreated {
                book_id,
                series_id,
                library_id,
            },
            timestamp,
            user_id: None,
        },
        RecordedEvent {
            event: EntityEvent::CoverUpdated {
                entity_type: EntityType::Book,
                entity_id: book_id,
                library_id: Some(library_id),
            },
            timestamp,
            user_id: None,
        },
    ];

    // Serialize to JSON (as would be stored in task result)
    let json_value = serde_json::to_value(&events).unwrap();

    // Deserialize back (as TaskListener would do)
    let deserialized: Vec<RecordedEvent> = serde_json::from_value(json_value).unwrap();

    assert_eq!(deserialized.len(), 2);
    assert!(matches!(
        deserialized[0].event,
        EntityEvent::BookCreated { .. }
    ));
    assert!(matches!(
        deserialized[1].event,
        EntityEvent::CoverUpdated { .. }
    ));
}

/// Test simulating the full event bridge flow:
/// 1. Worker records events
/// 2. Events are stored in task result
/// 3. TaskListener replays events to web server broadcaster
#[tokio::test]
async fn test_full_event_bridge_flow() {
    let library_id = Uuid::new_v4();
    let book_id = Uuid::new_v4();
    let series_id = Uuid::new_v4();

    // --- WORKER SIDE ---
    // Create recording broadcaster (simulating distributed worker)
    let worker_broadcaster = EventBroadcaster::new_with_recording(100, true);

    // Worker emits events during task execution
    let _ = worker_broadcaster.emit(EntityChangeEvent::new(
        EntityEvent::BookCreated {
            book_id,
            series_id,
            library_id,
        },
        None,
    ));

    let _ = worker_broadcaster.emit(EntityChangeEvent::new(
        EntityEvent::CoverUpdated {
            entity_type: EntityType::Book,
            entity_id: book_id,
            library_id: Some(library_id),
        },
        None,
    ));

    // Worker takes recorded events and stores in task result
    let recorded_events = worker_broadcaster.take_recorded_events();
    let task_result = json!({
        "generated": 1,
        "emitted_events": recorded_events
    });

    // --- WEB SERVER SIDE ---
    // Create web server broadcaster (non-recording)
    let web_broadcaster = EventBroadcaster::new(100);

    // Subscribe to receive replayed events
    let mut receiver = web_broadcaster.subscribe();

    // TaskListener extracts and replays events
    let events_to_replay: Vec<RecordedEvent> = task_result
        .get("emitted_events")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();

    assert_eq!(events_to_replay.len(), 2);

    // Replay events
    for recorded in events_to_replay {
        let event = EntityChangeEvent {
            event: recorded.event,
            timestamp: recorded.timestamp,
            user_id: recorded.user_id,
        };
        let _ = web_broadcaster.emit(event);
    }

    // Verify events were received
    let event1 = receiver.recv().await.unwrap();
    assert!(matches!(event1.event, EntityEvent::BookCreated { .. }));

    let event2 = receiver.recv().await.unwrap();
    assert!(matches!(event2.event, EntityEvent::CoverUpdated { .. }));
}

/// Test that non-recording broadcaster doesn't record events
#[tokio::test]
async fn test_non_recording_broadcaster_no_events() {
    let broadcaster = EventBroadcaster::new(100);

    let _ = broadcaster.emit(EntityChangeEvent::new(
        EntityEvent::BookCreated {
            book_id: Uuid::new_v4(),
            series_id: Uuid::new_v4(),
            library_id: Uuid::new_v4(),
        },
        None,
    ));

    let recorded = broadcaster.take_recorded_events();
    assert!(
        recorded.is_empty(),
        "Non-recording broadcaster should not record events"
    );
}

/// Test that recording is cleared after take
#[tokio::test]
async fn test_recording_cleared_after_take() {
    let broadcaster = EventBroadcaster::new_with_recording(100, true);

    let _ = broadcaster.emit(EntityChangeEvent::new(
        EntityEvent::BookCreated {
            book_id: Uuid::new_v4(),
            series_id: Uuid::new_v4(),
            library_id: Uuid::new_v4(),
        },
        None,
    ));

    assert_eq!(broadcaster.recorded_event_count(), 1);

    // Take events
    let _ = broadcaster.take_recorded_events();

    // Should be empty now
    assert_eq!(broadcaster.recorded_event_count(), 0);
    assert!(broadcaster.take_recorded_events().is_empty());
}

/// Test that all entity event types can be recorded and replayed
#[tokio::test]
async fn test_all_event_types_recordable() {
    let broadcaster = EventBroadcaster::new_with_recording(100, true);

    let library_id = Uuid::new_v4();
    let series_id = Uuid::new_v4();
    let book_id = Uuid::new_v4();

    // Emit all event types
    let events = vec![
        EntityEvent::BookCreated {
            book_id,
            series_id,
            library_id,
        },
        EntityEvent::BookUpdated {
            book_id,
            series_id,
            library_id,
            fields: Some(vec!["title".to_string()]),
        },
        EntityEvent::BookDeleted {
            book_id,
            series_id,
            library_id,
        },
        EntityEvent::SeriesCreated {
            series_id,
            library_id,
        },
        EntityEvent::SeriesUpdated {
            series_id,
            library_id,
            fields: None,
        },
        EntityEvent::SeriesDeleted {
            series_id,
            library_id,
        },
        EntityEvent::SeriesBulkPurged {
            series_id,
            library_id,
            count: 5,
        },
        EntityEvent::CoverUpdated {
            entity_type: EntityType::Book,
            entity_id: book_id,
            library_id: Some(library_id),
        },
        EntityEvent::LibraryUpdated { library_id },
        EntityEvent::LibraryDeleted { library_id },
    ];

    for event in &events {
        let _ = broadcaster.emit(EntityChangeEvent::new(event.clone(), None));
    }

    let recorded = broadcaster.take_recorded_events();
    assert_eq!(
        recorded.len(),
        events.len(),
        "All events should be recorded"
    );

    // Verify serialization works for all types
    let json = serde_json::to_string(&recorded).unwrap();
    let deserialized: Vec<RecordedEvent> = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.len(), events.len());
}
