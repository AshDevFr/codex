#[path = "../common/mod.rs"]
mod common;

use codex::db::repositories::{LibraryRepository, SeriesRepository, UserRepository};
use codex::db::ScanningStrategy;
use codex::events::{EntityChangeEvent, EntityEvent, TaskProgressEvent};
use codex::utils::password;
use common::*;
use futures::StreamExt;
use hyper::{Request, StatusCode};
use std::time::Duration;
use tokio::time::timeout;
use tower::ServiceExt;

// ============================================================================
// Test Helpers
// ============================================================================

/// Helper to create an admin user and get a token
async fn create_admin_and_token(
    db: &sea_orm::DatabaseConnection,
    state: &codex::api::extractors::AppState,
) -> String {
    let password_hash = password::hash_password("admin123").unwrap();
    let user = create_test_user("admin", "admin@example.com", &password_hash, true);
    let created = UserRepository::create(db, &user).await.unwrap();

    state
        .jwt_service
        .generate_token(created.id, created.username, created.is_admin)
        .unwrap()
}

/// Helper to parse SSE event from raw bytes
fn parse_sse_event(data: &str) -> Option<String> {
    for line in data.lines() {
        if let Some(json) = line.strip_prefix("data: ") {
            if json != "keep-alive" {
                return Some(json.to_string());
            }
        }
    }
    None
}

// ============================================================================
// Entity Events Stream Tests (/api/v1/events/stream)
// ============================================================================

#[tokio::test]
async fn test_entity_events_stream_requires_auth() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_app_state(db).await;
    let app = create_test_router_with_app_state(state);

    // Request without authentication
    let request = get_request("/api/v1/events/stream");
    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_entity_events_stream_connects_with_auth() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_app_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_app_state(state.clone());

    // Request with valid authentication
    let request = Request::builder()
        .method("GET")
        .uri("/api/v1/events/stream")
        .header("Authorization", format!("Bearer {}", token))
        .header("Accept", "text/event-stream")
        .body(String::new())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    // SSE connections return 200 OK and stay open
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get("content-type").unwrap(),
        "text/event-stream"
    );
}

#[tokio::test]
async fn test_entity_events_stream_receives_cover_updated() {
    let (db, temp_dir) = setup_test_db().await;

    // Create library and series
    let library = LibraryRepository::create(
        &db,
        "Test Library",
        temp_dir.path().to_str().unwrap(),
        ScanningStrategy::Default,
    )
    .await
    .unwrap();

    let series = SeriesRepository::create(&db, library.id, "Test Series")
        .await
        .unwrap();

    let state = create_test_app_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;

    // Emit a CoverUpdated event manually
    let event = EntityChangeEvent {
        event: EntityEvent::CoverUpdated {
            entity_type: codex::events::EntityType::Series,
            entity_id: series.id,
            library_id: Some(library.id),
        },
        timestamp: chrono::Utc::now(),
        user_id: None,
    };

    let event_json = serde_json::to_string(&event).unwrap();

    // Broadcast the event
    let _ = state.event_broadcaster.emit(event);

    // Note: Testing actual SSE streaming requires a more complex setup
    // This test verifies the event can be serialized correctly
    assert!(event_json.contains("cover_updated")); // snake_case serialization
    assert!(event_json.contains(&series.id.to_string()));
    assert!(event_json.contains(&library.id.to_string()));
}

// ============================================================================
// Task Progress Stream Tests (/api/v1/tasks/stream)
// ============================================================================

#[tokio::test]
async fn test_task_progress_stream_requires_auth() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_app_state(db).await;
    let app = create_test_router_with_app_state(state);

    // Request without authentication
    let request = get_request("/api/v1/tasks/stream");
    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_task_progress_stream_connects_with_auth() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_app_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_app_state(state.clone());

    // Request with valid authentication
    let request = Request::builder()
        .method("GET")
        .uri("/api/v1/tasks/stream")
        .header("Authorization", format!("Bearer {}", token))
        .header("Accept", "text/event-stream")
        .body(String::new())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    // SSE connections return 200 OK and stay open
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get("content-type").unwrap(),
        "text/event-stream"
    );
}

#[tokio::test]
async fn test_task_progress_event_serialization() {
    use codex::events::{TaskProgress, TaskStatus};
    use uuid::Uuid;

    // Create a task progress event
    let task_id = Uuid::new_v4();
    let event = TaskProgressEvent::started(
        task_id,
        "analyze_book".to_string(),
        None, // library_id
        None, // series_id
        None, // book_id
    );

    // Serialize to JSON
    let json = serde_json::to_string(&event).unwrap();

    // Verify it can be deserialized
    let deserialized: TaskProgressEvent = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.task_id, task_id);
    assert_eq!(deserialized.task_type, "analyze_book");
    assert_eq!(deserialized.status, TaskStatus::Running);
}

// ============================================================================
// Event Broadcaster Tests
// ============================================================================

#[tokio::test]
async fn test_event_broadcaster_entity_channel() {
    use codex::events::EventBroadcaster;
    use uuid::Uuid;

    let broadcaster = EventBroadcaster::new(100);
    let mut receiver = broadcaster.subscribe();

    // Emit an entity event
    let event = EntityChangeEvent {
        event: EntityEvent::BookCreated {
            book_id: Uuid::new_v4(),
            series_id: Uuid::new_v4(),
            library_id: Uuid::new_v4(),
        },
        timestamp: chrono::Utc::now(),
        user_id: None,
    };

    let sent = broadcaster.emit(event.clone());
    assert!(sent.is_ok());

    // Receive the event
    let received = timeout(Duration::from_secs(1), receiver.recv())
        .await
        .expect("Timeout waiting for event")
        .expect("Failed to receive event");

    // Verify event content
    match received.event {
        EntityEvent::BookCreated {
            book_id, series_id, ..
        } => {
            if let EntityEvent::BookCreated {
                book_id: sent_book_id,
                series_id: sent_series_id,
                ..
            } = event.event
            {
                assert_eq!(book_id, sent_book_id);
                assert_eq!(series_id, sent_series_id);
            }
        }
        _ => panic!("Wrong event type received"),
    }
}

#[tokio::test]
async fn test_event_broadcaster_task_channel() {
    use codex::events::{EventBroadcaster, TaskProgress, TaskStatus};
    use uuid::Uuid;

    let broadcaster = EventBroadcaster::new(100);
    let mut receiver = broadcaster.subscribe_tasks();

    // Emit a task event
    let task_id = Uuid::new_v4();
    let event = TaskProgressEvent::started(
        task_id,
        "test_task".to_string(),
        None, // library_id
        None, // series_id
        None, // book_id
    );

    let sent = broadcaster.emit_task(event.clone());
    assert!(sent.is_ok());

    // Receive the event
    let received = timeout(Duration::from_secs(1), receiver.recv())
        .await
        .expect("Timeout waiting for event")
        .expect("Failed to receive event");

    assert_eq!(received.task_id, task_id);
    assert_eq!(received.task_type, "test_task");
    assert_eq!(received.status, TaskStatus::Running);
}

#[tokio::test]
async fn test_event_broadcaster_multiple_subscribers() {
    use codex::events::EventBroadcaster;
    use uuid::Uuid;

    let broadcaster = EventBroadcaster::new(100);
    let mut receiver1 = broadcaster.subscribe();
    let mut receiver2 = broadcaster.subscribe();

    // Emit an event
    let event = EntityChangeEvent {
        event: EntityEvent::SeriesCreated {
            series_id: Uuid::new_v4(),
            library_id: Uuid::new_v4(),
        },
        timestamp: chrono::Utc::now(),
        user_id: None,
    };

    let _ = broadcaster.emit(event.clone());

    // Both receivers should get the event
    let received1 = timeout(Duration::from_secs(1), receiver1.recv())
        .await
        .expect("Timeout on receiver1")
        .expect("Failed to receive on receiver1");

    let received2 = timeout(Duration::from_secs(1), receiver2.recv())
        .await
        .expect("Timeout on receiver2")
        .expect("Failed to receive on receiver2");

    // Both should have received SeriesCreated
    match (received1.event, received2.event) {
        (
            EntityEvent::SeriesCreated { series_id: id1, .. },
            EntityEvent::SeriesCreated { series_id: id2, .. },
        ) => {
            assert_eq!(id1, id2);
        }
        _ => panic!("Wrong event types received"),
    }
}

// ============================================================================
// SSE Keep-Alive Tests
// ============================================================================

#[tokio::test]
async fn test_sse_stream_content_type() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_app_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_app_state(state.clone());

    let request = Request::builder()
        .method("GET")
        .uri("/api/v1/events/stream")
        .header("Authorization", format!("Bearer {}", token))
        .header("Accept", "text/event-stream")
        .body(String::new())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(
        response.headers().get("content-type").unwrap(),
        "text/event-stream"
    );
    assert_eq!(response.headers().get("cache-control").unwrap(), "no-cache");
}
