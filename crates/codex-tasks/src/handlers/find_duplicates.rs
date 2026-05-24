use anyhow::Result;
use sea_orm::DatabaseConnection;
use std::sync::Arc;
use tracing::{info, warn};

use crate::types::TaskResult;
use codex_db::entities::tasks;
use codex_db::repositories::{
    BookDuplicatesRepository, SeriesDuplicatesRepository, SettingsRepository,
};
use codex_events::EventBroadcaster;

use super::TaskHandler;

/// Settings key that holds the JSON-encoded array of trusted
/// `series_external_ids.source` values. Seeded by
/// `m20260520_000086_seed_duplicate_detection_settings`.
pub const TRUSTED_EXTERNAL_ID_SOURCES_KEY: &str = "duplicate_detection.trusted_external_id_sources";

/// Handler for finding duplicate books and series.
pub struct FindDuplicatesHandler;

impl Default for FindDuplicatesHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl FindDuplicatesHandler {
    pub fn new() -> Self {
        Self
    }
}

/// Resolve the trusted-source whitelist for the external-ID duplicate pass.
/// Reads `duplicate_detection.trusted_external_id_sources` (a JSON array)
/// directly from the settings table so changes take effect on the next scan
/// without restarting the worker. Falls back to an empty whitelist if the
/// setting is missing or malformed.
pub async fn load_trusted_external_id_sources(db: &DatabaseConnection) -> Vec<String> {
    match SettingsRepository::get_value::<Vec<String>>(db, TRUSTED_EXTERNAL_ID_SOURCES_KEY).await {
        Ok(Some(sources)) => sources,
        Ok(None) => Vec::new(),
        Err(e) => {
            warn!(
                "Failed to load `{}`; treating as empty (external-ID pass disabled): {}",
                TRUSTED_EXTERNAL_ID_SOURCES_KEY, e
            );
            Vec::new()
        }
    }
}

impl TaskHandler for FindDuplicatesHandler {
    fn handle<'a>(
        &'a self,
        _task: &'a tasks::Model,
        db: &'a DatabaseConnection,
        _event_broadcaster: Option<&'a Arc<EventBroadcaster>>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<TaskResult>> + Send + 'a>> {
        Box::pin(async move {
            info!("Starting duplicate detection scan");

            let trusted = load_trusted_external_id_sources(db).await;

            let book_groups = BookDuplicatesRepository::rebuild_from_books(db).await?;
            let series_groups =
                SeriesDuplicatesRepository::rebuild_from_series(db, &trusted).await?;

            info!(
                "Duplicate detection complete: {} book groups, {} series groups",
                book_groups, series_groups
            );

            Ok(TaskResult::success_with_data(
                format!(
                    "Found {} book and {} series duplicate groups",
                    book_groups, series_groups
                ),
                serde_json::json!({
                    "duplicate_groups": book_groups,
                    "book_duplicate_groups": book_groups,
                    "series_duplicate_groups": series_groups,
                }),
            ))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_handler_creation() {
        let _handler = FindDuplicatesHandler::new();
    }
}
