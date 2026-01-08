use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "settings_history")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,

    pub setting_id: Uuid,
    pub key: String,
    pub old_value: Option<String>,
    pub new_value: String,
    pub changed_by: Uuid,
    pub changed_at: DateTime<Utc>,
    pub change_reason: Option<String>,
    pub ip_address: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::settings::Entity",
        from = "Column::SettingId",
        to = "super::settings::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    Settings,
    #[sea_orm(
        belongs_to = "super::users::Entity",
        from = "Column::ChangedBy",
        to = "super::users::Column::Id",
        on_update = "NoAction",
        on_delete = "NoAction"
    )]
    Users,
}

impl Related<super::settings::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Settings.def()
    }
}

impl Related<super::users::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Users.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
