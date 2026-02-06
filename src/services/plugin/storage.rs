//! Plugin Storage Protocol Types
//!
//! Defines the JSON-RPC request/response types for plugin storage operations.
//! Plugins use these methods to persist per-user data like taste profiles,
//! sync state, and cached recommendations.
//!
//! ## Architecture
//!
//! Storage is scoped per user-plugin instance. Plugins only specify a key;
//! the host resolves the user_plugin_id from the connection context.
//! This provides architectural isolation - plugins cannot address other
//! plugins' or users' data.
//!
//! ## Methods
//!
//! - `storage/get` - Get a value by key
//! - `storage/set` - Set a value (upsert) with optional TTL
//! - `storage/delete` - Delete a value by key
//! - `storage/list` - List all keys
//! - `storage/clear` - Clear all data

use serde::{Deserialize, Serialize};
use serde_json::Value;

// =============================================================================
// Storage Request Types
// =============================================================================

/// Parameters for `storage/get` method
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageGetRequest {
    /// Storage key to retrieve
    pub key: String,
}

/// Parameters for `storage/set` method
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageSetRequest {
    /// Storage key
    pub key: String,
    /// JSON data to store
    pub data: Value,
    /// Optional expiration timestamp (ISO 8601)
    /// If set, the data will be automatically cleaned up after this time
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<String>,
}

/// Parameters for `storage/delete` method
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageDeleteRequest {
    /// Storage key to delete
    pub key: String,
}

// Note: `storage/list` and `storage/clear` take no parameters

// =============================================================================
// Storage Response Types
// =============================================================================

/// Response from `storage/get` method
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageGetResponse {
    /// The stored data, or null if key doesn't exist
    pub data: Option<Value>,
    /// Expiration timestamp (ISO 8601) if TTL was set
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<String>,
}

/// Response from `storage/set` method
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageSetResponse {
    /// Always true on success
    pub success: bool,
}

/// Response from `storage/delete` method
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageDeleteResponse {
    /// Whether the key existed and was deleted
    pub deleted: bool,
}

/// Individual key entry in `storage/list` response
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageKeyEntry {
    /// Storage key name
    pub key: String,
    /// Expiration timestamp (ISO 8601) if TTL was set
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<String>,
    /// Last update timestamp (ISO 8601)
    pub updated_at: String,
}

/// Response from `storage/list` method
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageListResponse {
    /// All keys for this plugin instance (excluding expired)
    pub keys: Vec<StorageKeyEntry>,
}

/// Response from `storage/clear` method
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageClearResponse {
    /// Number of entries deleted
    pub deleted_count: u64,
}

// =============================================================================
// Permission Check
// =============================================================================

/// Check if a method name is a storage method
pub fn is_storage_method(method: &str) -> bool {
    matches!(
        method,
        "storage/get" | "storage/set" | "storage/delete" | "storage/list" | "storage/clear"
    )
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_storage_get_request_serialization() {
        let req = StorageGetRequest {
            key: "taste_profile".to_string(),
        };
        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["key"], "taste_profile");
    }

    #[test]
    fn test_storage_get_request_deserialization() {
        let json = json!({"key": "sync_state"});
        let req: StorageGetRequest = serde_json::from_value(json).unwrap();
        assert_eq!(req.key, "sync_state");
    }

    #[test]
    fn test_storage_set_request_serialization() {
        let req = StorageSetRequest {
            key: "recommendations".to_string(),
            data: json!({"items": [1, 2, 3], "score": 0.95}),
            expires_at: Some("2026-02-07T00:00:00Z".to_string()),
        };
        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["key"], "recommendations");
        assert_eq!(json["data"]["score"], 0.95);
        assert_eq!(json["expiresAt"], "2026-02-07T00:00:00Z");
    }

    #[test]
    fn test_storage_set_request_without_ttl() {
        let req = StorageSetRequest {
            key: "profile".to_string(),
            data: json!({"version": 1}),
            expires_at: None,
        };
        let json = serde_json::to_value(&req).unwrap();
        assert!(!json.as_object().unwrap().contains_key("expiresAt"));
    }

    #[test]
    fn test_storage_set_request_deserialization_with_ttl() {
        let json = json!({
            "key": "cache",
            "data": [1, 2, 3],
            "expiresAt": "2026-03-01T12:00:00Z"
        });
        let req: StorageSetRequest = serde_json::from_value(json).unwrap();
        assert_eq!(req.key, "cache");
        assert_eq!(req.data, json!([1, 2, 3]));
        assert_eq!(req.expires_at.unwrap(), "2026-03-01T12:00:00Z");
    }

    #[test]
    fn test_storage_set_request_deserialization_without_ttl() {
        let json = json!({
            "key": "state",
            "data": {"cursor": "abc123"}
        });
        let req: StorageSetRequest = serde_json::from_value(json).unwrap();
        assert!(req.expires_at.is_none());
    }

    #[test]
    fn test_storage_delete_request_serialization() {
        let req = StorageDeleteRequest {
            key: "old_cache".to_string(),
        };
        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["key"], "old_cache");
    }

    #[test]
    fn test_storage_get_response_with_data() {
        let resp = StorageGetResponse {
            data: Some(json!({"version": 2, "items": []})),
            expires_at: Some("2026-02-08T00:00:00Z".to_string()),
        };
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["data"]["version"], 2);
        assert_eq!(json["expiresAt"], "2026-02-08T00:00:00Z");
    }

    #[test]
    fn test_storage_get_response_null_data() {
        let resp = StorageGetResponse {
            data: None,
            expires_at: None,
        };
        let json = serde_json::to_value(&resp).unwrap();
        assert!(json["data"].is_null());
        assert!(!json.as_object().unwrap().contains_key("expiresAt"));
    }

    #[test]
    fn test_storage_set_response() {
        let resp = StorageSetResponse { success: true };
        let json = serde_json::to_value(&resp).unwrap();
        assert!(json["success"].as_bool().unwrap());
    }

    #[test]
    fn test_storage_delete_response() {
        let resp = StorageDeleteResponse { deleted: true };
        let json = serde_json::to_value(&resp).unwrap();
        assert!(json["deleted"].as_bool().unwrap());

        let resp2 = StorageDeleteResponse { deleted: false };
        let json2 = serde_json::to_value(&resp2).unwrap();
        assert!(!json2["deleted"].as_bool().unwrap());
    }

    #[test]
    fn test_storage_list_response() {
        let resp = StorageListResponse {
            keys: vec![
                StorageKeyEntry {
                    key: "profile".to_string(),
                    expires_at: None,
                    updated_at: "2026-02-06T10:00:00Z".to_string(),
                },
                StorageKeyEntry {
                    key: "cache".to_string(),
                    expires_at: Some("2026-02-07T00:00:00Z".to_string()),
                    updated_at: "2026-02-06T11:00:00Z".to_string(),
                },
            ],
        };
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["keys"].as_array().unwrap().len(), 2);
        assert_eq!(json["keys"][0]["key"], "profile");
        assert!(
            !json["keys"][0]
                .as_object()
                .unwrap()
                .contains_key("expiresAt")
        );
        assert_eq!(json["keys"][1]["expiresAt"], "2026-02-07T00:00:00Z");
    }

    #[test]
    fn test_storage_clear_response() {
        let resp = StorageClearResponse { deleted_count: 5 };
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["deletedCount"], 5);
    }

    #[test]
    fn test_is_storage_method() {
        assert!(is_storage_method("storage/get"));
        assert!(is_storage_method("storage/set"));
        assert!(is_storage_method("storage/delete"));
        assert!(is_storage_method("storage/list"));
        assert!(is_storage_method("storage/clear"));
        assert!(!is_storage_method("metadata/series/search"));
        assert!(!is_storage_method("initialize"));
        assert!(!is_storage_method("storage/unknown"));
    }

    #[test]
    fn test_storage_get_response_deserialization() {
        let json = json!({
            "data": {"genres": ["action", "drama"]},
            "expiresAt": "2026-12-31T23:59:59Z"
        });
        let resp: StorageGetResponse = serde_json::from_value(json).unwrap();
        assert!(resp.data.is_some());
        assert_eq!(resp.expires_at.unwrap(), "2026-12-31T23:59:59Z");
    }

    #[test]
    fn test_storage_key_entry_serialization() {
        let entry = StorageKeyEntry {
            key: "test_key".to_string(),
            expires_at: None,
            updated_at: "2026-02-06T12:00:00Z".to_string(),
        };
        let json = serde_json::to_value(&entry).unwrap();
        assert_eq!(json["key"], "test_key");
        assert_eq!(json["updatedAt"], "2026-02-06T12:00:00Z");
        assert!(!json.as_object().unwrap().contains_key("expiresAt"));
    }
}
