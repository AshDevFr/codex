//! Read progress batching service
//!
//! Collects read progress updates in memory and flushes them to the database
//! periodically to reduce database load during high-traffic page viewing.

use anyhow::Result;
use dashmap::DashMap;
use sea_orm::DatabaseConnection;
use std::sync::Arc;
use tokio::sync::Notify;
use tokio::time::{interval, Duration as TokioDuration};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, warn};
use uuid::Uuid;

use crate::db::repositories::ReadProgressRepository;

/// Maximum number of entries before forcing a flush
const MAX_BUFFER_SIZE: usize = 100;

/// Default flush interval in seconds
const DEFAULT_FLUSH_INTERVAL_SECS: u64 = 5;

/// A pending progress update
#[derive(Debug, Clone)]
struct PendingProgress {
    page_number: i32,
    total_pages: i32,
}

/// Read progress batching service
///
/// Collects read progress updates in memory and periodically flushes them to
/// the database. This reduces the number of database operations during
/// high-traffic page viewing scenarios.
#[derive(Clone)]
pub struct ReadProgressService {
    /// In-memory buffer of pending updates: (user_id, book_id) -> progress
    buffer: Arc<DashMap<(Uuid, Uuid), PendingProgress>>,
    /// Database connection for flushing
    db: DatabaseConnection,
    /// Notify when buffer reaches max size to trigger early flush
    flush_notify: Arc<Notify>,
}

impl ReadProgressService {
    /// Create a new read progress service
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            buffer: Arc::new(DashMap::new()),
            db,
            flush_notify: Arc::new(Notify::new()),
        }
    }

    /// Record a progress update
    ///
    /// Updates are buffered in memory and flushed periodically. If the buffer
    /// reaches MAX_BUFFER_SIZE, an early flush is triggered.
    ///
    /// Only forward progress is recorded - if the user is viewing an earlier
    /// page than their current progress, the update is ignored.
    pub async fn record_progress(
        &self,
        user_id: Uuid,
        book_id: Uuid,
        page_number: i32,
        total_pages: i32,
    ) {
        let key = (user_id, book_id);

        // Check if we should update (only forward progress)
        let should_update = match self.buffer.get(&key) {
            Some(existing) => page_number > existing.page_number,
            None => {
                // Check database for existing progress
                match ReadProgressRepository::get_by_user_and_book(&self.db, user_id, book_id).await
                {
                    Ok(Some(progress)) => page_number > progress.current_page,
                    Ok(None) => true,
                    Err(e) => {
                        warn!(
                            "Failed to check existing progress for book {}: {}",
                            book_id, e
                        );
                        // On error, still allow the update to avoid losing progress
                        true
                    }
                }
            }
        };

        if should_update {
            self.buffer.insert(
                key,
                PendingProgress {
                    page_number,
                    total_pages,
                },
            );

            // Trigger early flush if buffer is full
            if self.buffer.len() >= MAX_BUFFER_SIZE {
                self.flush_notify.notify_one();
            }
        }
    }

    /// Get the current number of pending updates
    #[cfg(test)]
    pub fn pending_count(&self) -> usize {
        self.buffer.len()
    }

    /// Flush all pending progress updates to the database
    pub async fn flush(&self) -> Result<usize> {
        // Take all pending entries
        let entries: Vec<_> = self
            .buffer
            .iter()
            .map(|entry| {
                let (user_id, book_id) = *entry.key();
                let progress = entry.value().clone();
                (user_id, book_id, progress)
            })
            .collect();

        if entries.is_empty() {
            return Ok(0);
        }

        // Clear buffer before processing to avoid blocking new updates
        // (entries we just collected will be processed)
        for (user_id, book_id, _) in &entries {
            self.buffer.remove(&(*user_id, *book_id));
        }

        let count = entries.len();
        debug!("Flushing {} read progress updates", count);

        // Process each entry
        for (user_id, book_id, progress) in entries {
            let is_completed = progress.page_number >= progress.total_pages;

            if let Err(e) = ReadProgressRepository::upsert(
                &self.db,
                user_id,
                book_id,
                progress.page_number,
                is_completed,
            )
            .await
            {
                error!(
                    "Failed to flush read progress for user {} book {}: {}",
                    user_id, book_id, e
                );
                // Re-add to buffer for retry on next flush
                self.buffer.insert(
                    (user_id, book_id),
                    PendingProgress {
                        page_number: progress.page_number,
                        total_pages: progress.total_pages,
                    },
                );
            }
        }

        Ok(count)
    }

    /// Start the background flush job
    ///
    /// Accepts a `CancellationToken` for graceful shutdown support.
    /// Returns a `JoinHandle` that can be awaited on shutdown.
    pub fn start_background_flush(
        self: Arc<Self>,
        cancel_token: CancellationToken,
    ) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let mut flush_interval =
                interval(TokioDuration::from_secs(DEFAULT_FLUSH_INTERVAL_SECS));

            loop {
                tokio::select! {
                    _ = cancel_token.cancelled() => {
                        // Final flush before shutdown
                        debug!("Read progress service shutting down, performing final flush");
                        if let Err(e) = self.flush().await {
                            error!("Failed to flush read progress during shutdown: {}", e);
                        }
                        break;
                    }
                    _ = self.flush_notify.notified() => {
                        // Buffer full, trigger early flush
                        debug!("Read progress buffer full, triggering early flush");
                        if let Err(e) = self.flush().await {
                            error!("Failed to flush read progress (buffer full): {}", e);
                        }
                    }
                    _ = flush_interval.tick() => {
                        if let Err(e) = self.flush().await {
                            error!("Failed to flush read progress: {}", e);
                        }
                    }
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::entities::{books, users};
    use crate::db::repositories::{
        BookRepository, LibraryRepository, ReadProgressRepository, SeriesRepository, UserRepository,
    };
    use crate::db::test_helpers::setup_test_db;
    use crate::models::ScanningStrategy;
    use crate::utils::password;
    use chrono::Utc;
    use std::time::Duration;

    async fn create_test_user(db: &DatabaseConnection) -> users::Model {
        let password_hash = password::hash_password("password").unwrap();
        let user = users::Model {
            id: Uuid::new_v4(),
            username: format!("testuser_{}", Uuid::new_v4()),
            email: format!("test_{}@example.com", Uuid::new_v4()),
            password_hash,
            role: "admin".to_string(),
            is_active: true,
            email_verified: false,
            permissions: serde_json::json!([]),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            last_login_at: None,
        };
        UserRepository::create(db, &user).await.unwrap()
    }

    async fn create_test_book(db: &DatabaseConnection, page_count: i32) -> books::Model {
        let library = LibraryRepository::create(
            db,
            &format!("Test Library {}", Uuid::new_v4()),
            &format!("/test/library_{}", Uuid::new_v4()),
            ScanningStrategy::Default,
        )
        .await
        .unwrap();

        let series = SeriesRepository::create(db, library.id, "Test Series", None)
            .await
            .unwrap();

        let book = books::Model {
            id: Uuid::new_v4(),
            series_id: series.id,
            library_id: library.id,
            file_path: format!("/test/book_{}.cbz", Uuid::new_v4()),
            file_name: "book.cbz".to_string(),
            file_size: 1024,
            file_hash: format!("hash_{}", Uuid::new_v4()),
            partial_hash: String::new(),
            format: "cbz".to_string(),
            page_count,
            deleted: false,
            analyzed: false,
            analysis_error: None,
            modified_at: Utc::now(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            thumbnail_path: None,
            thumbnail_generated_at: None,
        };
        BookRepository::create(db, &book, None).await.unwrap()
    }

    #[tokio::test]
    async fn test_record_progress_buffers_updates() {
        let db = setup_test_db().await;
        let service = ReadProgressService::new(db.clone());

        let user = create_test_user(&db).await;
        let book = create_test_book(&db, 50).await;

        // Record progress
        service
            .record_progress(user.id, book.id, 10, book.page_count)
            .await;

        // Should be buffered, not in database yet
        assert_eq!(service.pending_count(), 1);
        let db_progress = ReadProgressRepository::get_by_user_and_book(&db, user.id, book.id)
            .await
            .unwrap();
        assert!(db_progress.is_none());
    }

    #[tokio::test]
    async fn test_flush_writes_to_database() {
        let db = setup_test_db().await;
        let service = ReadProgressService::new(db.clone());

        let user = create_test_user(&db).await;
        let book = create_test_book(&db, 50).await;

        // Record progress
        service
            .record_progress(user.id, book.id, 10, book.page_count)
            .await;

        // Flush
        let count = service.flush().await.unwrap();
        assert_eq!(count, 1);
        assert_eq!(service.pending_count(), 0);

        // Should be in database now
        let db_progress = ReadProgressRepository::get_by_user_and_book(&db, user.id, book.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(db_progress.current_page, 10);
        assert!(!db_progress.completed);
    }

    #[tokio::test]
    async fn test_only_forward_progress_recorded() {
        let db = setup_test_db().await;
        let service = ReadProgressService::new(db.clone());

        let user = create_test_user(&db).await;
        let book = create_test_book(&db, 50).await;

        // Record progress to page 20
        service
            .record_progress(user.id, book.id, 20, book.page_count)
            .await;

        // Try to record earlier page - should be ignored
        service
            .record_progress(user.id, book.id, 10, book.page_count)
            .await;

        // Flush and verify only page 20 is recorded
        service.flush().await.unwrap();
        let db_progress = ReadProgressRepository::get_by_user_and_book(&db, user.id, book.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(db_progress.current_page, 20);
    }

    #[tokio::test]
    async fn test_forward_progress_updates_buffer() {
        let db = setup_test_db().await;
        let service = ReadProgressService::new(db.clone());

        let user = create_test_user(&db).await;
        let book = create_test_book(&db, 50).await;

        // Record progress to page 10
        service
            .record_progress(user.id, book.id, 10, book.page_count)
            .await;

        // Record forward progress to page 30
        service
            .record_progress(user.id, book.id, 30, book.page_count)
            .await;

        // Flush and verify latest progress is recorded
        service.flush().await.unwrap();
        let db_progress = ReadProgressRepository::get_by_user_and_book(&db, user.id, book.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(db_progress.current_page, 30);
    }

    #[tokio::test]
    async fn test_marks_completed_on_last_page() {
        let db = setup_test_db().await;
        let service = ReadProgressService::new(db.clone());

        let user = create_test_user(&db).await;
        let book = create_test_book(&db, 50).await;

        // Record progress to last page
        service
            .record_progress(user.id, book.id, 50, book.page_count)
            .await;

        // Flush and verify completion
        service.flush().await.unwrap();
        let db_progress = ReadProgressRepository::get_by_user_and_book(&db, user.id, book.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(db_progress.current_page, 50);
        assert!(db_progress.completed);
    }

    #[tokio::test]
    async fn test_multiple_users_and_books() {
        let db = setup_test_db().await;
        let service = ReadProgressService::new(db.clone());

        let user1 = create_test_user(&db).await;
        let user2 = create_test_user(&db).await;
        let book1 = create_test_book(&db, 50).await;
        let book2 = create_test_book(&db, 100).await;

        // Record progress for multiple users and books
        service
            .record_progress(user1.id, book1.id, 10, book1.page_count)
            .await;
        service
            .record_progress(user1.id, book2.id, 20, book2.page_count)
            .await;
        service
            .record_progress(user2.id, book1.id, 30, book1.page_count)
            .await;

        assert_eq!(service.pending_count(), 3);

        // Flush all
        let count = service.flush().await.unwrap();
        assert_eq!(count, 3);

        // Verify all progress recorded
        let p1 = ReadProgressRepository::get_by_user_and_book(&db, user1.id, book1.id)
            .await
            .unwrap()
            .unwrap();
        let p2 = ReadProgressRepository::get_by_user_and_book(&db, user1.id, book2.id)
            .await
            .unwrap()
            .unwrap();
        let p3 = ReadProgressRepository::get_by_user_and_book(&db, user2.id, book1.id)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(p1.current_page, 10);
        assert_eq!(p2.current_page, 20);
        assert_eq!(p3.current_page, 30);
    }

    #[tokio::test]
    async fn test_respects_existing_database_progress() {
        let db = setup_test_db().await;
        let service = ReadProgressService::new(db.clone());

        let user = create_test_user(&db).await;
        let book = create_test_book(&db, 50).await;

        // Create existing progress in database
        ReadProgressRepository::upsert(&db, user.id, book.id, 25, false)
            .await
            .unwrap();

        // Try to record earlier page - should be ignored
        service
            .record_progress(user.id, book.id, 10, book.page_count)
            .await;

        // Buffer should be empty since progress was rejected
        assert_eq!(service.pending_count(), 0);

        // Record forward progress - should be accepted
        service
            .record_progress(user.id, book.id, 35, book.page_count)
            .await;
        assert_eq!(service.pending_count(), 1);
    }

    #[tokio::test]
    async fn test_background_flush_graceful_shutdown() {
        let db = setup_test_db().await;
        let service = Arc::new(ReadProgressService::new(db.clone()));

        let user = create_test_user(&db).await;
        let book = create_test_book(&db, 50).await;

        // Start background flush
        let cancel_token = CancellationToken::new();
        let handle = service.clone().start_background_flush(cancel_token.clone());

        // Record progress
        service
            .record_progress(user.id, book.id, 10, book.page_count)
            .await;

        // Cancel and wait for shutdown (should trigger final flush)
        cancel_token.cancel();
        tokio::time::timeout(Duration::from_secs(5), handle)
            .await
            .expect("Background task should complete")
            .expect("Task should not panic");

        // Verify progress was flushed
        let db_progress = ReadProgressRepository::get_by_user_and_book(&db, user.id, book.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(db_progress.current_page, 10);
    }

    #[tokio::test]
    async fn test_flush_empty_buffer() {
        let db = setup_test_db().await;
        let service = ReadProgressService::new(db);

        // Flushing empty buffer should succeed
        let count = service.flush().await.unwrap();
        assert_eq!(count, 0);
    }
}
