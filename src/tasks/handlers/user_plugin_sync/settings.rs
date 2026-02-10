//! Codex generic sync settings — server-interpreted preferences that control
//! which entries to build and send to the plugin.

/// JSON key for the Codex-reserved namespace in user plugin config.
///
/// User plugin config objects may contain a `_codex` key whose value holds
/// server-interpreted preferences (e.g. `includeCompleted`, `syncRatings`).
/// The plugin itself never reads this namespace — it controls server behavior.
pub(crate) const CODEX_CONFIG_NAMESPACE: &str = "_codex";

/// Codex generic sync settings — server-interpreted preferences that control
/// which entries to build and send to the plugin. Stored in the user plugin
/// config under the `_codex` namespace (e.g. `config._codex.includeCompleted`).
///
/// These are NOT plugin config — the plugin never reads them. They control
/// the server's data-source behavior: filtering, progress counting, ratings.
#[derive(Debug, Clone)]
pub(crate) struct CodexSyncSettings {
    /// Include series where all local books are marked as read. Default: true.
    pub include_completed: bool,
    /// Include series where at least one book has been started. Default: true.
    pub include_in_progress: bool,
    /// Count partially-read books in the progress count. Default: false.
    pub count_partial_progress: bool,
    /// Include scores and notes in push/pull. Default: true.
    pub sync_ratings: bool,
    /// Include series without external IDs (for title-based search fallback).
    /// When enabled, entries with `external_id: ""` and `title` populated are
    /// sent so the plugin can search the external service by title. Default: false.
    pub search_fallback: bool,
}

impl CodexSyncSettings {
    /// Parse Codex sync settings from the `_codex` namespace in user plugin config.
    ///
    /// Example config shape:
    /// ```json
    /// {
    ///   "_codex": {
    ///     "includeCompleted": true,
    ///     "includeInProgress": true,
    ///     "countPartialProgress": false,
    ///     "syncRatings": true
    ///   },
    ///   "progressUnit": "volumes",
    ///   ...
    /// }
    /// ```
    pub fn from_user_config(config: &serde_json::Value) -> Self {
        let codex = config
            .get(CODEX_CONFIG_NAMESPACE)
            .unwrap_or(&serde_json::Value::Null);
        Self {
            include_completed: codex
                .get("includeCompleted")
                .and_then(|v| v.as_bool())
                .unwrap_or(true),
            include_in_progress: codex
                .get("includeInProgress")
                .and_then(|v| v.as_bool())
                .unwrap_or(true),
            count_partial_progress: codex
                .get("countPartialProgress")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            sync_ratings: codex
                .get("syncRatings")
                .and_then(|v| v.as_bool())
                .unwrap_or(true),
            search_fallback: codex
                .get("searchFallback")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
        }
    }
}
