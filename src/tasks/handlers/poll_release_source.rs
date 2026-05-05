//! Handler for the `PollReleaseSource` task.
//!
//! Resolves the source's owning plugin, calls `releases/poll`, runs returned
//! candidates through the matcher + threshold gate, and writes accepted
//! candidates to the ledger. On success updates `last_polled_at` (and
//! optionally `etag`); on failure records `last_error`.
//!
//! Plugins MAY also stream candidates via the `releases/record` reverse-RPC
//! during the poll call. Both paths land in the same ledger; cross-channel
//! dedup is handled by the ledger's `(source_id, external_release_id)`
//! constraint.
//!
//! Key invariants:
//!
//! - **Idempotent.** Re-running this task for a source that polled
//!   successfully a moment ago re-hits the upstream but the ledger drops
//!   duplicates.
//! - **Bounded by per-task timeout.** A long-running plugin call won't
//!   block the worker pool indefinitely (see `plugin.task_request_timeout_seconds`
//!   setting; defaults inherit `PluginManager::default_request_timeout`).
//! - **Permission-gated upstream.** The plugin's manifest must declare the
//!   `release_source` capability; the `releases/*` reverse-RPC dispatcher
//!   enforces this. This handler trusts the plugin name on the source row
//!   and lets the dispatcher reject misuse.

use anyhow::Result;
use chrono::Utc;
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::db::entities::release_sources::plugin_id as source_plugin_id;
use crate::db::entities::tasks;
use crate::db::repositories::{
    NewReleaseEntry, PluginsRepository, ReleaseLedgerRepository, ReleaseSourceRepository,
    SeriesTrackingRepository,
};
use crate::events::{EntityChangeEvent, EventBroadcaster};
use crate::services::SettingsService;
use crate::services::plugin::PluginManager;
use crate::services::plugin::handle::PluginError;
use crate::services::plugin::protocol::{ReleasePollRequest, ReleasePollResponse, methods};
use crate::services::release::backoff::{HostBackoff, is_backoff_status};
use crate::services::release::matcher::{evaluate, resolve_threshold};
use crate::tasks::handlers::TaskHandler;
use crate::tasks::types::TaskResult;

/// Default plugin task timeout in seconds (5 minutes — same as user_plugin_sync).
const DEFAULT_TASK_TIMEOUT_SECS: u64 = 300;

/// Result of a `PollReleaseSource` task. Stored on the `tasks.result` JSON
/// column for observability and consumed by tests.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PollReleaseSourceResult {
    pub source_id: Uuid,
    /// Number of candidates the plugin returned in its response payload.
    pub candidates_returned: u32,
    /// Number of candidates accepted by the matcher and recorded.
    pub candidates_recorded: u32,
    /// Number of candidates dropped before the ledger (validation failures
    /// or below-threshold).
    pub candidates_rejected: u32,
    /// Number of accepted candidates that landed as a duplicate.
    pub candidates_deduped: u32,
    /// Whether the upstream returned `304 Not Modified` (or the plugin's
    /// equivalent).
    pub not_modified: bool,
    /// Whether the source was skipped because the plugin couldn't be
    /// reached or the source was disabled.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skipped_reason: Option<String>,
}

/// Handler for `PollReleaseSource`.
pub struct PollReleaseSourceHandler {
    plugin_manager: Arc<PluginManager>,
    settings_service: Option<Arc<SettingsService>>,
    backoff: HostBackoff,
}

impl PollReleaseSourceHandler {
    pub fn new(plugin_manager: Arc<PluginManager>) -> Self {
        Self {
            plugin_manager,
            settings_service: None,
            backoff: HostBackoff::new(),
        }
    }

    pub fn with_settings_service(mut self, settings_service: Arc<SettingsService>) -> Self {
        self.settings_service = Some(settings_service);
        self
    }

    /// Override the shared backoff tracker. Most callers want the default
    /// (each handler with its own state); the scheduler may pass a shared
    /// one once it consumes backoff for interval scaling.
    pub fn with_backoff(mut self, backoff: HostBackoff) -> Self {
        self.backoff = backoff;
        self
    }

    #[allow(dead_code)] // Public for tests + scheduler reuse.
    pub fn backoff(&self) -> HostBackoff {
        self.backoff.clone()
    }

    async fn task_request_timeout(&self) -> Option<Duration> {
        if let Some(ref settings) = self.settings_service {
            let secs = settings
                .get_uint(
                    "plugin.task_request_timeout_seconds",
                    DEFAULT_TASK_TIMEOUT_SECS,
                )
                .await
                .unwrap_or(DEFAULT_TASK_TIMEOUT_SECS);
            Some(Duration::from_secs(secs))
        } else {
            None
        }
    }
}

impl TaskHandler for PollReleaseSourceHandler {
    fn handle<'a>(
        &'a self,
        task: &'a tasks::Model,
        db: &'a DatabaseConnection,
        event_broadcaster: Option<&'a Arc<EventBroadcaster>>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<TaskResult>> + Send + 'a>> {
        Box::pin(async move {
            // Extract task params.
            let params = task
                .params
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("Missing params in poll_release_source task"))?;
            let source_id: Uuid = params
                .get("source_id")
                .and_then(|v| v.as_str())
                .and_then(|s| Uuid::parse_str(s).ok())
                .ok_or_else(|| anyhow::anyhow!("Missing or invalid source_id in params"))?;

            info!("Task {}: Polling release source {}", task.id, source_id);

            // Load the source row.
            let source = match ReleaseSourceRepository::get_by_id(db, source_id).await {
                Ok(Some(s)) => s,
                Ok(None) => {
                    let msg = format!("source {} not found", source_id);
                    warn!("Task {}: {}", task.id, msg);
                    return Ok(TaskResult::failure(msg));
                }
                Err(e) => {
                    let msg = format!("failed to load source {}: {}", source_id, e);
                    error!("Task {}: {}", task.id, msg);
                    return Ok(TaskResult::failure(msg));
                }
            };

            if !source.enabled {
                debug!(
                    "Task {}: Source {} is disabled; skipping",
                    task.id, source.id
                );
                return Ok(TaskResult::success_with_data(
                    "source disabled",
                    json!(PollReleaseSourceResult {
                        source_id,
                        skipped_reason: Some("source_disabled".to_string()),
                        ..Default::default()
                    }),
                ));
            }

            // Synthetic in-core sources (Phase 5 metadata-piggyback) don't
            // route through a plugin process. We don't have a code path for
            // them yet; record a benign skip so the scheduler doesn't loop.
            if source.plugin_id == source_plugin_id::CORE {
                debug!(
                    "Task {}: Source {} is in-core (plugin_id=core); skipping (Phase 5 territory)",
                    task.id, source.id
                );
                return Ok(TaskResult::success_with_data(
                    "core-source has no poll path yet",
                    json!(PollReleaseSourceResult {
                        source_id,
                        skipped_reason: Some("core_source_no_poll_path".to_string()),
                        ..Default::default()
                    }),
                ));
            }

            // Resolve the plugin row by name.
            let plugin_row = match PluginsRepository::get_by_name(db, &source.plugin_id).await {
                Ok(Some(p)) => p,
                Ok(None) => {
                    let msg = format!("plugin {} not registered", source.plugin_id);
                    warn!("Task {}: {}", task.id, msg);
                    record_error(db, source.id, &msg).await;
                    return Ok(TaskResult::failure(msg));
                }
                Err(e) => {
                    let msg = format!("failed to lookup plugin: {}", e);
                    error!("Task {}: {}", task.id, msg);
                    record_error(db, source.id, &msg).await;
                    return Ok(TaskResult::failure(msg));
                }
            };

            if !plugin_row.enabled {
                let msg = format!("plugin {} disabled", source.plugin_id);
                warn!("Task {}: {}", task.id, msg);
                return Ok(TaskResult::success_with_data(
                    "plugin disabled",
                    json!(PollReleaseSourceResult {
                        source_id,
                        skipped_reason: Some("plugin_disabled".to_string()),
                        ..Default::default()
                    }),
                ));
            }

            // Spawn / get the plugin handle.
            let handle = match self.plugin_manager.get_or_spawn(plugin_row.id).await {
                Ok(h) => h,
                Err(e) => {
                    let msg = format!("failed to start plugin: {}", e);
                    error!("Task {}: {}", task.id, msg);
                    record_error(db, source.id, &msg).await;
                    return Ok(TaskResult::failure(msg));
                }
            };

            // Build the poll request.
            let req = ReleasePollRequest {
                source_id: source.id,
                source_key: Some(source.source_key.clone()),
                config: source.config.clone(),
                etag: source.etag.clone(),
            };
            let timeout = self.task_request_timeout().await;
            let response_fut = handle.call_method::<ReleasePollRequest, ReleasePollResponse>(
                methods::RELEASES_POLL,
                req,
            );
            let response_result = if let Some(t) = timeout {
                match tokio::time::timeout(t, response_fut).await {
                    Ok(r) => r,
                    Err(_) => {
                        let msg = format!("poll timed out after {:?}", t);
                        warn!("Task {}: {}", task.id, msg);
                        record_error(db, source.id, &msg).await;
                        return Ok(TaskResult::failure(msg));
                    }
                }
            } else {
                response_fut.await
            };

            let response = match response_result {
                Ok(r) => r,
                Err(e) => {
                    // Plugin errors map to source `last_error`. Backoff:
                    // if the plugin reports a rate limit via RPC we honor
                    // it host-wide on best-effort (hostname unknown here),
                    // but we still mark the source as errored.
                    let msg = format!("plugin call failed: {}", e);
                    if let PluginError::Rpc(_) = &e
                        && let Some(retry_after) = e.rpc_retry_after_seconds()
                    {
                        debug!(
                            "Task {}: Plugin reported rate-limit retryAfter={}s",
                            task.id, retry_after
                        );
                    }
                    error!("Task {}: {}", task.id, msg);
                    record_error(db, source.id, &msg).await;
                    return Ok(TaskResult::failure(msg));
                }
            };

            // Apply per-host backoff signals based on the upstream status
            // the plugin observed (if any). The plugin is expected to set
            // `upstream_status` on its response so we can throttle without
            // each plugin re-implementing backoff.
            let mut backoff_url: Option<String> = None;
            if let Some(status) = response.upstream_status {
                if is_backoff_status(status) {
                    // Pluck a host hint from the source's `display_name` or
                    // `config` if present. Many plugins encode the upstream
                    // base URL in `config.url`. If we can't find one, the
                    // backoff is keyed by the plugin name as a fallback so
                    // siblings on the same plugin still cooperate.
                    let url_hint = derive_url_hint(&source);
                    self.backoff.record_http_error(&url_hint, status).await;
                    backoff_url = Some(url_hint);
                    warn!(
                        "Task {}: Source {} got upstream status {}; backoff multiplier {}",
                        task.id,
                        source.id,
                        status,
                        self.backoff
                            .multiplier(backoff_url.as_deref().unwrap_or(""))
                            .await
                    );
                } else if (200..400).contains(&status) {
                    let url_hint = derive_url_hint(&source);
                    self.backoff.record_success(&url_hint).await;
                    backoff_url = Some(url_hint);
                }
            }

            // Process candidates (the plugin may have streamed some via
            // reverse-RPC already; those are already on the ledger).
            // Snapshot fields needed *after* the consume-loop below so we
            // can still build the `last_summary` once `response.candidates`
            // is moved.
            let response_etag = response.etag.clone();
            let response_not_modified = response.not_modified;
            let response_upstream_status = response.upstream_status;

            let mut result = PollReleaseSourceResult {
                source_id,
                candidates_returned: response.candidates.len() as u32,
                not_modified: response.not_modified.unwrap_or(false),
                ..Default::default()
            };

            for cand in response.candidates {
                let series_id = cand.series_match.codex_series_id;
                let threshold = match SeriesTrackingRepository::get(db, series_id).await {
                    Ok(Some(row)) => resolve_threshold(row.confidence_threshold_override),
                    Ok(None) => resolve_threshold(None),
                    Err(e) => {
                        warn!(
                            "Task {}: tracking lookup failed for series {}: {}",
                            task.id, series_id, e
                        );
                        result.candidates_rejected += 1;
                        continue;
                    }
                };
                match evaluate(cand, threshold) {
                    Ok(accepted) => {
                        let entry: NewReleaseEntry = accepted.into_ledger_entry(source.id);
                        match ReleaseLedgerRepository::record(db, entry).await {
                            Ok(outcome) => {
                                if outcome.deduped {
                                    result.candidates_deduped += 1;
                                } else {
                                    result.candidates_recorded += 1;
                                    if let Some(broadcaster) = event_broadcaster {
                                        emit_release_announced(
                                            broadcaster,
                                            &outcome.row,
                                            &source.plugin_id,
                                        );
                                    }
                                }
                            }
                            Err(e) => {
                                warn!(
                                    "Task {}: ledger write failed for source {}: {}",
                                    task.id, source.id, e
                                );
                                result.candidates_rejected += 1;
                            }
                        }
                    }
                    Err(reason) => {
                        debug!("Task {}: candidate rejected: {}", task.id, reason);
                        result.candidates_rejected += 1;
                    }
                }
            }

            // Persist source state. If we hit a successful 2xx upstream we
            // already noted it for backoff; clear `last_error` and stamp
            // `last_polled_at`. The one-line `summary` is surfaced in the
            // Release tracking UI under the per-row status badge so users
            // can see *why* a poll returned no announcements (no tracked
            // series, upstream not modified, …) without container logs.
            let polled_at = Utc::now();
            let summary =
                build_poll_summary(response_not_modified, response_upstream_status, &result);
            if let Err(e) = ReleaseSourceRepository::record_poll_success(
                db,
                source.id,
                polled_at,
                response_etag,
                Some(summary),
            )
            .await
            {
                warn!("Task {}: failed to persist source state: {}", task.id, e);
            }

            // If the plugin signalled an upstream error code but didn't
            // return an RPC error, also stamp `last_error` so admins see
            // it in the UI.
            if let Some(status) = response_upstream_status
                && is_backoff_status(status)
            {
                let _ = ReleaseSourceRepository::record_poll_error(
                    db,
                    source.id,
                    &format!("upstream returned {}", status),
                    polled_at,
                )
                .await;
            }

            // Reset backoff on a clean run if we didn't already.
            if backoff_url.is_none() && response_upstream_status.is_none() {
                let url_hint = derive_url_hint(&source);
                self.backoff.record_success(&url_hint).await;
            }

            let message = format!(
                "Polled {}: returned {}, recorded {}, deduped {}, rejected {}",
                source.display_name,
                result.candidates_returned,
                result.candidates_recorded,
                result.candidates_deduped,
                result.candidates_rejected
            );
            info!("Task {}: {}", task.id, message);
            Ok(TaskResult::success_with_data(message, json!(result)))
        })
    }
}

/// Build the one-line `last_summary` string written to `release_sources`
/// after a successful poll, intended for direct display under the Release
/// tracking row's status badge.
///
/// Example outputs:
/// - `"Up to date — upstream returned 304 (not modified)"`
/// - `"Fetched 0 items"` (e.g. no tracked series with aliases for the source)
/// - `"Fetched 12 items, recorded 0 (12 already in ledger)"`
/// - `"Fetched 5 items, recorded 1, dropped 4 below threshold"`
/// - `"Upstream warning: HTTP 429"` (when the plugin reports an error code
///   but didn't fail the RPC outright)
pub(crate) fn build_poll_summary(
    not_modified: Option<bool>,
    upstream_status: Option<u16>,
    result: &PollReleaseSourceResult,
) -> String {
    if matches!(not_modified, Some(true)) {
        return "Up to date — upstream returned 304 (not modified)".to_string();
    }

    let returned = result.candidates_returned;
    let recorded = result.candidates_recorded;
    let deduped = result.candidates_deduped;
    let rejected = result.candidates_rejected;

    let mut s = match returned {
        0 => "Fetched 0 items".to_string(),
        1 => format!("Fetched 1 item, recorded {}", recorded),
        n => format!("Fetched {} items, recorded {}", n, recorded),
    };
    if deduped > 0 {
        s.push_str(&format!(" ({} already in ledger)", deduped));
    }
    if rejected > 0 {
        s.push_str(&format!(", dropped {} below threshold", rejected));
    }

    // Upstream warning takes a trailing-suffix slot so the count info isn't
    // lost. Backoff-significant statuses (429 / 5xx) are paired with a
    // `last_error` write elsewhere; this is just a friendly inline note.
    if let Some(status) = upstream_status
        && is_backoff_status(status)
    {
        s.push_str(&format!(" · upstream warning: HTTP {}", status));
    }

    s
}

/// Emit a `ReleaseAnnounced` event for a freshly-inserted ledger row.
///
/// Failure to broadcast (no subscribers, channel closed) is a benign noop —
/// the ledger row is the source of truth, the SSE event is a UX nicety.
pub(crate) fn emit_release_announced(
    broadcaster: &EventBroadcaster,
    row: &crate::db::entities::release_ledger::Model,
    plugin_id: &str,
) {
    let _ = broadcaster.emit(EntityChangeEvent::release_announced(row, plugin_id));
}

/// Best-effort URL hint extraction used for backoff keying.
///
/// Looks in `config.url`, `config.feed_url`, and `config.base_url` in that
/// order; falls back to the plugin name (so all sources on the same plugin
/// share a backoff key).
fn derive_url_hint(source: &crate::db::entities::release_sources::Model) -> String {
    if let Some(cfg) = source.config.as_ref() {
        for key in ["url", "feedUrl", "feed_url", "baseUrl", "base_url"] {
            if let Some(v) = cfg.get(key).and_then(|v| v.as_str())
                && !v.is_empty()
            {
                return v.to_string();
            }
        }
    }
    source.plugin_id.clone()
}

async fn record_error(db: &DatabaseConnection, source_id: Uuid, message: &str) {
    if let Err(e) =
        ReleaseSourceRepository::record_poll_error(db, source_id, message, Utc::now()).await
    {
        warn!(
            "Failed to persist poll error on source {}: {}",
            source_id, e
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::ScanningStrategy;
    use crate::db::entities::release_sources::kind;
    use crate::db::repositories::{
        LibraryRepository, NewReleaseSource, ReleaseSourceRepository, SeriesRepository,
        SeriesTrackingRepository, TrackingUpdate,
    };
    use crate::db::test_helpers::create_test_db;

    use crate::events::EntityEvent;

    /// `emit_release_announced` produces a `ReleaseAnnounced` event whose
    /// fields mirror the ledger row and the source's plugin id.
    #[test]
    fn emit_release_announced_emits_matching_event() {
        let broadcaster = EventBroadcaster::new(8);
        let mut rx = broadcaster.subscribe();

        let row = crate::db::entities::release_ledger::Model {
            id: Uuid::new_v4(),
            series_id: Uuid::new_v4(),
            source_id: Uuid::new_v4(),
            external_release_id: "ext-1".to_string(),
            info_hash: None,
            chapter: Some(143.0),
            volume: Some(15),
            language: Some("en".to_string()),
            format_hints: None,
            group_or_uploader: None,
            payload_url: "https://example.com/r/1".to_string(),
            confidence: 0.95,
            state: "announced".to_string(),
            metadata: None,
            observed_at: Utc::now(),
            created_at: Utc::now(),
        };

        emit_release_announced(&broadcaster, &row, "release-mangaupdates");

        let event = rx.try_recv().expect("expected one event");
        match event.event {
            EntityEvent::ReleaseAnnounced {
                ledger_id,
                series_id,
                source_id,
                plugin_id,
                chapter,
                volume,
                language,
            } => {
                assert_eq!(ledger_id, row.id);
                assert_eq!(series_id, row.series_id);
                assert_eq!(source_id, row.source_id);
                assert_eq!(plugin_id, "release-mangaupdates");
                assert_eq!(chapter, Some(143.0));
                assert_eq!(volume, Some(15));
                assert_eq!(language, "en");
            }
            other => panic!("unexpected event: {:?}", other),
        }
    }

    /// Emitting with no subscribers must not panic — the broadcast send error
    /// is intentionally swallowed.
    #[test]
    fn emit_release_announced_tolerates_no_subscribers() {
        let broadcaster = EventBroadcaster::new(8);
        let row = crate::db::entities::release_ledger::Model {
            id: Uuid::new_v4(),
            series_id: Uuid::new_v4(),
            source_id: Uuid::new_v4(),
            external_release_id: "ext-2".to_string(),
            info_hash: None,
            chapter: None,
            volume: None,
            language: None,
            format_hints: None,
            group_or_uploader: None,
            payload_url: "https://example.com/r/2".to_string(),
            confidence: 0.8,
            state: "announced".to_string(),
            metadata: None,
            observed_at: Utc::now(),
            created_at: Utc::now(),
        };
        emit_release_announced(&broadcaster, &row, "release-nyaa");
    }

    #[test]
    fn derive_url_hint_uses_config_url_when_present() {
        let mut model = make_model();
        model.config = Some(json!({"url": "https://nyaa.si/feed"}));
        assert_eq!(derive_url_hint(&model), "https://nyaa.si/feed");
    }

    #[test]
    fn derive_url_hint_falls_back_to_plugin_name() {
        let model = make_model();
        assert_eq!(derive_url_hint(&model), "release-nyaa");
    }

    #[test]
    fn derive_url_hint_supports_alternate_keys() {
        let mut model = make_model();
        model.config = Some(json!({"feedUrl": "https://example.com/x"}));
        assert_eq!(derive_url_hint(&model), "https://example.com/x");
    }

    fn make_model() -> crate::db::entities::release_sources::Model {
        crate::db::entities::release_sources::Model {
            id: Uuid::new_v4(),
            plugin_id: "release-nyaa".to_string(),
            source_key: "k".to_string(),
            display_name: "n".to_string(),
            kind: kind::RSS_UPLOADER.to_string(),
            enabled: true,
            poll_interval_s: 3600,
            last_polled_at: None,
            last_error: None,
            last_error_at: None,
            etag: None,
            config: None,
            last_summary: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    /// A poll task referencing a missing source must fail without panic.
    #[tokio::test]
    async fn task_fails_when_source_missing() {
        let (db, _t) = create_test_db().await;
        let conn = db.sea_orm_connection().clone();
        let plugin_manager = Arc::new(PluginManager::with_defaults(Arc::new(conn.clone())));
        let handler = PollReleaseSourceHandler::new(plugin_manager);

        let task = make_task(json!({"source_id": Uuid::new_v4().to_string()}));
        let result = handler.handle(&task, &conn, None).await.unwrap();
        assert!(!result.success);
        assert!(result.message.unwrap().contains("not found"));
    }

    /// A disabled source short-circuits cleanly.
    #[tokio::test]
    async fn task_skips_disabled_source() {
        let (db, _t) = create_test_db().await;
        let conn = db.sea_orm_connection().clone();
        let library = LibraryRepository::create(&conn, "L", "/l", ScanningStrategy::Default)
            .await
            .unwrap();
        let _series = SeriesRepository::create(&conn, library.id, "S", None)
            .await
            .unwrap();

        let source = ReleaseSourceRepository::create(
            &conn,
            NewReleaseSource {
                plugin_id: "release-nyaa".to_string(),
                source_key: "k".to_string(),
                display_name: "Nyaa".to_string(),
                kind: kind::RSS_UPLOADER.to_string(),
                poll_interval_s: 3600,
                enabled: Some(false),
                config: None,
            },
        )
        .await
        .unwrap();

        let plugin_manager = Arc::new(PluginManager::with_defaults(Arc::new(conn.clone())));
        let handler = PollReleaseSourceHandler::new(plugin_manager);

        let task = make_task(json!({"source_id": source.id.to_string()}));
        let result = handler.handle(&task, &conn, None).await.unwrap();
        assert!(result.success);
        let data = result.data.unwrap();
        assert_eq!(data["skippedReason"], "source_disabled");
    }

    /// A source with `plugin_id="core"` short-circuits with the
    /// `core_source_no_poll_path` reason instead of trying to spawn a plugin.
    #[tokio::test]
    async fn task_skips_core_source() {
        let (db, _t) = create_test_db().await;
        let conn = db.sea_orm_connection().clone();
        let _library = LibraryRepository::create(&conn, "L", "/l", ScanningStrategy::Default)
            .await
            .unwrap();

        let source = ReleaseSourceRepository::create(
            &conn,
            NewReleaseSource {
                plugin_id: source_plugin_id::CORE.to_string(),
                source_key: "metadata-piggyback".to_string(),
                display_name: "Metadata gap".to_string(),
                kind: kind::METADATA_PIGGYBACK.to_string(),
                poll_interval_s: 86_400,
                enabled: None,
                config: None,
            },
        )
        .await
        .unwrap();

        let plugin_manager = Arc::new(PluginManager::with_defaults(Arc::new(conn.clone())));
        let handler = PollReleaseSourceHandler::new(plugin_manager);

        let task = make_task(json!({"source_id": source.id.to_string()}));
        let result = handler.handle(&task, &conn, None).await.unwrap();
        assert!(result.success);
        assert_eq!(
            result.data.unwrap()["skippedReason"],
            "core_source_no_poll_path"
        );
    }

    /// A source whose `plugin_id` doesn't match a `plugins` row records the
    /// error on `last_error` and surfaces a failure result.
    #[tokio::test]
    async fn task_fails_when_plugin_not_registered() {
        let (db, _t) = create_test_db().await;
        let conn = db.sea_orm_connection().clone();
        let _library = LibraryRepository::create(&conn, "L", "/l", ScanningStrategy::Default)
            .await
            .unwrap();
        let source = ReleaseSourceRepository::create(
            &conn,
            NewReleaseSource {
                plugin_id: "release-nonexistent".to_string(),
                source_key: "k".to_string(),
                display_name: "Nope".to_string(),
                kind: kind::RSS_UPLOADER.to_string(),
                poll_interval_s: 3600,
                enabled: None,
                config: None,
            },
        )
        .await
        .unwrap();

        // Pre-existing tracking row makes the path complete.
        let series = SeriesRepository::create(&conn, _library.id, "X", None)
            .await
            .unwrap();
        SeriesTrackingRepository::upsert(
            &conn,
            series.id,
            TrackingUpdate {
                tracked: Some(true),
                ..Default::default()
            },
        )
        .await
        .unwrap();

        let plugin_manager = Arc::new(PluginManager::with_defaults(Arc::new(conn.clone())));
        let handler = PollReleaseSourceHandler::new(plugin_manager);

        let task = make_task(json!({"source_id": source.id.to_string()}));
        let result = handler.handle(&task, &conn, None).await.unwrap();
        assert!(!result.success);

        let after = ReleaseSourceRepository::get_by_id(&conn, source.id)
            .await
            .unwrap()
            .unwrap();
        assert!(after.last_error.is_some());
    }

    fn make_task(params: serde_json::Value) -> tasks::Model {
        tasks::Model {
            id: Uuid::new_v4(),
            task_type: "poll_release_source".to_string(),
            library_id: None,
            series_id: None,
            book_id: None,
            params: Some(params),
            status: "pending".to_string(),
            priority: 170,
            locked_by: None,
            locked_until: None,
            attempts: 0,
            max_attempts: 3,
            last_error: None,
            reschedule_count: 0,
            max_reschedules: 5,
            result: None,
            scheduled_for: Utc::now(),
            created_at: Utc::now(),
            started_at: None,
            completed_at: None,
        }
    }

    // -------------------------------------------------------------------------
    // build_poll_summary — pins the user-facing copy that lands under the
    // Release tracking row's status badge.
    // -------------------------------------------------------------------------

    fn empty_result() -> PollReleaseSourceResult {
        PollReleaseSourceResult {
            source_id: Uuid::new_v4(),
            ..Default::default()
        }
    }

    #[test]
    fn build_poll_summary_reports_not_modified_explicitly() {
        let r = empty_result();
        let s = build_poll_summary(Some(true), None, &r);
        assert_eq!(s, "Up to date — upstream returned 304 (not modified)");
    }

    #[test]
    fn build_poll_summary_zero_items() {
        let r = empty_result();
        let s = build_poll_summary(Some(false), None, &r);
        assert_eq!(s, "Fetched 0 items");
    }

    #[test]
    fn build_poll_summary_one_item_uses_singular() {
        let mut r = empty_result();
        r.candidates_returned = 1;
        r.candidates_recorded = 1;
        let s = build_poll_summary(None, None, &r);
        assert_eq!(s, "Fetched 1 item, recorded 1");
    }

    #[test]
    fn build_poll_summary_includes_dedup_and_threshold_breakdown() {
        let mut r = empty_result();
        r.candidates_returned = 12;
        r.candidates_recorded = 1;
        r.candidates_deduped = 7;
        r.candidates_rejected = 4;
        let s = build_poll_summary(None, None, &r);
        assert_eq!(
            s,
            "Fetched 12 items, recorded 1 (7 already in ledger), dropped 4 below threshold"
        );
    }

    #[test]
    fn build_poll_summary_appends_upstream_warning_for_backoff_status() {
        let mut r = empty_result();
        r.candidates_returned = 0;
        let s = build_poll_summary(None, Some(429), &r);
        assert_eq!(s, "Fetched 0 items · upstream warning: HTTP 429");
    }

    #[test]
    fn build_poll_summary_does_not_append_for_clean_2xx() {
        let mut r = empty_result();
        r.candidates_returned = 2;
        r.candidates_recorded = 2;
        let s = build_poll_summary(None, Some(200), &r);
        assert_eq!(s, "Fetched 2 items, recorded 2");
    }
}
