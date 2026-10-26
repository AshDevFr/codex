//! `SeaORM` Entity for access_group_library_rules table
//!
//! Per-group allow/deny rules on libraries.

use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::user_sharing_tags::AccessMode;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "access_group_library_rules")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub access_group_id: Uuid,
    pub library_id: Uuid,
    pub access_mode: String,
    pub created_at: DateTime<Utc>,
}

impl Model {
    pub fn get_access_mode(&self) -> AccessMode {
        self.access_mode.parse().unwrap_or(AccessMode::Allow)
    }
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::access_groups::Entity",
        from = "Column::AccessGroupId",
        to = "super::access_groups::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    AccessGroup,
    #[sea_orm(
        belongs_to = "super::libraries::Entity",
        from = "Column::LibraryId",
        to = "super::libraries::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    Library,
}

impl Related<super::access_groups::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::AccessGroup.def()
    }
}

impl Related<super::libraries::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Library.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
