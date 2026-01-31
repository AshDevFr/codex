use sea_orm::Statement;
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        // Update validation_rules for cron settings to include input_type
        let cron_settings = vec!["deduplication.cron_schedule", "pdf_cache.cron_schedule"];

        for key in cron_settings {
            db.execute(Statement::from_string(
                manager.get_database_backend(),
                format!(
                    r#"UPDATE settings SET validation_rules = '{{"input_type": "cron"}}' WHERE key = '{}'"#,
                    key
                ),
            ))
            .await?;
        }

        // Update validation_rules for json settings
        db.execute(Statement::from_string(
            manager.get_database_backend(),
            r#"UPDATE settings SET validation_rules = '{"input_type": "json"}' WHERE key = 'display.custom_metadata_template'"#.to_owned(),
        ))
        .await?;

        // Update validation_rules for select settings (metrics retention already has enum, add input_type)
        db.execute(Statement::from_string(
            manager.get_database_backend(),
            r#"UPDATE settings SET validation_rules = '{"input_type": "select", "options": ["disabled", "7", "30", "90", "180"]}' WHERE key = 'metrics.task_retention_days'"#.to_owned(),
        ))
        .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        // Reset validation_rules for cron settings
        let cron_settings = vec!["deduplication.cron_schedule", "pdf_cache.cron_schedule"];

        for key in cron_settings {
            db.execute(Statement::from_string(
                manager.get_database_backend(),
                format!(
                    "UPDATE settings SET validation_rules = NULL WHERE key = '{}'",
                    key
                ),
            ))
            .await?;
        }

        // Reset validation_rules for json settings
        db.execute(Statement::from_string(
            manager.get_database_backend(),
            "UPDATE settings SET validation_rules = NULL WHERE key = 'display.custom_metadata_template'"
                .to_owned(),
        ))
        .await?;

        // Reset validation_rules for select settings (restore original enum format)
        db.execute(Statement::from_string(
            manager.get_database_backend(),
            r#"UPDATE settings SET validation_rules = '{"enum": ["disabled", "7", "30", "90", "180"]}' WHERE key = 'metrics.task_retention_days'"#.to_owned(),
        ))
        .await?;

        Ok(())
    }
}
