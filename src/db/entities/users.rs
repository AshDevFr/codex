use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::api::permissions::UserRole;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "users")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    #[sea_orm(unique)]
    pub username: String,
    #[sea_orm(unique)]
    pub email: String,
    pub password_hash: String,
    /// User role for RBAC (reader, maintainer, admin)
    pub role: String,
    pub is_active: bool,
    pub email_verified: bool,
    /// Custom permissions that extend the role's base permissions
    pub permissions: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub last_login_at: Option<DateTime<Utc>>,
}

impl Model {
    /// Get the user's role as a UserRole enum
    pub fn get_role(&self) -> UserRole {
        self.role.parse().unwrap_or_default()
    }
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::read_progress::Entity")]
    ReadProgress,
    #[sea_orm(has_many = "super::user_sharing_tags::Entity")]
    UserSharingTags,
}

impl Related<super::read_progress::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ReadProgress.def()
    }
}

impl Related<super::user_sharing_tags::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::UserSharingTags.def()
    }
}

impl Related<super::sharing_tags::Entity> for Entity {
    fn to() -> RelationDef {
        super::user_sharing_tags::Relation::SharingTag.def()
    }
    fn via() -> Option<RelationDef> {
        Some(super::user_sharing_tags::Relation::User.def().rev())
    }
}

impl ActiveModelBehavior for ActiveModel {}
