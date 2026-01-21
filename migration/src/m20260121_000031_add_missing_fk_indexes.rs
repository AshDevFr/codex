use sea_orm_migration::prelude::*;

/// Migration to add missing indexes on foreign key columns.
///
/// These indexes are critical for query performance. Without them,
/// queries that filter or join on these columns require full table scans.
///
/// Critical indexes added:
/// - books.series_id: Used in list_by_series queries (was causing connection pool exhaustion)
/// - books.library_id: Used in list_by_library queries
/// - pages.book_id: Used in page retrieval queries (critical read path)
/// - book_metadata.book_id: Used in book detail queries with metadata joins
/// - read_progress.user_id: Used in user reading history queries
/// - task_metrics.library_id: Used in metrics filtering by library
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // CRITICAL: Index on books.series_id
        // This was causing connection pool exhaustion with 7000+ books
        // Every list_by_series query was doing a full table scan
        manager
            .create_index(
                Index::create()
                    .name("idx_books_series_id")
                    .table(Books::Table)
                    .col(Books::SeriesId)
                    .to_owned(),
            )
            .await?;

        // Index on books.library_id for efficient library-scoped queries
        // Note: There's already a composite unique index (library_id, file_path)
        // but a single-column index is more efficient for library-only filters
        manager
            .create_index(
                Index::create()
                    .name("idx_books_library_id")
                    .table(Books::Table)
                    .col(Books::LibraryId)
                    .to_owned(),
            )
            .await?;

        // CRITICAL: Index on pages.book_id for page retrieval
        // This is the critical read path when viewing book pages
        manager
            .create_index(
                Index::create()
                    .name("idx_pages_book_id")
                    .table(Pages::Table)
                    .col(Pages::BookId)
                    .to_owned(),
            )
            .await?;

        // Index on book_metadata.book_id for metadata lookups
        // Used when fetching book details with metadata
        manager
            .create_index(
                Index::create()
                    .name("idx_book_metadata_book_id")
                    .table(BookMetadata::Table)
                    .col(BookMetadata::BookId)
                    .to_owned(),
            )
            .await?;

        // Index on read_progress.user_id for user reading history queries
        // The composite unique index (user_id, book_id) exists but a single-column
        // index is more efficient for "all books for user" queries
        manager
            .create_index(
                Index::create()
                    .name("idx_read_progress_user_id")
                    .table(ReadProgress::Table)
                    .col(ReadProgress::UserId)
                    .to_owned(),
            )
            .await?;

        // Index on task_metrics.library_id for metrics filtering
        manager
            .create_index(
                Index::create()
                    .name("idx_task_metrics_library_id")
                    .table(TaskMetrics::Table)
                    .col(TaskMetrics::LibraryId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name("idx_books_series_id")
                    .table(Books::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_books_library_id")
                    .table(Books::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_pages_book_id")
                    .table(Pages::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_book_metadata_book_id")
                    .table(BookMetadata::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_read_progress_user_id")
                    .table(ReadProgress::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_task_metrics_library_id")
                    .table(TaskMetrics::Table)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
enum Books {
    Table,
    SeriesId,
    LibraryId,
}

#[derive(DeriveIden)]
enum Pages {
    Table,
    BookId,
}

#[derive(DeriveIden)]
enum BookMetadata {
    Table,
    BookId,
}

#[derive(DeriveIden)]
enum ReadProgress {
    Table,
    UserId,
}

#[derive(DeriveIden)]
enum TaskMetrics {
    Table,
    LibraryId,
}
