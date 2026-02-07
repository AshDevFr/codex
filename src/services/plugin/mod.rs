//! Plugin System for External Metadata Providers
//!
//! This module implements an MCP-style plugin system that allows Codex to communicate
//! with external processes for metadata fetching. Plugins can be written in any language
//! and communicate via JSON-RPC 2.0 over stdio.
//!
//! ## Architecture
//!
//! ```text
//! ┌───────────────────────────────────────────────────────────────────┐
//! │                          CODEX SERVER                             │
//! ├───────────────────────────────────────────────────────────────────┤
//! │  ┌─────────────────────────────────────────────────────────────┐  │
//! │  │                     Plugin Manager                          │  │
//! │  │  • Spawns plugin processes (command + args)                 │  │
//! │  │  • Communicates via stdio/JSON-RPC                          │  │
//! │  │  • Enforces RBAC permissions on writes                      │  │
//! │  │  • Monitors health, restarts on failure                     │  │
//! │  │  • Rate limits requests per plugin (token bucket)           │  │
//! │  └──────────────────────────┬──────────────────────────────────┘  │
//! │             ┌───────────────┼───────────────┐                     │
//! │             ▼               ▼               ▼                     │
//! │    ┌─────────────┐  ┌─────────────┐  ┌─────────────┐              │
//! │    │   Plugin    │  │   Plugin    │  │   Plugin    │              │
//! │    │  Process 1  │  │  Process 2  │  │  Process N  │              │
//! │    │ stdin/stdout│  │ stdin/stdout│  │ stdin/stdout│              │
//! │    └─────────────┘  └─────────────┘  └─────────────┘              │
//! └───────────────────────────────────────────────────────────────────┘
//! ```
//!
//! ## Plugin Process Ownership
//!
//! Plugins are spawned by whichever Codex process needs them:
//!
//! - **`codex serve`**: Spawns plugins for HTTP API requests (search, get metadata)
//! - **`codex worker`**: Spawns plugins for background tasks (auto-match during library scans)
//!
//! **Important considerations:**
//!
//! 1. **No shared state**: Each Codex process maintains its own `PluginManager` and plugin
//!    instances. If you run multiple workers, each will spawn its own copy of plugins.
//!
//! 2. **Stateless plugins required**: Plugins MUST be stateless. Do not rely on in-memory
//!    state between requests. Any persistent state should be stored externally (database,
//!    file system, etc.).
//!
//! 3. **Rate limits are per-process**: Rate limiting is enforced per `PluginManager` instance.
//!    Running 3 workers means 3x the effective rate limit for external APIs. Configure
//!    `rate_limit_requests_per_minute` conservatively when running multiple workers.
//!
//! 4. **Process lifecycle**: Plugin processes are spawned on first use and kept alive for
//!    reuse. They are shut down when the parent Codex process exits or during health check
//!    failures.
//!
//! ## Security Features
//!
//! - **Command allowlist**: Only whitelisted commands can be executed (see [`process::is_command_allowed`])
//! - **Credential redaction**: Sensitive values use [`secrets::SecretValue`] to prevent log exposure
//! - **Output size limits**: Plugin responses are limited to prevent memory exhaustion
//! - **Rate limiting**: Per-plugin token bucket rate limiting protects external APIs
//!
//! ## Modules
//!
//! - [`protocol`]: JSON-RPC types, manifest schema, metadata types
//! - [`process`]: Process spawning and stdio management
//! - [`rpc`]: JSON-RPC client over stdio
//! - [`handle`]: Plugin lifecycle management
//! - [`health`]: Health monitoring and failure tracking
//! - [`secrets`]: Secure credential handling with redaction

pub mod encryption;
pub mod handle;
pub mod health;
pub mod manager;
pub mod process;
pub mod protocol;
pub mod rpc;
pub mod secrets;
pub mod storage;
pub mod storage_handler;
pub mod sync;

// Re-exports for public API
// Note: Many of these are designed for future use or exposed for the complete API surface.
// The #[allow(unused_imports)] suppresses warnings for exports that aren't yet consumed
// internally but are part of the public module interface.
#[allow(unused_imports)]
pub use handle::PluginHandle;
#[allow(unused_imports)]
pub use health::{HealthMonitor, HealthState, HealthTracker};
#[allow(unused_imports)]
pub use manager::{PluginManager, PluginManagerConfig, PluginManagerError};
#[allow(unused_imports)]
pub use protocol::{PluginCapabilities, PluginManifest, PluginSeriesMetadata};
#[allow(unused_imports)]
pub use rpc::RpcClient;
