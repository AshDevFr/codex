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
use tokio::sync::{Mutex, Notify, RwLock, mpsc};
use tokio::time::timeout;
use tracing::{Instrument, debug, error, warn};

use super::permissions::{self, PermissionError};
use super::process::{PluginProcess, ProcessError};
use super::protocol::{
    JSONRPC_VERSION, JsonRpcError, JsonRpcRequest, JsonRpcResponse, PluginCapabilities, RequestId,
    error_codes,
};
use super::releases_handler::{ReleasesRequestHandler, is_releases_method};
use super::storage::is_storage_method;
use super::storage_handler::StorageRequestHandler;

/// Bag of handlers + capabilities that mediate plugin reverse-RPC calls.
///
/// Constructed before the plugin starts, but the capability snapshot and the
/// release-source handler are filled in once `initialize` returns and the
/// host knows what the plugin can do. The reader task holds an `Arc<RwLock>`
/// to this struct so updates land without restarting the task.
pub struct ReverseRpcContext {
    storage_handler: Option<StorageRequestHandler>,
    releases_handler: Option<ReleasesRequestHandler>,
    /// `None` until the plugin has been initialized.
    capabilities: Option<PluginCapabilities>,
    /// Notified once `capabilities` is populated. Lets the dispatcher park an
    /// early reverse-RPC call (one that raced ahead of the host installing the
    /// post-`initialize` context) until the plugin is ready, instead of
    /// bouncing it with `METHOD_NOT_FOUND` and relying on plugin-side retries.
    ready: Arc<Notify>,
}

impl ReverseRpcContext {
    pub fn new() -> Self {
        Self {
            storage_handler: None,
            releases_handler: None,
            capabilities: None,
            ready: Arc::new(Notify::new()),
        }
    }

    pub fn with_storage(storage_handler: StorageRequestHandler) -> Self {
        Self {
            storage_handler: Some(storage_handler),
            releases_handler: None,
            capabilities: None,
            ready: Arc::new(Notify::new()),
        }
    }

    /// Replace the plugin's capability snapshot, used by [`super::handle::PluginHandle`]
    /// once `initialize` returns. Wakes any reverse-RPC calls parked in the
    /// readiness barrier (see [`await_capabilities`]).
    pub fn set_capabilities(&mut self, caps: PluginCapabilities) {
        self.capabilities = Some(caps);
        self.ready.notify_waiters();
    }

    /// Install the releases handler. Called after capabilities are known
    /// and the plugin declared `release_source`.
    pub fn set_releases_handler(&mut self, handler: ReleasesRequestHandler) {
        self.releases_handler = Some(handler);
    }

    /// Whether the post-`initialize` capability snapshot has been installed.
    ///
    /// A reverse-RPC that arrives while this is `false` is racing `initialize`:
    /// the plugin fired it from `onInitialize` before the host processed the
    /// `initialize` response that installs capabilities. Such calls must not be
    /// dispatched inline on the task awaiting that response (see
    /// [`dispatch_reverse_rpc_frame`]).
    pub fn capabilities_ready(&self) -> bool {
        self.capabilities.is_some()
    }
}

impl Default for ReverseRpcContext {
    fn default() -> Self {
        Self::new()
    }
}

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

/// Frame delivered from the response reader to a pending forward call.
///
/// Forward calls await an `mpsc::Receiver<PendingFrame>` instead of a single
/// `oneshot::Receiver`. The reader pushes either:
/// - one `Response` (terminal — the receiver loop stops), or
/// - zero or more `ReverseRpc` frames (mid-flight — the caller dispatches
///   each one on its own tokio task and writes the response back to the
///   plugin), followed eventually by exactly one `Response`.
///
/// Routing reverse-RPCs back to the caller (instead of dispatching them on
/// the reader task) is what lets task-local context — most importantly the
/// recording broadcaster set up by [`crate::tasks::worker`] — propagate into
/// the dispatcher. Without this, events emitted by reverse-RPC handlers
/// (like `releases/record`) would have no recording context and would never
/// reach the web server's SSE stream in distributed deployments.
enum PendingFrame {
    /// The plugin returned a response for this forward call. Terminal.
    Response(Result<Value, RpcError>),
    /// The plugin made a reverse-RPC call while servicing this forward
    /// call. The caller must dispatch and write the response back.
    ReverseRpc(JsonRpcRequest),
}

/// Pending request waiting for a response
struct PendingRequest {
    tx: mpsc::UnboundedSender<PendingFrame>,
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
    /// Reverse-RPC handlers + capability snapshot, mutable after init.
    reverse_ctx: Arc<RwLock<ReverseRpcContext>>,
}

impl RpcClient {
    /// Create a new RPC client wrapping a plugin process
    pub fn new(process: PluginProcess, default_timeout: Duration) -> Self {
        Self::new_internal(process, default_timeout, ReverseRpcContext::new())
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
        Self::new_internal(
            process,
            default_timeout,
            ReverseRpcContext::with_storage(storage_handler),
        )
    }

    fn new_internal(
        process: PluginProcess,
        default_timeout: Duration,
        ctx: ReverseRpcContext,
    ) -> Self {
        let process = Arc::new(Mutex::new(process));
        let pending: Arc<Mutex<HashMap<i64, PendingRequest>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let process_alive = Arc::new(AtomicBool::new(true));
        let reverse_ctx = Arc::new(RwLock::new(ctx));

        // Start the response reader task
        let reader_handle = {
            let process = Arc::clone(&process);
            let pending = Arc::clone(&pending);
            let process_alive = Arc::clone(&process_alive);
            let reverse_ctx = Arc::clone(&reverse_ctx);
            tokio::spawn(async move {
                response_reader_task(process, pending, process_alive, reverse_ctx).await;
            })
        };

        Self {
            process,
            next_id: AtomicI64::new(1),
            pending,
            default_timeout,
            reader_handle: Some(reader_handle),
            process_alive,
            reverse_ctx,
        }
    }

    /// Update the reverse-RPC context after initialization. Used by
    /// [`super::handle::PluginHandle`] to inject the capability snapshot and
    /// install the releases handler once the manifest is known.
    pub async fn update_reverse_ctx<F>(&self, f: F)
    where
        F: FnOnce(&mut ReverseRpcContext),
    {
        let mut ctx = self.reverse_ctx.write().await;
        f(&mut ctx);
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

    /// Send a request and wait for a response with custom timeout.
    ///
    /// While awaiting the response, this also services any reverse-RPC
    /// requests the plugin makes that are tagged with `parent_request_id =
    /// id` of this call. Dispatching here (rather than on the reader task)
    /// keeps the dispatch on the caller's tokio task, so task-local state
    /// (notably the recording broadcaster set by the worker) propagates into
    /// the reverse-RPC handlers — see [`PendingFrame`] for context.
    ///
    /// The `request_timeout` bounds *the entire forward call*, including
    /// any reverse-RPC servicing in between. That matches the previous
    /// semantics from the caller's point of view.
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
            parent_request_id: None,
        };

        let request_json = serde_json::to_string(&request)?;
        debug!(
            id = id,
            method = method,
            request_len = request_json.len(),
            "Sending RPC request"
        );

        // Create response channel. Unbounded because reverse-RPCs are
        // dispatched inline and the queue depth is naturally bounded by the
        // plugin's behavior; bounding it would risk deadlock if the plugin
        // bursts reverse-RPCs faster than the caller drains them.
        let (tx, mut rx) = mpsc::unbounded_channel::<PendingFrame>();
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
        // Span around the stdio write so its duration is attributable
        // separately from waiting for the response. Most calls spend
        // microseconds here; a slow write usually means a wedged plugin.
        async {
            let process = self.process.lock().await;
            process.write_line(&request_json).await
        }
        .instrument(tracing::info_span!(
            "plugin.rpc.write",
            otel.kind = "internal",
            rpc.id = id,
            rpc.method = method,
            request_len = request_json.len(),
        ))
        .await?;

        // Loop, servicing reverse-RPC frames until the response frame
        // arrives or we time out. Dispatching reverse-RPCs here (on the
        // caller's task) is what lets task-local recording broadcasters
        // propagate into the handlers — see [`PendingFrame`].
        debug!(
            id = id,
            timeout_ms = request_timeout.as_millis(),
            "Waiting for RPC response"
        );
        let wait_span = tracing::info_span!(
            "plugin.rpc.wait",
            otel.kind = "internal",
            rpc.id = id,
            rpc.method = method,
            timeout_ms = request_timeout.as_millis() as u64,
        );
        let response_result = timeout(request_timeout, async {
            loop {
                match rx.recv().await {
                    Some(PendingFrame::Response(result)) => return Ok::<_, RpcError>(result),
                    Some(PendingFrame::ReverseRpc(reverse_request)) => {
                        // Service the reverse-RPC. When the plugin is already
                        // initialized this dispatches inline on the caller's
                        // task so task-locals propagate; when it's still racing
                        // `initialize` it spawns, so this loop stays free to
                        // receive the `initialize` response that installs the
                        // capabilities the reverse-RPC is waiting on. See
                        // [`dispatch_reverse_rpc_frame`].
                        let reverse_method = reverse_request.method.clone();
                        dispatch_reverse_rpc_frame(
                            reverse_request,
                            reverse_method,
                            Arc::clone(&self.reverse_ctx),
                            Arc::clone(&self.process),
                        )
                        .await;
                    }
                    None => {
                        // Channel closed — plugin process died and the
                        // reader cancelled all pending requests.
                        return Err(RpcError::Cancelled);
                    }
                }
            }
        })
        .instrument(wait_span)
        .await;

        let result = match response_result {
            Ok(Ok(result)) => {
                debug!(id = id, "RPC response received");
                result
            }
            Ok(Err(RpcError::Cancelled)) => {
                error!(
                    id = id,
                    method = method,
                    "RPC request cancelled - response channel closed (plugin process may have died)"
                );
                self.remove_pending(id).await;
                return Err(RpcError::Cancelled);
            }
            Ok(Err(e)) => {
                self.remove_pending(id).await;
                return Err(e);
            }
            Err(_) => {
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
                let _ = req
                    .tx
                    .send(PendingFrame::Response(Err(RpcError::Cancelled)));
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

/// How long a reverse-RPC call parks waiting for the plugin's capabilities to
/// be installed before giving up with `METHOD_NOT_FOUND`. Real initialization
/// completes in milliseconds; this is a generous upper bound for a process
/// under load. Only calls that race `initialize` ever wait at all.
const REVERSE_RPC_READINESS_TIMEOUT: Duration = Duration::from_secs(5);

/// Park until the plugin's capabilities are installed (`set_capabilities`), or
/// `timeout` elapses. Returns immediately when capabilities are already set
/// (the common case). This closes the startup race where a plugin fires a
/// reverse-RPC from `onInitialize` before the host installs the post-`initialize`
/// context, without requiring plugins to retry on `METHOD_NOT_FOUND`.
async fn await_capabilities(reverse_ctx: &Arc<RwLock<ReverseRpcContext>>, timeout: Duration) {
    // Fast path + grab the notify handle without holding the read lock across
    // the await below.
    let ready = {
        let guard = reverse_ctx.read().await;
        if guard.capabilities.is_some() {
            return;
        }
        guard.ready.clone()
    };

    let sleep = tokio::time::sleep(timeout);
    tokio::pin!(sleep);

    loop {
        // Register interest *before* re-checking the condition so a notify that
        // fires between the check and the await isn't lost (the canonical
        // `tokio::sync::Notify` pattern, via `enable()`).
        let notified = ready.notified();
        tokio::pin!(notified);
        notified.as_mut().enable();

        if reverse_ctx.read().await.capabilities.is_some() {
            return;
        }

        tokio::select! {
            _ = &mut sleep => return,
            _ = &mut notified => { /* woken — re-check on the next iteration */ }
        }
    }
}

/// Dispatch a single reverse-RPC request to the appropriate handler after
/// running the permission check.
///
/// Permission failures map to:
/// - `Denied` → `AUTH_FAILED` (the plugin called a method it isn't allowed
///   to call; tracing-friendly).
/// - `UnknownMethod` → `METHOD_NOT_FOUND` (no mapping; either a typo or the
///   method belongs to a future namespace).
async fn dispatch_reverse_rpc(
    method: &str,
    request: &JsonRpcRequest,
    reverse_ctx: &Arc<RwLock<ReverseRpcContext>>,
    readiness_timeout: Duration,
) -> JsonRpcResponse {
    let request_id = request.id.clone();

    // Readiness barrier. A plugin can fire a reverse-RPC (e.g. a release
    // source's `register_sources`) from its `onInitialize` the instant after
    // it returns the manifest — before the host has finished installing the
    // post-`initialize` context (capabilities + handlers). Rather than bounce
    // that call and depend on plugin-side retries, park here until the
    // capabilities are installed or the timeout elapses.
    await_capabilities(reverse_ctx, readiness_timeout).await;

    // Take a read snapshot of the context. We keep it as long as we're
    // dispatching so the handlers don't get swapped mid-call.
    let ctx_guard = reverse_ctx.read().await;

    // 1. Permission check. If capabilities are *still* unset after the
    //    readiness wait (initialization stalled or failed), we return
    //    METHOD_NOT_FOUND rather than AUTH_FAILED. From the plugin's
    //    perspective the method isn't dispatchable *yet* — distinguishing this
    //    from a real permission denial lets a plugin SDK retry with backoff as
    //    a last resort. AUTH_FAILED stays reserved for actual
    //    capability-declined-method denials.
    let caps = match ctx_guard.capabilities.as_ref() {
        Some(c) => c,
        None => {
            warn!(
                method = %method,
                "Reverse-RPC call still uninitialized after readiness wait; deferring (METHOD_NOT_FOUND)"
            );
            return JsonRpcResponse::error(
                Some(request_id),
                JsonRpcError::new(
                    error_codes::METHOD_NOT_FOUND,
                    "plugin not initialized; capabilities unknown",
                ),
            );
        }
    };

    if let Err(err) = permissions::enforce(method, caps) {
        match &err {
            PermissionError::Denied { required, .. } => {
                warn!(method = %method, required = ?required, "Permission denied for reverse-RPC call");
                return JsonRpcResponse::error(
                    Some(request_id),
                    JsonRpcError::new(error_codes::AUTH_FAILED, err.to_string()),
                );
            }
            PermissionError::UnknownMethod { .. } => {
                warn!(method = %method, "Unknown reverse-RPC method");
                return JsonRpcResponse::error(
                    Some(request_id),
                    JsonRpcError::new(error_codes::METHOD_NOT_FOUND, err.to_string()),
                );
            }
        }
    }

    // 2. Route to the matching handler.
    if is_storage_method(method) {
        match ctx_guard.storage_handler.as_ref() {
            Some(handler) => {
                debug!(method = %method, "Routing to storage handler");
                handler.handle_request(request).await
            }
            None => {
                warn!(method = %method, "Storage method called but no storage handler installed");
                JsonRpcResponse::error(
                    Some(request_id),
                    JsonRpcError::new(
                        error_codes::METHOD_NOT_FOUND,
                        "Storage is not available for this plugin",
                    ),
                )
            }
        }
    } else if is_releases_method(method) {
        match ctx_guard.releases_handler.as_ref() {
            Some(handler) => {
                debug!(method = %method, "Routing to releases handler");
                handler.handle_request(request).await
            }
            None => {
                warn!(method = %method, "Releases method called but no releases handler installed");
                JsonRpcResponse::error(
                    Some(request_id),
                    JsonRpcError::new(
                        error_codes::INTERNAL_ERROR,
                        "Releases handler not configured",
                    ),
                )
            }
        }
    } else {
        // Permission check passed but no handler match — should be
        // unreachable if the permissions table and handler set agree.
        warn!(method = %method, "Permission-allowed method has no handler routing");
        JsonRpcResponse::error(
            Some(request_id),
            JsonRpcError::new(
                error_codes::METHOD_NOT_FOUND,
                format!("No handler for method `{}`", method),
            ),
        )
    }
}

/// Task that reads lines from the plugin process and routes them.
///
/// Handles three categories of message:
/// 1. **Responses**: Lines with `result` or `error` → routed to the matching
///    pending caller via [`PendingFrame::Response`].
/// 2. **Reverse-RPC requests with a `parentRequestId`**: routed to the
///    pending caller of that forward call via [`PendingFrame::ReverseRpc`].
///    The caller dispatches on its own tokio task so task-locals propagate.
/// 3. **Reverse-RPC requests without a `parentRequestId`** (legacy plugins
///    that predate the field, or true orphans): dispatched on the reader
///    task as before. These won't have a recording broadcaster in scope and
///    won't replay in distributed deployments — but that's no regression
///    from the prior behavior.
async fn response_reader_task(
    process: Arc<Mutex<PluginProcess>>,
    pending: Arc<Mutex<HashMap<i64, PendingRequest>>>,
    process_alive: Arc<AtomicBool>,
    reverse_ctx: Arc<RwLock<ReverseRpcContext>>,
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
            let request: JsonRpcRequest = match serde_json::from_value(json_value) {
                Ok(r) => r,
                Err(e) => {
                    warn!(error = %e, method = %method, "Failed to parse reverse-RPC request");
                    continue;
                }
            };

            // Try to route to the originating forward call so dispatch
            // happens on the caller's task (and task-locals propagate).
            let parent_id = request
                .parent_request_id
                .as_ref()
                .and_then(parent_id_to_i64);

            if let Some(parent_id) = parent_id {
                let routed = {
                    let pending_map = pending.lock().await;
                    pending_map.get(&parent_id).map(|p| p.tx.clone())
                };
                if let Some(tx) = routed {
                    if let Err(send_err) = tx.send(PendingFrame::ReverseRpc(request)) {
                        // Receiver dropped between lookup and send — race
                        // with timeout/shutdown. Fall back to dispatching
                        // on the reader so the plugin still gets a response.
                        let dropped = match send_err.0 {
                            PendingFrame::ReverseRpc(req) => req,
                            // Unreachable: we just constructed a ReverseRpc
                            // frame above, and `send` returns whatever it
                            // failed to deliver.
                            PendingFrame::Response(_) => continue,
                        };
                        warn!(
                            method = %method,
                            parent_id = parent_id,
                            "Caller dropped pending channel; falling back to reader-task dispatch"
                        );
                        dispatch_reverse_rpc_frame(
                            dropped,
                            method.clone(),
                            Arc::clone(&reverse_ctx),
                            Arc::clone(&process),
                        )
                        .await;
                    }
                    continue;
                }
                warn!(
                    method = %method,
                    parent_id = parent_id,
                    "Reverse-RPC parent request id not found in pending map; dispatching on reader"
                );
            }

            // No parent id, or parent not pending: dispatch on the reader
            // (spawning if the plugin isn't initialized yet, so the reader
            // keeps draining stdout).
            dispatch_reverse_rpc_frame(
                request,
                method,
                Arc::clone(&reverse_ctx),
                Arc::clone(&process),
            )
            .await;
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

            if req.tx.send(PendingFrame::Response(result)).is_err() {
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
        let _ = req.tx.send(PendingFrame::Response(Err(RpcError::Process(
            ProcessError::ProcessTerminated,
        ))));
    }
}

/// Coerce a reverse-RPC `parentRequestId` to the `i64` we use as our
/// pending-map key. Numbers map directly; strings parse as numbers (the host
/// only ever issues numeric ids, but the field type is `RequestId` for
/// protocol generality).
fn parent_id_to_i64(id: &RequestId) -> Option<i64> {
    match id {
        RequestId::Number(n) => Some(*n),
        RequestId::String(s) => s.parse::<i64>().ok(),
    }
}

/// Dispatch a reverse-RPC, choosing inline vs. spawned execution based on
/// whether the plugin has finished initializing.
///
/// - **Capabilities installed** (the steady state — e.g. an `announce` during
///   `poll`): dispatch inline on the current task so task-local context (the
///   recording broadcaster from [`crate::tasks::worker`]) propagates into the
///   handler.
/// - **Capabilities not yet installed**: the reverse-RPC is racing
///   `initialize` — the plugin fired it from `onInitialize` before the host
///   processed the `initialize` response that installs those capabilities.
///   Dispatching inline would park the readiness barrier on the very task that
///   must drain that response (the forward-call loop, or the reader), a
///   guaranteed deadlock until the readiness timeout (observed as a ~5s connect
///   stall plus a failed `register_sources`). Spawn instead so that task stays
///   free. No recording broadcaster is ever in scope during `initialize`, so
///   spawning loses nothing.
async fn dispatch_reverse_rpc_frame(
    request: JsonRpcRequest,
    method: String,
    reverse_ctx: Arc<RwLock<ReverseRpcContext>>,
    process: Arc<Mutex<PluginProcess>>,
) {
    let ready = reverse_ctx.read().await.capabilities_ready();
    if ready {
        dispatch_and_write(request, method, reverse_ctx, process).await;
    } else {
        tokio::spawn(dispatch_and_write(request, method, reverse_ctx, process));
    }
}

/// Dispatch a reverse-RPC on the *current* task and write the response back
/// to the plugin. Used as the fallback when no parent forward call is
/// available to dispatch on (legacy plugins, or the parent's caller has
/// already gone away).
async fn dispatch_and_write(
    request: JsonRpcRequest,
    method: String,
    reverse_ctx: Arc<RwLock<ReverseRpcContext>>,
    process: Arc<Mutex<PluginProcess>>,
) {
    let request_id = request.id.clone();
    let response = dispatch_reverse_rpc(
        &method,
        &request,
        &reverse_ctx,
        REVERSE_RPC_READINESS_TIMEOUT,
    )
    .await;
    let response_json = match serde_json::to_string(&response) {
        Ok(j) => j,
        Err(e) => {
            error!(error = %e, method = %method, "Failed to serialize reverse-RPC response");
            let fallback = JsonRpcResponse::error(
                Some(request_id),
                JsonRpcError::new(error_codes::INTERNAL_ERROR, "failed to serialize response"),
            );
            serde_json::to_string(&fallback).unwrap_or_default()
        }
    };
    let process_guard = process.lock().await;
    if let Err(e) = process_guard.write_line(&response_json).await {
        error!(error = %e, method = %method, "Failed to write reverse-RPC response to plugin");
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

    /// Reverse-RPC dispatch parks on the readiness barrier when capabilities
    /// aren't installed, then — if they never arrive within the timeout —
    /// rejects with `METHOD_NOT_FOUND` (rather than `AUTH_FAILED`). The short
    /// timeout here exercises the give-up path; `test_dispatch_waits_for_late_capabilities`
    /// covers the success path where caps land mid-wait.
    #[tokio::test]
    async fn test_dispatch_rejects_before_init() {
        let ctx = Arc::new(RwLock::new(ReverseRpcContext::new()));
        let request = JsonRpcRequest::new(
            1i64,
            super::super::protocol::methods::STORAGE_GET,
            Some(json!({"key": "x"})),
        );
        let resp =
            dispatch_reverse_rpc(&request.method, &request, &ctx, Duration::from_millis(50)).await;
        assert!(resp.is_error());
        assert_eq!(resp.error.unwrap().code, error_codes::METHOD_NOT_FOUND);
    }

    /// The readiness barrier: a reverse-RPC that arrives before the host has
    /// installed capabilities must not be bounced. It parks until
    /// `set_capabilities` runs, then dispatches normally — here a storage call
    /// with no handler resolves to the "no storage handler" error (proving it
    /// got *past* the capabilities gate) rather than the pre-init rejection.
    #[tokio::test]
    async fn test_dispatch_waits_for_late_capabilities() {
        use super::super::protocol::PluginCapabilities;

        let ctx = Arc::new(RwLock::new(ReverseRpcContext::new()));
        let request = JsonRpcRequest::new(
            1i64,
            super::super::protocol::methods::STORAGE_GET,
            Some(json!({"key": "x"})),
        );

        // Install capabilities shortly after dispatch begins, simulating the
        // host finishing `update_reverse_ctx` while an early call is parked.
        let ctx_writer = Arc::clone(&ctx);
        let setter = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(20)).await;
            ctx_writer
                .write()
                .await
                .set_capabilities(PluginCapabilities::default());
        });

        // Generous barrier timeout so the late caps win the race.
        let resp =
            dispatch_reverse_rpc(&request.method, &request, &ctx, Duration::from_secs(2)).await;
        setter.await.unwrap();

        assert!(resp.is_error());
        // Past the capabilities gate: storage is permitted but no handler is
        // installed, so we get the handler-missing error, NOT a pre-init bounce.
        // (Both are METHOD_NOT_FOUND, so assert on the message to disambiguate.)
        let err = resp.error.unwrap();
        assert_eq!(err.code, error_codes::METHOD_NOT_FOUND);
        assert!(
            err.message.contains("Storage is not available"),
            "expected to pass the readiness gate and reach the storage handler check, got: {}",
            err.message
        );
    }

    /// A plugin without `release_source` calling `releases/record` should be
    /// rejected with AUTH_FAILED, regardless of payload.
    #[tokio::test]
    async fn test_dispatch_denies_release_method_without_capability() {
        use super::super::protocol::{MetadataContentType, PluginCapabilities};

        let mut ctx_inner = ReverseRpcContext::new();
        ctx_inner.set_capabilities(PluginCapabilities {
            metadata_provider: vec![MetadataContentType::Series],
            ..Default::default()
        });
        let ctx = Arc::new(RwLock::new(ctx_inner));

        let request = JsonRpcRequest::new(
            1i64,
            super::super::protocol::methods::RELEASES_RECORD,
            Some(json!({})),
        );
        let resp =
            dispatch_reverse_rpc(&request.method, &request, &ctx, Duration::from_millis(50)).await;
        assert!(resp.is_error());
        assert_eq!(resp.error.unwrap().code, error_codes::AUTH_FAILED);
    }

    /// Unknown methods are rejected with `METHOD_NOT_FOUND` (rather than
    /// silently ignored, as the previous code did).
    #[tokio::test]
    async fn test_dispatch_unknown_method_returns_method_not_found() {
        use super::super::protocol::PluginCapabilities;

        let mut ctx_inner = ReverseRpcContext::new();
        ctx_inner.set_capabilities(PluginCapabilities::default());
        let ctx = Arc::new(RwLock::new(ctx_inner));

        let request = JsonRpcRequest::new(1i64, "frobnicate/zap", Some(json!({})));
        let resp =
            dispatch_reverse_rpc(&request.method, &request, &ctx, Duration::from_millis(50)).await;
        assert!(resp.is_error());
        assert_eq!(resp.error.unwrap().code, error_codes::METHOD_NOT_FOUND);
    }

    /// Storage methods (`AlwaysAllowed`) work for any plugin once initialized,
    /// but if no storage handler is installed they fall through to a clear
    /// error rather than silently failing.
    #[tokio::test]
    async fn test_dispatch_storage_without_handler_returns_method_not_found() {
        use super::super::protocol::PluginCapabilities;

        let mut ctx_inner = ReverseRpcContext::new();
        ctx_inner.set_capabilities(PluginCapabilities::default());
        let ctx = Arc::new(RwLock::new(ctx_inner));

        let request = JsonRpcRequest::new(
            1i64,
            super::super::protocol::methods::STORAGE_GET,
            Some(json!({"key": "x"})),
        );
        let resp =
            dispatch_reverse_rpc(&request.method, &request, &ctx, Duration::from_millis(50)).await;
        assert!(resp.is_error());
        assert_eq!(resp.error.unwrap().code, error_codes::METHOD_NOT_FOUND);
    }

    /// `parentRequestId` round-trips through serde with the camelCase wire
    /// name and is omitted when None. This is the protocol contract we
    /// share with the plugin SDK.
    #[test]
    fn parent_request_id_serializes_as_camel_case_and_omits_when_none() {
        let mut req = JsonRpcRequest::new(42i64, "releases/record", Some(json!({"x": 1})));
        // Default: omitted on the wire.
        let json = serde_json::to_string(&req).unwrap();
        assert!(
            !json.contains("parentRequestId"),
            "absent field should be skipped: {json}"
        );

        // Set: serialized as camelCase.
        req.parent_request_id = Some(RequestId::Number(7));
        let json = serde_json::to_string(&req).unwrap();
        assert!(
            json.contains("\"parentRequestId\":7"),
            "expected camelCase parentRequestId in: {json}"
        );

        // Round-trip: a wire payload deserializes back with the field set.
        let wire = r#"{"jsonrpc":"2.0","id":1,"method":"releases/record","parentRequestId":99}"#;
        let parsed: JsonRpcRequest = serde_json::from_str(wire).unwrap();
        assert!(matches!(
            parsed.parent_request_id,
            Some(RequestId::Number(99))
        ));
    }

    /// `parent_id_to_i64` accepts both numeric and string ids — we use it to
    /// look up the parent forward call in the pending map, which is keyed by
    /// `i64`. The host only ever issues numeric ids, but the protocol type
    /// is `RequestId` for generality.
    #[test]
    fn parent_id_to_i64_handles_numeric_and_string_ids() {
        assert_eq!(parent_id_to_i64(&RequestId::Number(42)), Some(42));
        assert_eq!(
            parent_id_to_i64(&RequestId::String("17".to_string())),
            Some(17)
        );
        assert_eq!(
            parent_id_to_i64(&RequestId::String("nope".to_string())),
            None
        );
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

    /// `call_with_timeout` must honor the *caller-supplied* deadline, not the
    /// client's configured default. Regression guard for the release-poll
    /// timeout bug where the per-handle 30s default fired before the
    /// task-level 5min wrapper could take effect — the fix is to plumb the
    /// caller's deadline straight through to this primitive.
    #[tokio::test]
    async fn call_with_timeout_uses_caller_deadline_not_default() {
        // `sleep` doesn't read stdin (writes go to the kernel pipe buffer)
        // and writes nothing to stdout, so the reader never observes a
        // response and the call must terminate on the caller's deadline.
        // Using `cat` here would echo the request back and the reader
        // would mis-interpret the echo as a response frame.
        let config = PluginProcessConfig::new("sleep").arg("60");
        let process = PluginProcess::spawn_unchecked(&config).await.unwrap();

        // Configure the client with a long default; we want to prove the
        // shorter caller-supplied timeout wins.
        let client = RpcClient::new(process, Duration::from_secs(30));

        let start = std::time::Instant::now();
        let result: Result<serde_json::Value, _> = client
            .call_with_timeout(
                "anything",
                serde_json::json!({}),
                Duration::from_millis(150),
            )
            .await;
        let elapsed = start.elapsed();

        match result {
            Err(RpcError::Timeout(d)) => {
                assert_eq!(d, Duration::from_millis(150));
            }
            other => panic!("expected RpcError::Timeout(150ms), got {:?}", other),
        }
        // Generous upper bound — we just need to prove the 30s default
        // didn't fire. CI sometimes adds noise so we allow 5s of headroom.
        assert!(
            elapsed < Duration::from_secs(5),
            "timeout should fire near 150ms, took {:?}",
            elapsed
        );
    }

    /// Regression: a plugin that fires a reverse-RPC from `onInitialize`
    /// (tagged with the `initialize` call's id) before returning the
    /// `initialize` response must not stall the forward call.
    ///
    /// The host installs capabilities only *after* `call("initialize")`
    /// returns, so a reverse-RPC routed back into the in-flight `initialize`
    /// call's channel cannot be dispatched inline: parking it on the readiness
    /// barrier blocks the very loop that must drain the `initialize` response,
    /// deadlocking until the 5s `REVERSE_RPC_READINESS_TIMEOUT` fires. That bug
    /// surfaced as ~5s plugin "connect" times and a failed
    /// `releases/register_sources` (so release sources never materialized).
    ///
    /// With the fix, the racing reverse-RPC is spawned off and `initialize`
    /// returns promptly. We assert it completes well under the readiness
    /// timeout.
    #[tokio::test]
    async fn initialize_not_blocked_by_reverse_rpc_from_on_initialize() {
        if std::process::Command::new("node")
            .arg("--version")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| !s.success())
            .unwrap_or(true)
        {
            eprintln!("skipping: node not available");
            return;
        }

        // Mock plugin: on `initialize`, emit a reverse-RPC tagged with the
        // forward call's id *before* the `initialize` response, mimicking a
        // release-source plugin calling `releases/register_sources` from
        // `onInitialize`. Stays alive afterward (readline keeps stdin open).
        let script = r#"
        const readline = require('readline');
        const rl = readline.createInterface({ input: process.stdin });
        rl.on('line', (line) => {
            let req;
            try { req = JSON.parse(line); } catch (e) { return; }
            if (req.method === 'initialize') {
                // Reverse-RPC first, parented to the initialize call.
                process.stdout.write(JSON.stringify({
                    jsonrpc: '2.0', id: 9999, method: 'storage/get',
                    params: { key: 'x' }, parentRequestId: req.id,
                }) + '\n');
                // Then the initialize response.
                process.stdout.write(JSON.stringify({
                    jsonrpc: '2.0', id: req.id, result: { ok: true },
                }) + '\n');
            }
        });
        "#;

        let temp_dir = tempfile::tempdir().unwrap();
        let script_path = temp_dir.path().join("mock-init-reverse-rpc.js");
        std::fs::write(&script_path, script).unwrap();

        let config = PluginProcessConfig::new("node").arg(script_path.to_str().unwrap());
        let process = PluginProcess::spawn_unchecked(&config).await.unwrap();
        // Long default timeout so we prove the *fix* unblocks the call, not a
        // short forward-call timeout masking the stall.
        let client = RpcClient::new(process, Duration::from_secs(30));

        let start = std::time::Instant::now();
        let result: Result<Value, _> = client.call("initialize", json!({})).await;
        let elapsed = start.elapsed();

        assert!(result.is_ok(), "initialize should succeed: {result:?}");
        // Pre-fix this took ~5s (the readiness timeout). Assert comfortably
        // under that while leaving headroom for node startup on slow CI.
        assert!(
            elapsed < Duration::from_secs(3),
            "initialize stalled on a reverse-RPC fired during onInitialize ({elapsed:?}); \
             the reverse-RPC must be dispatched off the forward-call loop"
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
