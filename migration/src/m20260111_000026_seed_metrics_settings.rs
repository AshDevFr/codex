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

        // Check if the metrics retention setting already exists (idempotent)
        let exists_result = db
            .query_one(Statement::from_string(
                manager.get_database_backend(),
                "SELECT COUNT(*) as count FROM settings WHERE key = 'metrics.task_retention_days'"
                    .to_owned(),
            ))
            .await?;

        if let Some(row) = exists_result {
            let count: i64 = row.try_get("", "count")?;
            if count > 0 {
                // Setting already exists, skip
                return Ok(());
            }
        }

        // Seed metrics retention setting
        // Values: "disabled", "7", "30", "90", "180"
        let setting = ActiveModel {
            id: Set(Uuid::new_v4()),
            key: Set("metrics.task_retention_days".to_string()),
            value: Set("30".to_string()),
            value_type: Set("String".to_string()),
            category: Set("Metrics".to_string()),
            description: Set(
                "Task metrics retention period. Values: disabled (no persistence), 7, 30, 90, or 180 days. Metrics older than this will be automatically cleaned up."
                    .to_string(),
            ),
            is_sensitive: Set(false),
            default_value: Set("30".to_string()),
            validation_rules: Set(Some(
                r#"{"enum": ["disabled", "7", "30", "90", "180"]}"#.to_string(),
            )),
            min_value: Set(None),
            max_value: Set(None),
            updated_at: Set(chrono::Utc::now()),
            updated_by: Set(None),
            version: Set(1),
            deleted_at: Set(None),
        };

        setting.insert(db).await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        db.execute(Statement::from_string(
            manager.get_database_backend(),
            "DELETE FROM settings WHERE key = 'metrics.task_retention_days'".to_owned(),
        ))
        .await?;

        Ok(())
    }
}
