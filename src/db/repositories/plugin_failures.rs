//! Plugin Failures Repository
//!
//! Provides CRUD operations for plugin failure records, enabling time-windowed failure tracking.
//! This allows the system to auto-disable plugins based on failure rate within a time window
//! (e.g., 3 failures in 1 hour) rather than simple consecutive failure counts.
//!
//! ## Key Features
//!
//! - Record individual failure events with error details
//! - Count failures within a configurable time window
//! - Get recent failures for debugging
//! - Cleanup expired failures

use crate::db::entities::plugin_failures::{self, Entity as PluginFailures};
use anyhow::Result;
use chrono::{Duration, Utc};
use sea_orm::*;
use uuid::Uuid;

/// Context for a plugin failure event
#[derive(Debug, Clone, Default)]
pub struct FailureContext {
    /// Error code for categorization
    pub error_code: Option<String>,
    /// Which method failed
    pub method: Option<String>,
    /// JSON-RPC request ID if applicable
    pub request_id: Option<String>,
    /// Additional context (parameters, stack trace, etc.)
    pub context: Option<serde_json::Value>,
    /// Sanitized summary of the request parameters (sensitive fields redacted)
    pub request_summary: Option<String>,
}

/// Fields that should be redacted from request summaries
#[allow(dead_code)] // Available for callers to use when recording failures
const SENSITIVE_FIELD_PATTERNS: &[&str] = &[
    "key",
    "secret",
    "token",
    "password",
    "credential",
    "auth",
    "bearer",
    "api_key",
    "apikey",
];

/// Redact sensitive fields from a JSON value
///
/// Any object keys containing the patterns in `SENSITIVE_FIELD_PATTERNS` (case-insensitive)
/// will have their values replaced with "[REDACTED]".
#[allow(dead_code)] // Available for callers to use when recording failures
pub fn redact_sensitive_fields(value: &serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(map) => {
            let redacted: serde_json::Map<String, serde_json::Value> = map
                .iter()
                .map(|(k, v)| {
                    let key_lower = k.to_lowercase();
                    let is_sensitive = SENSITIVE_FIELD_PATTERNS
                        .iter()
                        .any(|pattern| key_lower.contains(pattern));

                    if is_sensitive {
                        (
                            k.clone(),
                            serde_json::Value::String("[REDACTED]".to_string()),
                        )
                    } else {
                        (k.clone(), redact_sensitive_fields(v))
                    }
                })
                .collect();
            serde_json::Value::Object(redacted)
        }
        serde_json::Value::Array(arr) => {
            serde_json::Value::Array(arr.iter().map(redact_sensitive_fields).collect())
        }
        // Primitive values pass through unchanged
        other => other.clone(),
    }
}

/// Create a redacted summary string from request parameters
///
/// Converts the value to JSON, redacts sensitive fields, and truncates if necessary.
#[allow(dead_code)] // Available for callers to use when recording failures
pub fn create_request_summary(params: &serde_json::Value, max_length: usize) -> String {
    let redacted = redact_sensitive_fields(params);
    let json_str = serde_json::to_string(&redacted).unwrap_or_else(|_| "{}".to_string());

    if json_str.len() > max_length {
        format!("{}...", &json_str[..max_length.saturating_sub(3)])
    } else {
        json_str
    }
}

pub struct PluginFailuresRepository;

impl PluginFailuresRepository {
    // =========================================================================
    // Create Operations
    // =========================================================================

    /// Record a new failure event for a plugin
    ///
    /// # Arguments
    /// * `db` - Database connection
    /// * `plugin_id` - ID of the plugin that failed
    /// * `error_message` - Human-readable error message
    /// * `failure_context` - Additional context about the failure
    /// * `retention_days` - How long to keep the failure record (default: 30 days)
    pub async fn record_failure(
        db: &DatabaseConnection,
        plugin_id: Uuid,
        error_message: &str,
        failure_context: FailureContext,
        retention_days: Option<i64>,
    ) -> Result<plugin_failures::Model> {
        let now = Utc::now();
        let retention = retention_days.unwrap_or(30);
        let expires_at = now + Duration::days(retention);

        let failure = plugin_failures::ActiveModel {
            id: Set(Uuid::new_v4()),
            plugin_id: Set(plugin_id),
            error_message: Set(error_message.to_string()),
            error_code: Set(failure_context.error_code),
            method: Set(failure_context.method),
            request_id: Set(failure_context.request_id),
            context: Set(failure_context.context),
            request_summary: Set(failure_context.request_summary),
            occurred_at: Set(now),
            expires_at: Set(expires_at),
        };

        let result = failure.insert(db).await?;
        Ok(result)
    }

    // =========================================================================
    // Read Operations
    // =========================================================================

    /// Count failures for a plugin within a time window
    ///
    /// # Arguments
    /// * `db` - Database connection
    /// * `plugin_id` - ID of the plugin
    /// * `window_seconds` - Time window in seconds (e.g., 3600 for 1 hour)
    ///
    /// # Returns
    /// Number of failures within the time window
    pub async fn count_failures_in_window(
        db: &DatabaseConnection,
        plugin_id: Uuid,
        window_seconds: i64,
    ) -> Result<u64> {
        let window_start = Utc::now() - Duration::seconds(window_seconds);

        let count = PluginFailures::find()
            .filter(plugin_failures::Column::PluginId.eq(plugin_id))
            .filter(plugin_failures::Column::OccurredAt.gte(window_start))
            .count(db)
            .await?;

        Ok(count)
    }

    /// Get recent failures for a plugin
    ///
    /// # Arguments
    /// * `db` - Database connection
    /// * `plugin_id` - ID of the plugin
    /// * `limit` - Maximum number of failures to return
    ///
    /// # Returns
    /// List of recent failures, ordered by most recent first
    #[allow(dead_code)] // Available for future use
    pub async fn get_recent_failures(
        db: &DatabaseConnection,
        plugin_id: Uuid,
        limit: u64,
    ) -> Result<Vec<plugin_failures::Model>> {
        let failures = PluginFailures::find()
            .filter(plugin_failures::Column::PluginId.eq(plugin_id))
            .order_by_desc(plugin_failures::Column::OccurredAt)
            .limit(limit)
            .all(db)
            .await?;

        Ok(failures)
    }

    /// Get all failures for a plugin with pagination
    ///
    /// # Arguments
    /// * `db` - Database connection
    /// * `plugin_id` - ID of the plugin
    /// * `limit` - Maximum number of failures to return
    /// * `offset` - Number of failures to skip
    ///
    /// # Returns
    /// Tuple of (failures, total count)
    pub async fn get_failures_paginated(
        db: &DatabaseConnection,
        plugin_id: Uuid,
        limit: u64,
        offset: u64,
    ) -> Result<(Vec<plugin_failures::Model>, u64)> {
        let total = PluginFailures::find()
            .filter(plugin_failures::Column::PluginId.eq(plugin_id))
            .count(db)
            .await?;

        let failures = PluginFailures::find()
            .filter(plugin_failures::Column::PluginId.eq(plugin_id))
            .order_by_desc(plugin_failures::Column::OccurredAt)
            .limit(limit)
            .offset(offset)
            .all(db)
            .await?;

        Ok((failures, total))
    }

    /// Get a single failure by ID
    #[allow(dead_code)] // Available for future use
    pub async fn get_by_id(
        db: &DatabaseConnection,
        id: Uuid,
    ) -> Result<Option<plugin_failures::Model>> {
        let failure = PluginFailures::find_by_id(id).one(db).await?;
        Ok(failure)
    }

    // =========================================================================
    // Delete Operations
    // =========================================================================

    /// Clean up expired failures
    ///
    /// # Returns
    /// Number of failures deleted
    #[allow(dead_code)] // Called by scheduled cleanup task
    pub async fn cleanup_expired(db: &DatabaseConnection) -> Result<u64> {
        let now = Utc::now();

        let result = PluginFailures::delete_many()
            .filter(plugin_failures::Column::ExpiresAt.lt(now))
            .exec(db)
            .await?;

        Ok(result.rows_affected)
    }

    /// Delete all failures for a plugin
    ///
    /// # Returns
    /// Number of failures deleted
    #[allow(dead_code)] // Available for future use
    pub async fn delete_all_for_plugin(db: &DatabaseConnection, plugin_id: Uuid) -> Result<u64> {
        let result = PluginFailures::delete_many()
            .filter(plugin_failures::Column::PluginId.eq(plugin_id))
            .exec(db)
            .await?;

        Ok(result.rows_affected)
    }

    /// Delete failures older than a specific timestamp for a plugin
    #[allow(dead_code)] // Available for future use
    pub async fn delete_older_than(
        db: &DatabaseConnection,
        plugin_id: Uuid,
        before: chrono::DateTime<Utc>,
    ) -> Result<u64> {
        let result = PluginFailures::delete_many()
            .filter(plugin_failures::Column::PluginId.eq(plugin_id))
            .filter(plugin_failures::Column::OccurredAt.lt(before))
            .exec(db)
            .await?;

        Ok(result.rows_affected)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::entities::plugin_failures::error_codes;
    use crate::db::repositories::PluginsRepository;
    use crate::db::test_helpers::setup_test_db;
    use crate::services::plugin::protocol::PluginScope;
    use std::env;
    use tokio::time::sleep;

    fn setup_test_encryption_key() {
        if env::var("CODEX_ENCRYPTION_KEY").is_err() {
            // SAFETY: Tests are run with --test-threads=1 or use serial execution,
            // so there's no concurrent access to environment variables.
            unsafe {
                env::set_var(
                    "CODEX_ENCRYPTION_KEY",
                    "AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8=",
                );
            }
        }
    }

    async fn create_test_plugin(db: &DatabaseConnection) -> Uuid {
        setup_test_encryption_key();
        let plugin = PluginsRepository::create(
            db,
            &format!("test_{}", Uuid::new_v4()),
            "Test Plugin",
            None,
            "system",
            "node",
            vec![],
            vec![],
            None,
            vec![],
            vec![PluginScope::SeriesDetail],
            vec![], // library_ids - empty means all libraries
            None,
            "env",
            None,
            true,
            None,
            Some(60), // rate_limit_requests_per_minute
        )
        .await
        .unwrap();
        plugin.id
    }

    #[tokio::test]
    async fn test_record_failure() {
        let db = setup_test_db().await;
        let plugin_id = create_test_plugin(&db).await;

        let params = serde_json::json!({"query": "one piece"});
        let failure = PluginFailuresRepository::record_failure(
            &db,
            plugin_id,
            "Connection timeout after 30s",
            FailureContext {
                error_code: Some(error_codes::TIMEOUT.to_string()),
                method: Some("metadata/search".to_string()),
                request_id: Some("req-123".to_string()),
                context: Some(params.clone()),
                request_summary: Some(create_request_summary(&params, 1000)),
            },
            None,
        )
        .await
        .unwrap();

        assert_eq!(failure.plugin_id, plugin_id);
        assert_eq!(failure.error_message, "Connection timeout after 30s");
        assert_eq!(failure.error_code, Some(error_codes::TIMEOUT.to_string()));
        assert_eq!(failure.method, Some("metadata/search".to_string()));
        assert_eq!(failure.request_id, Some("req-123".to_string()));
        assert!(failure.context.is_some());
        assert!(failure.request_summary.is_some());
    }

    #[tokio::test]
    async fn test_count_failures_in_window() {
        let db = setup_test_db().await;
        let plugin_id = create_test_plugin(&db).await;

        // Record 3 failures
        for i in 0..3 {
            PluginFailuresRepository::record_failure(
                &db,
                plugin_id,
                &format!("Error {}", i),
                FailureContext::default(),
                None,
            )
            .await
            .unwrap();
        }

        // Count failures in 1 hour window
        let count = PluginFailuresRepository::count_failures_in_window(&db, plugin_id, 3600)
            .await
            .unwrap();
        assert_eq!(count, 3);

        // Count with very short window (should be 0 after a small delay)
        // Since all failures just occurred, they should still be within even a 1-second window
        let count = PluginFailuresRepository::count_failures_in_window(&db, plugin_id, 1)
            .await
            .unwrap();
        assert_eq!(count, 3); // All failures are very recent
    }

    #[tokio::test]
    async fn test_get_recent_failures() {
        let db = setup_test_db().await;
        let plugin_id = create_test_plugin(&db).await;

        // Record 5 failures with small delays
        for i in 0..5 {
            PluginFailuresRepository::record_failure(
                &db,
                plugin_id,
                &format!("Error {}", i),
                FailureContext {
                    error_code: Some(format!("CODE_{}", i)),
                    ..Default::default()
                },
                None,
            )
            .await
            .unwrap();
            // Small delay to ensure ordering
            sleep(std::time::Duration::from_millis(10)).await;
        }

        // Get last 3 failures
        let failures = PluginFailuresRepository::get_recent_failures(&db, plugin_id, 3)
            .await
            .unwrap();

        assert_eq!(failures.len(), 3);
        // Should be in descending order (most recent first)
        assert_eq!(failures[0].error_message, "Error 4");
        assert_eq!(failures[1].error_message, "Error 3");
        assert_eq!(failures[2].error_message, "Error 2");
    }

    #[tokio::test]
    async fn test_get_failures_paginated() {
        let db = setup_test_db().await;
        let plugin_id = create_test_plugin(&db).await;

        // Record 5 failures
        for i in 0..5 {
            PluginFailuresRepository::record_failure(
                &db,
                plugin_id,
                &format!("Error {}", i),
                FailureContext::default(),
                None,
            )
            .await
            .unwrap();
        }

        // Get page 1 (first 2)
        let (failures, total) =
            PluginFailuresRepository::get_failures_paginated(&db, plugin_id, 2, 0)
                .await
                .unwrap();

        assert_eq!(total, 5);
        assert_eq!(failures.len(), 2);

        // Get page 2 (next 2)
        let (failures, _) = PluginFailuresRepository::get_failures_paginated(&db, plugin_id, 2, 2)
            .await
            .unwrap();

        assert_eq!(failures.len(), 2);
    }

    #[tokio::test]
    async fn test_delete_all_for_plugin() {
        let db = setup_test_db().await;
        let plugin_id = create_test_plugin(&db).await;
        let other_plugin_id = create_test_plugin(&db).await;

        // Record failures for both plugins
        for i in 0..3 {
            PluginFailuresRepository::record_failure(
                &db,
                plugin_id,
                &format!("Error {}", i),
                FailureContext::default(),
                None,
            )
            .await
            .unwrap();
        }

        PluginFailuresRepository::record_failure(
            &db,
            other_plugin_id,
            "Other plugin error",
            FailureContext::default(),
            None,
        )
        .await
        .unwrap();

        // Delete all for first plugin
        let deleted = PluginFailuresRepository::delete_all_for_plugin(&db, plugin_id)
            .await
            .unwrap();

        assert_eq!(deleted, 3);

        // Verify first plugin has no failures
        let count = PluginFailuresRepository::count_failures_in_window(&db, plugin_id, 3600)
            .await
            .unwrap();
        assert_eq!(count, 0);

        // Verify other plugin still has failures
        let count = PluginFailuresRepository::count_failures_in_window(&db, other_plugin_id, 3600)
            .await
            .unwrap();
        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn test_cleanup_expired() {
        let db = setup_test_db().await;
        let plugin_id = create_test_plugin(&db).await;

        // Record a failure with very short retention (essentially already expired)
        // We'll use -1 days to make it expire immediately
        let failure = PluginFailuresRepository::record_failure(
            &db,
            plugin_id,
            "Expired error",
            FailureContext::default(),
            Some(-1), // Already expired
        )
        .await
        .unwrap();

        // Verify failure exists
        let found = PluginFailuresRepository::get_by_id(&db, failure.id)
            .await
            .unwrap();
        assert!(found.is_some());

        // Cleanup expired
        let deleted = PluginFailuresRepository::cleanup_expired(&db)
            .await
            .unwrap();
        assert_eq!(deleted, 1);

        // Verify failure is gone
        let found = PluginFailuresRepository::get_by_id(&db, failure.id)
            .await
            .unwrap();
        assert!(found.is_none());
    }

    #[tokio::test]
    async fn test_custom_retention_period() {
        let db = setup_test_db().await;
        let plugin_id = create_test_plugin(&db).await;

        // Record with custom 7-day retention
        let failure = PluginFailuresRepository::record_failure(
            &db,
            plugin_id,
            "Short retention error",
            FailureContext::default(),
            Some(7),
        )
        .await
        .unwrap();

        // Verify expires_at is approximately 7 days from now
        let now = Utc::now();
        let expected_expiry = now + Duration::days(7);
        let diff = (failure.expires_at - expected_expiry).num_seconds().abs();

        // Allow 5 seconds of tolerance
        assert!(diff < 5, "Expiry time should be ~7 days from now");
    }

    #[tokio::test]
    async fn test_failures_isolated_by_plugin() {
        let db = setup_test_db().await;
        let plugin_a = create_test_plugin(&db).await;
        let plugin_b = create_test_plugin(&db).await;

        // Record failures for plugin A
        for _ in 0..5 {
            PluginFailuresRepository::record_failure(
                &db,
                plugin_a,
                "Plugin A error",
                FailureContext::default(),
                None,
            )
            .await
            .unwrap();
        }

        // Record failures for plugin B
        for _ in 0..2 {
            PluginFailuresRepository::record_failure(
                &db,
                plugin_b,
                "Plugin B error",
                FailureContext::default(),
                None,
            )
            .await
            .unwrap();
        }

        // Verify counts are isolated
        let count_a = PluginFailuresRepository::count_failures_in_window(&db, plugin_a, 3600)
            .await
            .unwrap();
        let count_b = PluginFailuresRepository::count_failures_in_window(&db, plugin_b, 3600)
            .await
            .unwrap();

        assert_eq!(count_a, 5);
        assert_eq!(count_b, 2);
    }

    // =========================================================================
    // Redaction Tests
    // =========================================================================

    #[test]
    fn test_redact_sensitive_fields_simple() {
        let input = serde_json::json!({
            "query": "one piece",
            "api_key": "secret-123",
            "page": 1
        });

        let redacted = redact_sensitive_fields(&input);

        assert_eq!(redacted["query"], "one piece");
        assert_eq!(redacted["api_key"], "[REDACTED]");
        assert_eq!(redacted["page"], 1);
    }

    #[test]
    fn test_redact_sensitive_fields_nested() {
        let input = serde_json::json!({
            "auth": {
                "token": "bearer-xyz",
                "username": "admin"
            },
            "data": {
                "password": "secret",
                "value": 42
            }
        });

        let redacted = redact_sensitive_fields(&input);

        // Top-level "auth" key should be redacted entirely
        assert_eq!(redacted["auth"], "[REDACTED]");
        // Nested "password" should be redacted, but "value" preserved
        assert_eq!(redacted["data"]["password"], "[REDACTED]");
        assert_eq!(redacted["data"]["value"], 42);
    }

    #[test]
    fn test_redact_sensitive_fields_array() {
        let input = serde_json::json!({
            "items": [
                {"name": "item1", "secret_key": "abc"},
                {"name": "item2", "secret_key": "xyz"}
            ]
        });

        let redacted = redact_sensitive_fields(&input);

        let items = redacted["items"].as_array().unwrap();
        assert_eq!(items[0]["name"], "item1");
        assert_eq!(items[0]["secret_key"], "[REDACTED]");
        assert_eq!(items[1]["name"], "item2");
        assert_eq!(items[1]["secret_key"], "[REDACTED]");
    }

    #[test]
    fn test_redact_sensitive_fields_case_insensitive() {
        let input = serde_json::json!({
            "API_KEY": "key1",
            "ApiKey": "key2",
            "apikey": "key3",
            "TOKEN": "tok1",
            "Bearer_Token": "tok2"
        });

        let redacted = redact_sensitive_fields(&input);

        assert_eq!(redacted["API_KEY"], "[REDACTED]");
        assert_eq!(redacted["ApiKey"], "[REDACTED]");
        assert_eq!(redacted["apikey"], "[REDACTED]");
        assert_eq!(redacted["TOKEN"], "[REDACTED]");
        assert_eq!(redacted["Bearer_Token"], "[REDACTED]");
    }

    #[test]
    fn test_redact_sensitive_fields_preserves_non_sensitive() {
        let input = serde_json::json!({
            "query": "search term",
            "limit": 10,
            "offset": 0,
            "series_id": "12345",
            "include_covers": true,
            "tags": ["action", "adventure"]
        });

        let redacted = redact_sensitive_fields(&input);

        assert_eq!(redacted["query"], "search term");
        assert_eq!(redacted["limit"], 10);
        assert_eq!(redacted["offset"], 0);
        assert_eq!(redacted["series_id"], "12345");
        assert_eq!(redacted["include_covers"], true);
        assert_eq!(redacted["tags"][0], "action");
    }

    #[test]
    fn test_create_request_summary_basic() {
        let input = serde_json::json!({
            "query": "one piece",
            "page": 1
        });

        let summary = create_request_summary(&input, 1000);

        // Should be valid JSON containing the fields
        assert!(summary.contains("query"));
        assert!(summary.contains("one piece"));
        assert!(summary.contains("page"));
    }

    #[test]
    fn test_create_request_summary_truncates() {
        let input = serde_json::json!({
            "description": "A very long description that should be truncated when the max length is small"
        });

        let summary = create_request_summary(&input, 30);

        assert!(summary.len() <= 30);
        assert!(summary.ends_with("..."));
    }

    #[test]
    fn test_create_request_summary_redacts_and_truncates() {
        let input = serde_json::json!({
            "query": "search",
            "api_key": "super-secret-key-that-is-very-long"
        });

        let summary = create_request_summary(&input, 100);

        // Should contain redacted version
        assert!(summary.contains("[REDACTED]"));
        // Should NOT contain the actual secret
        assert!(!summary.contains("super-secret-key"));
    }

    #[tokio::test]
    async fn test_record_failure_with_request_summary() {
        let db = setup_test_db().await;
        let plugin_id = create_test_plugin(&db).await;

        let params = serde_json::json!({
            "query": "one piece",
            "api_key": "secret-123"
        });

        let failure = PluginFailuresRepository::record_failure(
            &db,
            plugin_id,
            "API error",
            FailureContext {
                error_code: Some("API_ERROR".to_string()),
                method: Some("metadata/search".to_string()),
                request_id: None,
                context: None,
                request_summary: Some(create_request_summary(&params, 1000)),
            },
            None,
        )
        .await
        .unwrap();

        // Verify request_summary is stored
        assert!(failure.request_summary.is_some());
        let summary = failure.request_summary.unwrap();

        // Should contain redacted value, not the secret
        assert!(summary.contains("[REDACTED]"));
        assert!(!summary.contains("secret-123"));
        assert!(summary.contains("one piece"));
    }
}
