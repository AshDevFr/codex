//! Storage Request Handler
//!
//! Processes storage method requests from plugins on the host side.
//! When a plugin sends a `storage/*` JSON-RPC request, the host intercepts it
//! and handles it using the database repository, then sends back the response.
//!
//! This implements the "reverse RPC" pattern where the plugin acts as client
//! and the host acts as server for storage operations.

use chrono::{DateTime, Utc};
use sea_orm::DatabaseConnection;
use serde_json::Value;
use tracing::{debug, error, warn};
use uuid::Uuid;

use super::protocol::{JsonRpcError, JsonRpcRequest, JsonRpcResponse, error_codes, methods};
use super::storage::{
    StorageClearResponse, StorageDeleteRequest, StorageDeleteResponse, StorageGetRequest,
    StorageGetResponse, StorageKeyEntry, StorageListResponse, StorageSetRequest,
    StorageSetResponse,
};
use crate::db::repositories::UserPluginDataRepository;

/// Handles storage requests from plugins.
///
/// This handler is created per-connection with a specific `user_plugin_id`,
/// providing architectural isolation - each handler can only access its own
/// user-plugin instance's data.
#[derive(Clone)]
pub struct StorageRequestHandler {
    db: DatabaseConnection,
    user_plugin_id: Uuid,
}

impl StorageRequestHandler {
    /// Create a new storage handler for a specific user-plugin instance
    pub fn new(db: DatabaseConnection, user_plugin_id: Uuid) -> Self {
        Self { db, user_plugin_id }
    }

    /// Handle a storage JSON-RPC request and return a response
    pub async fn handle_request(&self, request: &JsonRpcRequest) -> JsonRpcResponse {
        let id = request.id.clone();
        let method = request.method.as_str();

        debug!(
            method = method,
            user_plugin_id = %self.user_plugin_id,
            "Handling storage request"
        );

        match method {
            methods::STORAGE_GET => self.handle_get(request).await,
            methods::STORAGE_SET => self.handle_set(request).await,
            methods::STORAGE_DELETE => self.handle_delete(request).await,
            methods::STORAGE_LIST => self.handle_list(request).await,
            methods::STORAGE_CLEAR => self.handle_clear(request).await,
            _ => JsonRpcResponse::error(
                Some(id),
                JsonRpcError::new(
                    error_codes::METHOD_NOT_FOUND,
                    format!("Unknown storage method: {}", method),
                ),
            ),
        }
    }

    async fn handle_get(&self, request: &JsonRpcRequest) -> JsonRpcResponse {
        let id = request.id.clone();

        let params: StorageGetRequest = match Self::parse_params(&request.params) {
            Ok(p) => p,
            Err(resp) => return resp.with_id(id),
        };

        match UserPluginDataRepository::get(&self.db, self.user_plugin_id, &params.key).await {
            Ok(Some(entry)) => {
                let response = StorageGetResponse {
                    data: Some(entry.data),
                    expires_at: entry.expires_at.map(|dt| dt.to_rfc3339()),
                };
                JsonRpcResponse::success(id, serde_json::to_value(response).unwrap())
            }
            Ok(None) => {
                let response = StorageGetResponse {
                    data: None,
                    expires_at: None,
                };
                JsonRpcResponse::success(id, serde_json::to_value(response).unwrap())
            }
            Err(e) => {
                error!(error = %e, "Storage get failed");
                JsonRpcResponse::error(
                    Some(id),
                    JsonRpcError::new(error_codes::INTERNAL_ERROR, format!("Storage error: {}", e)),
                )
            }
        }
    }

    async fn handle_set(&self, request: &JsonRpcRequest) -> JsonRpcResponse {
        let id = request.id.clone();

        let params: StorageSetRequest = match Self::parse_params(&request.params) {
            Ok(p) => p,
            Err(resp) => return resp.with_id(id),
        };

        // Parse optional expires_at
        let expires_at: Option<DateTime<Utc>> = match &params.expires_at {
            Some(ts) => match DateTime::parse_from_rfc3339(ts) {
                Ok(dt) => Some(dt.with_timezone(&Utc)),
                Err(e) => {
                    warn!(error = %e, timestamp = ts, "Invalid expires_at timestamp");
                    return JsonRpcResponse::error(
                        Some(id),
                        JsonRpcError::new(
                            error_codes::INVALID_PARAMS,
                            format!("Invalid expiresAt timestamp: {}", e),
                        ),
                    );
                }
            },
            None => None,
        };

        match UserPluginDataRepository::set(
            &self.db,
            self.user_plugin_id,
            &params.key,
            params.data,
            expires_at,
        )
        .await
        {
            Ok(_) => {
                let response = StorageSetResponse { success: true };
                JsonRpcResponse::success(id, serde_json::to_value(response).unwrap())
            }
            Err(e) => {
                error!(error = %e, "Storage set failed");
                JsonRpcResponse::error(
                    Some(id),
                    JsonRpcError::new(error_codes::INTERNAL_ERROR, format!("Storage error: {}", e)),
                )
            }
        }
    }

    async fn handle_delete(&self, request: &JsonRpcRequest) -> JsonRpcResponse {
        let id = request.id.clone();

        let params: StorageDeleteRequest = match Self::parse_params(&request.params) {
            Ok(p) => p,
            Err(resp) => return resp.with_id(id),
        };

        match UserPluginDataRepository::delete(&self.db, self.user_plugin_id, &params.key).await {
            Ok(deleted) => {
                let response = StorageDeleteResponse { deleted };
                JsonRpcResponse::success(id, serde_json::to_value(response).unwrap())
            }
            Err(e) => {
                error!(error = %e, "Storage delete failed");
                JsonRpcResponse::error(
                    Some(id),
                    JsonRpcError::new(error_codes::INTERNAL_ERROR, format!("Storage error: {}", e)),
                )
            }
        }
    }

    async fn handle_list(&self, request: &JsonRpcRequest) -> JsonRpcResponse {
        let id = request.id.clone();

        match UserPluginDataRepository::list_keys(&self.db, self.user_plugin_id).await {
            Ok(entries) => {
                let keys: Vec<StorageKeyEntry> = entries
                    .into_iter()
                    .map(|e| StorageKeyEntry {
                        key: e.key,
                        expires_at: e.expires_at.map(|dt| dt.to_rfc3339()),
                        updated_at: e.updated_at.to_rfc3339(),
                    })
                    .collect();
                let response = StorageListResponse { keys };
                JsonRpcResponse::success(id, serde_json::to_value(response).unwrap())
            }
            Err(e) => {
                error!(error = %e, "Storage list failed");
                JsonRpcResponse::error(
                    Some(id),
                    JsonRpcError::new(error_codes::INTERNAL_ERROR, format!("Storage error: {}", e)),
                )
            }
        }
    }

    async fn handle_clear(&self, request: &JsonRpcRequest) -> JsonRpcResponse {
        let id = request.id.clone();

        match UserPluginDataRepository::clear_all(&self.db, self.user_plugin_id).await {
            Ok(count) => {
                let response = StorageClearResponse {
                    deleted_count: count,
                };
                JsonRpcResponse::success(id, serde_json::to_value(response).unwrap())
            }
            Err(e) => {
                error!(error = %e, "Storage clear failed");
                JsonRpcResponse::error(
                    Some(id),
                    JsonRpcError::new(error_codes::INTERNAL_ERROR, format!("Storage error: {}", e)),
                )
            }
        }
    }

    /// Parse JSON-RPC params into the expected type
    #[allow(clippy::result_large_err)]
    fn parse_params<T: serde::de::DeserializeOwned>(
        params: &Option<Value>,
    ) -> Result<T, JsonRpcResponse> {
        let params = params.as_ref().ok_or_else(|| {
            JsonRpcResponse::error(
                None,
                JsonRpcError::new(error_codes::INVALID_PARAMS, "params is required"),
            )
        })?;

        serde_json::from_value::<T>(params.clone()).map_err(|e| {
            JsonRpcResponse::error(
                None,
                JsonRpcError::new(
                    error_codes::INVALID_PARAMS,
                    format!("Invalid params: {}", e),
                ),
            )
        })
    }
}

/// Helper trait to set the ID on a response that was created without one
trait WithId {
    fn with_id(self, id: super::protocol::RequestId) -> Self;
}

impl WithId for JsonRpcResponse {
    fn with_id(mut self, id: super::protocol::RequestId) -> Self {
        self.id = Some(id);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::entities::plugins;
    use crate::db::entities::users;
    use crate::db::repositories::{PluginsRepository, UserPluginsRepository, UserRepository};
    use crate::db::test_helpers::setup_test_db;
    use crate::services::plugin::protocol::RequestId;
    use serde_json::json;

    async fn create_test_user(db: &DatabaseConnection) -> users::Model {
        let user = users::Model {
            id: Uuid::new_v4(),
            username: format!("storuser_{}", Uuid::new_v4()),
            email: format!("stor_{}@example.com", Uuid::new_v4()),
            password_hash: "hash123".to_string(),
            role: "reader".to_string(),
            is_active: true,
            email_verified: false,
            permissions: json!([]),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            last_login_at: None,
        };
        UserRepository::create(db, &user).await.unwrap()
    }

    async fn create_test_plugin(db: &DatabaseConnection) -> plugins::Model {
        PluginsRepository::create(
            db,
            &format!("stor_plugin_{}", Uuid::new_v4()),
            "Storage Test Plugin",
            Some("A test plugin"),
            "user",
            "node",
            vec!["index.js".to_string()],
            vec![],
            None,
            vec![],
            vec![],
            vec![],
            None,
            "env",
            None,
            true,
            None,
            None,
        )
        .await
        .unwrap()
    }

    async fn setup_handler(db: &DatabaseConnection) -> (StorageRequestHandler, Uuid) {
        let user = create_test_user(db).await;
        let plugin = create_test_plugin(db).await;
        let user_plugin = UserPluginsRepository::create(db, plugin.id, user.id)
            .await
            .unwrap();
        let handler = StorageRequestHandler::new(db.clone(), user_plugin.id);
        (handler, user_plugin.id)
    }

    fn make_request(method: &str, params: Option<Value>) -> JsonRpcRequest {
        JsonRpcRequest::new(1i64, method, params)
    }

    #[tokio::test]
    async fn test_storage_get_nonexistent() {
        let db = setup_test_db().await;
        let (handler, _) = setup_handler(&db).await;

        let request = make_request("storage/get", Some(json!({"key": "missing"})));
        let response = handler.handle_request(&request).await;

        assert!(!response.is_error());
        let result: StorageGetResponse = serde_json::from_value(response.result.unwrap()).unwrap();
        assert!(result.data.is_none());
    }

    #[tokio::test]
    async fn test_storage_set_and_get() {
        let db = setup_test_db().await;
        let (handler, _) = setup_handler(&db).await;

        // Set
        let set_req = make_request(
            "storage/set",
            Some(json!({"key": "profile", "data": {"score": 0.95}})),
        );
        let set_resp = handler.handle_request(&set_req).await;
        assert!(!set_resp.is_error());
        let set_result: StorageSetResponse =
            serde_json::from_value(set_resp.result.unwrap()).unwrap();
        assert!(set_result.success);

        // Get
        let get_req = make_request("storage/get", Some(json!({"key": "profile"})));
        let get_resp = handler.handle_request(&get_req).await;
        assert!(!get_resp.is_error());
        let get_result: StorageGetResponse =
            serde_json::from_value(get_resp.result.unwrap()).unwrap();
        assert_eq!(get_result.data.unwrap(), json!({"score": 0.95}));
    }

    #[tokio::test]
    async fn test_storage_set_with_ttl() {
        let db = setup_test_db().await;
        let (handler, _) = setup_handler(&db).await;

        let set_req = make_request(
            "storage/set",
            Some(json!({
                "key": "cache",
                "data": [1, 2, 3],
                "expiresAt": "2030-12-31T23:59:59Z"
            })),
        );
        let set_resp = handler.handle_request(&set_req).await;
        assert!(!set_resp.is_error());

        let get_req = make_request("storage/get", Some(json!({"key": "cache"})));
        let get_resp = handler.handle_request(&get_req).await;
        let result: StorageGetResponse = serde_json::from_value(get_resp.result.unwrap()).unwrap();
        assert_eq!(result.data.unwrap(), json!([1, 2, 3]));
        assert!(result.expires_at.is_some());
    }

    #[tokio::test]
    async fn test_storage_set_invalid_timestamp() {
        let db = setup_test_db().await;
        let (handler, _) = setup_handler(&db).await;

        let set_req = make_request(
            "storage/set",
            Some(json!({
                "key": "bad",
                "data": "test",
                "expiresAt": "not-a-timestamp"
            })),
        );
        let resp = handler.handle_request(&set_req).await;
        assert!(resp.is_error());
        assert_eq!(resp.error.unwrap().code, error_codes::INVALID_PARAMS);
    }

    #[tokio::test]
    async fn test_storage_delete() {
        let db = setup_test_db().await;
        let (handler, _) = setup_handler(&db).await;

        // Set then delete
        let set_req = make_request("storage/set", Some(json!({"key": "temp", "data": "value"})));
        handler.handle_request(&set_req).await;

        let del_req = make_request("storage/delete", Some(json!({"key": "temp"})));
        let del_resp = handler.handle_request(&del_req).await;
        assert!(!del_resp.is_error());
        let result: StorageDeleteResponse =
            serde_json::from_value(del_resp.result.unwrap()).unwrap();
        assert!(result.deleted);

        // Delete nonexistent
        let del_req2 = make_request("storage/delete", Some(json!({"key": "nope"})));
        let del_resp2 = handler.handle_request(&del_req2).await;
        let result2: StorageDeleteResponse =
            serde_json::from_value(del_resp2.result.unwrap()).unwrap();
        assert!(!result2.deleted);
    }

    #[tokio::test]
    async fn test_storage_list() {
        let db = setup_test_db().await;
        let (handler, _) = setup_handler(&db).await;

        // Set some keys
        for key in &["alpha", "beta", "gamma"] {
            let req = make_request("storage/set", Some(json!({"key": key, "data": key})));
            handler.handle_request(&req).await;
        }

        let list_req = make_request("storage/list", None);
        let list_resp = handler.handle_request(&list_req).await;
        assert!(!list_resp.is_error());
        let result: StorageListResponse =
            serde_json::from_value(list_resp.result.unwrap()).unwrap();
        assert_eq!(result.keys.len(), 3);
        assert_eq!(result.keys[0].key, "alpha");
        assert_eq!(result.keys[1].key, "beta");
        assert_eq!(result.keys[2].key, "gamma");
    }

    #[tokio::test]
    async fn test_storage_clear() {
        let db = setup_test_db().await;
        let (handler, _) = setup_handler(&db).await;

        // Set some keys
        for key in &["a", "b", "c"] {
            let req = make_request("storage/set", Some(json!({"key": key, "data": 1})));
            handler.handle_request(&req).await;
        }

        let clear_req = make_request("storage/clear", None);
        let clear_resp = handler.handle_request(&clear_req).await;
        assert!(!clear_resp.is_error());
        let result: StorageClearResponse =
            serde_json::from_value(clear_resp.result.unwrap()).unwrap();
        assert_eq!(result.deleted_count, 3);

        // Verify empty
        let list_req = make_request("storage/list", None);
        let list_resp = handler.handle_request(&list_req).await;
        let list_result: StorageListResponse =
            serde_json::from_value(list_resp.result.unwrap()).unwrap();
        assert!(list_result.keys.is_empty());
    }

    #[tokio::test]
    async fn test_storage_missing_params() {
        let db = setup_test_db().await;
        let (handler, _) = setup_handler(&db).await;

        let req = make_request("storage/get", None);
        let resp = handler.handle_request(&req).await;
        assert!(resp.is_error());
        assert_eq!(resp.error.unwrap().code, error_codes::INVALID_PARAMS);
    }

    #[tokio::test]
    async fn test_storage_invalid_params() {
        let db = setup_test_db().await;
        let (handler, _) = setup_handler(&db).await;

        let req = make_request("storage/get", Some(json!({"wrong_field": "test"})));
        let resp = handler.handle_request(&req).await;
        assert!(resp.is_error());
        assert_eq!(resp.error.unwrap().code, error_codes::INVALID_PARAMS);
    }

    #[tokio::test]
    async fn test_storage_unknown_method() {
        let db = setup_test_db().await;
        let (handler, _) = setup_handler(&db).await;

        let req = make_request("storage/unknown", Some(json!({})));
        let resp = handler.handle_request(&req).await;
        assert!(resp.is_error());
        assert_eq!(resp.error.unwrap().code, error_codes::METHOD_NOT_FOUND);
    }

    #[tokio::test]
    async fn test_storage_data_isolation() {
        let db = setup_test_db().await;

        // Create two handlers (different user-plugin instances)
        let (handler1, _) = setup_handler(&db).await;
        let (handler2, _) = setup_handler(&db).await;

        // Set same key in both
        let set1 = make_request(
            "storage/set",
            Some(json!({"key": "shared_key", "data": {"owner": "user1"}})),
        );
        handler1.handle_request(&set1).await;

        let set2 = make_request(
            "storage/set",
            Some(json!({"key": "shared_key", "data": {"owner": "user2"}})),
        );
        handler2.handle_request(&set2).await;

        // Each should see their own data
        let get1 = make_request("storage/get", Some(json!({"key": "shared_key"})));
        let resp1 = handler1.handle_request(&get1).await;
        let data1: StorageGetResponse = serde_json::from_value(resp1.result.unwrap()).unwrap();
        assert_eq!(data1.data.unwrap(), json!({"owner": "user1"}));

        let get2 = make_request("storage/get", Some(json!({"key": "shared_key"})));
        let resp2 = handler2.handle_request(&get2).await;
        let data2: StorageGetResponse = serde_json::from_value(resp2.result.unwrap()).unwrap();
        assert_eq!(data2.data.unwrap(), json!({"owner": "user2"}));
    }

    #[tokio::test]
    async fn test_storage_upsert() {
        let db = setup_test_db().await;
        let (handler, _) = setup_handler(&db).await;

        // Set initial
        let set1 = make_request(
            "storage/set",
            Some(json!({"key": "version", "data": {"v": 1}})),
        );
        handler.handle_request(&set1).await;

        // Upsert
        let set2 = make_request(
            "storage/set",
            Some(json!({"key": "version", "data": {"v": 2}})),
        );
        handler.handle_request(&set2).await;

        // Verify updated
        let get = make_request("storage/get", Some(json!({"key": "version"})));
        let resp = handler.handle_request(&get).await;
        let result: StorageGetResponse = serde_json::from_value(resp.result.unwrap()).unwrap();
        assert_eq!(result.data.unwrap(), json!({"v": 2}));
    }

    #[tokio::test]
    async fn test_storage_list_empty() {
        let db = setup_test_db().await;
        let (handler, _) = setup_handler(&db).await;

        let list_req = make_request("storage/list", None);
        let resp = handler.handle_request(&list_req).await;
        assert!(!resp.is_error());
        let result: StorageListResponse = serde_json::from_value(resp.result.unwrap()).unwrap();
        assert!(result.keys.is_empty());
    }

    #[tokio::test]
    async fn test_response_has_correct_id() {
        let db = setup_test_db().await;
        let (handler, _) = setup_handler(&db).await;

        let request = JsonRpcRequest::new(42i64, "storage/get", Some(json!({"key": "test"})));
        let response = handler.handle_request(&request).await;
        assert_eq!(response.id, Some(RequestId::Number(42)));

        let request2 = JsonRpcRequest::new("abc".to_string(), "storage/list", None);
        let response2 = handler.handle_request(&request2).await;
        assert_eq!(response2.id, Some(RequestId::String("abc".to_string())));
    }
}
