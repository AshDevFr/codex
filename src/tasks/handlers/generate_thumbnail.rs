use anyhow::{anyhow, Result};
use sea_orm::DatabaseConnection;
use std::sync::Arc;
use tracing::{debug, error, info, warn};

use crate::db::entities::book_error::{BookError, BookErrorType};
use crate::db::entities::tasks;
use crate::db::repositories::{BookRepository, SeriesRepository, TaskRepository};
use crate::events::{EntityChangeEvent, EntityEvent, EntityType, EventBroadcaster};
use crate::services::ThumbnailService;
use crate::tasks::handlers::TaskHandler;
use crate::tasks::types::{TaskResult, TaskType};

pub struct GenerateThumbnailHandler {
    thumbnail_service: Arc<ThumbnailService>,
}

impl GenerateThumbnailHandler {
    pub fn new(thumbnail_service: Arc<ThumbnailService>) -> Self {
        Self { thumbnail_service }
    }
}

impl TaskHandler for GenerateThumbnailHandler {
    fn handle<'a>(
        &'a self,
        task: &'a tasks::Model,
        db: &'a DatabaseConnection,
        event_broadcaster: Option<&'a Arc<EventBroadcaster>>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<TaskResult>> + Send + 'a>> {
        Box::pin(async move {
            let book_id = task
                .book_id
                .ok_or_else(|| anyhow!("Missing book_id for GenerateThumbnail task"))?;

            // Extract force parameter from task params (default: false)
            let force = task
                .params
                .as_ref()
                .and_then(|p| p.get("force"))
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            info!(
                "Task {}: Generating thumbnail for book {} (force={})",
                task.id, book_id, force
            );

            // Fetch book
            let book = BookRepository::get_by_id(db, book_id)
                .await?
                .ok_or_else(|| anyhow!("Book not found: {}", book_id))?;

            // Check if thumbnail already exists (unless force=true)
            if !force && self.thumbnail_service.thumbnail_exists(book_id).await {
                debug!("Thumbnail already exists for book {}, skipping", book_id);
                return Ok(TaskResult::success(format!(
                    "Thumbnail already exists for book {}",
                    book_id
                )));
            }

            // If force=true and thumbnail exists, delete it first
            if force && self.thumbnail_service.thumbnail_exists(book_id).await {
                debug!("Force regenerating thumbnail for book {}", book_id);
                if let Err(e) = self.thumbnail_service.delete_thumbnail(db, book_id).await {
                    warn!(
                        "Failed to delete existing thumbnail for book {}: {}",
                        book_id, e
                    );
                }
            }

            // Generate thumbnail
            match self.thumbnail_service.generate_thumbnail(db, &book).await {
                Ok(path) => {
                    info!(
                        "Task {}: Generated thumbnail for book {} at {:?}",
                        task.id, book_id, path
                    );

                    // Clear thumbnail error on success
                    if let Err(e) =
                        BookRepository::clear_error(db, book_id, BookErrorType::Thumbnail).await
                    {
                        warn!(
                            "Failed to clear thumbnail error for book {}: {}",
                            book_id, e
                        );
                    }

                    // If this book is the first in its series, regenerate the series thumbnail
                    // to use this book's new cover
                    if let Ok(is_first) = BookRepository::is_first_in_series(db, book_id).await {
                        if is_first {
                            debug!(
                                "Book {} is first in series {}, queueing series thumbnail regeneration",
                                book_id, book.series_id
                            );
                            // Queue a task to regenerate the series thumbnail with force=true
                            // This will delete the old thumbnail and create a new one from this book
                            let task_type = TaskType::GenerateSeriesThumbnail {
                                series_id: book.series_id,
                                force: true,
                            };
                            if let Err(e) = TaskRepository::enqueue(db, task_type, 0, None).await {
                                warn!(
                                    "Failed to queue series thumbnail regeneration for series {}: {}",
                                    book.series_id, e
                                );
                            }
                        }
                    }

                    // Emit CoverUpdated event to notify UI
                    if let Some(broadcaster) = event_broadcaster {
                        if let Ok(Some(series)) =
                            SeriesRepository::get_by_id(db, book.series_id).await
                        {
                            let event = EntityChangeEvent {
                                event: EntityEvent::CoverUpdated {
                                    entity_type: EntityType::Book,
                                    entity_id: book_id,
                                    library_id: Some(series.library_id),
                                },
                                user_id: None,
                                timestamp: chrono::Utc::now(),
                            };

                            if let Err(e) = broadcaster.emit(event) {
                                warn!(
                                    "Failed to emit CoverUpdated event for book {}: {:?}",
                                    book_id, e
                                );
                            }
                        }
                    }

                    Ok(TaskResult::success_with_data(
                        format!("Generated thumbnail for book {}", book_id),
                        serde_json::json!({
                            "book_id": book_id,
                            "path": path.to_string_lossy(),
                            "force": force,
                        }),
                    ))
                }
                Err(e) => {
                    let error_msg = format!("Failed to generate thumbnail: {}", e);
                    error!(
                        "Task {}: Thumbnail generation failed for book {}: {}",
                        task.id, book_id, e
                    );

                    // Store the thumbnail error for UI display
                    let book_error = BookError::new(&error_msg);
                    if let Err(set_err) =
                        BookRepository::set_error(db, book_id, BookErrorType::Thumbnail, book_error)
                            .await
                    {
                        warn!(
                            "Failed to set thumbnail error for book {}: {}",
                            book_id, set_err
                        );
                    }

                    Err(anyhow!(
                        "Failed to generate thumbnail for book {}: {}",
                        book_id,
                        e
                    ))
                }
            }
        })
    }
}
