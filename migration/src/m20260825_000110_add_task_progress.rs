//! Add `progress` to `tasks`.
//!
//! The worker writes `result` when a task finishes, so anything reading a task
//! row while it runs sees nothing. `GET /libraries/{id}/scan-status` therefore
//! could report a scan's counts only after it ended, which is the opposite of
//! when a client wants them.
//!
//! A single JSON column rather than typed per-count columns: the shape differs
//! by task type, and every other long-running task has the same gap.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Tasks::Table)
                    .add_column(ColumnDef::new(Tasks::Progress).json_binary())
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Tasks::Table)
                    .drop_column(Tasks::Progress)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
enum Tasks {
    Table,
    Progress,
}
