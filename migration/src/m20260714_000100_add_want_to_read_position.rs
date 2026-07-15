//! Add `position` column to `want_to_read`.
//!
//! Manual queue order for the per-user want-to-read queue. Honored by the
//! `custom` sort; new entries append at max+1 so they land at the end of a
//! customized order. Rows predating this migration all default to 0 and tie-
//! break on `added_at` until the user reorders.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(WantToRead::Table)
                    .add_column(
                        ColumnDef::new(WantToRead::Position)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(WantToRead::Table)
                    .drop_column(WantToRead::Position)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
enum WantToRead {
    Table,
    Position,
}
