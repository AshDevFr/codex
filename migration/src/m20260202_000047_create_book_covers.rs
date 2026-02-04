//! Create book_covers table for multiple cover images per book
//!
//! This table enables:
//! - Storing multiple cover images per book
//! - Tracking cover source (extracted, plugin, custom, url)
//! - Selecting a primary cover for display
//! - Storing image dimensions for display optimization
//!
//! Mirrors the series_covers table pattern.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let is_postgres = manager.get_database_backend() == sea_orm::DatabaseBackend::Postgres;

        let mut table = Table::create();
        table.table(BookCovers::Table).if_not_exists();

        // ID column - different defaults for Postgres vs SQLite
        if is_postgres {
            table.col(
                ColumnDef::new(BookCovers::Id)
                    .uuid()
                    .not_null()
                    .primary_key()
                    .extra("DEFAULT gen_random_uuid()"),
            );
        } else {
            table.col(
                ColumnDef::new(BookCovers::Id)
                    .uuid()
                    .not_null()
                    .primary_key(),
            );
        }

        manager
            .create_table(
                table
                    // Foreign key to books
                    .col(ColumnDef::new(BookCovers::BookId).uuid().not_null())
                    // Source identifier: 'extracted', 'plugin:openlibrary', 'custom', 'url'
                    .col(
                        ColumnDef::new(BookCovers::Source)
                            .string_len(100)
                            .not_null(),
                    )
                    // Local file path to cover image
                    .col(ColumnDef::new(BookCovers::Path).string_len(1000).not_null())
                    // Whether this cover is selected as primary
                    .col(
                        ColumnDef::new(BookCovers::IsSelected)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    // Image dimensions (optional, for display optimization)
                    .col(ColumnDef::new(BookCovers::Width).integer())
                    .col(ColumnDef::new(BookCovers::Height).integer())
                    // Timestamps
                    .col({
                        let mut col = ColumnDef::new(BookCovers::CreatedAt);
                        col.timestamp_with_time_zone().not_null();
                        if is_postgres {
                            col.extra("DEFAULT NOW()");
                        } else {
                            col.extra("DEFAULT CURRENT_TIMESTAMP");
                        }
                        col
                    })
                    .col({
                        let mut col = ColumnDef::new(BookCovers::UpdatedAt);
                        col.timestamp_with_time_zone().not_null();
                        if is_postgres {
                            col.extra("DEFAULT NOW()");
                        } else {
                            col.extra("DEFAULT CURRENT_TIMESTAMP");
                        }
                        col
                    })
                    // Foreign key constraint
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_book_covers_book_id")
                            .from(BookCovers::Table, BookCovers::BookId)
                            .to(Books::Table, Books::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::NoAction),
                    )
                    .to_owned(),
            )
            .await?;

        // Index on book_id for efficient lookups by book
        manager
            .create_index(
                Index::create()
                    .name("idx_book_covers_book_id")
                    .table(BookCovers::Table)
                    .col(BookCovers::BookId)
                    .to_owned(),
            )
            .await?;

        // Index on is_selected for finding primary covers
        manager
            .create_index(
                Index::create()
                    .name("idx_book_covers_is_selected")
                    .table(BookCovers::Table)
                    .col(BookCovers::IsSelected)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(BookCovers::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum BookCovers {
    Table,
    Id,
    BookId,
    Source,
    Path,
    IsSelected,
    Width,
    Height,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum Books {
    Table,
    Id,
}
