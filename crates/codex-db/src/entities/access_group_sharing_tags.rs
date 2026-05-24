//! `SeaORM` Entity for access_group_sharing_tags table
//!
//! Per-group sharing tag grants. Mirrors the shape of `user_sharing_tags`
//! but scoped to an access group instead of a single user.

use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::user_sharing_tags::AccessMode;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "access_group_sharing_tags")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub access_group_id: Uuid,
    pub sharing_tag_id: Uuid,
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
        belongs_to = "super::sharing_tags::Entity",
        from = "Column::SharingTagId",
        to = "super::sharing_tags::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    SharingTag,
}

impl Related<super::access_groups::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::AccessGroup.def()
    }
}

impl Related<super::sharing_tags::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::SharingTag.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
