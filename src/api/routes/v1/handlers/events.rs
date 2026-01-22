use crate::api::{error::ApiError, extractors::AuthContext, permissions::Permission, AppState};
use axum::{
    extract::State,
    response::sse::{Event, KeepAlive, Sse},
};
use futures::stream::Stream;
use std::{convert::Infallible, sync::Arc, time::Duration};
use tokio::time::timeout;
use tracing::{debug, warn};

/// Subscribe to real-time entity change events via SSE
///
/// Clients can subscribe to this endpoint to receive real-time notifications
/// about entity changes (books, series, libraries) happening in the system.
///
/// ## Authentication
/// Requires valid authentication with `LibrariesRead` permission.
///
/// ## Event Format
/// Events are sent as JSON-encoded `EntityChangeEvent` objects with the following structure:
/// ```json
/// {
///   "type": "book_created",
///   "book_id": "uuid",
///   "series_id": "uuid",
///   "library_id": "uuid",
///   "timestamp": "2024-01-06T12:00:00Z",
///   "user_id": "uuid"
/// }
/// ```
///
/// ## Keep-Alive
/// A keep-alive message is sent every 15 seconds to prevent connection timeout.
#[utoipa::path(
    get,
    path = "/api/v1/events/stream",
    responses(
        (status = 200, description = "SSE stream of entity change events", content_type = "text/event-stream"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "events"
)]
pub async fn entity_events_stream(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, ApiError> {
    // Require read access to libraries
    auth.require_permission(&Permission::LibrariesRead)?;

    debug!(
        "Client subscribed to entity events (user_id: {}, username: {})",
        auth.user_id, auth.username
    );

    // Subscribe to the event broadcaster
    let mut receiver = state.event_broadcaster.subscribe();

    // Clone broadcaster to check shutdown status
    let broadcaster = state.event_broadcaster.clone();

    // Create SSE stream with timeout to detect client disconnects
    let stream = async_stream::stream! {
        loop {
            // Check if broadcaster is shutting down
            if broadcaster.is_shutdown() {
                debug!("Broadcaster shutdown detected, ending entity events stream");
                break;
            }

            // Use timeout to detect if client has disconnected
            // If no event for 30 seconds (2x keep-alive), assume disconnect
            match timeout(Duration::from_secs(30), receiver.recv()).await {
                Ok(Ok(event)) => {
                    // Check for shutdown signal
                    if event.is_shutdown() {
                        debug!("Received shutdown signal, ending entity events stream");
                        break;
                    }

                    // Serialize event to JSON
                    match serde_json::to_string(&event) {
                        Ok(json) => {
                            debug!("Sending entity event to client: {:?}", event.event);
                            yield Ok(Event::default().data(json));
                        }
                        Err(e) => {
                            warn!("Failed to serialize entity event: {}", e);
                            continue;
                        }
                    }
                }
                Ok(Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped))) => {
                    warn!("Client lagged behind, skipped {} events", skipped);
                    // Continue receiving - client will catch up
                    continue;
                }
                Ok(Err(tokio::sync::broadcast::error::RecvError::Closed)) => {
                    debug!("Event broadcaster closed, ending stream");
                    break;
                }
                Err(_) => {
                    // Timeout - keep-alive should have triggered within 30s
                    // Check shutdown status and continue
                    if broadcaster.is_shutdown() {
                        debug!("Broadcaster shutdown detected during timeout, ending stream");
                        break;
                    }
                    continue;
                }
            }
        }

        debug!("Entity events stream ended for user {}", auth.user_id);
    };

    Ok(Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("keep-alive"),
    ))
}

/// Subscribe to real-time task progress events via SSE
///
/// Clients can subscribe to this endpoint to receive real-time notifications
/// about background task progress (analyze_book, generate_thumbnails, etc.).
///
/// ## Authentication
/// Requires valid authentication with `LibrariesRead` permission.
///
/// ## Event Format
/// Events are sent as JSON-encoded `TaskProgressEvent` objects with the following structure:
/// ```json
/// {
///   "task_id": "uuid",
///   "task_type": "analyze_book",
///   "status": "running",
///   "progress": {
///     "current": 5,
///     "total": 10,
///     "message": "Processing book 5 of 10"
///   },
///   "started_at": "2024-01-06T12:00:00Z",
///   "library_id": "uuid"
/// }
/// ```
///
/// ## Keep-Alive
/// A keep-alive message is sent every 15 seconds to prevent connection timeout.
#[utoipa::path(
    get,
    path = "/api/v1/tasks/stream",
    responses(
        (status = 200, description = "SSE stream of task progress events", content_type = "text/event-stream"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "events"
)]
pub async fn task_progress_stream(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, ApiError> {
    // Require read access to libraries
    auth.require_permission(&Permission::LibrariesRead)?;

    debug!(
        "Client subscribed to task progress events (user_id: {}, username: {})",
        auth.user_id, auth.username
    );

    // Subscribe to the task progress broadcaster
    let mut receiver = state.event_broadcaster.subscribe_tasks();

    // Clone broadcaster to check shutdown status
    let broadcaster = state.event_broadcaster.clone();

    // Create SSE stream with timeout to detect client disconnects
    let stream = async_stream::stream! {
        loop {
            // Check if broadcaster is shutting down
            if broadcaster.is_shutdown() {
                debug!("Broadcaster shutdown detected, ending task progress stream");
                break;
            }

            // Use timeout to detect if client has disconnected
            // If no event for 30 seconds (2x keep-alive), assume disconnect
            match timeout(Duration::from_secs(30), receiver.recv()).await {
                Ok(Ok(event)) => {
                    // Check for shutdown signal
                    if event.is_shutdown() {
                        debug!("Received shutdown signal, ending task progress stream");
                        break;
                    }

                    // Serialize event to JSON
                    match serde_json::to_string(&event) {
                        Ok(json) => {
                            debug!(
                                "Sending task progress event to client: task_id={}, type={}, status={:?}",
                                event.task_id, event.task_type, event.status
                            );
                            yield Ok(Event::default().data(json));
                        }
                        Err(e) => {
                            warn!("Failed to serialize task progress event: {}", e);
                            continue;
                        }
                    }
                }
                Ok(Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped))) => {
                    warn!("Client lagged behind, skipped {} task events", skipped);
                    // Continue receiving - client will catch up
                    continue;
                }
                Ok(Err(tokio::sync::broadcast::error::RecvError::Closed)) => {
                    debug!("Task progress broadcaster closed, ending stream");
                    break;
                }
                Err(_) => {
                    // Timeout - keep-alive should have triggered within 30s
                    // Check shutdown status and continue
                    if broadcaster.is_shutdown() {
                        debug!("Broadcaster shutdown detected during timeout, ending stream");
                        break;
                    }
                    continue;
                }
            }
        }

        debug!("Task progress stream ended for user {}", auth.user_id);
    };

    Ok(Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("keep-alive"),
    ))
}

#[cfg(test)]
mod tests {
    use crate::events::{EntityChangeEvent, EntityEvent, EventBroadcaster};
    use uuid::Uuid;

    #[tokio::test]
    async fn test_event_serialization() {
        let event = EntityChangeEvent::new(
            EntityEvent::BookCreated {
                book_id: Uuid::new_v4(),
                series_id: Uuid::new_v4(),
                library_id: Uuid::new_v4(),
            },
            Some(Uuid::new_v4()),
        );

        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("book_created"));
        assert!(json.contains("timestamp"));
    }

    #[tokio::test]
    async fn test_broadcaster_integration() {
        let broadcaster = EventBroadcaster::new(100);
        let mut receiver = broadcaster.subscribe();

        let event = EntityChangeEvent::new(
            EntityEvent::SeriesCreated {
                series_id: Uuid::new_v4(),
                library_id: Uuid::new_v4(),
            },
            None,
        );

        broadcaster.emit(event.clone()).unwrap();

        let received = receiver.recv().await.unwrap();
        assert_eq!(received.library_id(), event.library_id());
    }
}
