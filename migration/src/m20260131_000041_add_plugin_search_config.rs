//! Add search configuration columns to plugins table
//!
//! This migration adds:
//! - `search_query_template`: Handlebars template for customizing search queries
//! - `search_preprocessing_rules`: JSON array of regex rules to clean search queries
//! - `auto_match_conditions`: JSON object defining when auto-matching should run for this plugin
//! - `use_existing_external_id`: Boolean to skip search when external ID exists
//!
//! Example search_query_template:
//! ```handlebars
//! {{title}}{{#if year}} ({{year}}){{/if}}
//! ```
//!
//! Example search_preprocessing_rules:
//! ```json
//! [
//!   {"pattern": "\\s*\\(.*\\)$", "replacement": "", "description": "Remove parenthetical suffixes"}
//! ]
//! ```
//!
//! Example auto_match_conditions:
//! ```json
//! {
//!   "mode": "all",
//!   "rules": [
//!     {"field": "external_ids.plugin:mangabaka", "operator": "is_null"}
//!   ]
//! }
//! ```

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Add search_query_template column
        manager
            .alter_table(
                Table::alter()
                    .table(Plugins::Table)
                    .add_column(ColumnDef::new(Plugins::SearchQueryTemplate).text())
                    .to_owned(),
            )
            .await?;

        // Add search_preprocessing_rules column
        manager
            .alter_table(
                Table::alter()
                    .table(Plugins::Table)
                    .add_column(ColumnDef::new(Plugins::SearchPreprocessingRules).text())
                    .to_owned(),
            )
            .await?;

        // Add auto_match_conditions column
        manager
            .alter_table(
                Table::alter()
                    .table(Plugins::Table)
                    .add_column(ColumnDef::new(Plugins::AutoMatchConditions).text())
                    .to_owned(),
            )
            .await?;

        // Add use_existing_external_id column with default true
        manager
            .alter_table(
                Table::alter()
                    .table(Plugins::Table)
                    .add_column(
                        ColumnDef::new(Plugins::UseExistingExternalId)
                            .boolean()
                            .not_null()
                            .default(true),
                    )
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Drop use_existing_external_id column
        manager
            .alter_table(
                Table::alter()
                    .table(Plugins::Table)
                    .drop_column(Plugins::UseExistingExternalId)
                    .to_owned(),
            )
            .await?;

        // Drop auto_match_conditions column
        manager
            .alter_table(
                Table::alter()
                    .table(Plugins::Table)
                    .drop_column(Plugins::AutoMatchConditions)
                    .to_owned(),
            )
            .await?;

        // Drop search_preprocessing_rules column
        manager
            .alter_table(
                Table::alter()
                    .table(Plugins::Table)
                    .drop_column(Plugins::SearchPreprocessingRules)
                    .to_owned(),
            )
            .await?;

        // Drop search_query_template column
        manager
            .alter_table(
                Table::alter()
                    .table(Plugins::Table)
                    .drop_column(Plugins::SearchQueryTemplate)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
enum Plugins {
    Table,
    SearchQueryTemplate,
    SearchPreprocessingRules,
    AutoMatchConditions,
    UseExistingExternalId,
}
