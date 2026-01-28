//! Shared metadata application service.
//!
//! This module provides a unified implementation for applying plugin metadata to series,
//! used by both synchronous API endpoints and background task handlers.

use anyhow::{Context, Result};
use sea_orm::prelude::Decimal;
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tracing::warn;
use uuid::Uuid;

use crate::db::entities::plugins::{Model as Plugin, PluginPermission};
use crate::db::entities::series_metadata::Model as SeriesMetadata;
use crate::db::entities::SeriesStatus;
use crate::db::repositories::{
    AlternateTitleRepository, ExternalLinkRepository, ExternalRatingRepository, GenreRepository,
    SeriesMetadataRepository, TagRepository,
};
use crate::events::EventBroadcaster;
use crate::services::plugin::PluginSeriesMetadata;
use crate::services::ThumbnailService;

use super::CoverService;

/// A field that was skipped during metadata application.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkippedField {
    pub field: String,
    pub reason: String,
}

/// Result of applying metadata to a series.
#[derive(Debug, Clone)]
pub struct MetadataApplyResult {
    /// Fields that were successfully applied.
    pub applied_fields: Vec<String>,
    /// Fields that were skipped (with reasons).
    pub skipped_fields: Vec<SkippedField>,
}

/// Options for controlling metadata application behavior.
#[derive(Clone, Default)]
pub struct ApplyOptions {
    /// If Some, only apply fields in this set. If None, apply all fields.
    pub fields_filter: Option<HashSet<String>>,
    /// Thumbnail service for downloading covers. If None, covers will be skipped.
    pub thumbnail_service: Option<Arc<ThumbnailService>>,
    /// Event broadcaster for emitting real-time events. If None, events won't be emitted.
    pub event_broadcaster: Option<Arc<EventBroadcaster>>,
}

/// Service for applying plugin metadata to series.
pub struct MetadataApplier;

impl MetadataApplier {
    /// Apply metadata from a plugin to a series.
    ///
    /// This function applies all metadata fields from the plugin, respecting:
    /// - Field locks (user has locked the field from being updated)
    /// - Plugin permissions (plugin is not allowed to update this field)
    /// - Optional field filtering (only apply specific fields)
    pub async fn apply(
        db: &DatabaseConnection,
        series_id: Uuid,
        library_id: Uuid,
        plugin: &Plugin,
        metadata: &PluginSeriesMetadata,
        current_metadata: Option<&SeriesMetadata>,
        options: &ApplyOptions,
    ) -> Result<MetadataApplyResult> {
        let mut applied_fields = Vec::new();
        let mut skipped_fields = Vec::new();

        // Helper to check if a field should be applied based on the filter
        let should_apply_field = |field: &str| -> bool {
            options
                .fields_filter
                .as_ref()
                .is_none_or(|filter| filter.contains(field))
        };

        // Helper to check permission and lock
        let check_field = |field: &str,
                           is_locked: bool,
                           permission: PluginPermission|
         -> Result<bool, SkippedField> {
            if is_locked {
                Err(SkippedField {
                    field: field.to_string(),
                    reason: "Field is locked".to_string(),
                })
            } else if !plugin.has_permission(&permission) {
                Err(SkippedField {
                    field: field.to_string(),
                    reason: "Plugin does not have permission".to_string(),
                })
            } else {
                Ok(true)
            }
        };

        // Title
        if should_apply_field("title") {
            if let Some(title) = &metadata.title {
                let is_locked = current_metadata.map(|m| m.title_lock).unwrap_or(false);
                match check_field("title", is_locked, PluginPermission::MetadataWriteTitle) {
                    Ok(_) => {
                        let title_sort = current_metadata.and_then(|m| m.title_sort.clone());
                        SeriesMetadataRepository::update_title(
                            db,
                            series_id,
                            title.clone(),
                            title_sort,
                        )
                        .await
                        .context("Failed to update title")?;
                        applied_fields.push("title".to_string());
                    }
                    Err(skip) => skipped_fields.push(skip),
                }
            }
        }

        // Alternate Titles
        if should_apply_field("alternateTitles") && !metadata.alternate_titles.is_empty() {
            let is_locked = current_metadata.map(|m| m.title_lock).unwrap_or(false);
            match check_field(
                "alternateTitles",
                is_locked,
                PluginPermission::MetadataWriteTitle,
            ) {
                Ok(_) => {
                    // Delete existing alternate titles
                    AlternateTitleRepository::delete_all_for_series(db, series_id)
                        .await
                        .context("Failed to delete old alternate titles")?;

                    // Add new alternate titles with unique labels
                    // Track label counts to make duplicates unique (e.g., "en", "en-2", "en-3")
                    let mut label_counts: HashMap<String, u32> = HashMap::new();

                    for alt_title in &metadata.alternate_titles {
                        // Use language or title_type as base label, defaulting to "alternate"
                        let base_label = alt_title
                            .language
                            .clone()
                            .or_else(|| alt_title.title_type.clone())
                            .unwrap_or_else(|| "alternate".to_string());

                        // Make label unique by appending count suffix for duplicates
                        let count = label_counts.entry(base_label.clone()).or_insert(0);
                        *count += 1;
                        let label = if *count == 1 {
                            base_label
                        } else {
                            format!("{}-{}", base_label, count)
                        };

                        AlternateTitleRepository::create(db, series_id, &label, &alt_title.title)
                            .await
                            .context("Failed to create alternate title")?;
                    }
                    applied_fields.push("alternateTitles".to_string());
                }
                Err(skip) => skipped_fields.push(skip),
            }
        }

        // Summary
        if should_apply_field("summary") {
            if let Some(summary) = &metadata.summary {
                let is_locked = current_metadata.map(|m| m.summary_lock).unwrap_or(false);
                match check_field("summary", is_locked, PluginPermission::MetadataWriteSummary) {
                    Ok(_) => {
                        SeriesMetadataRepository::update_summary(
                            db,
                            series_id,
                            Some(summary.clone()),
                        )
                        .await
                        .context("Failed to update summary")?;
                        applied_fields.push("summary".to_string());
                    }
                    Err(skip) => skipped_fields.push(skip),
                }
            }
        }

        // Year
        if should_apply_field("year") {
            if let Some(year) = metadata.year {
                let is_locked = current_metadata.map(|m| m.year_lock).unwrap_or(false);
                match check_field("year", is_locked, PluginPermission::MetadataWriteYear) {
                    Ok(_) => {
                        SeriesMetadataRepository::update_year(db, series_id, Some(year))
                            .await
                            .context("Failed to update year")?;
                        applied_fields.push("year".to_string());
                    }
                    Err(skip) => skipped_fields.push(skip),
                }
            }
        }

        // Status
        if should_apply_field("status") {
            if let Some(status) = &metadata.status {
                let is_locked = current_metadata.map(|m| m.status_lock).unwrap_or(false);
                match check_field("status", is_locked, PluginPermission::MetadataWriteStatus) {
                    Ok(_) => {
                        let status_str = match status {
                            SeriesStatus::Ongoing => "ongoing",
                            SeriesStatus::Ended => "ended",
                            SeriesStatus::Hiatus => "hiatus",
                            SeriesStatus::Abandoned => "abandoned",
                            SeriesStatus::Unknown => "unknown",
                        };
                        SeriesMetadataRepository::update_status(
                            db,
                            series_id,
                            Some(status_str.to_string()),
                        )
                        .await
                        .context("Failed to update status")?;
                        applied_fields.push("status".to_string());
                    }
                    Err(skip) => skipped_fields.push(skip),
                }
            }
        }

        // Publisher
        if should_apply_field("publisher") {
            if let Some(publisher) = &metadata.publisher {
                let is_locked = current_metadata.map(|m| m.publisher_lock).unwrap_or(false);
                match check_field(
                    "publisher",
                    is_locked,
                    PluginPermission::MetadataWritePublisher,
                ) {
                    Ok(_) => {
                        SeriesMetadataRepository::update_publisher(
                            db,
                            series_id,
                            Some(publisher.clone()),
                            None,
                        )
                        .await
                        .context("Failed to update publisher")?;
                        applied_fields.push("publisher".to_string());
                    }
                    Err(skip) => skipped_fields.push(skip),
                }
            }
        }

        // Age Rating
        if should_apply_field("ageRating") {
            if let Some(age_rating) = metadata.age_rating {
                let is_locked = current_metadata.map(|m| m.age_rating_lock).unwrap_or(false);
                match check_field(
                    "ageRating",
                    is_locked,
                    PluginPermission::MetadataWriteAgeRating,
                ) {
                    Ok(_) => {
                        SeriesMetadataRepository::update_age_rating(
                            db,
                            series_id,
                            Some(age_rating),
                        )
                        .await
                        .context("Failed to update age rating")?;
                        applied_fields.push("ageRating".to_string());
                    }
                    Err(skip) => skipped_fields.push(skip),
                }
            }
        }

        // Language
        if should_apply_field("language") {
            if let Some(language) = &metadata.language {
                let is_locked = current_metadata.map(|m| m.language_lock).unwrap_or(false);
                match check_field(
                    "language",
                    is_locked,
                    PluginPermission::MetadataWriteLanguage,
                ) {
                    Ok(_) => {
                        SeriesMetadataRepository::update_language(
                            db,
                            series_id,
                            Some(language.clone()),
                        )
                        .await
                        .context("Failed to update language")?;
                        applied_fields.push("language".to_string());
                    }
                    Err(skip) => skipped_fields.push(skip),
                }
            }
        }

        // Reading Direction
        if should_apply_field("readingDirection") {
            if let Some(reading_direction) = &metadata.reading_direction {
                let is_locked = current_metadata
                    .map(|m| m.reading_direction_lock)
                    .unwrap_or(false);
                match check_field(
                    "readingDirection",
                    is_locked,
                    PluginPermission::MetadataWriteReadingDirection,
                ) {
                    Ok(_) => {
                        SeriesMetadataRepository::update_reading_direction(
                            db,
                            series_id,
                            Some(reading_direction.clone()),
                        )
                        .await
                        .context("Failed to update reading direction")?;
                        applied_fields.push("readingDirection".to_string());
                    }
                    Err(skip) => skipped_fields.push(skip),
                }
            }
        }

        // Total Book Count
        if should_apply_field("totalBookCount") {
            if let Some(total_book_count) = metadata.total_book_count {
                let is_locked = current_metadata
                    .map(|m| m.total_book_count_lock)
                    .unwrap_or(false);
                match check_field(
                    "totalBookCount",
                    is_locked,
                    PluginPermission::MetadataWriteTotalBookCount,
                ) {
                    Ok(_) => {
                        SeriesMetadataRepository::update_total_book_count(
                            db,
                            series_id,
                            Some(total_book_count),
                        )
                        .await
                        .context("Failed to update total book count")?;
                        applied_fields.push("totalBookCount".to_string());
                    }
                    Err(skip) => skipped_fields.push(skip),
                }
            }
        }

        // Genres - uses set_genres_for_series which replaces all
        if should_apply_field("genres") && !metadata.genres.is_empty() {
            let is_locked = current_metadata.map(|m| m.genres_lock).unwrap_or(false);
            if is_locked {
                skipped_fields.push(SkippedField {
                    field: "genres".to_string(),
                    reason: "Field is locked".to_string(),
                });
            } else if !plugin.has_permission(&PluginPermission::MetadataWriteGenres) {
                skipped_fields.push(SkippedField {
                    field: "genres".to_string(),
                    reason: "Plugin does not have permission".to_string(),
                });
            } else {
                GenreRepository::set_genres_for_series(db, series_id, metadata.genres.clone())
                    .await
                    .context("Failed to set genres")?;
                applied_fields.push("genres".to_string());
            }
        }

        // Tags - uses set_tags_for_series which replaces all
        if should_apply_field("tags") && !metadata.tags.is_empty() {
            let is_locked = current_metadata.map(|m| m.tags_lock).unwrap_or(false);
            if is_locked {
                skipped_fields.push(SkippedField {
                    field: "tags".to_string(),
                    reason: "Field is locked".to_string(),
                });
            } else if !plugin.has_permission(&PluginPermission::MetadataWriteTags) {
                skipped_fields.push(SkippedField {
                    field: "tags".to_string(),
                    reason: "Plugin does not have permission".to_string(),
                });
            } else {
                TagRepository::set_tags_for_series(db, series_id, metadata.tags.clone())
                    .await
                    .context("Failed to set tags")?;
                applied_fields.push("tags".to_string());
            }
        }

        // Authors - not yet implemented in series_metadata
        if should_apply_field("authors") && !metadata.authors.is_empty() {
            skipped_fields.push(SkippedField {
                field: "authors".to_string(),
                reason: "Authors field not yet implemented".to_string(),
            });
        }

        // Artists - not yet implemented in series_metadata
        if should_apply_field("artists") && !metadata.artists.is_empty() {
            skipped_fields.push(SkippedField {
                field: "artists".to_string(),
                reason: "Artists field not yet implemented".to_string(),
            });
        }

        // External Links
        if should_apply_field("externalLinks") && !metadata.external_links.is_empty() {
            if !plugin.has_permission(&PluginPermission::MetadataWriteLinks) {
                skipped_fields.push(SkippedField {
                    field: "externalLinks".to_string(),
                    reason: "Plugin does not have permission".to_string(),
                });
            } else {
                for link in &metadata.external_links {
                    ExternalLinkRepository::upsert(db, series_id, &link.label, &link.url, None)
                        .await
                        .context("Failed to upsert external link")?;
                }
                applied_fields.push("externalLinks".to_string());
            }
        }

        // External Ratings (primary rating from plugin)
        if should_apply_field("rating") {
            if let Some(rating) = &metadata.rating {
                if !plugin.has_permission(&PluginPermission::MetadataWriteRatings) {
                    skipped_fields.push(SkippedField {
                        field: "rating".to_string(),
                        reason: "Plugin does not have permission".to_string(),
                    });
                } else {
                    let score = Decimal::from_f64_retain(rating.score)
                        .unwrap_or_else(|| Decimal::new(0, 0));
                    ExternalRatingRepository::upsert(
                        db,
                        series_id,
                        &rating.source,
                        score,
                        rating.vote_count,
                    )
                    .await
                    .context("Failed to upsert external rating")?;
                    applied_fields.push("rating".to_string());
                }
            }
        }

        // Multiple external ratings
        if should_apply_field("externalRatings") && !metadata.external_ratings.is_empty() {
            if !plugin.has_permission(&PluginPermission::MetadataWriteRatings) {
                if !skipped_fields.iter().any(|f| f.field == "rating") {
                    skipped_fields.push(SkippedField {
                        field: "externalRatings".to_string(),
                        reason: "Plugin does not have permission".to_string(),
                    });
                }
            } else {
                for rating in &metadata.external_ratings {
                    let score = Decimal::from_f64_retain(rating.score)
                        .unwrap_or_else(|| Decimal::new(0, 0));
                    ExternalRatingRepository::upsert(
                        db,
                        series_id,
                        &rating.source,
                        score,
                        rating.vote_count,
                    )
                    .await
                    .context("Failed to upsert external rating")?;
                }
                if !applied_fields.contains(&"rating".to_string()) {
                    applied_fields.push("externalRatings".to_string());
                }
            }
        }

        // Cover URL - download and apply cover from plugin
        if should_apply_field("coverUrl") {
            if let Some(cover_url) = &metadata.cover_url {
                if !plugin.has_permission(&PluginPermission::MetadataWriteCovers) {
                    skipped_fields.push(SkippedField {
                        field: "coverUrl".to_string(),
                        reason: "Plugin does not have permission".to_string(),
                    });
                } else if let Some(thumbnail_service) = &options.thumbnail_service {
                    match CoverService::download_and_apply(
                        db,
                        thumbnail_service,
                        series_id,
                        library_id,
                        cover_url,
                        &plugin.name,
                        options.event_broadcaster.as_ref(),
                    )
                    .await
                    {
                        Ok(_) => {
                            applied_fields.push("coverUrl".to_string());
                        }
                        Err(e) => {
                            warn!("Failed to download cover: {}", e);
                            skipped_fields.push(SkippedField {
                                field: "coverUrl".to_string(),
                                reason: format!("Failed to download cover: {}", e),
                            });
                        }
                    }
                } else {
                    skipped_fields.push(SkippedField {
                        field: "coverUrl".to_string(),
                        reason: "ThumbnailService not available".to_string(),
                    });
                }
            }
        }

        Ok(MetadataApplyResult {
            applied_fields,
            skipped_fields,
        })
    }
}
