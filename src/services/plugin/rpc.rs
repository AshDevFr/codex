//! JSON-RPC Client for Plugin Communication
//!
//! This module provides a JSON-RPC client that communicates with plugins over stdio.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::time::Duration;

use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;
use tokio::sync::{Mutex, oneshot};
use tokio::time::timeout;
use tracing::{debug, error, warn};

use super::process::{PluginProcess, ProcessError};
use super::protocol::{
    JSONRPC_VERSION, JsonRpcError, JsonRpcRequest, JsonRpcResponse, RequestId, error_codes,
};
use super::storage::is_storage_method;
use super::storage_handler::StorageRequestHandler;

/// Error type for RPC operations
#[derive(Debug, thiserror::Error)]
pub enum RpcError {
    #[error("Process error: {0}")]
    Process(#[from] ProcessError),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Request timed out after {0:?}")]
    Timeout(Duration),

    #[error("Plugin error: {message}")]
    PluginError {
        code: i32,
        message: String,
        data: Option<Value>,
    },

    #[error("Invalid response: {0}")]
    InvalidResponse(String),

    #[error("Request cancelled")]
    Cancelled,

    #[error("Rate limited: retry after {retry_after_seconds} seconds")]
    RateLimited { retry_after_seconds: u64 },

    #[error("Resource not found: {0}")]
    NotFound(String),

    #[error("Authentication failed: {0}")]
    AuthFailed(String),

    #[error("External API error: {0}")]
    ApiError(String),

    #[error("Plugin configuration error: {0}")]
    ConfigError(String),
}

impl From<JsonRpcError> for RpcError {
    fn from(err: JsonRpcError) -> Self {
        match err.code {
            error_codes::RATE_LIMITED => {
                let retry_after = err
                    .data
                    .as_ref()
                    .and_then(|d| d.get("retryAfterSeconds"))
                    .and_then(|v| v.as_u64())
                    .unwrap_or(60);
                RpcError::RateLimited {
                    retry_after_seconds: retry_after,
                }
            }
            error_codes::NOT_FOUND => RpcError::NotFound(err.message),
            error_codes::AUTH_FAILED => RpcError::AuthFailed(err.message),
            error_codes::API_ERROR => RpcError::ApiError(err.message),
            error_codes::CONFIG_ERROR => RpcError::ConfigError(err.message),
            _ => RpcError::PluginError {
                code: err.code,
                message: err.message,
                data: err.data,
            },
        }
    }
}

/// Pending request waiting for a response
struct PendingRequest {
    tx: oneshot::Sender<Result<Value, RpcError>>,
}

/// JSON-RPC client for communicating with a plugin process
pub struct RpcClient {
    /// The plugin process
    process: Arc<Mutex<PluginProcess>>,
    /// Next request ID
    next_id: AtomicI64,
    /// Pending requests waiting for responses
    pending: Arc<Mutex<HashMap<i64, PendingRequest>>>,
    /// Default request timeout
    default_timeout: Duration,
    /// Response reader task handle
    reader_handle: Option<tokio::task::JoinHandle<()>>,
    /// Flag indicating if the process is still alive.
    /// Set to false when the response reader task detects process termination.
    /// This prevents writing to a dead process, which would cause EPIPE errors.
    process_alive: Arc<AtomicBool>,
}

impl RpcClient {
    /// Create a new RPC client wrapping a plugin process
    pub fn new(process: PluginProcess, default_timeout: Duration) -> Self {
        Self::new_internal(process, default_timeout, None)
    }

    /// Create a new RPC client with storage request handling support.
    ///
    /// When a plugin sends a `storage/*` JSON-RPC request on its stdout,
    /// the reader task will intercept it, process it via the `StorageRequestHandler`,
    /// and write the response back to the plugin's stdin. This enables bidirectional
    /// RPC for user plugin storage operations.
    pub fn new_with_storage(
        process: PluginProcess,
        default_timeout: Duration,
        storage_handler: StorageRequestHandler,
    ) -> Self {
        Self::new_internal(process, default_timeout, Some(storage_handler))
    }

    fn new_internal(
        process: PluginProcess,
        default_timeout: Duration,
        storage_handler: Option<StorageRequestHandler>,
    ) -> Self {
        let process = Arc::new(Mutex::new(process));
        let pending: Arc<Mutex<HashMap<i64, PendingRequest>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let process_alive = Arc::new(AtomicBool::new(true));

        // Start the response reader task
        let reader_handle = {
            let process = Arc::clone(&process);
            let pending = Arc::clone(&pending);
            let process_alive = Arc::clone(&process_alive);
            tokio::spawn(async move {
                response_reader_task(process, pending, process_alive, storage_handler).await;
            })
        };

        Self {
            process,
            next_id: AtomicI64::new(1),
            pending,
            default_timeout,
            reader_handle: Some(reader_handle),
            process_alive,
        }
    }

    /// Send a request and wait for a response
    pub async fn call<P, R>(&self, method: &str, params: P) -> Result<R, RpcError>
    where
        P: Serialize,
        R: DeserializeOwned,
    {
        self.call_with_timeout(method, params, self.default_timeout)
            .await
    }

    /// Send a request and wait for a response with custom timeout
    pub async fn call_with_timeout<P, R>(
        &self,
        method: &str,
        params: P,
        request_timeout: Duration,
    ) -> Result<R, RpcError>
    where
        P: Serialize,
        R: DeserializeOwned,
    {
        // Check if the process is still alive before attempting to send.
        // This prevents EPIPE errors when trying to write to a dead process.
        if !self.process_alive.load(Ordering::Acquire) {
            error!(
                method = method,
                "RPC request failed - plugin process is not alive (terminated or crashed)"
            );
            return Err(RpcError::Process(ProcessError::ProcessTerminated));
        }

        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let params_value = serde_json::to_value(params)?;

        let request = JsonRpcRequest {
            jsonrpc: JSONRPC_VERSION.to_string(),
            id: RequestId::Number(id),
            method: method.to_string(),
            params: if params_value.is_null() {
                None
            } else {
                Some(params_value)
            },
        };

        let request_json = serde_json::to_string(&request)?;
        debug!(
            id = id,
            method = method,
            request_len = request_json.len(),
            "Sending RPC request"
        );

        // Create response channel
        let (tx, rx) = oneshot::channel();
        {
            let mut pending = self.pending.lock().await;
            pending.insert(id, PendingRequest { tx });
        }

        // Send request (double-check process is alive to handle race conditions)
        if !self.process_alive.load(Ordering::Acquire) {
            error!(
                id = id,
                method = method,
                "RPC request failed - plugin process died between check and send"
            );
            self.remove_pending(id).await;
            return Err(RpcError::Process(ProcessError::ProcessTerminated));
        }
        {
            let process = self.process.lock().await;
            process.write_line(&request_json).await?;
        }

        // Wait for response with timeout
        debug!(
            id = id,
            timeout_ms = request_timeout.as_millis(),
            "Waiting for RPC response"
        );
        let result = match timeout(request_timeout, rx).await {
            Ok(Ok(result)) => {
                debug!(id = id, "RPC response received");
                result
            }
            Ok(Err(_)) => {
                // Channel was closed - likely because the plugin process died
                // and the response reader task cancelled all pending requests
                error!(
                    id = id,
                    method = method,
                    "RPC request cancelled - response channel closed (plugin process may have died)"
                );
                self.remove_pending(id).await;
                return Err(RpcError::Cancelled);
            }
            Err(_) => {
                // Timeout
                error!(
                    id = id,
                    timeout_ms = request_timeout.as_millis(),
                    method = method,
                    "RPC request timed out"
                );
                self.remove_pending(id).await;
                return Err(RpcError::Timeout(request_timeout));
            }
        };

        // Parse the result
        let value = result?;
        debug!(id = id, "RPC response parsed successfully");
        let parsed: R = serde_json::from_value(value)?;
        Ok(parsed)
    }

    /// Send a request without parameters
    pub async fn call_no_params<R>(&self, method: &str) -> Result<R, RpcError>
    where
        R: DeserializeOwned,
    {
        self.call::<(), R>(method, ()).await
    }

    /// Shutdown the RPC client and kill the process
    pub async fn shutdown(&mut self, timeout_duration: Duration) -> Result<i32, ProcessError> {
        // Mark process as not alive immediately to prevent new requests
        self.process_alive.store(false, Ordering::Release);

        // Cancel the reader task
        if let Some(handle) = self.reader_handle.take() {
            handle.abort();
        }

        // Cancel all pending requests
        {
            let mut pending = self.pending.lock().await;
            for (_, req) in pending.drain() {
                let _ = req.tx.send(Err(RpcError::Cancelled));
            }
        }

        // Shutdown the process
        let mut process = self.process.lock().await;
        process.shutdown(timeout_duration).await
    }

    /// Remove a pending request
    async fn remove_pending(&self, id: i64) {
        let mut pending = self.pending.lock().await;
        pending.remove(&id);
    }
}

impl Drop for RpcClient {
    fn drop(&mut self) {
        // Abort the reader task to release its Arc<Mutex<PluginProcess>> reference.
        // Without this, the reader task keeps the Arc alive indefinitely, preventing
        // the PluginProcess from being dropped and its kill_on_drop(true) from firing.
        // This is the fix for the plugin process leak bug.
        if let Some(handle) = self.reader_handle.take() {
            handle.abort();
        }
    }
}

/// Task that reads lines from the plugin process and dispatches them.
///
/// Handles two types of messages:
/// 1. **Responses**: Lines with `result` or `error` → dispatched to pending requests
/// 2. **Reverse RPC requests**: Lines with `method` (e.g., `storage/*`) → handled by
///    the storage handler and response written back to the plugin's stdin
async fn response_reader_task(
    process: Arc<Mutex<PluginProcess>>,
    pending: Arc<Mutex<HashMap<i64, PendingRequest>>>,
    process_alive: Arc<AtomicBool>,
    storage_handler: Option<StorageRequestHandler>,
) {
    debug!("RPC response reader task started");
    loop {
        // Acquire lock briefly and use timeout to prevent holding lock while waiting
        // This allows write operations to acquire the lock between read attempts
        let line = {
            let mut process = process.lock().await;
            match tokio::time::timeout(Duration::from_millis(100), process.read_line()).await {
                Ok(Ok(line)) => Some(line),
                Ok(Err(e)) => {
                    warn!(
                        error = %e,
                        "Response reader stopping due to read error - plugin process may have crashed"
                    );
                    break;
                }
                Err(_) => None, // Timeout - release lock and retry
            }
        };

        // If timeout, loop to try again (releases lock first)
        let line = match line {
            Some(l) => l,
            None => continue,
        };

        if line.is_empty() {
            continue;
        }

        // Log the line (truncate for readability, respecting UTF-8 char boundaries)
        let log_preview = if line.len() > 200 {
            // Find a valid UTF-8 char boundary at or before position 200
            let mut end = 200;
            while end > 0 && !line.is_char_boundary(end) {
                end -= 1;
            }
            format!("{}...", &line[..end])
        } else {
            line.clone()
        };
        debug!(bytes = line.len(), preview = %log_preview, "Received line from plugin");

        // Parse as generic JSON to determine if it's a request or response
        let json_value: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(e) => {
                warn!(error = %e, line = %line, "Failed to parse plugin output as JSON");
                continue;
            }
        };

        // Check if this is a reverse RPC request from the plugin (has "method" field)
        let is_request = json_value
            .get("method")
            .and_then(|m| m.as_str())
            .map(|m| m.to_string());

        if let Some(method) = is_request {
            if is_storage_method(&method) {
                if let Some(ref handler) = storage_handler {
                    // Parse as a full request
                    let request: JsonRpcRequest = match serde_json::from_value(json_value) {
                        Ok(r) => r,
                        Err(e) => {
                            warn!(error = %e, "Failed to parse storage request");
                            continue;
                        }
                    };

                    debug!(method = %method, "Handling reverse RPC storage request from plugin");
                    let response = handler.handle_request(&request).await;

                    // Write the response back to the plugin's stdin
                    let response_json = match serde_json::to_string(&response) {
                        Ok(j) => j,
                        Err(e) => {
                            error!(error = %e, "Failed to serialize storage response");
                            continue;
                        }
                    };

                    let process = process.lock().await;
                    if let Err(e) = process.write_line(&response_json).await {
                        error!(error = %e, "Failed to write storage response to plugin");
                    }
                } else {
                    warn!(
                        method = %method,
                        "Plugin sent storage request but no storage handler is configured"
                    );
                    // Send error response back to plugin
                    if let Ok(request) = serde_json::from_value::<JsonRpcRequest>(json_value) {
                        let error_response = JsonRpcResponse::error(
                            Some(request.id),
                            JsonRpcError::new(
                                error_codes::METHOD_NOT_FOUND,
                                "Storage is not available for this plugin",
                            ),
                        );
                        if let Ok(resp_json) = serde_json::to_string(&error_response) {
                            let process = process.lock().await;
                            let _ = process.write_line(&resp_json).await;
                        }
                    }
                }
                continue;
            }
            // Non-storage methods from the plugin are not supported
            warn!(method = %method, "Plugin sent unsupported reverse RPC request");
            continue;
        }

        // Check if the raw JSON has a "result" key before deserialization.
        // JSON-RPC allows `"result": null` as a valid success response, but
        // serde deserializes `null` into `None` for `Option<Value>`, making it
        // indistinguishable from a missing field. We track this explicitly.
        let has_result_key = json_value
            .as_object()
            .is_some_and(|obj| obj.contains_key("result"));

        // Normal response processing
        let response: JsonRpcResponse = match serde_json::from_value(json_value) {
            Ok(r) => r,
            Err(e) => {
                warn!(error = %e, line = %line, "Failed to parse plugin response as JSON-RPC");
                continue;
            }
        };

        // Get the request ID
        let id = match &response.id {
            Some(RequestId::Number(id)) => *id,
            Some(RequestId::String(id)) => match id.parse::<i64>() {
                Ok(id) => id,
                Err(_) => {
                    warn!("Invalid string request ID: {}", id);
                    continue;
                }
            },
            None => {
                // This is a notification or error without ID
                if let Some(err) = response.error {
                    error!(
                        "Plugin error without request ID: {} (code: {})",
                        err.message, err.code
                    );
                }
                continue;
            }
        };

        // Find and complete the pending request
        let pending_req = {
            let mut pending_map = pending.lock().await;
            pending_map.remove(&id)
        };

        if let Some(req) = pending_req {
            let result = if let Some(err) = response.error {
                Err(RpcError::from(err))
            } else if let Some(result) = response.result {
                Ok(result)
            } else if has_result_key {
                // "result": null is a valid JSON-RPC success response
                Ok(Value::Null)
            } else {
                Err(RpcError::InvalidResponse(
                    "Response has neither result nor error".to_string(),
                ))
            };

            if req.tx.send(result).is_err() {
                debug!("Request {} receiver dropped", id);
            }
        } else {
            warn!("Received response for unknown request ID: {}", id);
        }
    }

    // Mark the process as no longer alive.
    // This prevents new requests from being sent to the dead process,
    // which would cause EPIPE errors.
    error!("Plugin process ended unexpectedly - marking as not alive to prevent further requests");
    process_alive.store(false, Ordering::Release);

    // Process ended - cancel all pending requests
    let mut pending_map = pending.lock().await;
    let pending_count = pending_map.len();
    if pending_count > 0 {
        error!(
            pending_count = pending_count,
            "Cancelling {} pending RPC requests due to plugin process exit - these tasks will fail",
            pending_count
        );
    }
    for (id, req) in pending_map.drain() {
        error!(
            request_id = id,
            "Cancelling pending request due to plugin process exit"
        );
        let _ = req
            .tx
            .send(Err(RpcError::Process(ProcessError::ProcessTerminated)));
    }
}

#[cfg(test)]
mod tests {
    use super::super::process::PluginProcessConfig;
    use super::*;
    use serde_json::json;

    // Helper to create a mock plugin script (used in integration tests)
    #[allow(dead_code)]
    fn create_mock_plugin_script() -> String {
        // This is a simple Node.js script that echoes requests
        // In real tests, we'd use a proper mock plugin
        r#"
        const readline = require('readline');
        const rl = readline.createInterface({ input: process.stdin });

        rl.on('line', (line) => {
            try {
                const request = JSON.parse(line);
                let response;

                if (request.method === 'initialize') {
                    response = {
                        jsonrpc: '2.0',
                        id: request.id,
                        result: {
                            name: 'test-plugin',
                            displayName: 'Test Plugin',
                            version: '1.0.0',
                            protocolVersion: '1.0',
                            capabilities: { metadataProvider: ['series'] }
                        }
                    };
                } else if (request.method === 'ping') {
                    response = {
                        jsonrpc: '2.0',
                        id: request.id,
                        result: 'pong'
                    };
                } else if (request.method === 'echo') {
                    response = {
                        jsonrpc: '2.0',
                        id: request.id,
                        result: request.params
                    };
                } else {
                    response = {
                        jsonrpc: '2.0',
                        id: request.id,
                        error: { code: -32601, message: 'Method not found' }
                    };
                }

                console.log(JSON.stringify(response));
            } catch (e) {
                console.log(JSON.stringify({
                    jsonrpc: '2.0',
                    id: null,
                    error: { code: -32700, message: 'Parse error' }
                }));
            }
        });
        "#
        .to_string()
    }

    #[test]
    fn test_rpc_error_from_json_error() {
        let err = JsonRpcError::new(error_codes::NOT_FOUND, "Series not found");
        let rpc_err = RpcError::from(err);
        assert!(matches!(rpc_err, RpcError::NotFound(_)));
    }

    #[test]
    fn test_rpc_error_rate_limited() {
        let err = JsonRpcError::with_data(
            error_codes::RATE_LIMITED,
            "Rate limited",
            json!({"retryAfterSeconds": 120}),
        );
        let rpc_err = RpcError::from(err);
        match rpc_err {
            RpcError::RateLimited {
                retry_after_seconds,
            } => {
                assert_eq!(retry_after_seconds, 120);
            }
            _ => panic!("Expected RateLimited error"),
        }
    }

    #[test]
    fn test_rpc_error_auth_failed() {
        let err = JsonRpcError::new(error_codes::AUTH_FAILED, "Invalid API key");
        let rpc_err = RpcError::from(err);
        assert!(matches!(rpc_err, RpcError::AuthFailed(_)));
    }

    #[test]
    fn test_rpc_error_api_error() {
        let err = JsonRpcError::new(error_codes::API_ERROR, "API error: 400 Bad Request");
        let rpc_err = RpcError::from(err);
        match rpc_err {
            RpcError::ApiError(msg) => {
                assert_eq!(msg, "API error: 400 Bad Request");
            }
            _ => panic!("Expected ApiError"),
        }
    }

    #[test]
    fn test_rpc_error_config_error() {
        let err = JsonRpcError::new(error_codes::CONFIG_ERROR, "API key is required");
        let rpc_err = RpcError::from(err);
        match rpc_err {
            RpcError::ConfigError(msg) => {
                assert_eq!(msg, "API key is required");
            }
            _ => panic!("Expected ConfigError"),
        }
    }

    /// Verify that dropping an RpcClient aborts the reader task, releasing the
    /// Arc<Mutex<PluginProcess>> so kill_on_drop(true) can fire on the child process.
    #[tokio::test]
    async fn test_rpc_client_drop_aborts_reader_task() {
        // Use `cat` as a trivial long-running process (reads stdin forever).
        // We use spawn_unchecked to bypass the OnceLock-cached allowlist, which
        // can't be modified at runtime and causes flaky failures when other tests
        // initialize it first without `cat` in the list.
        let config = PluginProcessConfig::new("cat");
        let process = PluginProcess::spawn_unchecked(&config).await.unwrap();

        // Create RpcClient — this spawns the reader task
        let client = RpcClient::new(process, Duration::from_secs(5));

        // Grab a clone of the reader task's Arc to verify it gets released
        let process_arc = Arc::clone(&client.process);

        // Before drop: the Arc has at least 2 strong refs (client + reader task)
        assert!(Arc::strong_count(&process_arc) >= 2);

        // Drop the client — this should abort the reader task
        drop(client);

        // Give the abort a moment to propagate
        tokio::time::sleep(Duration::from_millis(50)).await;

        // After drop: only our local clone of the Arc should remain
        assert_eq!(
            Arc::strong_count(&process_arc),
            1,
            "Reader task should have been aborted, releasing its Arc reference"
        );
    }

    // Integration test with actual process would look like:
    // #[tokio::test]
    // async fn test_rpc_client_integration() {
    //     // This would require Node.js to be installed
    //     // Skip if not available
    //     if std::process::Command::new("node").arg("--version").status().is_err() {
    //         return;
    //     }
    //
    //     // Create temp file with mock plugin script
    //     let script = create_mock_plugin_script();
    //     let temp_dir = tempfile::tempdir().unwrap();
    //     let script_path = temp_dir.path().join("mock-plugin.js");
    //     std::fs::write(&script_path, script).unwrap();
    //
    //     let config = PluginProcessConfig::new("node")
    //         .arg(script_path.to_str().unwrap());
    //
    //     let process = PluginProcess::spawn(&config).await.unwrap();
    //     let mut client = RpcClient::new(process, Duration::from_secs(5));
    //
    //     // Test ping
    //     let pong: String = client.call_no_params("ping").await.unwrap();
    //     assert_eq!(pong, "pong");
    //
    //     // Test echo
    //     let echoed: Value = client.call("echo", json!({"test": "data"})).await.unwrap();
    //     assert_eq!(echoed["test"], "data");
    //
    //     // Cleanup
    //     client.shutdown(Duration::from_secs(1)).await.unwrap();
    // }
}
