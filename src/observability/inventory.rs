//! Background poller that refreshes the inventory metric atomics.
//!
//! The OTel observable gauges read these atomics synchronously on each
//! collection cycle (see `metrics::install_inventory_gauges`). Polling the
//! database from inside a sync gauge callback is not feasible because the
//! SDK calls the callback from a non-tokio thread; we keep the DB queries
//! on the async runtime and the gauge callbacks read the cached values.

use std::sync::Arc;
use std::time::Duration;

use sea_orm::DatabaseConnection;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::warn;

use codex_db::repositories::MetricsRepository;

/// Spawn the inventory snapshot poller. Runs every `interval` until the
/// cancellation token fires.
pub fn spawn_poller(
    db: Arc<DatabaseConnection>,
    interval: Duration,
    cancel: CancellationToken,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        // Refresh once immediately so the first export cycle has fresh data.
        refresh(&db).await;

        let mut ticker = tokio::time::interval(interval);
        // Skip the immediate tick (we just did one).
        ticker.tick().await;

        loop {
            tokio::select! {
                _ = cancel.cancelled() => break,
                _ = ticker.tick() => refresh(&db).await,
            }
        }
    })
}

async fn refresh(db: &DatabaseConnection) {
    let libraries = MetricsRepository::count_libraries(db).await;
    let series = MetricsRepository::count_series(db).await;
    let books = MetricsRepository::count_books(db).await;
    let users = MetricsRepository::count_users(db).await;
    let pages = MetricsRepository::count_pages(db).await;

    let (Ok(libraries), Ok(series), Ok(books), Ok(users), Ok(pages)) =
        (libraries, series, books, users, pages)
    else {
        warn!("Inventory metric refresh failed; leaving previous snapshot in place");
        return;
    };

    super::metrics::update_inventory_snapshot(libraries, series, books, users, pages);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::Ordering;

    #[tokio::test]
    async fn refresh_writes_snapshot_atomics() {
        // Empty in-memory SQLite with the schema migrated so the count
        // queries return zero rather than erroring. The cheapest way to
        // exercise the refresh path end-to-end without coupling the test to
        // a fixture builder.
        let db = codex_db::test_helpers::setup_test_db().await;

        // Pre-load known sentinel values so we can detect that the refresh
        // overwrote them with zeros (or any other DB count).
        super::super::metrics::update_inventory_snapshot(99, 99, 99, 99, 99);

        refresh(&db).await;

        let snap = super::super::metrics::inventory_snapshot();
        assert_eq!(snap.libraries.load(Ordering::Relaxed), 0);
        assert_eq!(snap.series.load(Ordering::Relaxed), 0);
        assert_eq!(snap.books.load(Ordering::Relaxed), 0);
    }
}
