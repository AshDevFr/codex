//! Create book_external_ids table for tracking external provider IDs
//!
//! This table enables:
//! - Tracking which external source a book was matched from (e.g., plugin:openlibrary, epub, pdf, manual)
//! - Storing the external ID for efficient re-fetching without search
//! - Recording when metadata was last synced and a hash for change detection
//!
//! Mirrors the series_external_ids table pattern.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let is_postgres = manager.get_database_backend() == sea_orm::DatabaseBackend::Postgres;

        let mut table = Table::create();
        table.table(BookExternalIds::Table).if_not_exists();

        // ID column - different defaults for Postgres vs SQLite
        if is_postgres {
            table.col(
                ColumnDef::new(BookExternalIds::Id)
                    .uuid()
                    .not_null()
                    .primary_key()
                    .extra("DEFAULT gen_random_uuid()"),
            );
        } else {
            table.col(
                ColumnDef::new(BookExternalIds::Id)
                    .uuid()
                    .not_null()
                    .primary_key(),
            );
        }

        manager
            .create_table(
                table
                    // Foreign key to books
                    .col(ColumnDef::new(BookExternalIds::BookId).uuid().not_null())
                    // Source identifier: 'plugin:openlibrary', 'epub', 'pdf', 'manual'
                    .col(
                        ColumnDef::new(BookExternalIds::Source)
                            .string_len(100)
                            .not_null(),
                    )
                    // ID in the external system (ISBN, OLID, etc.)
                    .col(
                        ColumnDef::new(BookExternalIds::ExternalId)
                            .text()
                            .not_null(),
                    )
                    // Full URL to the source page (optional convenience)
                    .col(ColumnDef::new(BookExternalIds::ExternalUrl).text())
                    // Hash of last fetched metadata for change detection
                    .col(ColumnDef::new(BookExternalIds::MetadataHash).string_len(64))
                    // When metadata was last synced from this source
                    .col(ColumnDef::new(BookExternalIds::LastSyncedAt).timestamp_with_time_zone())
                    // Timestamps
                    .col({
                        let mut col = ColumnDef::new(BookExternalIds::CreatedAt);
                        col.timestamp_with_time_zone().not_null();
                        if is_postgres {
                            col.extra("DEFAULT NOW()");
                        } else {
                            col.extra("DEFAULT CURRENT_TIMESTAMP");
                        }
                        col
                    })
                    .col({
                        let mut col = ColumnDef::new(BookExternalIds::UpdatedAt);
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
                            .name("fk_book_external_ids_book_id")
                            .from(BookExternalIds::Table, BookExternalIds::BookId)
                            .to(Books::Table, Books::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::NoAction),
                    )
                    .to_owned(),
            )
            .await?;

        // Unique constraint: one external ID per source per book
        manager
            .create_index(
                Index::create()
                    .name("idx_book_external_ids_unique")
                    .table(BookExternalIds::Table)
                    .col(BookExternalIds::BookId)
                    .col(BookExternalIds::Source)
                    .unique()
                    .to_owned(),
            )
            .await?;

        // Index on book_id for efficient lookups by book
        manager
            .create_index(
                Index::create()
                    .name("idx_book_external_ids_book_id")
                    .table(BookExternalIds::Table)
                    .col(BookExternalIds::BookId)
                    .to_owned(),
            )
            .await?;

        // Index on source for filtering by source type
        manager
            .create_index(
                Index::create()
                    .name("idx_book_external_ids_source")
                    .table(BookExternalIds::Table)
                    .col(BookExternalIds::Source)
                    .to_owned(),
            )
            .await?;

        // Index on external_id for reverse lookups
        manager
            .create_index(
                Index::create()
                    .name("idx_book_external_ids_external_id")
                    .table(BookExternalIds::Table)
                    .col(BookExternalIds::ExternalId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(BookExternalIds::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum BookExternalIds {
    Table,
    Id,
    BookId,
    Source,
    ExternalId,
    ExternalUrl,
    MetadataHash,
    LastSyncedAt,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum Books {
    Table,
    Id,
}
