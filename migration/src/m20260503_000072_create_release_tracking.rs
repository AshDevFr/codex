//! Create release-tracking schema (Phase 1 of release-tracking implementation).
//!
//! Adds two tables that augment the existing `series` and `series_external_ids`
//! tables for tracked-series support:
//!
//! - `series_tracking` (1:1 with series, FK cascade): per-series flag + status
//!   metadata describing whether the series is being tracked for releases, and
//!   the latest known external chapter/volume so the matcher can compute
//!   "behind by N."
//! - `series_aliases`: title aliases used by sources without ID-based matching
//!   (e.g. Nyaa). Distinct from `series_alternate_titles`, which is purpose-built
//!   for labelled localized titles (Japanese/Romaji/English/Korean) - aliases
//!   are arbitrary normalized strings used solely for matching incoming release
//!   titles against tracked series.
//!
//! External IDs (MangaDex UUID, AniList, MAL, etc.) are stored in the existing
//! `series_external_ids` table and are NOT duplicated here.

use sea_orm_migration::prelude::*;

use crate::m20260103_000003_create_series::Series;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let is_postgres = manager.get_database_backend() == sea_orm::DatabaseBackend::Postgres;

        // ---------- series_tracking ----------
        let mut tracking = Table::create();
        tracking
            .table(SeriesTracking::Table)
            .if_not_exists()
            // Primary key is series_id (1:1 sidecar).
            .col(
                ColumnDef::new(SeriesTracking::SeriesId)
                    .uuid()
                    .not_null()
                    .primary_key(),
            )
            .col(
                ColumnDef::new(SeriesTracking::Tracked)
                    .boolean()
                    .not_null()
                    .default(false),
            )
            // 'ongoing' | 'complete' | 'hiatus' | 'cancelled' | 'unknown'
            .col(
                ColumnDef::new(SeriesTracking::TrackingStatus)
                    .string_len(20)
                    .not_null()
                    .default("unknown"),
            )
            .col(
                ColumnDef::new(SeriesTracking::TrackChapters)
                    .boolean()
                    .not_null()
                    .default(true),
            )
            .col(
                ColumnDef::new(SeriesTracking::TrackVolumes)
                    .boolean()
                    .not_null()
                    .default(true),
            )
            // Latest external chapter (decimal to handle 12.5 etc.) and volume.
            .col(ColumnDef::new(SeriesTracking::LatestKnownChapter).double())
            .col(ColumnDef::new(SeriesTracking::LatestKnownVolume).integer())
            // Sparse map: { "<volume>": { "first": <ch>, "last": <ch> } }
            .col(ColumnDef::new(SeriesTracking::VolumeChapterMap).json_binary())
            // Per-series overrides (null = use source/server default).
            .col(ColumnDef::new(SeriesTracking::PollIntervalOverrideS).integer())
            .col(ColumnDef::new(SeriesTracking::ConfidenceThresholdOverride).double())
            .col({
                let mut col = ColumnDef::new(SeriesTracking::CreatedAt);
                col.timestamp_with_time_zone().not_null();
                if is_postgres {
                    col.extra("DEFAULT NOW()");
                } else {
                    col.extra("DEFAULT CURRENT_TIMESTAMP");
                }
                col
            })
            .col({
                let mut col = ColumnDef::new(SeriesTracking::UpdatedAt);
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
                    .name("fk_series_tracking_series_id")
                    .from(SeriesTracking::Table, SeriesTracking::SeriesId)
                    .to(Series::Table, Series::Id)
                    .on_delete(ForeignKeyAction::Cascade)
                    .on_update(ForeignKeyAction::NoAction),
            );

        manager.create_table(tracking.to_owned()).await?;

        // Partial index for the hot path: "list all tracked series."
        // Use raw SQL because the DSL's partial-index support is uneven
        // across SQLite/Postgres in our SeaORM version.
        manager
            .get_connection()
            .execute_unprepared(
                "CREATE INDEX idx_series_tracking_tracked \
                 ON series_tracking(series_id) WHERE tracked = TRUE",
            )
            .await?;

        // ---------- series_aliases ----------
        let mut aliases = Table::create();
        aliases.table(SeriesAliases::Table).if_not_exists();

        if is_postgres {
            aliases.col(
                ColumnDef::new(SeriesAliases::Id)
                    .uuid()
                    .not_null()
                    .primary_key()
                    .extra("DEFAULT gen_random_uuid()"),
            );
        } else {
            aliases.col(
                ColumnDef::new(SeriesAliases::Id)
                    .uuid()
                    .not_null()
                    .primary_key(),
            );
        }

        aliases
            .col(ColumnDef::new(SeriesAliases::SeriesId).uuid().not_null())
            .col(
                ColumnDef::new(SeriesAliases::Alias)
                    .string_len(500)
                    .not_null(),
            )
            // Lowercased + punctuation-stripped, used for matching.
            .col(
                ColumnDef::new(SeriesAliases::Normalized)
                    .string_len(500)
                    .not_null(),
            )
            // 'metadata' | 'manual'
            .col(
                ColumnDef::new(SeriesAliases::Source)
                    .string_len(20)
                    .not_null(),
            )
            .col({
                let mut col = ColumnDef::new(SeriesAliases::CreatedAt);
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
                    .name("fk_series_aliases_series_id")
                    .from(SeriesAliases::Table, SeriesAliases::SeriesId)
                    .to(Series::Table, Series::Id)
                    .on_delete(ForeignKeyAction::Cascade)
                    .on_update(ForeignKeyAction::NoAction),
            );

        manager.create_table(aliases.to_owned()).await?;

        // Unique on (series_id, alias) - same alias can't be added twice for one series,
        // but the same alias string can exist on different series (which is fine and
        // expected for ambiguous titles).
        manager
            .create_index(
                Index::create()
                    .name("idx_series_aliases_unique")
                    .table(SeriesAliases::Table)
                    .col(SeriesAliases::SeriesId)
                    .col(SeriesAliases::Alias)
                    .unique()
                    .to_owned(),
            )
            .await?;

        // Index on normalized for matcher lookups (most-frequent access pattern).
        manager
            .create_index(
                Index::create()
                    .name("idx_series_aliases_normalized")
                    .table(SeriesAliases::Table)
                    .col(SeriesAliases::Normalized)
                    .to_owned(),
            )
            .await?;

        // FK index for joins back to series.
        manager
            .create_index(
                Index::create()
                    .name("idx_series_aliases_series_id")
                    .table(SeriesAliases::Table)
                    .col(SeriesAliases::SeriesId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(SeriesAliases::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(SeriesTracking::Table).to_owned())
            .await?;
        Ok(())
    }
}

#[derive(DeriveIden)]
pub enum SeriesTracking {
    Table,
    SeriesId,
    Tracked,
    TrackingStatus,
    TrackChapters,
    TrackVolumes,
    LatestKnownChapter,
    LatestKnownVolume,
    VolumeChapterMap,
    PollIntervalOverrideS,
    ConfidenceThresholdOverride,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
pub enum SeriesAliases {
    Table,
    Id,
    SeriesId,
    Alias,
    Normalized,
    Source,
    CreatedAt,
}
