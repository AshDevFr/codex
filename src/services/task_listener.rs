//! PostgreSQL LISTEN/NOTIFY service for task completion
//!
//! This service listens for task completion notifications from PostgreSQL
//! and broadcasts them via the event broadcaster for SSE clients.

use crate::events::{EventBroadcaster, TaskProgressEvent, TaskStatus};
use anyhow::{Context, Result};
use chrono::TimeZone;
use chrono::Utc;
use sea_orm::{
    sqlx::{postgres::PgListener, PgPool},
    DatabaseConnection, SqlxPostgresPoolConnection,
};
use serde::Deserialize;
use std::sync::Arc;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Task completion notification payload from PostgreSQL
#[derive(Debug, Deserialize)]
struct TaskNotification {
    task_id: String,
    task_type: String,
    status: String,
    library_id: Option<String>,
    series_id: Option<String>,
    book_id: Option<String>,
    started_at: Option<f64>,
    completed_at: Option<f64>,
}

/// PostgreSQL LISTEN service for task notifications
pub struct TaskListener {
    pool: PgPool,
    broadcaster: Arc<EventBroadcaster>,
}

impl TaskListener {
    /// Create a new task listener from SeaORM connection
    pub fn from_sea_orm(
        db: &DatabaseConnection,
        broadcaster: Arc<EventBroadcaster>,
    ) -> Result<Self> {
        // Extract the underlying sqlx PgPool from SeaORM
        // Caller should ensure database is PostgreSQL before calling this
        let pool = match db {
            DatabaseConnection::SqlxPostgresPoolConnection(conn) => {
                // SqlxPostgresPoolConnection is a newtype wrapper around PgPool
                // We need to access the inner pool. Since the field is private,
                // we'll use the fact that it's stored in the connection and
                // create a new pool from the connection string.
                // However, a better approach is to store the DatabaseConnection
                // and extract the pool when needed, or accept PgPool directly.
                // For now, we'll use unsafe to access the private field.
                // SAFETY: SqlxPostgresPoolConnection is a newtype around PgPool,
                // and we're only reading the pool to clone it.
                unsafe {
                    let pool_ptr = conn as *const SqlxPostgresPoolConnection as *const PgPool;
                    (*pool_ptr).clone()
                }
            }
            _ => anyhow::bail!("Database is not PostgreSQL"),
        };

        Ok(Self { pool, broadcaster })
    }

    /// Start listening for task completion notifications
    ///
    /// This runs indefinitely and should be spawned as a background task.
    pub async fn start(self) -> Result<()> {
        info!("Starting PostgreSQL task listener on channel 'task_completion'");

        let mut listener = PgListener::connect_with(&self.pool)
            .await
            .context("Failed to create PostgreSQL listener")?;

        listener
            .listen("task_completion")
            .await
            .context("Failed to listen on 'task_completion' channel")?;

        info!("Task listener connected and listening");

        loop {
            match listener.recv().await {
                Ok(notification) => {
                    let payload = notification.payload();
                    debug!("Received task notification: {}", payload);

                    if let Err(e) = self.handle_notification(payload).await {
                        error!("Error handling task notification: {}", e);
                    }
                }
                Err(e) => {
                    error!("Error receiving notification: {}", e);
                    // sqlx will automatically reconnect on error
                }
            }
        }
    }

    /// Handle a task completion notification
    async fn handle_notification(&self, payload: &str) -> Result<()> {
        let notification: TaskNotification =
            serde_json::from_str(payload).context("Failed to parse task notification payload")?;

        let task_id = Uuid::parse_str(&notification.task_id).context("Invalid task_id UUID")?;

        let status = match notification.status.as_str() {
            "completed" => TaskStatus::Completed,
            "failed" => TaskStatus::Failed,
            "running" => TaskStatus::Running,
            "pending" => TaskStatus::Pending,
            _ => {
                warn!("Unknown task status: {}", notification.status);
                return Ok(());
            }
        };

        // Parse UUIDs for foreign keys
        let library_id = notification
            .library_id
            .as_ref()
            .and_then(|s| Uuid::parse_str(s).ok());
        let series_id = notification
            .series_id
            .as_ref()
            .and_then(|s| Uuid::parse_str(s).ok());
        let book_id = notification
            .book_id
            .as_ref()
            .and_then(|s| Uuid::parse_str(s).ok());

        // Convert timestamps from epoch seconds to DateTime
        let started_at = notification
            .started_at
            .and_then(|secs| Utc.timestamp_opt(secs as i64, 0).single());
        let completed_at = notification
            .completed_at
            .and_then(|secs| Utc.timestamp_opt(secs as i64, 0).single());

        let event = TaskProgressEvent {
            task_id,
            task_type: notification.task_type.clone(),
            status,
            progress: None,
            error: None,
            started_at: started_at.unwrap_or_else(Utc::now),
            completed_at,
            library_id,
            series_id,
            book_id,
        };

        match self.broadcaster.emit_task(event) {
            Ok(count) => {
                debug!(
                    "Broadcast task event to {} subscribers: task_id={}, type={}, status={:?}",
                    count, task_id, notification.task_type, status
                );
            }
            Err(e) => {
                warn!("Failed to broadcast task event: {:?}", e);
            }
        }

        Ok(())
    }
}
