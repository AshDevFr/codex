//! User preferences entity for per-user key-value settings storage

use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "user_preferences")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub user_id: Uuid,
    pub key: String,
    pub value: String,
    pub value_type: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::users::Entity",
        from = "Column::UserId",
        to = "super::users::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    Users,
}

impl Related<super::users::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Users.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

/// Valid preference value types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ValueType {
    String,
    Integer,
    Float,
    Boolean,
    Json,
}

impl ValueType {
    pub fn as_str(&self) -> &'static str {
        match self {
            ValueType::String => "string",
            ValueType::Integer => "integer",
            ValueType::Float => "float",
            ValueType::Boolean => "boolean",
            ValueType::Json => "json",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "string" => Some(ValueType::String),
            "integer" => Some(ValueType::Integer),
            "float" => Some(ValueType::Float),
            "boolean" => Some(ValueType::Boolean),
            "json" => Some(ValueType::Json),
            _ => None,
        }
    }
}

impl std::fmt::Display for ValueType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}
