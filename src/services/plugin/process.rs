//! Plugin Process Spawning and Management
//!
//! This module handles spawning plugin processes and managing their stdio streams.
//!
//! ## Security
//!
//! Plugin commands are validated against an allowlist before execution. This prevents
//! command injection attacks where a compromised admin account could execute arbitrary
//! commands.
//!
//! Default allowed commands: `node`, `npx`, `python`, `python3`, `uv`, `uvx`
//!
//! Custom commands can be added via:
//! - `CODEX_PLUGIN_ALLOWED_COMMANDS` env var (comma-separated list)
//! - Absolute paths starting with `/opt/codex/plugins/` are always allowed
//!
//! Note: This module provides complete process management infrastructure.
//! Some methods and error variants may not be called from external code yet
//! but are part of the complete API for plugin process management.

// Allow dead code for process management infrastructure that is part of the
// complete API surface but not yet fully integrated.
#![allow(dead_code)]

use std::collections::HashMap;
use std::path::Path;
use std::process::Stdio;
use std::sync::OnceLock;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::mpsc;
use tracing::{debug, error, warn};

// =============================================================================
// Command Allowlist
// =============================================================================

/// Environment variable for customizing allowed plugin commands
pub const ALLOWED_COMMANDS_ENV: &str = "CODEX_PLUGIN_ALLOWED_COMMANDS";

/// Default allowed commands for plugin execution
///
/// These are common runtimes used by plugins:
/// - `node`, `npx`: Node.js plugins
/// - `python`, `python3`: Python plugins
/// - `uv`, `uvx`: Python package runner (uv)
const DEFAULT_ALLOWED_COMMANDS: &[&str] = &["node", "npx", "python", "python3", "uv", "uvx"];

/// Path prefixes that are always allowed (for absolute paths to plugin binaries)
const ALLOWED_PATH_PREFIXES: &[&str] = &["/opt/codex/plugins/"];

/// Cached allowlist (initialized once on first use)
static COMMAND_ALLOWLIST: OnceLock<Vec<String>> = OnceLock::new();

/// Get the command allowlist, initializing from env var if needed
fn get_command_allowlist() -> &'static Vec<String> {
    COMMAND_ALLOWLIST.get_or_init(|| {
        let mut allowlist: Vec<String> = DEFAULT_ALLOWED_COMMANDS
            .iter()
            .map(|s| s.to_string())
            .collect();

        // Add custom commands from environment variable
        if let Ok(custom) = std::env::var(ALLOWED_COMMANDS_ENV) {
            for cmd in custom.split(',') {
                let cmd = cmd.trim();
                if !cmd.is_empty() && !allowlist.contains(&cmd.to_string()) {
                    allowlist.push(cmd.to_string());
                }
            }
        }

        allowlist
    })
}

/// Check if a command is in the allowlist
///
/// Returns `true` if the command is allowed, `false` otherwise.
///
/// A command is allowed if:
/// 1. It matches an entry in the allowlist (e.g., "node", "python")
/// 2. It's an absolute path starting with an allowed prefix (e.g., "/opt/codex/plugins/")
pub fn is_command_allowed(command: &str) -> bool {
    let allowlist = get_command_allowlist();

    // Check if command matches an allowlist entry
    if allowlist.iter().any(|allowed| allowed == command) {
        return true;
    }

    // Check if command is an absolute path under an allowed prefix
    if command.starts_with('/') {
        let path = Path::new(command);
        for prefix in ALLOWED_PATH_PREFIXES {
            if path.starts_with(prefix) {
                return true;
            }
        }
    }

    false
}

/// Get a human-readable description of allowed commands for error messages
pub fn allowed_commands_description() -> String {
    let allowlist = get_command_allowlist();
    let mut parts: Vec<String> = allowlist.iter().map(|s| format!("`{}`", s)).collect();

    for prefix in ALLOWED_PATH_PREFIXES {
        parts.push(format!("absolute paths under `{}`", prefix));
    }

    parts.join(", ")
}

// =============================================================================
// Output Size Limits
// =============================================================================

/// Maximum length of a single line from plugin stdout (1 MB)
///
/// Lines longer than this will be truncated to prevent memory exhaustion
/// from malicious or buggy plugins sending extremely long lines.
pub const MAX_LINE_LENGTH: usize = 1_048_576; // 1 MB

/// Maximum total bytes read from plugin stdout per session
///
/// This is a safety limit to prevent memory exhaustion. In practice,
/// most plugin responses are small JSON-RPC messages.
pub const MAX_TOTAL_OUTPUT: usize = 104_857_600; // 100 MB

// =============================================================================
// Process Configuration
// =============================================================================

/// Configuration for spawning a plugin process
#[derive(Debug, Clone)]
pub struct PluginProcessConfig {
    /// Command to execute (e.g., "node", "python", "/path/to/binary")
    pub command: String,
    /// Arguments to pass to the command
    pub args: Vec<String>,
    /// Environment variables to set (in addition to current env)
    pub env: HashMap<String, String>,
    /// Working directory for the process
    pub working_directory: Option<String>,
}

impl PluginProcessConfig {
    /// Create a new process configuration
    pub fn new(command: impl Into<String>) -> Self {
        Self {
            command: command.into(),
            args: Vec::new(),
            env: HashMap::new(),
            working_directory: None,
        }
    }

    /// Add an argument
    pub fn arg(mut self, arg: impl Into<String>) -> Self {
        self.args.push(arg.into());
        self
    }

    /// Add multiple arguments
    pub fn args(mut self, args: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.args.extend(args.into_iter().map(|a| a.into()));
        self
    }

    /// Set an environment variable
    pub fn env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.insert(key.into(), value.into());
        self
    }

    /// Set multiple environment variables
    pub fn envs(
        mut self,
        vars: impl IntoIterator<Item = (impl Into<String>, impl Into<String>)>,
    ) -> Self {
        for (k, v) in vars {
            self.env.insert(k.into(), v.into());
        }
        self
    }

    /// Set the working directory
    pub fn working_directory(mut self, dir: impl Into<String>) -> Self {
        self.working_directory = Some(dir.into());
        self
    }

    /// Validate that the command is allowed
    ///
    /// Returns `Ok(())` if the command is in the allowlist,
    /// or an error with a descriptive message if not.
    pub fn validate_command(&self) -> Result<(), ProcessError> {
        if is_command_allowed(&self.command) {
            Ok(())
        } else {
            Err(ProcessError::CommandNotAllowed {
                command: self.command.clone(),
                allowed: allowed_commands_description(),
            })
        }
    }
}

/// Error type for process operations
#[derive(Debug, thiserror::Error)]
pub enum ProcessError {
    #[error("Failed to spawn process: {0}")]
    SpawnFailed(#[from] std::io::Error),

    #[error("Command '{command}' is not in the plugin allowlist. Allowed: {allowed}")]
    CommandNotAllowed { command: String, allowed: String },

    #[error("Plugin output line too long ({length} bytes, max {max} bytes)")]
    LineTooLong { length: usize, max: usize },

    #[error("Plugin output exceeded size limit ({total} bytes, max {max} bytes)")]
    OutputTooLarge { total: usize, max: usize },

    #[error("Process stdin not available")]
    StdinUnavailable,

    #[error("Process stdout not available")]
    StdoutUnavailable,

    #[error("Failed to write to process stdin: {0}")]
    WriteFailed(std::io::Error),

    #[error("Failed to read from process stdout: {0}")]
    ReadFailed(std::io::Error),

    #[error("Process terminated unexpectedly")]
    ProcessTerminated,

    #[error("Process exited with code {0}")]
    ExitCode(i32),

    #[error("Channel closed")]
    ChannelClosed,
}

/// A spawned plugin process with stdio handles
pub struct PluginProcess {
    /// The child process handle
    child: Child,
    /// Sender for writing lines to stdin
    stdin_tx: mpsc::Sender<String>,
    /// Receiver for reading lines from stdout
    stdout_rx: mpsc::Receiver<String>,
}

impl PluginProcess {
    /// Spawn a new plugin process
    ///
    /// # Security
    ///
    /// The command is validated against the allowlist before spawning.
    /// If the command is not allowed, returns `ProcessError::CommandNotAllowed`.
    pub async fn spawn(config: &PluginProcessConfig) -> Result<Self, ProcessError> {
        // Validate command against allowlist before spawning
        config.validate_command()?;

        let mut cmd = Command::new(&config.command);

        // Add arguments
        cmd.args(&config.args);

        // Set environment variables
        for (key, value) in &config.env {
            cmd.env(key, value);
        }

        // Set working directory if specified
        if let Some(ref dir) = config.working_directory {
            cmd.current_dir(dir);
        }

        // Configure stdio
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit()) // Plugin stderr goes to Codex logs
            .kill_on_drop(true);

        debug!(
            command = %config.command,
            args = ?config.args,
            "Spawning plugin process"
        );

        let mut child = cmd.spawn()?;

        // Take ownership of stdio handles
        let stdin = child.stdin.take().ok_or(ProcessError::StdinUnavailable)?;
        let stdout = child.stdout.take().ok_or(ProcessError::StdoutUnavailable)?;

        // Create channels for async IO
        let (stdin_tx, stdin_rx) = mpsc::channel::<String>(32);
        let (stdout_tx, stdout_rx) = mpsc::channel::<String>(32);

        // Spawn stdin writer task
        tokio::spawn(stdin_writer_task(stdin, stdin_rx));

        // Spawn stdout reader task
        tokio::spawn(stdout_reader_task(stdout, stdout_tx));

        Ok(Self {
            child,
            stdin_tx,
            stdout_rx,
        })
    }

    /// Write a line to the process stdin
    pub async fn write_line(&self, line: &str) -> Result<(), ProcessError> {
        self.stdin_tx
            .send(line.to_string())
            .await
            .map_err(|_| ProcessError::ChannelClosed)
    }

    /// Read a line from the process stdout
    pub async fn read_line(&mut self) -> Result<String, ProcessError> {
        self.stdout_rx
            .recv()
            .await
            .ok_or(ProcessError::ProcessTerminated)
    }

    /// Check if the process is still running
    pub fn is_running(&mut self) -> bool {
        match self.child.try_wait() {
            Ok(None) => true,     // Still running
            Ok(Some(_)) => false, // Exited
            Err(_) => false,      // Error checking status
        }
    }

    /// Get the process ID
    pub fn pid(&self) -> Option<u32> {
        self.child.id()
    }

    /// Wait for the process to exit and return the exit code
    pub async fn wait(&mut self) -> Result<i32, ProcessError> {
        let status = self.child.wait().await?;
        Ok(status.code().unwrap_or(-1))
    }

    /// Kill the process
    pub async fn kill(&mut self) -> Result<(), ProcessError> {
        self.child.kill().await.map_err(ProcessError::SpawnFailed)
    }

    /// Gracefully shutdown: wait for timeout, then kill
    pub async fn shutdown(&mut self, timeout: std::time::Duration) -> Result<i32, ProcessError> {
        // Try to wait for process to exit gracefully
        match tokio::time::timeout(timeout, self.child.wait()).await {
            Ok(Ok(status)) => Ok(status.code().unwrap_or(-1)),
            Ok(Err(e)) => Err(ProcessError::SpawnFailed(e)),
            Err(_) => {
                // Timeout - force kill
                warn!("Plugin process did not exit gracefully, killing");
                self.kill().await?;
                Ok(-1)
            }
        }
    }
}

/// Task that writes lines to the process stdin
async fn stdin_writer_task(mut stdin: ChildStdin, mut rx: mpsc::Receiver<String>) {
    while let Some(line) = rx.recv().await {
        let line_with_newline = if line.ends_with('\n') {
            line
        } else {
            format!("{}\n", line)
        };

        if let Err(e) = stdin.write_all(line_with_newline.as_bytes()).await {
            error!("Failed to write to plugin stdin: {}", e);
            break;
        }

        if let Err(e) = stdin.flush().await {
            error!("Failed to flush plugin stdin: {}", e);
            break;
        }
    }
}

/// Task that reads lines from the process stdout with size limits
///
/// # Size Limits
///
/// - Lines longer than `MAX_LINE_LENGTH` (1 MB) are truncated with a warning
/// - Total output exceeding `MAX_TOTAL_OUTPUT` (100 MB) causes the reader to stop
///
/// These limits protect against memory exhaustion from malicious or buggy plugins.
async fn stdout_reader_task(stdout: ChildStdout, tx: mpsc::Sender<String>) {
    let mut reader = BufReader::new(stdout);
    let mut line = String::new();
    let mut total_bytes: usize = 0;

    loop {
        line.clear();
        match reader.read_line(&mut line).await {
            Ok(0) => {
                // EOF - process closed stdout
                debug!("Plugin stdout closed (EOF)");
                break;
            }
            Ok(bytes_read) => {
                total_bytes = total_bytes.saturating_add(bytes_read);

                // Check total output limit
                if total_bytes > MAX_TOTAL_OUTPUT {
                    error!(
                        "Plugin output exceeded size limit ({} bytes, max {} bytes)",
                        total_bytes, MAX_TOTAL_OUTPUT
                    );
                    break;
                }

                // Truncate oversized lines with warning
                let mut output = line.trim_end().to_string();
                if output.len() > MAX_LINE_LENGTH {
                    warn!(
                        "Plugin output line truncated ({} bytes, max {} bytes)",
                        output.len(),
                        MAX_LINE_LENGTH
                    );
                    output.truncate(MAX_LINE_LENGTH);
                }

                if tx.send(output).await.is_err() {
                    // Receiver dropped
                    break;
                }
            }
            Err(e) => {
                error!("Failed to read from plugin stdout: {}", e);
                break;
            }
        }
    }

    debug!(
        "Plugin stdout reader finished (total: {} bytes)",
        total_bytes
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // Command Allowlist Tests
    // =========================================================================

    #[test]
    fn test_default_commands_allowed() {
        // Default allowed commands should pass validation
        assert!(is_command_allowed("node"));
        assert!(is_command_allowed("npx"));
        assert!(is_command_allowed("python"));
        assert!(is_command_allowed("python3"));
        assert!(is_command_allowed("uv"));
        assert!(is_command_allowed("uvx"));
    }

    #[test]
    fn test_arbitrary_commands_blocked() {
        // Arbitrary commands should be blocked
        assert!(!is_command_allowed("rm"));
        assert!(!is_command_allowed("curl"));
        assert!(!is_command_allowed("wget"));
        assert!(!is_command_allowed("bash"));
        assert!(!is_command_allowed("sh"));
        assert!(!is_command_allowed("/bin/bash"));
        assert!(!is_command_allowed("cat"));
        assert!(!is_command_allowed("echo"));
    }

    #[test]
    fn test_allowed_path_prefix() {
        // Paths under allowed prefix should be allowed
        assert!(is_command_allowed("/opt/codex/plugins/my-plugin"));
        assert!(is_command_allowed("/opt/codex/plugins/metadata/mangabaka"));

        // Paths not under allowed prefix should be blocked
        assert!(!is_command_allowed("/usr/bin/node"));
        assert!(!is_command_allowed("/home/user/malicious"));
        assert!(!is_command_allowed("/opt/other/plugins/plugin"));
    }

    #[test]
    fn test_validate_command_success() {
        let config = PluginProcessConfig::new("node").arg("script.js");
        assert!(config.validate_command().is_ok());
    }

    #[test]
    fn test_validate_command_failure() {
        let config = PluginProcessConfig::new("rm").arg("-rf").arg("/");
        let result = config.validate_command();
        assert!(result.is_err());

        let err = result.unwrap_err();
        match err {
            ProcessError::CommandNotAllowed { command, allowed } => {
                assert_eq!(command, "rm");
                assert!(allowed.contains("node"));
            }
            _ => panic!("Expected CommandNotAllowed error"),
        }
    }

    #[test]
    fn test_allowed_commands_description() {
        let desc = allowed_commands_description();
        assert!(desc.contains("`node`"));
        assert!(desc.contains("`python`"));
        assert!(desc.contains("/opt/codex/plugins/"));
    }

    #[tokio::test]
    async fn test_spawn_blocked_command() {
        // Attempting to spawn a blocked command should fail with CommandNotAllowed
        let config = PluginProcessConfig::new("cat");
        let result = PluginProcess::spawn(&config).await;

        assert!(result.is_err());
        match result {
            Err(ProcessError::CommandNotAllowed { command, .. }) => {
                assert_eq!(command, "cat");
            }
            Err(e) => panic!("Expected CommandNotAllowed, got: {:?}", e),
            Ok(_) => panic!("Expected error, got Ok"),
        }
    }

    #[tokio::test]
    async fn test_spawn_allowed_command() {
        // Spawning an allowed command should work (if the command exists)
        // We use 'node --version' which should be available in most dev environments
        let config = PluginProcessConfig::new("node").arg("--version");

        // This may fail if node is not installed, but that's OK for the test
        // The important thing is that it doesn't fail with CommandNotAllowed
        let result = PluginProcess::spawn(&config).await;

        match result {
            Ok(mut process) => {
                // Successfully spawned - clean up
                let _ = process.kill().await;
            }
            Err(ProcessError::CommandNotAllowed { .. }) => {
                panic!("node should be in the allowlist");
            }
            Err(ProcessError::SpawnFailed(_)) => {
                // node might not be installed - that's OK
            }
            Err(e) => {
                panic!("Unexpected error: {:?}", e);
            }
        }
    }

    // =========================================================================
    // Process Config Tests
    // =========================================================================

    #[test]
    fn test_process_config_builder() {
        let config = PluginProcessConfig::new("node")
            .arg("script.js")
            .args(["--flag", "value"])
            .env("API_KEY", "secret")
            .working_directory("/tmp");

        assert_eq!(config.command, "node");
        assert_eq!(config.args, vec!["script.js", "--flag", "value"]);
        assert_eq!(config.env.get("API_KEY"), Some(&"secret".to_string()));
        assert_eq!(config.working_directory, Some("/tmp".to_string()));
    }

    // =========================================================================
    // Output Size Limits Tests
    // =========================================================================

    #[test]
    fn test_size_limit_constants() {
        // Verify the size limit constants are reasonable
        assert_eq!(MAX_LINE_LENGTH, 1_048_576); // 1 MB
        assert_eq!(MAX_TOTAL_OUTPUT, 104_857_600); // 100 MB

        // MAX_TOTAL_OUTPUT should be larger than MAX_LINE_LENGTH (compile-time check)
        const _: () = assert!(MAX_TOTAL_OUTPUT > MAX_LINE_LENGTH);
    }

    #[test]
    fn test_error_variants() {
        // Test that the error variants format correctly
        let err = ProcessError::LineTooLong {
            length: 2_000_000,
            max: MAX_LINE_LENGTH,
        };
        let msg = err.to_string();
        assert!(msg.contains("2000000"));
        assert!(msg.contains("1048576"));

        let err = ProcessError::OutputTooLarge {
            total: 200_000_000,
            max: MAX_TOTAL_OUTPUT,
        };
        let msg = err.to_string();
        assert!(msg.contains("200000000"));
        assert!(msg.contains("104857600"));
    }
}
