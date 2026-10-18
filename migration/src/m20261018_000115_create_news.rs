//! News shown to admins, imported from the Codex release feeds.
//!
//! `news_entries` holds one row per feed entry, keyed by the feed's own id,
//! which is stable forever and doubles as the dedupe key. Entries are kept
//! after they fall out of the feed's size cap so the history stays readable.
//!
//! `news_feeds` is fetch bookkeeping, one row per feed: the ETag for a
//! conditional request and when the last successful fetch happened. It lives
//! here rather than in `settings` because that table is what the admin
//! settings page lists, and none of this is a setting.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(NewsEntries::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(NewsEntries::Id)
                            .string()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(NewsEntries::Project).string().not_null())
                    .col(ColumnDef::new(NewsEntries::Kind).string().not_null())
                    .col(ColumnDef::new(NewsEntries::Title).string().not_null())
                    .col(ColumnDef::new(NewsEntries::Version).string().null())
                    .col(ColumnDef::new(NewsEntries::Summary).text().not_null())
                    .col(ColumnDef::new(NewsEntries::Body).text().not_null())
                    .col(ColumnDef::new(NewsEntries::Url).string().null())
                    .col(
                        ColumnDef::new(NewsEntries::PublishedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    // When this instance first stored the entry. Never updated:
                    // the earliest value is what tells a fresh install apart
                    // from one that has been reading the feed for months.
                    .col(
                        ColumnDef::new(NewsEntries::FirstSeenAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(NewsEntries::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_news_entries_published_at")
                    .table(NewsEntries::Table)
                    .col(NewsEntries::PublishedAt)
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(NewsFeeds::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(NewsFeeds::Project)
                            .string()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(NewsFeeds::Etag).string().null())
                    .col(
                        ColumnDef::new(NewsFeeds::LastSuccessAt)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(NewsFeeds::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(NewsFeeds::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(NewsEntries::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum NewsEntries {
    Table,
    Id,
    Project,
    Kind,
    Title,
    Version,
    Summary,
    Body,
    Url,
    PublishedAt,
    FirstSeenAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum NewsFeeds {
    Table,
    Project,
    Etag,
    LastSuccessAt,
    UpdatedAt,
}
