//! Add title preprocessing and auto-match configuration columns to libraries table
//!
//! This migration adds:
//! - `title_preprocessing_rules`: JSON array of regex rules to clean series titles during scan
//! - `auto_match_conditions`: JSON object defining when auto-matching should run
//!
//! Example title_preprocessing_rules:
//! ```json
//! [
//!   {"pattern": "\\s*\\(Digital\\)$", "replacement": "", "description": "Remove (Digital) suffix"}
//! ]
//! ```
//!
//! Example auto_match_conditions:
//! ```json
//! {
//!   "mode": "all",
//!   "rules": [
//!     {"field": "book_count", "operator": "gte", "value": 1}
//!   ]
//! }
//! ```

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Add title_preprocessing_rules column
        manager
            .alter_table(
                Table::alter()
                    .table(Libraries::Table)
                    .add_column(ColumnDef::new(Libraries::TitlePreprocessingRules).text())
                    .to_owned(),
            )
            .await?;

        // Add auto_match_conditions column
        manager
            .alter_table(
                Table::alter()
                    .table(Libraries::Table)
                    .add_column(ColumnDef::new(Libraries::AutoMatchConditions).text())
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Drop auto_match_conditions column
        manager
            .alter_table(
                Table::alter()
                    .table(Libraries::Table)
                    .drop_column(Libraries::AutoMatchConditions)
                    .to_owned(),
            )
            .await?;

        // Drop title_preprocessing_rules column
        manager
            .alter_table(
                Table::alter()
                    .table(Libraries::Table)
                    .drop_column(Libraries::TitlePreprocessingRules)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
enum Libraries {
    Table,
    TitlePreprocessingRules,
    AutoMatchConditions,
}
