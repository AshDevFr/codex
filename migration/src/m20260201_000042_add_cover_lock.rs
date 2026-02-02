use sea_orm_migration::prelude::*;

use crate::m20260103_000006_create_series_metadata::SeriesMetadata;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Add cover_lock column to series_metadata table
        // This prevents auto-fetch from changing the selected cover
        manager
            .alter_table(
                Table::alter()
                    .table(SeriesMetadata::Table)
                    .add_column(
                        ColumnDef::new(Alias::new("cover_lock"))
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(SeriesMetadata::Table)
                    .drop_column(Alias::new("cover_lock"))
                    .to_owned(),
            )
            .await
    }
}
