//! Cover download and application service.
//!
//! Handles downloading cover images from URLs and storing them in the database.

use anyhow::{Context, Result};
use chrono::Utc;
use sea_orm::DatabaseConnection;
use std::sync::Arc;
use tracing::warn;
use uuid::Uuid;

use crate::db::repositories::{SeriesCoversRepository, SeriesRepository, TaskRepository};
use crate::events::{EntityChangeEvent, EntityEvent, EntityType, EventBroadcaster};
use crate::services::ThumbnailService;
use crate::tasks::types::TaskType;

/// Service for downloading and applying cover images to series.
pub struct CoverService;

impl CoverService {
    /// Download a cover from URL and apply it to a series.
    ///
    /// If a cover from this plugin already exists, it will be replaced with the new one.
    /// This ensures the cover is always up-to-date with what the plugin provides.
    ///
    /// If `cover_locked` is true, the cover will be downloaded and saved, but it will NOT
    /// be automatically selected as the primary cover. This preserves the user's manual
    /// cover selection while still keeping the plugin cover available for future use.
    #[allow(clippy::too_many_arguments)]
    pub async fn download_and_apply(
        db: &DatabaseConnection,
        thumbnail_service: &ThumbnailService,
        series_id: Uuid,
        library_id: Uuid,
        cover_url: &str,
        plugin_name: &str,
        cover_locked: bool,
        event_broadcaster: Option<&Arc<EventBroadcaster>>,
    ) -> Result<()> {
        use tokio::fs;
        use tokio::io::AsyncWriteExt;

        // Check if a cover from this plugin already exists for this series
        let source = format!("plugin:{}", plugin_name);
        let existing_cover = SeriesCoversRepository::get_by_source(db, series_id, &source).await?;

        // Delete existing cover from this plugin if present
        if let Some(existing) = existing_cover {
            // Delete the old cover file
            if let Err(e) = fs::remove_file(&existing.path).await {
                warn!("Failed to delete old cover file {}: {}", existing.path, e);
            }
            // Delete the database record
            SeriesCoversRepository::delete(db, existing.id).await?;
        }

        // Download the image using reqwest
        let response = reqwest::get(cover_url)
            .await
            .context("Failed to download cover")?;

        if !response.status().is_success() {
            anyhow::bail!("Failed to download cover: HTTP {}", response.status());
        }

        let image_data = response
            .bytes()
            .await
            .context("Failed to read cover data")?
            .to_vec();

        // Validate that it's a valid image
        image::load_from_memory(&image_data).context("Invalid image file")?;

        // Compute hash of image data for deduplication
        let image_hash = crate::utils::hasher::hash_bytes(&image_data);
        let short_hash = &image_hash[..16];

        // Create covers directory within uploads dir if it doesn't exist
        let covers_dir = thumbnail_service.get_uploads_dir().join("covers");
        fs::create_dir_all(&covers_dir)
            .await
            .context("Failed to create covers directory")?;

        // Use series_id and image hash for filename
        let filename = format!("{}-{}.jpg", series_id, short_hash);
        let filepath = covers_dir.join(&filename);

        // Write the image file
        let mut file = fs::File::create(&filepath)
            .await
            .context("Failed to create cover file")?;

        file.write_all(&image_data)
            .await
            .context("Failed to write cover file")?;

        // Create a new cover with source = "plugin:{plugin_name}"
        // If cover is locked, don't auto-select - preserve user's existing cover selection.
        // If cover is not locked, this automatically deselects any previously selected cover.
        let should_select = !cover_locked;
        SeriesCoversRepository::create(
            db,
            series_id,
            &source,
            &filepath.to_string_lossy(),
            should_select,
            None,
            None,
        )
        .await
        .context("Failed to create cover record")?;

        // Touch series to update updated_at (for cache busting)
        SeriesRepository::touch(db, series_id).await?;

        // Only regenerate thumbnail and emit event if the cover was actually selected
        // (i.e., not locked). If locked, the displayed thumbnail remains unchanged.
        if should_select {
            // Queue thumbnail regeneration task
            Self::queue_thumbnail_regeneration(db, thumbnail_service, series_id).await;

            // Emit CoverUpdated event for real-time UI updates
            Self::emit_cover_updated_event(event_broadcaster, series_id, library_id);
        }

        Ok(())
    }

    /// Queue a task to regenerate the series thumbnail.
    async fn queue_thumbnail_regeneration(
        db: &DatabaseConnection,
        thumbnail_service: &ThumbnailService,
        series_id: Uuid,
    ) {
        // Delete cached thumbnail first
        if let Err(e) = thumbnail_service.delete_series_thumbnail(series_id).await {
            warn!(
                "Failed to delete series thumbnail cache for {}: {}",
                series_id, e
            );
        }

        // Queue regeneration task
        let task_type = TaskType::GenerateSeriesThumbnail {
            series_id,
            force: true,
        };
        if let Err(e) = TaskRepository::enqueue(db, task_type, 0, None).await {
            warn!(
                "Failed to queue series thumbnail regeneration task for {}: {}",
                series_id, e
            );
        }
    }

    /// Emit a CoverUpdated event for real-time UI updates.
    fn emit_cover_updated_event(
        event_broadcaster: Option<&Arc<EventBroadcaster>>,
        series_id: Uuid,
        library_id: Uuid,
    ) {
        if let Some(broadcaster) = event_broadcaster {
            let event = EntityChangeEvent {
                event: EntityEvent::CoverUpdated {
                    entity_type: EntityType::Series,
                    entity_id: series_id,
                    library_id: Some(library_id),
                },
                timestamp: Utc::now(),
                user_id: None,
            };
            let _ = broadcaster.emit(event);
        }
    }
}
