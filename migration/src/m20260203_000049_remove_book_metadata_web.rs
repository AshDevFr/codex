//! Remove web and web_lock columns from book_metadata table
//!
//! The `web` field from ComicInfo.xml is now stored in `book_external_links`
//! with source_name = "comicinfo". This migration:
//! 1. Migrates existing web values to book_external_links
//! 2. Drops the web and web_lock columns

use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::Statement;

use crate::m20260103_000014_create_book_metadata::BookMetadata;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        let is_postgres = manager.get_database_backend() == sea_orm::DatabaseBackend::Postgres;

        // Step 1: Migrate existing web values to book_external_links
        let uuid_expr = if is_postgres {
            "gen_random_uuid()"
        } else {
            "lower(hex(randomblob(4))) || '-' || lower(hex(randomblob(2))) || '-4' || substr(lower(hex(randomblob(2))),2) || '-' || substr('89ab', abs(random()) % 4 + 1, 1) || substr(lower(hex(randomblob(2))),2) || '-' || lower(hex(randomblob(6)))"
        };

        let sql = format!(
            "INSERT INTO book_external_links (id, book_id, source_name, url, created_at, updated_at)
             SELECT
                 {uuid_expr},
                 bm.book_id,
                 'comicinfo',
                 bm.web,
                 CURRENT_TIMESTAMP,
                 CURRENT_TIMESTAMP
             FROM book_metadata bm
             WHERE bm.web IS NOT NULL
               AND bm.web != ''
               AND NOT EXISTS (
                   SELECT 1 FROM book_external_links bel
                   WHERE bel.book_id = bm.book_id AND bel.source_name = 'comicinfo'
               )"
        );

        db.execute(Statement::from_string(manager.get_database_backend(), sql))
            .await?;

        // Step 2: Drop web_lock column
        manager
            .alter_table(
                Table::alter()
                    .table(BookMetadata::Table)
                    .drop_column(Alias::new("web_lock"))
                    .to_owned(),
            )
            .await?;

        // Step 3: Drop web column
        manager
            .alter_table(
                Table::alter()
                    .table(BookMetadata::Table)
                    .drop_column(Alias::new("web"))
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Re-add web column
        manager
            .alter_table(
                Table::alter()
                    .table(BookMetadata::Table)
                    .add_column(ColumnDef::new(Alias::new("web")).string())
                    .to_owned(),
            )
            .await?;

        // Re-add web_lock column
        manager
            .alter_table(
                Table::alter()
                    .table(BookMetadata::Table)
                    .add_column(
                        ColumnDef::new(Alias::new("web_lock"))
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .to_owned(),
            )
            .await?;

        // Migrate data back from book_external_links to web column
        let db = manager.get_connection();
        db.execute(Statement::from_string(
            manager.get_database_backend(),
            "UPDATE book_metadata SET web = (
                 SELECT url FROM book_external_links
                 WHERE book_external_links.book_id = book_metadata.book_id
                   AND book_external_links.source_name = 'comicinfo'
                 LIMIT 1
             )"
            .to_owned(),
        ))
        .await?;

        Ok(())
    }
}
