//! Create release-tracking ledger schema (Phase 2 of release-tracking implementation).
//!
//! Adds two tables that store release announcements emitted by source plugins:
//!
//! - `release_sources`: one row per logical source a plugin exposes. A single
//!   plugin can expose many sources (e.g., one per Nyaa uploader subscription).
//!   Tracks per-source poll cadence, last-poll status, and an opaque
//!   `etag`/cursor used for conditional fetches.
//! - `release_ledger`: the dedup-keyed announcement ledger. Sources write rows
//!   here; the inbox UI reads from it. Dedup keys: `(source_id,
//!   external_release_id)` (unique per source) and `info_hash` (unique
//!   globally where present, since two BitTorrent sources publishing the same
//!   torrent would share an info_hash).

use sea_orm_migration::prelude::*;

use crate::m20260103_000003_create_series::Series;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let is_postgres = manager.get_database_backend() == sea_orm::DatabaseBackend::Postgres;

        // ---------- release_sources ----------
        let mut sources = Table::create();
        sources.table(ReleaseSources::Table).if_not_exists();

        if is_postgres {
            sources.col(
                ColumnDef::new(ReleaseSources::Id)
                    .uuid()
                    .not_null()
                    .primary_key()
                    .extra("DEFAULT gen_random_uuid()"),
            );
        } else {
            sources.col(
                ColumnDef::new(ReleaseSources::Id)
                    .uuid()
                    .not_null()
                    .primary_key(),
            );
        }

        sources
            // Owning plugin. The string `"core"` is reserved for in-core
            // synthetic sources (e.g., metadata-piggyback in Phase 5) so we
            // don't need a foreign key to plugins.id (which would force every
            // synthetic source to also have a plugins row).
            .col(
                ColumnDef::new(ReleaseSources::PluginId)
                    .string_len(100)
                    .not_null(),
            )
            // Plugin-defined unique key, e.g. "nyaa:user:tsuna69".
            .col(
                ColumnDef::new(ReleaseSources::SourceKey)
                    .string_len(255)
                    .not_null(),
            )
            .col(
                ColumnDef::new(ReleaseSources::DisplayName)
                    .string_len(255)
                    .not_null(),
            )
            // 'rss-uploader' | 'rss-series' | 'api-feed' | 'metadata-feed' | 'metadata-piggyback'
            .col(
                ColumnDef::new(ReleaseSources::Kind)
                    .string_len(40)
                    .not_null(),
            )
            .col(
                ColumnDef::new(ReleaseSources::Enabled)
                    .boolean()
                    .not_null()
                    .default(true),
            )
            // Per-source cron schedule override. NULL means "inherit the
            // server-wide `release_tracking.default_cron_schedule` setting".
            // Stored as a 5-field POSIX cron expression (the host normalizes
            // to 6-field at scheduler-load time).
            .col(ColumnDef::new(ReleaseSources::CronSchedule).string_len(120))
            .col(ColumnDef::new(ReleaseSources::LastPolledAt).timestamp_with_time_zone())
            .col(ColumnDef::new(ReleaseSources::LastError).text())
            .col(ColumnDef::new(ReleaseSources::LastErrorAt).timestamp_with_time_zone())
            .col(ColumnDef::new(ReleaseSources::Etag).string_len(255))
            .col(ColumnDef::new(ReleaseSources::Config).json_binary())
            .col({
                let mut col = ColumnDef::new(ReleaseSources::CreatedAt);
                col.timestamp_with_time_zone().not_null();
                if is_postgres {
                    col.extra("DEFAULT NOW()");
                } else {
                    col.extra("DEFAULT CURRENT_TIMESTAMP");
                }
                col
            })
            .col({
                let mut col = ColumnDef::new(ReleaseSources::UpdatedAt);
                col.timestamp_with_time_zone().not_null();
                if is_postgres {
                    col.extra("DEFAULT NOW()");
                } else {
                    col.extra("DEFAULT CURRENT_TIMESTAMP");
                }
                col
            });

        manager.create_table(sources.to_owned()).await?;

        // (plugin_id, source_key) is the natural composite identity.
        manager
            .create_index(
                Index::create()
                    .name("idx_release_sources_plugin_key")
                    .table(ReleaseSources::Table)
                    .col(ReleaseSources::PluginId)
                    .col(ReleaseSources::SourceKey)
                    .unique()
                    .to_owned(),
            )
            .await?;

        // Hot path for the scheduler: enumerate enabled sources.
        manager
            .get_connection()
            .execute_unprepared(
                "CREATE INDEX idx_release_sources_enabled \
                 ON release_sources(id) WHERE enabled = TRUE",
            )
            .await?;

        // ---------- release_ledger ----------
        let mut ledger = Table::create();
        ledger.table(ReleaseLedger::Table).if_not_exists();

        if is_postgres {
            ledger.col(
                ColumnDef::new(ReleaseLedger::Id)
                    .uuid()
                    .not_null()
                    .primary_key()
                    .extra("DEFAULT gen_random_uuid()"),
            );
        } else {
            ledger.col(
                ColumnDef::new(ReleaseLedger::Id)
                    .uuid()
                    .not_null()
                    .primary_key(),
            );
        }

        ledger
            .col(ColumnDef::new(ReleaseLedger::SeriesId).uuid().not_null())
            .col(ColumnDef::new(ReleaseLedger::SourceId).uuid().not_null())
            // Plugin-stable identity for the release. Required - a source that
            // can't produce one is unusable for dedup.
            .col(
                ColumnDef::new(ReleaseLedger::ExternalReleaseId)
                    .string_len(500)
                    .not_null(),
            )
            // Optional. Torrent sources will have one; HTTP sources won't.
            .col(ColumnDef::new(ReleaseLedger::InfoHash).string_len(64))
            // Decimal for chapters (handles 12.5, 110.1, etc.). Volume is integer.
            .col(ColumnDef::new(ReleaseLedger::Chapter).double())
            .col(ColumnDef::new(ReleaseLedger::Volume).integer())
            .col(ColumnDef::new(ReleaseLedger::Language).string_len(20))
            // { "jxl": true, "container": "cbz", ... }
            .col(ColumnDef::new(ReleaseLedger::FormatHints).json_binary())
            .col(ColumnDef::new(ReleaseLedger::GroupOrUploader).string_len(255))
            // Where the user goes to acquire the release.
            .col(
                ColumnDef::new(ReleaseLedger::PayloadUrl)
                    .string_len(2048)
                    .not_null(),
            )
            .col(
                ColumnDef::new(ReleaseLedger::Confidence)
                    .double()
                    .not_null(),
            )
            // 'announced' | 'dismissed' | 'marked_acquired' | 'hidden'
            .col(
                ColumnDef::new(ReleaseLedger::State)
                    .string_len(20)
                    .not_null()
                    .default("announced"),
            )
            .col(ColumnDef::new(ReleaseLedger::Metadata).json_binary())
            .col(
                ColumnDef::new(ReleaseLedger::ObservedAt)
                    .timestamp_with_time_zone()
                    .not_null(),
            )
            .col({
                let mut col = ColumnDef::new(ReleaseLedger::CreatedAt);
                col.timestamp_with_time_zone().not_null();
                if is_postgres {
                    col.extra("DEFAULT NOW()");
                } else {
                    col.extra("DEFAULT CURRENT_TIMESTAMP");
                }
                col
            })
            .foreign_key(
                ForeignKey::create()
                    .name("fk_release_ledger_series_id")
                    .from(ReleaseLedger::Table, ReleaseLedger::SeriesId)
                    .to(Series::Table, Series::Id)
                    .on_delete(ForeignKeyAction::Cascade)
                    .on_update(ForeignKeyAction::NoAction),
            )
            .foreign_key(
                ForeignKey::create()
                    .name("fk_release_ledger_source_id")
                    .from(ReleaseLedger::Table, ReleaseLedger::SourceId)
                    .to(ReleaseSources::Table, ReleaseSources::Id)
                    .on_delete(ForeignKeyAction::Cascade)
                    .on_update(ForeignKeyAction::NoAction),
            );

        manager.create_table(ledger.to_owned()).await?;

        // Primary dedup key.
        manager
            .create_index(
                Index::create()
                    .name("idx_release_ledger_source_external")
                    .table(ReleaseLedger::Table)
                    .col(ReleaseLedger::SourceId)
                    .col(ReleaseLedger::ExternalReleaseId)
                    .unique()
                    .to_owned(),
            )
            .await?;

        // Cross-source dedup on info_hash where present. Partial unique index;
        // both Postgres and SQLite accept this `WHERE info_hash IS NOT NULL`
        // form via raw SQL.
        manager
            .get_connection()
            .execute_unprepared(
                "CREATE UNIQUE INDEX idx_release_ledger_info_hash \
                 ON release_ledger(info_hash) WHERE info_hash IS NOT NULL",
            )
            .await?;

        // Per-series ledger: ordered scan by observed_at desc.
        manager
            .create_index(
                Index::create()
                    .name("idx_release_ledger_series_observed")
                    .table(ReleaseLedger::Table)
                    .col(ReleaseLedger::SeriesId)
                    .col((ReleaseLedger::ObservedAt, IndexOrder::Desc))
                    .to_owned(),
            )
            .await?;

        // Inbox query: undismissed, ordered by observed_at desc. We use a
        // partial index on the announced state since that's the dominant
        // filter for the inbox view.
        manager
            .get_connection()
            .execute_unprepared(
                "CREATE INDEX idx_release_ledger_state_observed \
                 ON release_ledger(state, observed_at DESC) WHERE state = 'announced'",
            )
            .await?;

        // FK index for joins back to source.
        manager
            .create_index(
                Index::create()
                    .name("idx_release_ledger_source_id")
                    .table(ReleaseLedger::Table)
                    .col(ReleaseLedger::SourceId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ReleaseLedger::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(ReleaseSources::Table).to_owned())
            .await?;
        Ok(())
    }
}

#[derive(DeriveIden)]
pub enum ReleaseSources {
    Table,
    Id,
    PluginId,
    SourceKey,
    DisplayName,
    Kind,
    Enabled,
    CronSchedule,
    LastPolledAt,
    LastError,
    LastErrorAt,
    Etag,
    Config,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
pub enum ReleaseLedger {
    Table,
    Id,
    SeriesId,
    SourceId,
    ExternalReleaseId,
    InfoHash,
    Chapter,
    Volume,
    Language,
    FormatHints,
    GroupOrUploader,
    PayloadUrl,
    Confidence,
    State,
    Metadata,
    ObservedAt,
    CreatedAt,
}
