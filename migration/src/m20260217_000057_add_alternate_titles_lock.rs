use sea_orm_migration::prelude::*;

use crate::m20260103_000006_create_series_metadata::SeriesMetadata;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Add alternate_titles_lock column to series_metadata table
        // This allows users to lock alternate titles independently of the title field
        manager
            .alter_table(
                Table::alter()
                    .table(SeriesMetadata::Table)
                    .add_column(
                        ColumnDef::new(Alias::new("alternate_titles_lock"))
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
                    .drop_column(Alias::new("alternate_titles_lock"))
                    .to_owned(),
            )
            .await
    }
}
