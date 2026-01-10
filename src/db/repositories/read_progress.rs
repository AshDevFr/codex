use crate::db::entities::{read_progress, read_progress::Entity as ReadProgress};
use anyhow::Result;
use chrono::Utc;
use sea_orm::*;
use uuid::Uuid;

pub struct ReadProgressRepository;

impl ReadProgressRepository {
    /// Get reading progress for a specific user and book
    pub async fn get_by_user_and_book(
        db: &DatabaseConnection,
        user_id: Uuid,
        book_id: Uuid,
    ) -> Result<Option<read_progress::Model>> {
        let progress = ReadProgress::find()
            .filter(read_progress::Column::UserId.eq(user_id))
            .filter(read_progress::Column::BookId.eq(book_id))
            .one(db)
            .await?;

        Ok(progress)
    }

    /// Create or update reading progress for a user and book
    pub async fn upsert(
        db: &DatabaseConnection,
        user_id: Uuid,
        book_id: Uuid,
        current_page: i32,
        completed: bool,
    ) -> Result<read_progress::Model> {
        // Check if progress already exists
        let existing = Self::get_by_user_and_book(db, user_id, book_id).await?;

        let now = Utc::now();

        if let Some(existing_model) = existing {
            // Update existing progress
            let mut active_model: read_progress::ActiveModel = existing_model.clone().into();
            active_model.current_page = Set(current_page);
            active_model.completed = Set(completed);
            active_model.updated_at = Set(now);

            // Set completed_at if just marked as completed
            if completed && existing_model.completed_at.is_none() {
                active_model.completed_at = Set(Some(now));
            }

            let result = active_model.update(db).await?;
            Ok(result)
        } else {
            // Create new progress
            let new_progress = read_progress::ActiveModel {
                id: Set(Uuid::new_v4()),
                user_id: Set(user_id),
                book_id: Set(book_id),
                current_page: Set(current_page),
                completed: Set(completed),
                started_at: Set(now),
                updated_at: Set(now),
                completed_at: Set(if completed { Some(now) } else { None }),
            };

            let result = new_progress.insert(db).await?;
            Ok(result)
        }
    }

    /// Delete reading progress
    pub async fn delete(db: &DatabaseConnection, user_id: Uuid, book_id: Uuid) -> Result<()> {
        ReadProgress::delete_many()
            .filter(read_progress::Column::UserId.eq(user_id))
            .filter(read_progress::Column::BookId.eq(book_id))
            .exec(db)
            .await?;

        Ok(())
    }

    /// Get all reading progress for a user
    pub async fn get_by_user(
        db: &DatabaseConnection,
        user_id: Uuid,
    ) -> Result<Vec<read_progress::Model>> {
        let progress_list = ReadProgress::find()
            .filter(read_progress::Column::UserId.eq(user_id))
            .order_by_desc(read_progress::Column::UpdatedAt)
            .all(db)
            .await?;

        Ok(progress_list)
    }

    /// Get currently reading books (not completed, sorted by most recently updated)
    pub async fn get_currently_reading(
        db: &DatabaseConnection,
        user_id: Uuid,
        limit: u64,
    ) -> Result<Vec<read_progress::Model>> {
        let progress_list = ReadProgress::find()
            .filter(read_progress::Column::UserId.eq(user_id))
            .filter(read_progress::Column::Completed.eq(false))
            .order_by_desc(read_progress::Column::UpdatedAt)
            .limit(limit)
            .all(db)
            .await?;

        Ok(progress_list)
    }

    /// Get completed books for a user
    pub async fn get_completed(
        db: &DatabaseConnection,
        user_id: Uuid,
        limit: Option<u64>,
    ) -> Result<Vec<read_progress::Model>> {
        let mut query = ReadProgress::find()
            .filter(read_progress::Column::UserId.eq(user_id))
            .filter(read_progress::Column::Completed.eq(true))
            .order_by_desc(read_progress::Column::CompletedAt);

        if let Some(limit_val) = limit {
            query = query.limit(limit_val);
        }

        let progress_list = query.all(db).await?;

        Ok(progress_list)
    }

    /// Mark a book as read (completed) for a user
    /// Sets current_page to the book's last page
    pub async fn mark_as_read(
        db: &DatabaseConnection,
        user_id: Uuid,
        book_id: Uuid,
        page_count: i32,
    ) -> Result<read_progress::Model> {
        // Mark as completed with the last page (page_count - 1, since 0-indexed)
        let last_page = if page_count > 0 { page_count - 1 } else { 0 };
        Self::upsert(db, user_id, book_id, last_page, true).await
    }

    /// Mark a book as unread for a user
    /// Deletes the reading progress record entirely
    pub async fn mark_as_unread(
        db: &DatabaseConnection,
        user_id: Uuid,
        book_id: Uuid,
    ) -> Result<()> {
        Self::delete(db, user_id, book_id).await
    }

    /// Mark all books in a series as read for a user
    /// Returns the number of books marked as read
    pub async fn mark_series_as_read(
        db: &DatabaseConnection,
        user_id: Uuid,
        book_ids: Vec<(Uuid, i32)>, // Vec of (book_id, page_count)
    ) -> Result<usize> {
        let _now = Utc::now();
        let mut count = 0;

        // Process each book
        for (book_id, page_count) in book_ids {
            let last_page = if page_count > 0 { page_count - 1 } else { 0 };
            Self::upsert(db, user_id, book_id, last_page, true).await?;
            count += 1;
        }

        Ok(count)
    }

    /// Mark all books in a series as unread for a user
    /// Deletes all reading progress records for the books
    /// Returns the number of books marked as unread
    pub async fn mark_series_as_unread(
        db: &DatabaseConnection,
        user_id: Uuid,
        book_ids: Vec<Uuid>,
    ) -> Result<u64> {
        let result = ReadProgress::delete_many()
            .filter(read_progress::Column::UserId.eq(user_id))
            .filter(read_progress::Column::BookId.is_in(book_ids))
            .exec(db)
            .await?;

        Ok(result.rows_affected)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::permissions::ADMIN_PERMISSIONS;
    use crate::db::entities::{books, users};
    use crate::db::repositories::{
        BookRepository, LibraryRepository, SeriesRepository, UserRepository,
    };
    use crate::db::test_helpers::setup_test_db;
    use crate::models::ScanningStrategy;
    use crate::utils::password;
    use sea_orm::prelude::Decimal;

    async fn create_test_user(db: &DatabaseConnection) -> users::Model {
        let password_hash = password::hash_password("password").unwrap();
        let permissions_vec: Vec<_> = ADMIN_PERMISSIONS.iter().cloned().collect();
        let user = users::Model {
            id: Uuid::new_v4(),
            username: "testuser".to_string(),
            email: "test@example.com".to_string(),
            password_hash,
            is_admin: true,
            is_active: true,
            email_verified: false,
            permissions: serde_json::to_value(&permissions_vec).unwrap(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            last_login_at: None,
        };
        UserRepository::create(db, &user).await.unwrap()
    }

    async fn create_test_book(db: &DatabaseConnection) -> books::Model {
        // Create a library first
        let library = LibraryRepository::create(
            db,
            "Test Library",
            "/test/library",
            ScanningStrategy::Default,
        )
        .await
        .unwrap();

        // Create a series
        let series = SeriesRepository::create(db, library.id, "Test Series", None)
            .await
            .unwrap();

        // Create a book
        let book = books::Model {
            id: Uuid::new_v4(),
            series_id: series.id,
            library_id: library.id,
            title: Some("Test Book".to_string()),
            file_path: format!("/test/book_{}.cbz", Uuid::new_v4()),
            file_name: "book.cbz".to_string(),
            file_size: 1024,
            file_hash: format!("hash_{}", Uuid::new_v4()),
            partial_hash: String::new(),
            format: "cbz".to_string(),
            number: Some(Decimal::from(1)),
            page_count: 50,
            deleted: false,
            analyzed: false,
            modified_at: Utc::now(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            thumbnail_path: None,
            thumbnail_generated_at: None,
        };
        BookRepository::create(db, &book, None).await.unwrap()
    }

    #[tokio::test]
    async fn test_create_progress() {
        let db = setup_test_db().await;
        let user = create_test_user(&db).await;
        let book = create_test_book(&db).await;

        let progress = ReadProgressRepository::upsert(&db, user.id, book.id, 10, false)
            .await
            .unwrap();

        assert_eq!(progress.user_id, user.id);
        assert_eq!(progress.book_id, book.id);
        assert_eq!(progress.current_page, 10);
        assert!(!progress.completed);
        assert!(progress.completed_at.is_none());
    }

    #[tokio::test]
    async fn test_update_progress() {
        let db = setup_test_db().await;
        let user = create_test_user(&db).await;
        let book = create_test_book(&db).await;

        // Create initial progress
        ReadProgressRepository::upsert(&db, user.id, book.id, 10, false)
            .await
            .unwrap();

        // Update progress
        let updated = ReadProgressRepository::upsert(&db, user.id, book.id, 25, false)
            .await
            .unwrap();

        assert_eq!(updated.current_page, 25);
        assert!(!updated.completed);
    }

    #[tokio::test]
    async fn test_mark_as_completed() {
        let db = setup_test_db().await;
        let user = create_test_user(&db).await;
        let book = create_test_book(&db).await;

        // Create progress
        ReadProgressRepository::upsert(&db, user.id, book.id, 10, false)
            .await
            .unwrap();

        // Mark as completed
        let completed = ReadProgressRepository::upsert(&db, user.id, book.id, 50, true)
            .await
            .unwrap();

        assert!(completed.completed);
        assert!(completed.completed_at.is_some());
    }

    #[tokio::test]
    async fn test_get_by_user() {
        let db = setup_test_db().await;
        let user = create_test_user(&db).await;
        let book1 = create_test_book(&db).await;
        let book2 = create_test_book(&db).await;

        ReadProgressRepository::upsert(&db, user.id, book1.id, 10, false)
            .await
            .unwrap();
        ReadProgressRepository::upsert(&db, user.id, book2.id, 25, true)
            .await
            .unwrap();

        let progress_list = ReadProgressRepository::get_by_user(&db, user.id)
            .await
            .unwrap();

        assert_eq!(progress_list.len(), 2);
    }

    #[tokio::test]
    async fn test_get_currently_reading() {
        let db = setup_test_db().await;
        let user = create_test_user(&db).await;
        let book1 = create_test_book(&db).await;
        let book2 = create_test_book(&db).await;
        let book3 = create_test_book(&db).await;

        ReadProgressRepository::upsert(&db, user.id, book1.id, 10, false)
            .await
            .unwrap();
        ReadProgressRepository::upsert(&db, user.id, book2.id, 25, false)
            .await
            .unwrap();
        ReadProgressRepository::upsert(&db, user.id, book3.id, 50, true)
            .await
            .unwrap();

        let currently_reading = ReadProgressRepository::get_currently_reading(&db, user.id, 10)
            .await
            .unwrap();

        assert_eq!(currently_reading.len(), 2);
        assert!(!currently_reading[0].completed);
        assert!(!currently_reading[1].completed);
    }

    #[tokio::test]
    async fn test_get_completed() {
        let db = setup_test_db().await;
        let user = create_test_user(&db).await;
        let book1 = create_test_book(&db).await;
        let book2 = create_test_book(&db).await;

        ReadProgressRepository::upsert(&db, user.id, book1.id, 50, true)
            .await
            .unwrap();
        ReadProgressRepository::upsert(&db, user.id, book2.id, 25, false)
            .await
            .unwrap();

        let completed = ReadProgressRepository::get_completed(&db, user.id, None)
            .await
            .unwrap();

        assert_eq!(completed.len(), 1);
        assert!(completed[0].completed);
    }

    #[tokio::test]
    async fn test_delete_progress() {
        let db = setup_test_db().await;
        let user = create_test_user(&db).await;
        let book = create_test_book(&db).await;

        ReadProgressRepository::upsert(&db, user.id, book.id, 10, false)
            .await
            .unwrap();

        ReadProgressRepository::delete(&db, user.id, book.id)
            .await
            .unwrap();

        let progress = ReadProgressRepository::get_by_user_and_book(&db, user.id, book.id)
            .await
            .unwrap();

        assert!(progress.is_none());
    }

    #[tokio::test]
    async fn test_mark_as_read() {
        let db = setup_test_db().await;
        let user = create_test_user(&db).await;
        let book = create_test_book(&db).await;

        // Mark book as read
        let progress = ReadProgressRepository::mark_as_read(&db, user.id, book.id, book.page_count)
            .await
            .unwrap();

        assert_eq!(progress.user_id, user.id);
        assert_eq!(progress.book_id, book.id);
        assert_eq!(progress.current_page, book.page_count - 1); // 0-indexed
        assert!(progress.completed);
        assert!(progress.completed_at.is_some());
    }

    #[tokio::test]
    async fn test_mark_as_unread() {
        let db = setup_test_db().await;
        let user = create_test_user(&db).await;
        let book = create_test_book(&db).await;

        // Create progress first
        ReadProgressRepository::upsert(&db, user.id, book.id, 25, false)
            .await
            .unwrap();

        // Mark as unread
        ReadProgressRepository::mark_as_unread(&db, user.id, book.id)
            .await
            .unwrap();

        // Verify progress is deleted
        let progress = ReadProgressRepository::get_by_user_and_book(&db, user.id, book.id)
            .await
            .unwrap();

        assert!(progress.is_none());
    }

    #[tokio::test]
    async fn test_mark_series_as_read() {
        let db = setup_test_db().await;
        let user = create_test_user(&db).await;
        let book1 = create_test_book(&db).await;
        let book2 = create_test_book(&db).await;
        let book3 = create_test_book(&db).await;

        // Create book data with page counts
        let book_data = vec![
            (book1.id, book1.page_count),
            (book2.id, book2.page_count),
            (book3.id, book3.page_count),
        ];

        // Mark all books as read
        let count = ReadProgressRepository::mark_series_as_read(&db, user.id, book_data)
            .await
            .unwrap();

        assert_eq!(count, 3);

        // Verify all books are marked as read
        let progress1 = ReadProgressRepository::get_by_user_and_book(&db, user.id, book1.id)
            .await
            .unwrap()
            .unwrap();
        let progress2 = ReadProgressRepository::get_by_user_and_book(&db, user.id, book2.id)
            .await
            .unwrap()
            .unwrap();
        let progress3 = ReadProgressRepository::get_by_user_and_book(&db, user.id, book3.id)
            .await
            .unwrap()
            .unwrap();

        assert!(progress1.completed);
        assert!(progress2.completed);
        assert!(progress3.completed);
        assert_eq!(progress1.current_page, book1.page_count - 1);
        assert_eq!(progress2.current_page, book2.page_count - 1);
        assert_eq!(progress3.current_page, book3.page_count - 1);
    }

    #[tokio::test]
    async fn test_mark_series_as_unread() {
        let db = setup_test_db().await;
        let user = create_test_user(&db).await;
        let book1 = create_test_book(&db).await;
        let book2 = create_test_book(&db).await;
        let book3 = create_test_book(&db).await;

        // Create progress for all books
        ReadProgressRepository::upsert(&db, user.id, book1.id, 10, false)
            .await
            .unwrap();
        ReadProgressRepository::upsert(&db, user.id, book2.id, 20, true)
            .await
            .unwrap();
        ReadProgressRepository::upsert(&db, user.id, book3.id, 30, false)
            .await
            .unwrap();

        // Mark all books as unread
        let book_ids = vec![book1.id, book2.id, book3.id];
        let count = ReadProgressRepository::mark_series_as_unread(&db, user.id, book_ids)
            .await
            .unwrap();

        assert_eq!(count, 3);

        // Verify all progress is deleted
        let progress1 = ReadProgressRepository::get_by_user_and_book(&db, user.id, book1.id)
            .await
            .unwrap();
        let progress2 = ReadProgressRepository::get_by_user_and_book(&db, user.id, book2.id)
            .await
            .unwrap();
        let progress3 = ReadProgressRepository::get_by_user_and_book(&db, user.id, book3.id)
            .await
            .unwrap();

        assert!(progress1.is_none());
        assert!(progress2.is_none());
        assert!(progress3.is_none());
    }
}
