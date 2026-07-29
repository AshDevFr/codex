//! Add `condition` column to `collections`.
//!
//! Holds a serialized `SeriesCondition` when the collection's membership is
//! defined by a rule instead of a hand-picked list. `NULL` means the collection
//! is manual, and is the sole discriminator between the two kinds: a separate
//! `is_auto` boolean would be a second source of truth for the same fact.
//!
//! Purely additive and nullable, so every existing collection keeps behaving
//! exactly as before and the feature stays inert until someone writes a rule.
//! `json_binary` matches `filter_presets.condition`, which stores the same
//! condition grammar.

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
                    .add_column(ColumnDef::new(Collections::Condition).json_binary())
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
                    .drop_column(Collections::Condition)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
enum Collections {
    Table,
    Condition,
}
