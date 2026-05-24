//! `SeaORM` Entity for access_groups table
//!
//! Access groups bundle a set of sharing-tag allow/deny rules that can be
//! assigned to multiple users. Per-user grants in `user_sharing_tags` act
//! as overrides on top of group rules.

use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "access_groups")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    #[sea_orm(unique)]
    pub name: String,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::user_access_groups::Entity")]
    UserAccessGroups,
    #[sea_orm(has_many = "super::access_group_sharing_tags::Entity")]
    AccessGroupSharingTags,
    #[sea_orm(has_many = "super::access_group_oidc_mappings::Entity")]
    AccessGroupOidcMappings,
}

impl Related<super::user_access_groups::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::UserAccessGroups.def()
    }
}

impl Related<super::access_group_sharing_tags::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::AccessGroupSharingTags.def()
    }
}

impl Related<super::access_group_oidc_mappings::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::AccessGroupOidcMappings.def()
    }
}

impl Related<super::users::Entity> for Entity {
    fn to() -> RelationDef {
        super::user_access_groups::Relation::User.def()
    }
    fn via() -> Option<RelationDef> {
        Some(super::user_access_groups::Relation::AccessGroup.def().rev())
    }
}

impl Related<super::sharing_tags::Entity> for Entity {
    fn to() -> RelationDef {
        super::access_group_sharing_tags::Relation::SharingTag.def()
    }
    fn via() -> Option<RelationDef> {
        Some(
            super::access_group_sharing_tags::Relation::AccessGroup
                .def()
                .rev(),
        )
    }
}

impl ActiveModelBehavior for ActiveModel {}
