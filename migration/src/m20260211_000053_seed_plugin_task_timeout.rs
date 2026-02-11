use sea_orm::{ActiveModelTrait, Set, Statement, entity::prelude::*};
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

        // Check if the plugin.task_request_timeout_seconds setting already exists (idempotent)
        let exists_result = db
            .query_one(Statement::from_string(
                manager.get_database_backend(),
                "SELECT COUNT(*) as count FROM settings WHERE key = 'plugin.task_request_timeout_seconds'"
                    .to_owned(),
            ))
            .await?;

        let exists = if let Some(row) = exists_result {
            let count: i64 = row.try_get("", "count")?;
            count > 0
        } else {
            false
        };

        if !exists {
            let setting = ActiveModel {
                id: Set(Uuid::new_v4()),
                key: Set("plugin.task_request_timeout_seconds".to_string()),
                value: Set("300".to_string()),
                value_type: Set("Integer".to_string()),
                category: Set("Plugins".to_string()),
                description: Set(
                    "RPC timeout in seconds for plugin background tasks (sync, recommendations). \
                     This controls how long the task worker waits for a plugin response before \
                     timing out. Interactive HTTP requests always use the default 30-second timeout. \
                     Increase this if background tasks time out when calling slow external APIs."
                        .to_string(),
                ),
                is_sensitive: Set(false),
                default_value: Set("300".to_string()),
                validation_rules: Set(None),
                min_value: Set(Some(30)),
                max_value: Set(Some(1800)),
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

        db.execute(Statement::from_string(
            manager.get_database_backend(),
            "DELETE FROM settings WHERE key = 'plugin.task_request_timeout_seconds'".to_owned(),
        ))
        .await?;

        Ok(())
    }
}
