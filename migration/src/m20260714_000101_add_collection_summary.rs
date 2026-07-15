//! Add `summary` column to `collections`.
//!
//! Optional free-text description, mirroring the one read lists have had
//! since creation. Collections historically lacked it only because the model
//! copied Komga's API shape; the Komga compatibility DTO keeps not exposing
//! it.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Collections::Table)
                    .add_column(ColumnDef::new(Collections::Summary).text())
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Collections::Table)
                    .drop_column(Collections::Summary)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
enum Collections {
    Table,
    Summary,
}
