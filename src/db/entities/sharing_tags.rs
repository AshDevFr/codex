//! `SeaORM` Entity for sharing_tags table
//!
//! Sharing tag taxonomy table for controlling content access.
//! Series can be tagged with sharing tags, and users can be granted
//! access to content via these tags.

use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "sharing_tags")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    #[sea_orm(unique)]
    pub name: String,
    #[sea_orm(unique)]
    pub normalized_name: String,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::series_sharing_tags::Entity")]
    SeriesSharingTags,
    #[sea_orm(has_many = "super::user_sharing_tags::Entity")]
    UserSharingTags,
}

impl Related<super::series_sharing_tags::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::SeriesSharingTags.def()
    }
}

impl Related<super::user_sharing_tags::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::UserSharingTags.def()
    }
}

impl Related<super::series::Entity> for Entity {
    fn to() -> RelationDef {
        super::series_sharing_tags::Relation::Series.def()
    }
    fn via() -> Option<RelationDef> {
        Some(super::series_sharing_tags::Relation::SharingTag.def().rev())
    }
}

impl Related<super::users::Entity> for Entity {
    fn to() -> RelationDef {
        super::user_sharing_tags::Relation::User.def()
    }
    fn via() -> Option<RelationDef> {
        Some(super::user_sharing_tags::Relation::SharingTag.def().rev())
    }
}

impl ActiveModelBehavior for ActiveModel {}
