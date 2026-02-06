//! User Plugin Data entity for per-user plugin key-value storage
//!
//! This entity provides a key-value store scoped per user-plugin instance.
//! Plugins use this to persist stateful data like taste profiles,
//! sync state, cached recommendations, etc.
//!
//! ## Storage Isolation
//!
//! Data isolation is architectural — each entry is scoped to a specific
//! `user_plugin_id`, which itself is scoped to a (plugin_id, user_id) pair.
//! Plugins can only address their own data by key; the host resolves
//! the user+plugin scope from the connection context.
//!
//! ## TTL Support
//!
//! Entries can optionally have an `expires_at` timestamp for cached data.
//! A background cleanup task removes expired entries periodically.

use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "user_plugin_data")]
pub struct Model {
    /// Unique identifier for this data entry
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,

    /// Reference to the user-plugin instance (provides user + plugin scoping)
    pub user_plugin_id: Uuid,

    /// Storage key (e.g., "taste_profile", "recommendations", "sync_state")
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
        belongs_to = "super::user_plugins::Entity",
        from = "Column::UserPluginId",
        to = "super::user_plugins::Column::Id",
        on_delete = "Cascade"
    )]
    UserPlugin,
}

impl Related<super::user_plugins::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::UserPlugin.def()
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

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    fn test_model() -> Model {
        Model {
            id: Uuid::new_v4(),
            user_plugin_id: Uuid::new_v4(),
            key: "test_key".to_string(),
            data: serde_json::json!({"value": 42}),
            expires_at: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn test_is_expired_no_expiry() {
        let model = test_model();
        assert!(!model.is_expired());
    }

    #[test]
    fn test_is_expired_future() {
        let mut model = test_model();
        model.expires_at = Some(Utc::now() + Duration::hours(1));
        assert!(!model.is_expired());
    }

    #[test]
    fn test_is_expired_past() {
        let mut model = test_model();
        model.expires_at = Some(Utc::now() - Duration::hours(1));
        assert!(model.is_expired());
    }
}
