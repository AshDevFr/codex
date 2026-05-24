//! Plugin failure entity for time-windowed failure tracking
//!
//! This entity stores individual failure events for plugins, enabling:
//! - Time-windowed failure counting (e.g., 3 failures in 1 hour triggers auto-disable)
//! - Error message storage for debugging
//! - Automatic expiration and cleanup of old failures

use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "plugin_failures")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,

    /// Reference to the plugin that failed
    pub plugin_id: Uuid,

    /// Human-readable error message
    pub error_message: String,

    /// Error code for categorization (e.g., "TIMEOUT", "PROCESS_CRASHED", "RPC_ERROR")
    pub error_code: Option<String>,

    /// Which method failed (e.g., "initialize", "metadata/search", "shutdown")
    pub method: Option<String>,

    /// JSON-RPC request ID if applicable
    pub request_id: Option<String>,

    /// Additional context (parameters, stack trace, etc.)
    pub context: Option<serde_json::Value>,

    /// Sanitized summary of request parameters (sensitive fields redacted)
    pub request_summary: Option<String>,

    /// When the failure occurred
    pub occurred_at: DateTime<Utc>,

    /// When this failure record expires and should be deleted
    /// Default: 30 days from occurred_at
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
}

impl Related<super::plugins::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Plugin.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

// =============================================================================
// Error Code Constants
// =============================================================================

/// Common error codes for plugin failures
///
/// These constants are provided for consistency when recording plugin failures.
/// Not all are used internally - they're available for plugin implementations.
#[allow(dead_code)]
pub mod error_codes {
    /// Plugin process did not respond within timeout
    pub const TIMEOUT: &str = "TIMEOUT";

    /// Plugin process crashed or was terminated
    pub const PROCESS_CRASHED: &str = "PROCESS_CRASHED";

    /// JSON-RPC communication error
    pub const RPC_ERROR: &str = "RPC_ERROR";

    /// Plugin returned an error response
    pub const PLUGIN_ERROR: &str = "PLUGIN_ERROR";

    /// Failed to spawn the plugin process
    pub const SPAWN_ERROR: &str = "SPAWN_ERROR";

    /// Plugin initialization failed
    pub const INIT_ERROR: &str = "INIT_ERROR";

    /// Plugin returned invalid response format
    pub const INVALID_RESPONSE: &str = "INVALID_RESPONSE";

    /// Network error when plugin made external requests
    pub const NETWORK_ERROR: &str = "NETWORK_ERROR";

    /// Plugin encountered an internal error
    pub const INTERNAL_ERROR: &str = "INTERNAL_ERROR";
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_codes() {
        assert_eq!(error_codes::TIMEOUT, "TIMEOUT");
        assert_eq!(error_codes::PROCESS_CRASHED, "PROCESS_CRASHED");
        assert_eq!(error_codes::RPC_ERROR, "RPC_ERROR");
    }
}
