//! `SeaORM` Entity for user_sharing_tags table
//!
//! User grants for sharing tags. Controls which users can see content
//! with specific sharing tags.
//!
//! Access modes:
//! - `allow`: User can see content with this tag
//! - `deny`: User cannot see content with this tag (overrides allow)

use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "user_sharing_tags")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub user_id: Uuid,
    pub sharing_tag_id: Uuid,
    pub access_mode: String,
    pub created_at: DateTime<Utc>,
}

impl Model {
    /// Get the access mode as an enum
    pub fn get_access_mode(&self) -> AccessMode {
        self.access_mode.parse().unwrap_or(AccessMode::Allow)
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
        belongs_to = "super::sharing_tags::Entity",
        from = "Column::SharingTagId",
        to = "super::sharing_tags::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    SharingTag,
}

impl Related<super::users::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::User.def()
    }
}

impl Related<super::sharing_tags::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::SharingTag.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

/// Access mode for sharing tag grants
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum AccessMode {
    /// User can see content with this tag
    #[default]
    Allow,
    /// User cannot see content with this tag (overrides allow)
    Deny,
}

impl AccessMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            AccessMode::Allow => "allow",
            AccessMode::Deny => "deny",
        }
    }
}

impl FromStr for AccessMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "allow" => Ok(AccessMode::Allow),
            "deny" => Ok(AccessMode::Deny),
            _ => Err(format!("Unknown access mode: {}", s)),
        }
    }
}

impl std::fmt::Display for AccessMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}
