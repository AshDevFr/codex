//! Server-side state for a plugin OAuth connect flow that is in flight.
//!
//! Written when the authorization URL is built and deleted when the callback
//! consumes it. Lives in the database rather than process memory because the
//! two legs of the flow are separate HTTP requests that a load balancer is
//! free to route to different `codex serve` processes.

use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "user_plugin_oauth_states")]
pub struct Model {
    /// The CSRF state token, and the primary key.
    #[sea_orm(primary_key, auto_increment = false)]
    pub state: String,
    pub plugin_id: Uuid,
    pub user_id: Uuid,
    /// `None` when the plugin's OAuth config disables PKCE.
    pub pkce_verifier: Option<String>,
    pub pkce_challenge: Option<String>,
    /// Replayed verbatim in the token exchange, so it is stored rather than
    /// rebuilt from config that may have changed in the meantime.
    pub redirect_uri: String,
    pub created_at: DateTime<Utc>,
    /// When this flow stops being redeemable.
    pub expires_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::plugins::Entity",
        from = "Column::PluginId",
        to = "super::plugins::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    Plugin,
    #[sea_orm(
        belongs_to = "super::users::Entity",
        from = "Column::UserId",
        to = "super::users::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    User,
}

impl Related<super::plugins::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Plugin.def()
    }
}

impl Related<super::users::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::User.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
