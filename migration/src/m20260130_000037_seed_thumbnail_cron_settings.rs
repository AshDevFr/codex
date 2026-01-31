use sea_orm::{entity::prelude::*, ActiveModelTrait, Set, Statement};
use sea_orm_migration::prelude::*;
use uuid::Uuid;

#[derive(DeriveMigrationName)]
pub struct Migration;

// Define a minimal ActiveModel for settings to avoid circular dependencies
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "settings")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub key: String,
    pub value: String,
    pub value_type: String,
    pub category: String,
    pub description: String,
    pub is_sensitive: bool,
    pub default_value: String,
    pub validation_rules: Option<String>,
    pub min_value: Option<i64>,
    pub max_value: Option<i64>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub updated_by: Option<Uuid>,
    pub version: i32,
    pub deleted_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        // Seed thumbnail cron settings
        let settings = vec![
            // Book thumbnail cron schedule
            (
                "thumbnail.book_cron_schedule",
                "",
                "String",
                "Thumbnail",
                "Cron schedule for generating missing book thumbnails (e.g., '0 0 3 * * *' for daily at 3am). Leave empty to disable.",
                false,
                "",
                None,
                None,
                Some(r#"{"input_type": "cron"}"#),
            ),
            // Series thumbnail cron schedule
            (
                "thumbnail.series_cron_schedule",
                "",
                "String",
                "Thumbnail",
                "Cron schedule for generating missing series thumbnails (e.g., '0 0 4 * * *' for daily at 4am). Leave empty to disable.",
                false,
                "",
                None,
                None,
                Some(r#"{"input_type": "cron"}"#),
            ),
        ];

        for (
            key,
            value,
            value_type,
            category,
            description,
            is_sensitive,
            default_value,
            min_val,
            max_val,
            validation_rules,
        ) in settings
        {
            // Check if this setting already exists (idempotent)
            let exists = db
                .query_one(Statement::from_string(
                    manager.get_database_backend(),
                    format!("SELECT id FROM settings WHERE key = '{}'", key),
                ))
                .await?;

            if exists.is_some() {
                continue;
            }

            let setting = ActiveModel {
                id: Set(Uuid::new_v4()),
                key: Set(key.to_string()),
                value: Set(value.to_string()),
                value_type: Set(value_type.to_string()),
                category: Set(category.to_string()),
                description: Set(description.to_string()),
                is_sensitive: Set(is_sensitive),
                default_value: Set(default_value.to_string()),
                validation_rules: Set(validation_rules.map(|s: &str| s.to_string())),
                min_value: Set(min_val),
                max_value: Set(max_val),
                updated_at: Set(chrono::Utc::now()),
                updated_by: Set(None),
                version: Set(1),
                deleted_at: Set(None),
            };

            setting.insert(db).await?;
        }

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        // Delete the seeded thumbnail cron settings
        db.execute(Statement::from_string(
            manager.get_database_backend(),
            "DELETE FROM settings WHERE key IN ('thumbnail.book_cron_schedule', 'thumbnail.series_cron_schedule')".to_owned(),
        ))
        .await?;

        Ok(())
    }
}
