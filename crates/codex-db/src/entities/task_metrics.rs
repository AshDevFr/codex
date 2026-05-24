use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "task_metrics")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,

    // Time bucket
    pub period_start: DateTime<Utc>,
    pub period_type: String, // 'hour' or 'day'

    // Task identification
    pub task_type: String,
    pub library_id: Option<Uuid>,

    // Counts
    pub count: i32,
    pub succeeded: i32,
    pub failed: i32,
    pub retried: i32,

    // Timing (milliseconds)
    pub total_duration_ms: i64,
    pub min_duration_ms: Option<i64>,
    pub max_duration_ms: Option<i64>,
    pub total_queue_wait_ms: i64,

    // Percentile samples (JSON array of recent durations)
    pub duration_samples: Option<Json>,

    // Task-specific counters
    pub items_processed: i64,
    pub bytes_processed: i64,

    // Errors
    pub error_count: i32,
    pub last_error: Option<String>,
    pub last_error_at: Option<DateTime<Utc>>,

    // Metadata
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::libraries::Entity",
        from = "Column::LibraryId",
        to = "super::libraries::Column::Id",
        on_update = "NoAction",
        on_delete = "SetNull"
    )]
    Libraries,
}

impl Related<super::libraries::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Libraries.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
