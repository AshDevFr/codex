//! Plugin Data entity for system-scoped (per-plugin) key-value storage.
//!
//! Mirrors [`super::user_plugin_data`] but is keyed by `plugin_id` rather than
//! `user_plugin_id`. System plugins (e.g. release sources) run with no user
//! context, so they can't use the per-user store; this is their durable KV
//! bucket — used, for example, to persist a release feed cursor.
//!
//! ## Storage Isolation
//!
//! Each entry is scoped to a specific `plugin_id`. Plugins can only address
//! their own data by key; the host resolves the plugin scope from the
//! connection context.
//!
//! ## TTL Support
//!
//! Entries can optionally have an `expires_at` timestamp for cached data. A
//! background cleanup task removes expired entries periodically.

use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "plugin_data")]
pub struct Model {
    /// Unique identifier for this data entry
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,

    /// Reference to the system plugin (provides plugin scoping)
    pub plugin_id: Uuid,

    /// Storage key (e.g., "feed_cursor")
    pub key: String,

    /// Plugin-managed JSON data
    pub data: serde_json::Value,

    /// Optional TTL — entry is considered expired after this timestamp
    pub expires_at: Option<DateTime<Utc>>,

    /// When this entry was first created
    pub created_at: DateTime<Utc>,

    /// When this entry was last updated
    pub updated_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::plugins::Entity",
        from = "Column::PluginId",
        to = "super::plugins::Column::Id",
        on_delete = "Cascade"
    )]
    Plugin,
}

impl Related<super::plugins::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Plugin.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

// =============================================================================
// Helper Methods
// =============================================================================

impl Model {
    /// Check if this entry has expired
    pub fn is_expired(&self) -> bool {
        match self.expires_at {
            Some(expires_at) => Utc::now() >= expires_at,
            None => false, // No expiry means never expires
        }
    }
}
