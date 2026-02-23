use sea_orm_migration::prelude::*;

use crate::m20260103_000006_create_series_metadata::SeriesMetadata;
use crate::m20260103_000014_create_book_metadata::BookMetadata;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Add search_title column to series_metadata
        manager
            .alter_table(
                Table::alter()
                    .table(SeriesMetadata::Table)
                    .add_column(
                        ColumnDef::new(Alias::new("search_title"))
                            .string_len(500)
                            .not_null()
                            .default(""),
                    )
                    .to_owned(),
            )
            .await?;

        // Add search_title column to book_metadata
        manager
            .alter_table(
                Table::alter()
                    .table(BookMetadata::Table)
                    .add_column(
                        ColumnDef::new(Alias::new("search_title"))
                            .string()
                            .not_null()
                            .default(""),
                    )
                    .to_owned(),
            )
            .await?;

        // Populate search_title with LOWER(title) for existing rows
        // This is a best-effort population: SQLite LOWER() only handles ASCII,
        // but the application will compute proper Unicode-normalized values
        // on the next metadata write (scan, plugin update, or manual edit).
        let db = manager.get_connection();

        db.execute_unprepared("UPDATE series_metadata SET search_title = LOWER(title)")
            .await?;

        db.execute_unprepared("UPDATE book_metadata SET search_title = LOWER(COALESCE(title, ''))")
            .await?;

        // Add index on search_title for both tables to speed up LIKE queries
        manager
            .create_index(
                Index::create()
                    .name("idx_series_metadata_search_title")
                    .table(SeriesMetadata::Table)
                    .col(Alias::new("search_title"))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_book_metadata_search_title")
                    .table(BookMetadata::Table)
                    .col(Alias::new("search_title"))
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Drop indexes first
        manager
            .drop_index(
                Index::drop()
                    .name("idx_series_metadata_search_title")
                    .table(SeriesMetadata::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_book_metadata_search_title")
                    .table(BookMetadata::Table)
                    .to_owned(),
            )
            .await?;

        // Drop columns
        manager
            .alter_table(
                Table::alter()
                    .table(SeriesMetadata::Table)
                    .drop_column(Alias::new("search_title"))
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(BookMetadata::Table)
                    .drop_column(Alias::new("search_title"))
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}
