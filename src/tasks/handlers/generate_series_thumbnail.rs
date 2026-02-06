//! Handler for GenerateSeriesThumbnail task
//!
//! Generates a thumbnail for a series using the selected cover from series_covers,
//! or falls back to the first book's cover if no cover is selected.

use anyhow::{Result, anyhow};
use sea_orm::DatabaseConnection;
use std::sync::Arc;
use tracing::{debug, info, warn};

use crate::db::entities::tasks;
use crate::db::repositories::{BookRepository, SeriesCoversRepository, SeriesRepository};
use crate::events::{EntityChangeEvent, EntityEvent, EntityType, EventBroadcaster};
use crate::services::ThumbnailService;
use crate::tasks::handlers::TaskHandler;
use crate::tasks::types::TaskResult;

pub struct GenerateSeriesThumbnailHandler {
    thumbnail_service: Arc<ThumbnailService>,
}

impl GenerateSeriesThumbnailHandler {
    pub fn new(thumbnail_service: Arc<ThumbnailService>) -> Self {
        Self { thumbnail_service }
    }
}

impl TaskHandler for GenerateSeriesThumbnailHandler {
    fn handle<'a>(
        &'a self,
        task: &'a tasks::Model,
        db: &'a DatabaseConnection,
        event_broadcaster: Option<&'a Arc<EventBroadcaster>>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<TaskResult>> + Send + 'a>> {
        Box::pin(async move {
            let series_id = task
                .series_id
                .ok_or_else(|| anyhow!("Missing series_id for GenerateSeriesThumbnail task"))?;

            // Extract force parameter from task params (default: false)
            let force = task
                .params
                .as_ref()
                .and_then(|p| p.get("force"))
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            info!(
                "Task {}: Generating thumbnail for series {} (force={})",
                task.id, series_id, force
            );

            // Check if series exists
            let series = SeriesRepository::get_by_id(db, series_id)
                .await?
                .ok_or_else(|| anyhow!("Series not found: {}", series_id))?;

            // Check if series thumbnail already exists (unless force=true)
            if !force
                && let Some(_meta) = self
                    .thumbnail_service
                    .get_series_thumbnail_metadata(series_id)
                    .await
            {
                debug!(
                    "Series thumbnail already exists for series {}, skipping",
                    series_id
                );
                return Ok(TaskResult::success(format!(
                    "Thumbnail already exists for series {}",
                    series_id
                )));
            }

            // If force=true and thumbnail exists, delete it first
            if force {
                debug!("Force regenerating thumbnail for series {}", series_id);
                if let Err(e) = self
                    .thumbnail_service
                    .delete_series_thumbnail(series_id)
                    .await
                {
                    warn!(
                        "Failed to delete existing series thumbnail for series {}: {}",
                        series_id, e
                    );
                }
            }

            // First, check if there's a selected cover in series_covers
            if let Ok(Some(selected_cover)) =
                SeriesCoversRepository::get_selected(db, series_id).await
            {
                debug!(
                    "Found selected cover for series {}: source={}",
                    series_id, selected_cover.source
                );

                // Read the cover image file
                match tokio::fs::read(&selected_cover.path).await {
                    Ok(image_data) => {
                        // Generate thumbnail from the selected cover
                        match self
                            .thumbnail_service
                            .generate_thumbnail_from_image(db, image_data)
                            .await
                        {
                            Ok(thumbnail_data) => {
                                // Save series thumbnail
                                match self
                                    .thumbnail_service
                                    .save_series_thumbnail(series_id, &thumbnail_data)
                                    .await
                                {
                                    Ok(path) => {
                                        info!(
                                            "Task {}: Generated series thumbnail from selected cover ({}) at {:?}",
                                            task.id, selected_cover.source, path
                                        );

                                        // Emit CoverUpdated event for series
                                        emit_series_cover_updated(
                                            event_broadcaster,
                                            series_id,
                                            series.library_id,
                                        );

                                        return Ok(TaskResult::success_with_data(
                                            format!("Generated thumbnail for series {}", series_id),
                                            serde_json::json!({
                                                "series_id": series_id,
                                                "source": selected_cover.source,
                                                "path": path.to_string_lossy(),
                                                "force": force,
                                            }),
                                        ));
                                    }
                                    Err(e) => {
                                        warn!(
                                            "Failed to save series thumbnail from selected cover: {}",
                                            e
                                        );
                                        // Fall through to book-based thumbnail
                                    }
                                }
                            }
                            Err(e) => {
                                warn!("Failed to generate thumbnail from selected cover: {}", e);
                                // Fall through to book-based thumbnail
                            }
                        }
                    }
                    Err(e) => {
                        warn!(
                            "Failed to read selected cover file at {}: {}",
                            selected_cover.path, e
                        );
                        // Fall through to book-based thumbnail
                    }
                }
            }

            // No selected cover or failed to use it - fall back to first book's cover
            let first_book = BookRepository::get_first_in_series(db, series_id)
                .await?
                .ok_or_else(|| anyhow!("Series {} has no books", series_id))?;

            // First, try to use the book's existing cached thumbnail
            if let Ok(thumbnail_data) = self.thumbnail_service.read_thumbnail(first_book.id).await {
                // Save the book thumbnail as the series thumbnail
                match self
                    .thumbnail_service
                    .save_series_thumbnail(series_id, &thumbnail_data)
                    .await
                {
                    Ok(path) => {
                        info!(
                            "Task {}: Generated series thumbnail from cached book thumbnail at {:?}",
                            task.id, path
                        );

                        // Emit CoverUpdated event for series
                        emit_series_cover_updated(event_broadcaster, series_id, series.library_id);

                        return Ok(TaskResult::success_with_data(
                            format!("Generated thumbnail for series {}", series_id),
                            serde_json::json!({
                                "series_id": series_id,
                                "source_book_id": first_book.id,
                                "source": "cached_book_thumbnail",
                                "path": path.to_string_lossy(),
                                "force": force,
                            }),
                        ));
                    }
                    Err(e) => {
                        warn!("Failed to save series thumbnail from cached book: {}", e);
                        // Fall through to extract from book file
                    }
                }
            }

            // Fall back to extracting from the book file
            if first_book.page_count == 0 {
                return Err(anyhow!(
                    "First book {} in series {} has no pages",
                    first_book.id,
                    series_id
                ));
            }

            // Extract first page from book
            let image_data = extract_page_image(&first_book.file_path, &first_book.format, 1)
                .await
                .map_err(|e| {
                    anyhow!("Failed to extract page from book {}: {}", first_book.id, e)
                })?;

            // Generate thumbnail from image
            let thumbnail_data = self
                .thumbnail_service
                .generate_thumbnail_from_image(db, image_data)
                .await
                .map_err(|e| anyhow!("Failed to generate thumbnail image: {}", e))?;

            // Save series thumbnail
            match self
                .thumbnail_service
                .save_series_thumbnail(series_id, &thumbnail_data)
                .await
            {
                Ok(path) => {
                    info!(
                        "Task {}: Generated series thumbnail from book file at {:?}",
                        task.id, path
                    );

                    // Emit CoverUpdated event for series
                    emit_series_cover_updated(event_broadcaster, series_id, series.library_id);

                    Ok(TaskResult::success_with_data(
                        format!("Generated thumbnail for series {}", series_id),
                        serde_json::json!({
                            "series_id": series_id,
                            "source_book_id": first_book.id,
                            "source": "book_file",
                            "path": path.to_string_lossy(),
                            "force": force,
                        }),
                    ))
                }
                Err(e) => Err(anyhow!(
                    "Failed to save series thumbnail for series {}: {}",
                    series_id,
                    e
                )),
            }
        })
    }
}

/// Extract page image from book file
///
/// Uses spawn_blocking to avoid blocking the async runtime during CPU-intensive
/// image extraction operations (ZIP parsing, RAR extraction, EPUB parsing, PDF rendering)
async fn extract_page_image(
    file_path: &str,
    file_format: &str,
    page_number: i32,
) -> anyhow::Result<Vec<u8>> {
    let path = std::path::PathBuf::from(file_path);
    let format = file_format.to_uppercase();

    // Use spawn_blocking for CPU-intensive file parsing operations
    tokio::task::spawn_blocking(move || match format.as_str() {
        "CBZ" => crate::parsers::cbz::extract_page_from_cbz(&path, page_number),
        #[cfg(feature = "rar")]
        "CBR" => crate::parsers::cbr::extract_page_from_cbr(&path, page_number),
        "EPUB" => crate::parsers::epub::extract_page_from_epub(&path, page_number),
        "PDF" => crate::parsers::pdf::extract_page_from_pdf(&path, page_number),
        _ => anyhow::bail!("Unsupported format: {}", format),
    })
    .await
    .map_err(|e| anyhow::anyhow!("Task join error: {}", e))?
}

/// Helper to emit CoverUpdated event for a series
fn emit_series_cover_updated(
    event_broadcaster: Option<&Arc<EventBroadcaster>>,
    series_id: uuid::Uuid,
    library_id: uuid::Uuid,
) {
    if let Some(broadcaster) = event_broadcaster {
        let event = EntityChangeEvent {
            event: EntityEvent::CoverUpdated {
                entity_type: EntityType::Series,
                entity_id: series_id,
                library_id: Some(library_id),
            },
            user_id: None,
            timestamp: chrono::Utc::now(),
        };

        if let Err(e) = broadcaster.emit(event) {
            warn!(
                "Failed to emit CoverUpdated event for series {}: {:?}",
                series_id, e
            );
        }
    }
}
