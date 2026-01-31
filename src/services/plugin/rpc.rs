//! JSON-RPC Client for Plugin Communication
//!
//! This module provides a JSON-RPC client that communicates with plugins over stdio.
//!
//! Note: This module provides complete JSON-RPC client infrastructure.
//! Some methods may not be called from external code yet but are part of
//! the complete API for plugin RPC communication.

// Allow dead code for RPC client infrastructure that is part of the
// complete API surface but not yet fully integrated.
#![allow(dead_code)]

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;
use tokio::sync::{oneshot, Mutex};
use tokio::time::timeout;
use tracing::{debug, error, trace, warn};

use super::process::{PluginProcess, ProcessError};
use super::protocol::{
    error_codes, JsonRpcError, JsonRpcRequest, JsonRpcResponse, RequestId, JSONRPC_VERSION,
};

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
                response_reader_task(process, pending, process_alive).await;
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
            debug!(
                method = method,
                "Skipping RPC request - process is not alive"
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
                // Channel was closed (cancelled)
                debug!(id = id, "RPC request cancelled (channel closed)");
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

    /// Send a notification (no response expected)
    pub async fn notify<P>(&self, method: &str, params: P) -> Result<(), RpcError>
    where
        P: Serialize,
    {
        let params_value = serde_json::to_value(params)?;

        // Notifications don't have an id
        let request = serde_json::json!({
            "jsonrpc": JSONRPC_VERSION,
            "method": method,
            "params": params_value,
        });

        let request_json = serde_json::to_string(&request)?;
        trace!(method, "Sending RPC notification");

        let process = self.process.lock().await;
        process.write_line(&request_json).await?;
        Ok(())
    }

    /// Check if the underlying process is still running
    pub async fn is_running(&self) -> bool {
        // First check the fast atomic flag - if marked dead, don't bother checking process
        if !self.process_alive.load(Ordering::Acquire) {
            return false;
        }
        let mut process = self.process.lock().await;
        process.is_running()
    }

    /// Get the process ID
    pub async fn pid(&self) -> Option<u32> {
        let process = self.process.lock().await;
        process.pid()
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

/// Task that reads responses from the process and dispatches them
async fn response_reader_task(
    process: Arc<Mutex<PluginProcess>>,
    pending: Arc<Mutex<HashMap<i64, PendingRequest>>>,
    process_alive: Arc<AtomicBool>,
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

        // Log the response (truncate for readability, respecting UTF-8 char boundaries)
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

        // Parse the response
        let response: JsonRpcResponse = match serde_json::from_str(&line) {
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
    warn!(
        "Response reader task ending - marking plugin process as not alive to prevent EPIPE errors"
    );
    process_alive.store(false, Ordering::Release);

    // Process ended - cancel all pending requests
    let mut pending_map = pending.lock().await;
    let pending_count = pending_map.len();
    if pending_count > 0 {
        warn!(
            pending_count = pending_count,
            "Cancelling pending RPC requests due to plugin process exit"
        );
    }
    for (id, req) in pending_map.drain() {
        debug!("Cancelling pending request {} due to process exit", id);
        let _ = req
            .tx
            .send(Err(RpcError::Process(ProcessError::ProcessTerminated)));
    }
}

#[cfg(test)]
mod tests {
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
