//! `SeaORM` Entity for access_groups table
//!
//! Access groups bundle allow/deny rules on sharing tags, tags, genres and
//! libraries, and are assigned to many users. A user's personal grants in
//! `user_sharing_tags` count as one more group.

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
    /// New users join the default group when their account is created.
    /// At most one group has this set (partial unique index).
    pub is_default: bool,
    /// How allowed sharing tags combine: "any" (default) or "all"
    pub sharing_tag_match: String,
    /// How allowed tags combine: "any" (default) or "all"
    pub tag_match: String,
    /// How allowed genres combine: "any" (default) or "all"
    pub genre_match: String,
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

/// How a group's allowed values of one rule type combine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default, utoipa::ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum RuleMatch {
    /// A series matches if it carries any of the allowed values
    #[default]
    Any,
    /// A series matches only if it carries every allowed value
    All,
}

impl RuleMatch {
    pub fn as_str(&self) -> &'static str {
        match self {
            RuleMatch::Any => "any",
            RuleMatch::All => "all",
        }
    }
}

impl std::str::FromStr for RuleMatch {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "any" => Ok(RuleMatch::Any),
            "all" => Ok(RuleMatch::All),
            _ => Err(format!("Unknown rule match: {}", s)),
        }
    }
}

impl Model {
    pub fn get_sharing_tag_match(&self) -> RuleMatch {
        self.sharing_tag_match.parse().unwrap_or_default()
    }

    pub fn get_tag_match(&self) -> RuleMatch {
        self.tag_match.parse().unwrap_or_default()
    }

    pub fn get_genre_match(&self) -> RuleMatch {
        self.genre_match.parse().unwrap_or_default()
    }
}
