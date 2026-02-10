//! Handler for CleanupPluginData task
//!
//! Periodically cleans up expired key-value data from plugin storage
//! (`user_plugin_data` table). Entries with a past `expires_at` timestamp
//! are deleted in bulk. Also cleans up expired OAuth state flows from the
//! in-memory `OAuthStateManager` to prevent memory leaks.

use anyhow::Result;
use sea_orm::DatabaseConnection;
use serde_json::json;
use std::sync::Arc;
use tracing::info;

use crate::db::entities::tasks;
use crate::db::repositories::UserPluginDataRepository;
use crate::events::EventBroadcaster;
use crate::services::user_plugin::OAuthStateManager;
use crate::tasks::handlers::TaskHandler;
use crate::tasks::types::TaskResult;

/// Handler for cleaning up expired plugin storage data and OAuth state
#[derive(Default)]
pub struct CleanupPluginDataHandler {
    oauth_state_manager: Option<Arc<OAuthStateManager>>,
}

impl CleanupPluginDataHandler {
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the OAuth state manager for cleaning up expired OAuth flows
    pub fn with_oauth_state_manager(mut self, manager: Arc<OAuthStateManager>) -> Self {
        self.oauth_state_manager = Some(manager);
        self
    }
}

impl TaskHandler for CleanupPluginDataHandler {
    fn handle<'a>(
        &'a self,
        task: &'a tasks::Model,
        db: &'a DatabaseConnection,
        _event_broadcaster: Option<&'a Arc<EventBroadcaster>>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<TaskResult>> + Send + 'a>> {
        Box::pin(async move {
            info!("Task {}: Starting plugin data cleanup", task.id);

            let deleted_count = UserPluginDataRepository::cleanup_expired(db).await?;

            // Clean up expired OAuth pending flows from in-memory state
            let (oauth_cleaned, oauth_remaining) =
                if let Some(ref manager) = self.oauth_state_manager {
                    let cleaned = manager.cleanup_expired();
                    let remaining = manager.pending_count();
                    (cleaned, remaining)
                } else {
                    (0, 0)
                };

            info!(
                "Task {}: Plugin data cleanup complete - deleted {} expired storage entries, \
                 {} expired OAuth flows ({} still pending)",
                task.id, deleted_count, oauth_cleaned, oauth_remaining
            );

            Ok(TaskResult::success_with_data(
                format!(
                    "Cleaned up {} expired plugin data entries, {} expired OAuth flows",
                    deleted_count, oauth_cleaned
                ),
                json!({
                    "deleted_count": deleted_count,
                    "oauth_flows_cleaned": oauth_cleaned,
                    "oauth_flows_remaining": oauth_remaining,
                }),
            ))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::plugin::protocol::OAuthConfig;
    use uuid::Uuid;

    #[test]
    fn test_handler_creation() {
        let _handler = CleanupPluginDataHandler::new();
    }

    #[test]
    fn test_handler_with_oauth_state_manager() {
        let manager = Arc::new(OAuthStateManager::new());
        let handler = CleanupPluginDataHandler::new().with_oauth_state_manager(manager.clone());
        assert!(handler.oauth_state_manager.is_some());
    }

    #[test]
    fn test_cleanup_expired_oauth_flows() {
        let manager = Arc::new(OAuthStateManager::new());
        let config = OAuthConfig {
            authorization_url: "https://example.com/auth".to_string(),
            token_url: "https://example.com/token".to_string(),
            scopes: vec![],
            pkce: false,
            user_info_url: None,
            client_id: None,
        };

        // Create a fresh flow (should NOT be cleaned up)
        manager
            .start_oauth_flow(
                Uuid::new_v4(),
                Uuid::new_v4(),
                &config,
                "client-id",
                "https://example.com/callback",
            )
            .unwrap();

        assert_eq!(manager.pending_count(), 1);

        // Cleanup should not remove fresh flows
        let removed = manager.cleanup_expired();
        assert_eq!(removed, 0);
        assert_eq!(manager.pending_count(), 1);
    }

    #[test]
    fn test_handler_without_oauth_manager_still_works() {
        // Handler without OAuthStateManager should work fine (no-op for OAuth cleanup)
        let handler = CleanupPluginDataHandler::new();
        assert!(handler.oauth_state_manager.is_none());
    }
}
