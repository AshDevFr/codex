//! Add `media_url` + `media_url_kind` columns to `release_ledger`.
//!
//! Some sources (Nyaa especially) carry two URLs per release: a
//! human-readable landing page and the actual fetch URL (a `.torrent`,
//! magnet link, or direct download). The existing `payload_url` keeps the
//! landing page; this migration adds the second URL and a small enum
//! string describing what it points at so the inbox UI can render a
//! kind-specific icon (download arrow / magnet / etc.) next to the
//! standard external-link icon.
//!
//! Both columns are nullable — sources that only surface a single URL
//! (MangaUpdates) leave them empty.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Alias::new("release_ledger"))
                    .add_column(ColumnDef::new(Alias::new("media_url")).string_len(2048))
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(Alias::new("release_ledger"))
                    .add_column(ColumnDef::new(Alias::new("media_url_kind")).string_len(32))
                    .to_owned(),
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Alias::new("release_ledger"))
                    .drop_column(Alias::new("media_url_kind"))
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(Alias::new("release_ledger"))
                    .drop_column(Alias::new("media_url"))
                    .to_owned(),
            )
            .await?;
        Ok(())
    }
}
