//! Helpers for emitting `ReleaseAnnounced` notifications.
//!
//! Lives in `services::release` because both the services-side reverse-RPC
//! handler (plugin → host announce) and the tasks-side polling worker need
//! the same series-title lookup before broadcasting. Keeping the helper here
//! means tasks depends on services, not the other way around.

use codex_db::repositories::SeriesRepository;
use sea_orm::DatabaseConnection;
use tracing::warn;
use uuid::Uuid;

/// Resolve the display title for a series, preferring `series_metadata.title`
/// and falling back to the directory-derived `series.name`. Returns an empty
/// string if the series row is missing (shouldn't happen for a valid ledger
/// insert, but we don't want a notification failure to surface as a panic).
pub async fn lookup_series_title(db: &DatabaseConnection, series_id: Uuid) -> String {
    match SeriesRepository::get_with_metadata(db, series_id).await {
        Ok(Some((series, metadata))) => metadata.map(|m| m.title).unwrap_or(series.name),
        Ok(None) => String::new(),
        Err(e) => {
            warn!(
                "Failed to look up title for series {} (release notification): {}",
                series_id, e
            );
            String::new()
        }
    }
}
