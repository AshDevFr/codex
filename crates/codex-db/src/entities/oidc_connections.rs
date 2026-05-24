//! OIDC Connection entity for storing external identity provider connections
//!
//! This entity links Codex users to their external identity provider (IdP) accounts.
//! Supports multiple OIDC providers per user (e.g., a user can be linked to both
//! Authentik and Keycloak simultaneously).

use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "oidc_connections")]
pub struct Model {
    /// Unique identifier for this connection
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,

    /// The Codex user this connection belongs to
    pub user_id: Uuid,

    /// Name of the OIDC provider (e.g., "authentik", "keycloak")
    /// Must match the key used in the OIDC configuration
    pub provider_name: String,

    /// The subject (sub) claim from the IdP - unique identifier for the user at the IdP
    pub subject: String,

    /// Email address from the IdP (for audit/display purposes)
    pub email: Option<String>,

    /// Display name from the IdP (preferred_username or name claim)
    pub display_name: Option<String>,

    /// Groups/roles from the IdP (stored as JSON array for flexibility)
    pub groups: Option<serde_json::Value>,

    /// Hash of the access token (for token revocation checks)
    pub access_token_hash: Option<String>,

    /// Encrypted refresh token (for optional background refresh)
    #[serde(skip_serializing)]
    pub refresh_token_encrypted: Option<Vec<u8>>,

    /// When the access token expires
    pub token_expires_at: Option<DateTime<Utc>>,

    /// When this connection was first created
    pub created_at: DateTime<Utc>,

    /// When this connection was last updated
    pub updated_at: DateTime<Utc>,

    /// When this connection was last used for authentication
    pub last_used_at: Option<DateTime<Utc>>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::users::Entity",
        from = "Column::UserId",
        to = "super::users::Column::Id",
        on_delete = "Cascade"
    )]
    User,
}

impl Related<super::users::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::User.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
