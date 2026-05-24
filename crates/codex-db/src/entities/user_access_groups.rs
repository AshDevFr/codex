//! `SeaORM` Entity for user_access_groups table
//!
//! Junction table linking users to access groups (M:N). The `source` field
//! tracks provenance: `manual` for admin-assigned, `oidc` for IdP-synced
//! memberships (reconciled on login).

use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "user_access_groups")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub user_id: Uuid,
    pub access_group_id: Uuid,
    pub source: String,
    pub created_at: DateTime<Utc>,
}

impl Model {
    pub fn get_source(&self) -> MembershipSource {
        self.source.parse().unwrap_or(MembershipSource::Manual)
    }
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
    User,
    #[sea_orm(
        belongs_to = "super::access_groups::Entity",
        from = "Column::AccessGroupId",
        to = "super::access_groups::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    AccessGroup,
}

impl Related<super::users::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::User.def()
    }
}

impl Related<super::access_groups::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::AccessGroup.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

/// Provenance of a group membership
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum MembershipSource {
    /// Assigned by an admin through the UI/API
    #[default]
    Manual,
    /// Auto-assigned from an OIDC IdP group claim
    Oidc,
}

impl MembershipSource {
    pub fn as_str(&self) -> &'static str {
        match self {
            MembershipSource::Manual => "manual",
            MembershipSource::Oidc => "oidc",
        }
    }
}

impl FromStr for MembershipSource {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "manual" => Ok(MembershipSource::Manual),
            "oidc" => Ok(MembershipSource::Oidc),
            _ => Err(format!("Unknown membership source: {}", s)),
        }
    }
}

impl std::fmt::Display for MembershipSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}
