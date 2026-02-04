//! Shared book metadata application service.
//!
//! This module provides a unified implementation for applying plugin metadata to books,
//! used by both synchronous API endpoints and background task handlers.

use anyhow::{Context, Result};
use sea_orm::prelude::Decimal;
use sea_orm::DatabaseConnection;
use std::collections::HashSet;
use std::sync::Arc;
use tracing::warn;
use uuid::Uuid;

use crate::db::entities::book_metadata::Model as BookMetadata;
use crate::db::entities::plugins::{Model as Plugin, PluginPermission};
use crate::db::repositories::BookMetadataRepository;
use crate::events::EventBroadcaster;
use crate::services::plugin::protocol::PluginBookMetadata;
use crate::services::ThumbnailService;

use super::apply::SkippedField;
use super::CoverService;

/// Result of applying metadata to a book.
#[derive(Debug, Clone)]
pub struct BookMetadataApplyResult {
    /// Fields that were successfully applied.
    pub applied_fields: Vec<String>,
    /// Fields that were skipped (with reasons).
    pub skipped_fields: Vec<SkippedField>,
}

/// Options for controlling book metadata application behavior.
#[derive(Clone, Default)]
pub struct BookApplyOptions {
    /// If Some, only apply fields in this set. If None, apply all fields.
    pub fields_filter: Option<HashSet<String>>,
    /// Thumbnail service for downloading covers. If None, covers will be skipped.
    pub thumbnail_service: Option<Arc<ThumbnailService>>,
    /// Event broadcaster for emitting real-time events. If None, events won't be emitted.
    pub event_broadcaster: Option<Arc<EventBroadcaster>>,
    /// Library ID (required for cover events)
    pub library_id: Option<Uuid>,
}

/// Service for applying plugin metadata to books.
pub struct BookMetadataApplier;

impl BookMetadataApplier {
    /// Apply metadata from a plugin to a book.
    ///
    /// This function applies all metadata fields from the plugin, respecting:
    /// - Field locks (user has locked the field from being updated)
    /// - Plugin permissions (plugin is not allowed to update this field)
    /// - Optional field filtering (only apply specific fields)
    pub async fn apply(
        db: &DatabaseConnection,
        book_id: Uuid,
        plugin: &Plugin,
        metadata: &PluginBookMetadata,
        current_metadata: Option<&BookMetadata>,
        options: &BookApplyOptions,
    ) -> Result<BookMetadataApplyResult> {
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

        // We need to get the current metadata (or create a new one) to modify it
        let mut updated = match current_metadata {
            Some(m) => m.clone(),
            None => {
                // If there's no existing metadata record, we'll create one later
                // For now, get or create via the repository
                let existing = BookMetadataRepository::get_by_book_id(db, book_id).await?;
                match existing {
                    Some(m) => m,
                    None => {
                        // Create a blank metadata record first
                        BookMetadataRepository::create_with_title_and_number(
                            db, book_id, None, None,
                        )
                        .await
                        .context("Failed to create book metadata record")?
                    }
                }
            }
        };
        let mut changed = false;

        // Title
        if should_apply_field("title") {
            if let Some(title) = &metadata.title {
                let is_locked = current_metadata.map(|m| m.title_lock).unwrap_or(false);
                match check_field("title", is_locked, PluginPermission::MetadataWriteTitle) {
                    Ok(_) => {
                        updated.title = Some(title.clone());
                        // Auto-update title_sort
                        let title_sort_locked =
                            current_metadata.map(|m| m.title_sort_lock).unwrap_or(false);
                        if !title_sort_locked {
                            updated.title_sort = Some(title.clone());
                        }
                        applied_fields.push("title".to_string());
                        changed = true;
                    }
                    Err(skip) => skipped_fields.push(skip),
                }
            }
        }

        // Summary
        if should_apply_field("summary") {
            if let Some(summary) = &metadata.summary {
                let is_locked = current_metadata.map(|m| m.summary_lock).unwrap_or(false);
                match check_field("summary", is_locked, PluginPermission::MetadataWriteSummary) {
                    Ok(_) => {
                        updated.summary = Some(summary.clone());
                        applied_fields.push("summary".to_string());
                        changed = true;
                    }
                    Err(skip) => skipped_fields.push(skip),
                }
            }
        }

        // Book Type
        if should_apply_field("bookType") {
            if let Some(book_type) = &metadata.book_type {
                let is_locked = current_metadata.map(|m| m.book_type_lock).unwrap_or(false);
                match check_field(
                    "bookType",
                    is_locked,
                    PluginPermission::MetadataWriteBookType,
                ) {
                    Ok(_) => {
                        updated.book_type = Some(book_type.clone());
                        applied_fields.push("bookType".to_string());
                        changed = true;
                    }
                    Err(skip) => skipped_fields.push(skip),
                }
            }
        }

        // Subtitle
        if should_apply_field("subtitle") {
            if let Some(subtitle) = &metadata.subtitle {
                let is_locked = current_metadata.map(|m| m.subtitle_lock).unwrap_or(false);
                match check_field(
                    "subtitle",
                    is_locked,
                    PluginPermission::MetadataWriteSubtitle,
                ) {
                    Ok(_) => {
                        updated.subtitle = Some(subtitle.clone());
                        applied_fields.push("subtitle".to_string());
                        changed = true;
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
                        updated.publisher = Some(publisher.clone());
                        applied_fields.push("publisher".to_string());
                        changed = true;
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
                        updated.year = Some(year);
                        applied_fields.push("year".to_string());
                        changed = true;
                    }
                    Err(skip) => skipped_fields.push(skip),
                }
            }
        }

        // Authors JSON
        if should_apply_field("authors") && !metadata.authors.is_empty() {
            let is_locked = current_metadata
                .map(|m| m.authors_json_lock)
                .unwrap_or(false);
            match check_field("authors", is_locked, PluginPermission::MetadataWriteAuthors) {
                Ok(_) => {
                    let authors_json = serde_json::to_string(&metadata.authors)
                        .unwrap_or_else(|_| "[]".to_string());
                    updated.authors_json = Some(authors_json);
                    applied_fields.push("authors".to_string());
                    changed = true;
                }
                Err(skip) => skipped_fields.push(skip),
            }
        }

        // Translator
        if should_apply_field("translator") {
            if let Some(translator) = &metadata.translator {
                let is_locked = current_metadata.map(|m| m.translator_lock).unwrap_or(false);
                match check_field(
                    "translator",
                    is_locked,
                    PluginPermission::MetadataWriteTranslator,
                ) {
                    Ok(_) => {
                        updated.translator = Some(translator.clone());
                        applied_fields.push("translator".to_string());
                        changed = true;
                    }
                    Err(skip) => skipped_fields.push(skip),
                }
            }
        }

        // Edition
        if should_apply_field("edition") {
            if let Some(edition) = &metadata.edition {
                let is_locked = current_metadata.map(|m| m.edition_lock).unwrap_or(false);
                match check_field("edition", is_locked, PluginPermission::MetadataWriteEdition) {
                    Ok(_) => {
                        updated.edition = Some(edition.clone());
                        applied_fields.push("edition".to_string());
                        changed = true;
                    }
                    Err(skip) => skipped_fields.push(skip),
                }
            }
        }

        // Original Title
        if should_apply_field("originalTitle") {
            if let Some(original_title) = &metadata.original_title {
                let is_locked = current_metadata
                    .map(|m| m.original_title_lock)
                    .unwrap_or(false);
                match check_field(
                    "originalTitle",
                    is_locked,
                    PluginPermission::MetadataWriteOriginalTitle,
                ) {
                    Ok(_) => {
                        updated.original_title = Some(original_title.clone());
                        applied_fields.push("originalTitle".to_string());
                        changed = true;
                    }
                    Err(skip) => skipped_fields.push(skip),
                }
            }
        }

        // Original Year
        if should_apply_field("originalYear") {
            if let Some(original_year) = metadata.original_year {
                let is_locked = current_metadata
                    .map(|m| m.original_year_lock)
                    .unwrap_or(false);
                match check_field(
                    "originalYear",
                    is_locked,
                    PluginPermission::MetadataWriteOriginalYear,
                ) {
                    Ok(_) => {
                        updated.original_year = Some(original_year);
                        applied_fields.push("originalYear".to_string());
                        changed = true;
                    }
                    Err(skip) => skipped_fields.push(skip),
                }
            }
        }

        // Language
        if should_apply_field("language") {
            if let Some(language) = &metadata.language {
                let is_locked = current_metadata
                    .map(|m| m.language_iso_lock)
                    .unwrap_or(false);
                match check_field(
                    "language",
                    is_locked,
                    PluginPermission::MetadataWriteLanguage,
                ) {
                    Ok(_) => {
                        updated.language_iso = Some(language.clone());
                        applied_fields.push("language".to_string());
                        changed = true;
                    }
                    Err(skip) => skipped_fields.push(skip),
                }
            }
        }

        // ISBNs
        if should_apply_field("isbns") && !metadata.isbns.is_empty() {
            let is_locked = current_metadata.map(|m| m.isbns_lock).unwrap_or(false);
            match check_field("isbns", is_locked, PluginPermission::MetadataWriteIsbn) {
                Ok(_) => {
                    updated.isbns = Some(metadata.isbns.join(","));
                    applied_fields.push("isbns".to_string());
                    changed = true;
                }
                Err(skip) => skipped_fields.push(skip),
            }
        }

        // Series Position
        if should_apply_field("seriesPosition") {
            if let Some(series_position) = metadata.series_position {
                let is_locked = current_metadata
                    .map(|m| m.series_position_lock)
                    .unwrap_or(false);
                match check_field(
                    "seriesPosition",
                    is_locked,
                    PluginPermission::MetadataWriteSeriesPosition,
                ) {
                    Ok(_) => {
                        updated.series_position = Decimal::from_f64_retain(series_position);
                        applied_fields.push("seriesPosition".to_string());
                        changed = true;
                    }
                    Err(skip) => skipped_fields.push(skip),
                }
            }
        }

        // Series Total
        if should_apply_field("seriesTotal") {
            if let Some(series_total) = metadata.series_total {
                let is_locked = current_metadata
                    .map(|m| m.series_total_lock)
                    .unwrap_or(false);
                match check_field(
                    "seriesTotal",
                    is_locked,
                    PluginPermission::MetadataWriteSeriesPosition,
                ) {
                    Ok(_) => {
                        updated.series_total = Some(series_total);
                        applied_fields.push("seriesTotal".to_string());
                        changed = true;
                    }
                    Err(skip) => skipped_fields.push(skip),
                }
            }
        }

        // Subjects
        if should_apply_field("subjects") && !metadata.subjects.is_empty() {
            let is_locked = current_metadata.map(|m| m.subjects_lock).unwrap_or(false);
            match check_field(
                "subjects",
                is_locked,
                PluginPermission::MetadataWriteSubjects,
            ) {
                Ok(_) => {
                    let subjects_json = serde_json::to_string(&metadata.subjects)
                        .unwrap_or_else(|_| "[]".to_string());
                    updated.subjects = Some(subjects_json);
                    applied_fields.push("subjects".to_string());
                    changed = true;
                }
                Err(skip) => skipped_fields.push(skip),
            }
        }

        // Awards
        if should_apply_field("awards") && !metadata.awards.is_empty() {
            let is_locked = current_metadata
                .map(|m| m.awards_json_lock)
                .unwrap_or(false);
            match check_field("awards", is_locked, PluginPermission::MetadataWriteAwards) {
                Ok(_) => {
                    let awards_json = serde_json::to_string(&metadata.awards)
                        .unwrap_or_else(|_| "[]".to_string());
                    updated.awards_json = Some(awards_json);
                    applied_fields.push("awards".to_string());
                    changed = true;
                }
                Err(skip) => skipped_fields.push(skip),
            }
        }

        // Persist changes if anything was modified
        if changed {
            BookMetadataRepository::update(db, &updated)
                .await
                .context("Failed to update book metadata")?;
        }

        // Cover URL - download and apply cover from plugin
        if should_apply_field("coverUrl") {
            if let Some(cover_url) = &metadata.cover_url {
                let cover_locked = current_metadata.map(|m| m.cover_lock).unwrap_or(false);
                if !plugin.has_permission(&PluginPermission::MetadataWriteCovers) {
                    skipped_fields.push(SkippedField {
                        field: "coverUrl".to_string(),
                        reason: "Plugin does not have permission".to_string(),
                    });
                } else if let Some(thumbnail_service) = &options.thumbnail_service {
                    let library_id = options.library_id.unwrap_or_default();
                    match CoverService::download_and_apply_book_cover(
                        db,
                        thumbnail_service,
                        book_id,
                        library_id,
                        cover_url,
                        &plugin.name,
                        cover_locked,
                        options.event_broadcaster.as_ref(),
                    )
                    .await
                    {
                        Ok(_) => {
                            applied_fields.push("coverUrl".to_string());
                        }
                        Err(e) => {
                            warn!("Failed to download book cover: {}", e);
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

        Ok(BookMetadataApplyResult {
            applied_fields,
            skipped_fields,
        })
    }
}
