use sea_orm_migration::prelude::*;

/// Migration to add indexes for efficient sorting operations.
///
/// These indexes optimize common sort operations on series and books:
/// - Date sorting: created_at, updated_at on series and books tables
/// - Title sorting: title_sort on series_metadata and book_metadata tables
/// - Release date sorting: year on series_metadata and book_metadata tables
///
/// Without these indexes, sorting large collections requires full table scans
/// followed by expensive sort operations.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // ============================================
        // Series table indexes for date sorting
        // ============================================

        // Index on series.created_at for "date added" sorting
        manager
            .create_index(
                Index::create()
                    .name("idx_series_created_at")
                    .table(Series::Table)
                    .col(Series::CreatedAt)
                    .to_owned(),
            )
            .await?;

        // Index on series.updated_at for "date updated" sorting
        manager
            .create_index(
                Index::create()
                    .name("idx_series_updated_at")
                    .table(Series::Table)
                    .col(Series::UpdatedAt)
                    .to_owned(),
            )
            .await?;

        // ============================================
        // Books table indexes for date sorting
        // ============================================

        // Index on books.created_at for "date added" sorting
        manager
            .create_index(
                Index::create()
                    .name("idx_books_created_at")
                    .table(Books::Table)
                    .col(Books::CreatedAt)
                    .to_owned(),
            )
            .await?;

        // Index on books.updated_at for "date updated" sorting
        manager
            .create_index(
                Index::create()
                    .name("idx_books_updated_at")
                    .table(Books::Table)
                    .col(Books::UpdatedAt)
                    .to_owned(),
            )
            .await?;

        // ============================================
        // Series metadata indexes for title and year sorting
        // ============================================

        // Index on series_metadata.title_sort for name sorting
        // This is the primary sort field for series by name
        manager
            .create_index(
                Index::create()
                    .name("idx_series_metadata_title_sort")
                    .table(SeriesMetadata::Table)
                    .col(SeriesMetadata::TitleSort)
                    .to_owned(),
            )
            .await?;

        // Index on series_metadata.year for release date sorting
        manager
            .create_index(
                Index::create()
                    .name("idx_series_metadata_year")
                    .table(SeriesMetadata::Table)
                    .col(SeriesMetadata::Year)
                    .to_owned(),
            )
            .await?;

        // ============================================
        // Book metadata indexes for title and year sorting
        // ============================================

        // Index on book_metadata.title_sort for title sorting
        manager
            .create_index(
                Index::create()
                    .name("idx_book_metadata_title_sort")
                    .table(BookMetadata::Table)
                    .col(BookMetadata::TitleSort)
                    .to_owned(),
            )
            .await?;

        // Index on book_metadata.year for release date sorting
        manager
            .create_index(
                Index::create()
                    .name("idx_book_metadata_year")
                    .table(BookMetadata::Table)
                    .col(BookMetadata::Year)
                    .to_owned(),
            )
            .await?;

        // Index on book_metadata.number for chapter number sorting
        manager
            .create_index(
                Index::create()
                    .name("idx_book_metadata_number")
                    .table(BookMetadata::Table)
                    .col(BookMetadata::Number)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Drop all indexes in reverse order
        manager
            .drop_index(
                Index::drop()
                    .name("idx_book_metadata_number")
                    .table(BookMetadata::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_book_metadata_year")
                    .table(BookMetadata::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_book_metadata_title_sort")
                    .table(BookMetadata::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_series_metadata_year")
                    .table(SeriesMetadata::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_series_metadata_title_sort")
                    .table(SeriesMetadata::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_books_updated_at")
                    .table(Books::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_books_created_at")
                    .table(Books::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_series_updated_at")
                    .table(Series::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_series_created_at")
                    .table(Series::Table)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
enum Series {
    Table,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum Books {
    Table,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum SeriesMetadata {
    Table,
    TitleSort,
    Year,
}

#[derive(DeriveIden)]
enum BookMetadata {
    Table,
    TitleSort,
    Year,
    Number,
}
