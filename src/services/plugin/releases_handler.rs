//! Release-source reverse-RPC handler.
//!
//! Plugins that declare the `release_source` capability call these methods
//! to read tracked-series rows scoped to their declared needs (aliases /
//! external IDs), record release candidates in the host-side ledger, and
//! persist per-source state (etag, cursor, etc.) across polls.
//!
//! The dispatcher in [`super::rpc`] checks the plugin's manifest before
//! routing here (see [`super::permissions`]); this handler trusts that the
//! caller has the `release_source` capability and focuses on data scoping
//! and validation.

use std::collections::HashMap;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::Mutex;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use super::protocol::{
    JsonRpcError, JsonRpcRequest, JsonRpcResponse, ReleaseSourceCapability, RequestId, error_codes,
    methods,
};
use crate::db::entities::release_ledger::state as ledger_state;
use crate::db::entities::release_sources::kind as source_kind;
use crate::db::repositories::{
    NewReleaseSource, ReleaseLedgerRepository, ReleaseSourceRepository, SeriesAliasRepository,
    SeriesExternalIdRepository, SeriesRepository, SeriesTrackingRepository, TrackingUpdate,
};
use crate::scheduler::Scheduler;
use crate::services::release::auto_ignore::should_auto_ignore;
use crate::services::release::candidate::ReleaseCandidate;
use crate::services::release::languages::{includes, resolve_for_series};
use crate::services::release::matcher::{evaluate, resolve_threshold};

/// Default page size for `releases/list_tracked` when the caller doesn't
/// specify one. Matches the Phase 3 risk-mitigation note.
const DEFAULT_TRACKED_PAGE_SIZE: u64 = 200;
/// Hard cap on `limit` to keep a single page bounded.
const MAX_TRACKED_PAGE_SIZE: u64 = 1_000;

/// Reverse-RPC handler for the `releases/*` namespace.
///
/// Like [`super::storage_handler::StorageRequestHandler`], one instance is
/// created per plugin connection so the handler captures the plugin's
/// identity and capability declaration without re-querying on every call.
#[derive(Clone)]
pub struct ReleasesRequestHandler {
    db: DatabaseConnection,
    /// Plugin name (`manifest.name`). Must match `release_sources.plugin_id`
    /// for any source the plugin operates on.
    plugin_name: String,
    /// Snapshot of the plugin's `release_source` capability declaration. Used
    /// to scope `releases/list_tracked` responses to what the plugin asked
    /// for.
    capability: ReleaseSourceCapability,
    /// Optional scheduler reference used by `releases/register_sources` to
    /// reconcile schedules immediately after the source set changes.
    scheduler: Option<Arc<Mutex<Scheduler>>>,
}

impl ReleasesRequestHandler {
    pub fn new(
        db: DatabaseConnection,
        plugin_name: String,
        capability: ReleaseSourceCapability,
    ) -> Self {
        Self {
            db,
            plugin_name,
            capability,
            scheduler: None,
        }
    }

    /// Attach a scheduler reference so `releases/register_sources` reconciles
    /// schedules without waiting for a server restart. Builder-style.
    pub fn with_scheduler(mut self, scheduler: Arc<Mutex<Scheduler>>) -> Self {
        self.scheduler = Some(scheduler);
        self
    }

    /// Handle a `releases/*` JSON-RPC request and return a response.
    pub async fn handle_request(&self, request: &JsonRpcRequest) -> JsonRpcResponse {
        let id = request.id.clone();
        let method = request.method.as_str();

        debug!(
            method = method,
            plugin_name = %self.plugin_name,
            "Handling releases request"
        );

        match method {
            methods::RELEASES_LIST_TRACKED => self.handle_list_tracked(request).await,
            methods::RELEASES_RECORD => self.handle_record(request).await,
            methods::RELEASES_SOURCE_STATE_GET => self.handle_state_get(request).await,
            methods::RELEASES_SOURCE_STATE_SET => self.handle_state_set(request).await,
            methods::RELEASES_REGISTER_SOURCES => self.handle_register_sources(request).await,
            _ => JsonRpcResponse::error(
                Some(id),
                JsonRpcError::new(
                    error_codes::METHOD_NOT_FOUND,
                    format!("Unknown releases method: {}", method),
                ),
            ),
        }
    }

    async fn handle_list_tracked(&self, request: &JsonRpcRequest) -> JsonRpcResponse {
        let id = request.id.clone();
        let params: ListTrackedRequest = match parse_params(&request.params) {
            Ok(p) => p,
            Err(resp) => return resp.with_id(id),
        };

        if let Err(resp) = self.assert_source_belongs(&params.source_id, &id).await {
            return resp;
        }

        let limit = params
            .limit
            .map(|n| n.min(MAX_TRACKED_PAGE_SIZE))
            .unwrap_or(DEFAULT_TRACKED_PAGE_SIZE);
        let offset = params.offset.unwrap_or(0);

        // 1. List tracked series IDs.
        let series_ids =
            match SeriesTrackingRepository::list_tracked_ids(&self.db, limit, offset).await {
                Ok(ids) => ids,
                Err(e) => {
                    error!(error = %e, "tracked-series listing failed");
                    return JsonRpcResponse::error(
                        Some(id),
                        JsonRpcError::new(error_codes::INTERNAL_ERROR, format!("db error: {}", e)),
                    );
                }
            };

        // 2. Fetch the tracking rows for those series (so we can return
        //    latest_known_chapter / latest_known_volume).
        let mut entries: Vec<TrackedSeriesEntry> = Vec::with_capacity(series_ids.len());
        for sid in &series_ids {
            let tracking = match SeriesTrackingRepository::get(&self.db, *sid).await {
                Ok(Some(row)) => row,
                Ok(None) => continue, // Race: entry vanished between list and read.
                Err(e) => {
                    error!(error = %e, "series_tracking lookup failed");
                    return JsonRpcResponse::error(
                        Some(id),
                        JsonRpcError::new(error_codes::INTERNAL_ERROR, format!("db error: {}", e)),
                    );
                }
            };
            entries.push(TrackedSeriesEntry {
                series_id: *sid,
                aliases: None,
                external_ids: None,
                latest_known_chapter: tracking.latest_known_chapter,
                latest_known_volume: tracking.latest_known_volume,
            });
        }

        // 3. Scope the response based on what the plugin asked for in its
        //    manifest. Plugins that didn't declare `requires_aliases` don't
        //    get aliases; same for external IDs.
        if self.capability.requires_aliases {
            for entry in &mut entries {
                match SeriesAliasRepository::get_for_series(&self.db, entry.series_id).await {
                    Ok(rows) => {
                        entry.aliases = Some(rows.into_iter().map(|r| r.alias).collect::<Vec<_>>());
                    }
                    Err(e) => {
                        warn!(error = %e, series_id = %entry.series_id, "alias lookup failed");
                    }
                }
            }
        }

        if !self.capability.requires_external_ids.is_empty() {
            for entry in &mut entries {
                match SeriesExternalIdRepository::get_for_series(&self.db, entry.series_id).await {
                    Ok(rows) => {
                        // Filter: only sources the plugin asked for.
                        //
                        // Two namespace conventions exist in stored
                        // `series_external_ids.source` strings:
                        //
                        //   - `api:<service>`    (used by metadata plugins
                        //     like MangaBaka, OpenLibrary, AniList — this is
                        //     the dominant convention and the SDK docs).
                        //   - `plugin:<name>`    (legacy / plugin-private).
                        //
                        // Plugin manifests declare `requiresExternalIds`
                        // with the bare service name (e.g. "mangaupdates"),
                        // so we strip both prefixes before matching. The
                        // returned map is keyed by the bare name so plugins
                        // can read `externalIds["mangaupdates"]` regardless
                        // of how the row was stored.
                        let mut by_source: HashMap<String, String> = HashMap::new();
                        for row in rows {
                            let normalized = strip_external_id_namespace(&row.source);
                            if self
                                .capability
                                .requires_external_ids
                                .iter()
                                .any(|req| req == normalized)
                            {
                                by_source.insert(normalized.to_string(), row.external_id);
                            }
                        }
                        if !by_source.is_empty() {
                            entry.external_ids = Some(by_source);
                        }
                    }
                    Err(e) => {
                        warn!(error = %e, series_id = %entry.series_id, "external_id lookup failed");
                    }
                }
            }
        }

        let next_offset = if (entries.len() as u64) < limit {
            None
        } else {
            Some(offset + entries.len() as u64)
        };

        let response = ListTrackedResponse {
            tracked: entries,
            next_offset,
        };
        JsonRpcResponse::success(id, serde_json::to_value(response).unwrap())
    }

    async fn handle_record(&self, request: &JsonRpcRequest) -> JsonRpcResponse {
        let id = request.id.clone();
        let params: RecordRequest = match parse_params(&request.params) {
            Ok(p) => p,
            Err(resp) => return resp.with_id(id),
        };

        // 1. Verify the source belongs to this plugin.
        if let Err(resp) = self.assert_source_belongs(&params.source_id, &id).await {
            return resp;
        }

        // 2. Look up the tracking row up front. We need it both for the
        //    threshold and (post-insert) for the latest_known_* gate.
        let series_id = params.candidate.series_match.codex_series_id;
        let tracking_row = match SeriesTrackingRepository::get(&self.db, series_id).await {
            Ok(row) => row,
            Err(e) => {
                error!(error = %e, "tracking lookup failed during record");
                return JsonRpcResponse::error(
                    Some(id),
                    JsonRpcError::new(error_codes::INTERNAL_ERROR, format!("db error: {}", e)),
                );
            }
        };
        let threshold = resolve_threshold(
            tracking_row
                .as_ref()
                .and_then(|r| r.confidence_threshold_override),
        );

        // 3. Validate + threshold-gate the candidate.
        let accepted = match evaluate(params.candidate, threshold) {
            Ok(a) => a,
            Err(reason) => {
                debug!(
                    plugin = %self.plugin_name,
                    reject = %reason,
                    "candidate rejected"
                );
                return JsonRpcResponse::error(
                    Some(id),
                    JsonRpcError::new(error_codes::INVALID_PARAMS, reason.to_string()),
                );
            }
        };

        // Snapshot the candidate fields needed for the latest_known_* gate
        // before the move into the ledger entry.
        let candidate_chapter = accepted.candidate.chapter;
        let candidate_volume = accepted.candidate.volume;
        let candidate_language = accepted.candidate.language.clone();

        // Auto-ignore: if the user already owns this volume/chapter, insert
        // the row directly as `ignored` so it skips the inbox + notify path.
        // Best-effort; on failure we fall back to the default state.
        let initial_state = if candidate_volume.is_some() || candidate_chapter.is_some() {
            match SeriesRepository::get_owned_release_keys_for_series(&self.db, series_id).await {
                Ok(owned) => {
                    if should_auto_ignore(candidate_volume, candidate_chapter, &owned) {
                        Some(ledger_state::IGNORED.to_string())
                    } else {
                        None
                    }
                }
                Err(e) => {
                    warn!(error = %e, %series_id, "owned-keys lookup failed; defaulting to announced");
                    None
                }
            }
        } else {
            None
        };

        // 4. Hand off to the ledger (which is itself idempotent).
        let mut entry = accepted.into_ledger_entry(params.source_id);
        entry.initial_state = initial_state;
        let outcome = match ReleaseLedgerRepository::record(&self.db, entry).await {
            Ok(o) => o,
            Err(e) => {
                error!(error = %e, "ledger record failed");
                return JsonRpcResponse::error(
                    Some(id),
                    JsonRpcError::new(error_codes::INTERNAL_ERROR, format!("db error: {}", e)),
                );
            }
        };

        // 5. Advance series_tracking.latest_known_* to the high-water mark.
        //
        //    Skipped on dedup (the ledger already saw this release; we don't
        //    re-tick the high-water mark). Gated on the per-axis track_*
        //    flag — a series tracked only for volumes shouldn't have its
        //    chapter mark moved by chapter announcements. Also gated on the
        //    candidate's language being in the effective list, so that a
        //    plugin which forgets to filter by language can't pollute
        //    `latest_known_*` with out-of-language releases.
        if !outcome.deduped {
            if let Err(e) = self
                .advance_latest_known(
                    series_id,
                    tracking_row.as_ref(),
                    candidate_chapter,
                    candidate_volume,
                    &candidate_language,
                )
                .await
            {
                // The ledger row is already persisted; a follow-up tracking
                // failure is logged but does not fail the call. The next
                // successful record will catch up.
                warn!(error = %e, %series_id, "latest_known advance failed; ledger insert preserved");
            }

            // Emit through the task-local recording broadcaster set up by
            // `crate::tasks::worker` around the running task. This routes
            // the event into `tasks.result.emitted_events` so the web
            // server's `TaskListener` replays it to live SSE subscribers in
            // distributed deployments. In single-process mode the same
            // task-local points at the live broadcaster, so subscribers see
            // the event directly.
            //
            // No task-local set means we're handling a reverse-RPC outside
            // any task context (today: shouldn't happen for releases since
            // every record path runs inside a poll task). We log and skip
            // rather than silently emitting into a void.
            // Auto-ignored rows skip the announce event: the row is on the
            // ledger for audit/recovery, but the user already owns the
            // matching volume/chapter so there's nothing to notify about.
            if outcome.row.state != ledger_state::ANNOUNCED {
                debug!(
                    series_id = %outcome.row.series_id,
                    plugin = %self.plugin_name,
                    state = %outcome.row.state,
                    "Skipping release_announced emit for non-announced state"
                );
            } else if let Some(broadcaster) = crate::events::current_recording_broadcaster() {
                let _ = broadcaster.emit(crate::events::EntityChangeEvent::release_announced(
                    &outcome.row,
                    &self.plugin_name,
                ));
            } else {
                debug!(
                    series_id = %outcome.row.series_id,
                    plugin = %self.plugin_name,
                    "No recording broadcaster in scope; skipping release_announced emit"
                );
            }
        }

        let resp = RecordResponse {
            ledger_id: outcome.row.id,
            deduped: outcome.deduped,
        };
        JsonRpcResponse::success(id, serde_json::to_value(resp).unwrap())
    }

    /// Move `series_tracking.latest_known_chapter` and `latest_known_volume`
    /// forward to the candidate's values, gated on the per-axis `track_*` flag
    /// and the per-series effective language list. Stale candidates (smaller
    /// than current) and out-of-language candidates are silently no-ops on
    /// their respective axes. Out-of-language candidates skip *both* axes
    /// because the language gate sits above per-axis tracking.
    async fn advance_latest_known(
        &self,
        series_id: Uuid,
        tracking_row: Option<&crate::db::entities::series_tracking::Model>,
        candidate_chapter: Option<f64>,
        candidate_volume: Option<i32>,
        candidate_language: &str,
    ) -> Result<(), anyhow::Error> {
        // No tracking row → series isn't being tracked. Don't auto-create one
        // just because a stray candidate came in; the user explicitly opts in
        // via the tracking panel.
        let Some(row) = tracking_row else {
            return Ok(());
        };
        if !row.tracked {
            return Ok(());
        }

        // Language gate: out-of-language candidates do not advance the
        // high-water mark even if a buggy plugin records them.
        let effective = resolve_for_series(&self.db, row.languages.as_ref()).await?;
        if !includes(&effective, candidate_language) {
            return Ok(());
        }

        let mut update = TrackingUpdate::default();
        let mut dirty = false;

        if let Some(ch) = candidate_chapter
            && row.track_chapters
            && ch.is_finite()
        {
            let current = row.latest_known_chapter.unwrap_or(f64::NEG_INFINITY);
            if ch > current {
                update.latest_known_chapter = Some(Some(ch));
                dirty = true;
            }
        }

        if let Some(vol) = candidate_volume
            && row.track_volumes
        {
            let current = row.latest_known_volume.unwrap_or(i32::MIN);
            if vol > current {
                update.latest_known_volume = Some(Some(vol));
                dirty = true;
            }
        }

        if !dirty {
            return Ok(());
        }
        SeriesTrackingRepository::upsert(&self.db, series_id, update).await?;
        Ok(())
    }

    async fn handle_state_get(&self, request: &JsonRpcRequest) -> JsonRpcResponse {
        let id = request.id.clone();
        let params: SourceStateGetRequest = match parse_params(&request.params) {
            Ok(p) => p,
            Err(resp) => return resp.with_id(id),
        };

        if let Err(resp) = self.assert_source_belongs(&params.source_id, &id).await {
            return resp;
        }

        match ReleaseSourceRepository::get_by_id(&self.db, params.source_id).await {
            Ok(Some(row)) => {
                let resp = SourceStateView {
                    etag: row.etag,
                    last_polled_at: row.last_polled_at,
                    last_error: row.last_error,
                    last_error_at: row.last_error_at,
                };
                JsonRpcResponse::success(id, serde_json::to_value(resp).unwrap())
            }
            Ok(None) => JsonRpcResponse::error(
                Some(id),
                JsonRpcError::new(error_codes::NOT_FOUND, "source not found"),
            ),
            Err(e) => {
                error!(error = %e, "source state read failed");
                JsonRpcResponse::error(
                    Some(id),
                    JsonRpcError::new(error_codes::INTERNAL_ERROR, format!("db error: {}", e)),
                )
            }
        }
    }

    async fn handle_state_set(&self, request: &JsonRpcRequest) -> JsonRpcResponse {
        let id = request.id.clone();
        let params: SourceStateSetRequest = match parse_params(&request.params) {
            Ok(p) => p,
            Err(resp) => return resp.with_id(id),
        };

        if let Err(resp) = self.assert_source_belongs(&params.source_id, &id).await {
            return resp;
        }

        // Only `etag` is plugin-writable here. `last_polled_at` is set by the
        // host's poll task; status fields (`last_error`) are owned by the
        // host. If a plugin needs richer per-source state, it should use
        // `storage/*` against its own KV bucket.
        if params.etag.is_none() {
            return JsonRpcResponse::error(
                Some(id),
                JsonRpcError::new(error_codes::INVALID_PARAMS, "no writable fields supplied"),
            );
        }

        // record_poll_success has the side effect of clearing `last_error` —
        // that's not what plugins want here. Instead update etag in-place via
        // a small read-modify-write.
        let row = match ReleaseSourceRepository::get_by_id(&self.db, params.source_id).await {
            Ok(Some(r)) => r,
            Ok(None) => {
                return JsonRpcResponse::error(
                    Some(id),
                    JsonRpcError::new(error_codes::NOT_FOUND, "source not found"),
                );
            }
            Err(e) => {
                error!(error = %e, "source state lookup failed");
                return JsonRpcResponse::error(
                    Some(id),
                    JsonRpcError::new(error_codes::INTERNAL_ERROR, format!("db error: {}", e)),
                );
            }
        };

        use sea_orm::{ActiveModelTrait, Set};
        let mut active: crate::db::entities::release_sources::ActiveModel = row.into();
        active.etag = Set(params.etag.clone());
        active.updated_at = Set(Utc::now());
        match active.update(&self.db).await {
            Ok(_) => JsonRpcResponse::success(
                id,
                serde_json::json!({"success": true, "etag": params.etag}),
            ),
            Err(e) => {
                error!(error = %e, "source state write failed");
                JsonRpcResponse::error(
                    Some(id),
                    JsonRpcError::new(error_codes::INTERNAL_ERROR, format!("db error: {}", e)),
                )
            }
        }
    }

    /// Replace the set of `release_sources` rows owned by this plugin.
    ///
    /// This is the materialization endpoint plugins call from `onInitialize`
    /// (and on any subsequent config change, which is delivered via plugin
    /// process restart). Each call carries the plugin's full desired-state
    /// list:
    ///
    /// - **Upsert** every entry on `(plugin_id, source_key)`. New rows are
    ///   inserted; existing rows have only the plugin-owned descriptive
    ///   fields refreshed. User-managed fields (`enabled`, `cron_schedule`)
    ///   survive across re-registrations so an admin's schedule override or
    ///   disable toggle isn't trampled when the plugin restarts.
    /// - **Prune** rows owned by this plugin whose `source_key` is not in the
    ///   request. Deletes cascade to `release_ledger`. An empty `sources`
    ///   list wipes the plugin's row set, which is the correct behavior when
    ///   an admin clears the plugin's config.
    /// - **Reconcile** the scheduler so newly-registered sources start polling
    ///   on their next cron tick (and pruned ones stop). Best-effort: if the
    ///   reconcile fails (or no scheduler is wired), the call still succeeds
    ///   because the row writes are persisted.
    ///
    /// `kind` is validated against the `release_source` capability the plugin
    /// declared in its manifest, so a plugin can't register sources of a
    /// `kind` outside its declared capability surface. New rows always start
    /// with `cron_schedule = NULL` (inherit the server-wide default); admins
    /// override per-row in the settings UI.
    async fn handle_register_sources(&self, request: &JsonRpcRequest) -> JsonRpcResponse {
        let id = request.id.clone();
        let params: RegisterSourcesRequest = match parse_params(&request.params) {
            Ok(p) => p,
            Err(resp) => return resp.with_id(id),
        };

        // Validate every source up front so we don't write partial state on a
        // bad request.
        for src in &params.sources {
            if src.source_key.trim().is_empty() {
                return JsonRpcResponse::error(
                    Some(id),
                    JsonRpcError::new(error_codes::INVALID_PARAMS, "source_key cannot be empty"),
                );
            }
            if src.display_name.trim().is_empty() {
                return JsonRpcResponse::error(
                    Some(id),
                    JsonRpcError::new(error_codes::INVALID_PARAMS, "display_name cannot be empty"),
                );
            }
            if !source_kind::is_valid(&src.kind) {
                return JsonRpcResponse::error(
                    Some(id),
                    JsonRpcError::new(
                        error_codes::INVALID_PARAMS,
                        format!("invalid kind: {}", src.kind),
                    ),
                );
            }
            if !self.capability.kinds.iter().any(|k| k.as_str() == src.kind) {
                return JsonRpcResponse::error(
                    Some(id),
                    JsonRpcError::new(
                        error_codes::INVALID_PARAMS,
                        format!(
                            "kind {} not declared in plugin's release_source capability",
                            src.kind
                        ),
                    ),
                );
            }
        }
        // Reject duplicate source_keys in the same request — they would
        // collapse to one row at upsert time and silently drop the second
        // entry's display_name/config, which is almost always a plugin bug.
        let mut seen: std::collections::HashSet<&str> = std::collections::HashSet::new();
        for src in &params.sources {
            if !seen.insert(src.source_key.as_str()) {
                return JsonRpcResponse::error(
                    Some(id),
                    JsonRpcError::new(
                        error_codes::INVALID_PARAMS,
                        format!("duplicate source_key in request: {}", src.source_key),
                    ),
                );
            }
        }

        let keep_keys: Vec<String> = params
            .sources
            .iter()
            .map(|s| s.source_key.clone())
            .collect();

        // Upsert each source. New rows start with `cron_schedule = NULL`,
        // i.e. they inherit the server-wide
        // `release_tracking.default_cron_schedule`. Admins override per-row
        // via the settings UI; existing rows preserve their override on
        // re-register.
        let mut registered = 0u32;
        for src in params.sources {
            let new = NewReleaseSource {
                plugin_id: self.plugin_name.clone(),
                source_key: src.source_key,
                display_name: src.display_name,
                kind: src.kind,
                enabled: None,
                config: src.config,
            };
            if let Err(e) = ReleaseSourceRepository::upsert(&self.db, new).await {
                error!(error = %e, "release source upsert failed");
                return JsonRpcResponse::error(
                    Some(id),
                    JsonRpcError::new(error_codes::INTERNAL_ERROR, format!("db error: {}", e)),
                );
            }
            registered += 1;
        }

        // Prune sources the plugin no longer declares.
        let pruned = match ReleaseSourceRepository::delete_by_plugin_excluding(
            &self.db,
            &self.plugin_name,
            &keep_keys,
        )
        .await
        {
            Ok(n) => n,
            Err(e) => {
                error!(error = %e, "release source prune failed");
                return JsonRpcResponse::error(
                    Some(id),
                    JsonRpcError::new(error_codes::INTERNAL_ERROR, format!("db error: {}", e)),
                );
            }
        };

        info!(
            plugin = %self.plugin_name,
            registered,
            pruned,
            "release sources registered"
        );

        // Reconcile schedules. Best-effort — log failures but don't fail the
        // RPC, since the rows are already persisted and the next scheduler
        // start (or HTTP-driven reconcile) will catch up.
        if let Some(ref scheduler) = self.scheduler {
            let mut guard = scheduler.lock().await;
            if let Err(e) = guard.reconcile_release_sources().await {
                warn!(error = %e, "scheduler reconcile after register_sources failed");
            }
        }

        let response = RegisterSourcesResponse {
            registered,
            pruned: pruned as u32,
        };
        JsonRpcResponse::success(id, serde_json::to_value(response).unwrap())
    }

    /// Confirm `source_id` exists and belongs to the calling plugin. Returns
    /// an error response if either check fails.
    async fn assert_source_belongs(
        &self,
        source_id: &Uuid,
        request_id: &RequestId,
    ) -> Result<(), JsonRpcResponse> {
        let row = match ReleaseSourceRepository::get_by_id(&self.db, *source_id).await {
            Ok(Some(r)) => r,
            Ok(None) => {
                return Err(JsonRpcResponse::error(
                    Some(request_id.clone()),
                    JsonRpcError::new(error_codes::NOT_FOUND, "source not found"),
                ));
            }
            Err(e) => {
                error!(error = %e, "source lookup failed");
                return Err(JsonRpcResponse::error(
                    Some(request_id.clone()),
                    JsonRpcError::new(error_codes::INTERNAL_ERROR, format!("db error: {}", e)),
                ));
            }
        };
        if row.plugin_id != self.plugin_name {
            warn!(
                source_id = %source_id,
                source_plugin = %row.plugin_id,
                caller = %self.plugin_name,
                "plugin tried to operate on a source it does not own"
            );
            return Err(JsonRpcResponse::error(
                Some(request_id.clone()),
                JsonRpcError::new(
                    error_codes::AUTH_FAILED,
                    "source does not belong to calling plugin",
                ),
            ));
        }
        Ok(())
    }
}

// =============================================================================
// Wire-format request/response types
// =============================================================================

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ListTrackedRequest {
    source_id: Uuid,
    #[serde(default)]
    limit: Option<u64>,
    /// Offset-based pagination is the simplest fit for SeaORM's
    /// `list_tracked_ids` helper. Plugins call with `next_offset` from the
    /// previous response.
    #[serde(default)]
    offset: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ListTrackedResponse {
    tracked: Vec<TrackedSeriesEntry>,
    #[serde(skip_serializing_if = "Option::is_none")]
    next_offset: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TrackedSeriesEntry {
    series_id: Uuid,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    aliases: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    external_ids: Option<HashMap<String, String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    latest_known_chapter: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    latest_known_volume: Option<i32>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RecordRequest {
    source_id: Uuid,
    candidate: ReleaseCandidate,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RecordResponse {
    ledger_id: Uuid,
    deduped: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SourceStateGetRequest {
    source_id: Uuid,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SourceStateSetRequest {
    source_id: Uuid,
    /// Only `etag` is plugin-writable. Future plugin-controlled fields can
    /// be added here.
    #[serde(default)]
    etag: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RegisterSourcesRequest {
    sources: Vec<RegisteredSourceInput>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RegisteredSourceInput {
    /// Stable per-plugin identifier for the source. Opaque to the host.
    source_key: String,
    /// Human-readable label shown in the Release tracking settings table.
    display_name: String,
    /// One of the canonical `release_sources.kind` values; must also be
    /// declared in the plugin's `release_source` capability.
    kind: String,
    /// Optional opaque per-source config snapshot. Stored on the row for
    /// the host's reference; the plugin reads its own admin config directly.
    #[serde(default)]
    config: Option<Value>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct RegisterSourcesResponse {
    /// Number of sources upserted (created or refreshed).
    registered: u32,
    /// Number of sources removed because they were not in the request.
    pruned: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SourceStateView {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    etag: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    last_polled_at: Option<DateTime<Utc>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    last_error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    last_error_at: Option<DateTime<Utc>>,
}

/// Strip a leading namespace prefix (`api:`, `plugin:`) from an external-ID
/// `source` string and return the bare service name.
///
/// Stored `series_external_ids.source` values use one of:
///   - `api:<service>` (dominant; written by metadata plugins like
///     MangaBaka, OpenLibrary, AniList).
///   - `plugin:<name>` (legacy plugin-private form).
///   - `<service>` (bare; older rows).
///
/// Plugin manifests declare `requiresExternalIds` with the bare service
/// name, so we normalize on read. Anything else (`urn:...`, `mal:`, etc.)
/// passes through unchanged.
pub(crate) fn strip_external_id_namespace(source: &str) -> &str {
    if let Some(rest) = source.strip_prefix("api:") {
        return rest;
    }
    if let Some(rest) = source.strip_prefix("plugin:") {
        return rest;
    }
    source
}

// =============================================================================
// Param parsing helpers
// =============================================================================

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

trait WithId {
    fn with_id(self, id: RequestId) -> Self;
}

impl WithId for JsonRpcResponse {
    fn with_id(mut self, id: RequestId) -> Self {
        self.id = Some(id);
        self
    }
}

/// Whether a method belongs to the `releases/*` namespace.
pub fn is_releases_method(method: &str) -> bool {
    matches!(
        method,
        "releases/list_tracked"
            | "releases/record"
            | "releases/source_state/get"
            | "releases/source_state/set"
            | "releases/register_sources"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::ScanningStrategy;
    use crate::db::entities::release_sources::{self, kind};
    use crate::db::repositories::{
        LibraryRepository, NewReleaseSource, ReleaseSourceRepository, ReleaseSourceUpdate,
        SeriesAliasRepository, SeriesExternalIdRepository, SeriesRepository,
        SeriesTrackingRepository, TrackingUpdate,
    };
    use crate::db::test_helpers::create_test_db;
    use crate::services::plugin::protocol::ReleaseSourceKind;
    use crate::services::release::candidate::SeriesMatch;
    use serde_json::json;

    fn make_capability(
        requires_aliases: bool,
        requires_external_ids: Vec<&str>,
    ) -> ReleaseSourceCapability {
        ReleaseSourceCapability {
            kinds: vec![ReleaseSourceKind::RssUploader],
            requires_aliases,
            requires_external_ids: requires_external_ids
                .into_iter()
                .map(|s| s.to_string())
                .collect(),
            can_announce_chapters: true,
            can_announce_volumes: true,
        }
    }

    async fn setup(db: &DatabaseConnection, plugin_name: &str) -> (Uuid, Uuid) {
        let library = LibraryRepository::create(db, "Lib", "/lib", ScanningStrategy::Default)
            .await
            .unwrap();
        let series = SeriesRepository::create(db, library.id, "Series", None)
            .await
            .unwrap();
        SeriesTrackingRepository::upsert(
            db,
            series.id,
            TrackingUpdate {
                tracked: Some(true),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        let source = ReleaseSourceRepository::create(
            db,
            NewReleaseSource {
                plugin_id: plugin_name.to_string(),
                source_key: "feed:1".to_string(),
                display_name: "Feed 1".to_string(),
                kind: kind::RSS_UPLOADER.to_string(),
                enabled: None,
                config: None,
            },
        )
        .await
        .unwrap();
        (series.id, source.id)
    }

    fn make_request(method: &str, params: Value) -> JsonRpcRequest {
        JsonRpcRequest::new(1i64, method, Some(params))
    }

    fn good_candidate(series_id: Uuid) -> ReleaseCandidate {
        ReleaseCandidate {
            series_match: SeriesMatch {
                codex_series_id: series_id,
                confidence: 0.95,
                reason: "alias-exact".to_string(),
            },
            external_release_id: "rel-1".to_string(),
            chapter: Some(143.0),
            volume: None,
            language: "en".to_string(),
            format_hints: None,
            group_or_uploader: Some("tsuna69".to_string()),
            payload_url: "https://example.com/r/1".to_string(),
            media_url: None,
            media_url_kind: None,
            info_hash: None,
            metadata: None,
            observed_at: Utc::now(),
        }
    }

    #[tokio::test]
    async fn list_tracked_returns_tracked_series_only() {
        let (db, _t) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let (_series, source_id) = setup(conn, "release-nyaa").await;

        let handler = ReleasesRequestHandler::new(
            conn.clone(),
            "release-nyaa".to_string(),
            make_capability(false, vec![]),
        );
        let req = make_request(
            methods::RELEASES_LIST_TRACKED,
            json!({"sourceId": source_id}),
        );
        let resp = handler.handle_request(&req).await;
        assert!(!resp.is_error(), "unexpected error: {:?}", resp.error);
        let body: ListTrackedResponse = serde_json::from_value(resp.result.unwrap()).unwrap();
        assert_eq!(body.tracked.len(), 1);
        // No aliases/external_ids requested.
        assert!(body.tracked[0].aliases.is_none());
        assert!(body.tracked[0].external_ids.is_none());
    }

    #[tokio::test]
    async fn list_tracked_includes_aliases_when_requested() {
        let (db, _t) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let (series_id, source_id) = setup(conn, "release-nyaa").await;
        SeriesAliasRepository::create(conn, series_id, "Punpun", "manual")
            .await
            .unwrap();

        let handler = ReleasesRequestHandler::new(
            conn.clone(),
            "release-nyaa".to_string(),
            make_capability(true, vec![]),
        );
        let req = make_request(
            methods::RELEASES_LIST_TRACKED,
            json!({"sourceId": source_id}),
        );
        let resp = handler.handle_request(&req).await;
        let body: ListTrackedResponse = serde_json::from_value(resp.result.unwrap()).unwrap();
        let entry = &body.tracked[0];
        let aliases = entry.aliases.as_ref().unwrap();
        assert_eq!(aliases, &vec!["Punpun".to_string()]);
    }

    #[tokio::test]
    async fn list_tracked_filters_external_ids_to_declared_sources() {
        let (db, _t) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let (series_id, source_id) = setup(conn, "release-mu").await;
        // Two external IDs - one matching the manifest, one not.
        SeriesExternalIdRepository::upsert(
            conn,
            series_id,
            "plugin:mangaupdates",
            "12345",
            None,
            None,
        )
        .await
        .unwrap();
        SeriesExternalIdRepository::upsert(conn, series_id, "plugin:anilist", "999", None, None)
            .await
            .unwrap();

        let handler = ReleasesRequestHandler::new(
            conn.clone(),
            "release-mu".to_string(),
            make_capability(false, vec!["mangaupdates"]),
        );
        let req = make_request(
            methods::RELEASES_LIST_TRACKED,
            json!({"sourceId": source_id}),
        );
        let resp = handler.handle_request(&req).await;
        let body: ListTrackedResponse = serde_json::from_value(resp.result.unwrap()).unwrap();
        let ext = body.tracked[0].external_ids.as_ref().unwrap();
        assert_eq!(ext.len(), 1, "only requested source should leak");
        assert_eq!(ext.get("mangaupdates").map(String::as_str), Some("12345"));
        assert!(ext.get("anilist").is_none());
    }

    #[tokio::test]
    async fn list_tracked_accepts_api_prefixed_external_ids() {
        // Regression: MangaBaka writes external IDs as `api:mangaupdates`
        // (the dominant convention per the SDK docs). The host used to
        // strip only `plugin:`, so MangaUpdates plugins received zero IDs
        // and reported "Fetched 0 items" forever. Strip both prefixes.
        let (db, _t) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let (series_id, source_id) = setup(conn, "release-mu").await;

        SeriesExternalIdRepository::upsert(
            conn,
            series_id,
            "api:mangaupdates",
            "12345",
            None,
            None,
        )
        .await
        .unwrap();

        let handler = ReleasesRequestHandler::new(
            conn.clone(),
            "release-mu".to_string(),
            make_capability(false, vec!["mangaupdates"]),
        );
        let req = make_request(
            methods::RELEASES_LIST_TRACKED,
            json!({"sourceId": source_id}),
        );
        let resp = handler.handle_request(&req).await;
        let body: ListTrackedResponse = serde_json::from_value(resp.result.unwrap()).unwrap();
        let ext = body.tracked[0].external_ids.as_ref().unwrap();
        assert_eq!(
            ext.get("mangaupdates").map(String::as_str),
            Some("12345"),
            "api: prefix should be stripped to match bare-name manifest declaration"
        );
    }

    #[test]
    fn strip_external_id_namespace_handles_known_prefixes() {
        assert_eq!(
            strip_external_id_namespace("api:mangaupdates"),
            "mangaupdates"
        );
        assert_eq!(strip_external_id_namespace("plugin:anilist"), "anilist");
        assert_eq!(strip_external_id_namespace("mangadex"), "mangadex");
        // Unknown prefixes pass through — we'd rather fail closed than guess.
        assert_eq!(
            strip_external_id_namespace("urn:isbn:1234"),
            "urn:isbn:1234"
        );
        assert_eq!(strip_external_id_namespace(""), "");
    }

    #[tokio::test]
    async fn list_tracked_rejects_source_owned_by_other_plugin() {
        let (db, _t) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let (_series, source_id) = setup(conn, "release-nyaa").await;

        let handler = ReleasesRequestHandler::new(
            conn.clone(),
            "release-other".to_string(),
            make_capability(false, vec![]),
        );
        let req = make_request(
            methods::RELEASES_LIST_TRACKED,
            json!({"sourceId": source_id}),
        );
        let resp = handler.handle_request(&req).await;
        assert!(resp.is_error());
        assert_eq!(resp.error.unwrap().code, error_codes::AUTH_FAILED);
    }

    /// `releases/record` emits a `ReleaseAnnounced` event on insert (via the
    /// task-local recording broadcaster set up by the worker) and suppresses
    /// it on dedup.
    #[tokio::test]
    async fn record_emits_release_announced_on_insert_only() {
        use crate::events::{EntityEvent, EventBroadcaster, with_recording_broadcaster};

        let (db, _t) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let (series_id, source_id) = setup(conn, "release-nyaa").await;

        let broadcaster = std::sync::Arc::new(EventBroadcaster::new(8));
        let mut rx = broadcaster.subscribe();

        let handler = ReleasesRequestHandler::new(
            conn.clone(),
            "release-nyaa".to_string(),
            make_capability(false, vec![]),
        );

        let cand = good_candidate(series_id);
        let req = make_request(
            methods::RELEASES_RECORD,
            json!({"sourceId": source_id, "candidate": cand}),
        );

        let req_clone = req.clone();
        let handler_clone = handler.clone();
        let first = with_recording_broadcaster(broadcaster.clone(), async move {
            handler_clone.handle_request(&req_clone).await
        })
        .await;
        assert!(!first.is_error(), "unexpected error: {:?}", first.error);
        let body: RecordResponse = serde_json::from_value(first.result.unwrap()).unwrap();
        assert!(!body.deduped);

        let event = rx.try_recv().expect("expected ReleaseAnnounced");
        match event.event {
            EntityEvent::ReleaseAnnounced {
                series_id: ev_series,
                source_id: ev_source,
                plugin_id,
                chapter,
                language,
                ..
            } => {
                assert_eq!(ev_series, series_id);
                assert_eq!(ev_source, source_id);
                assert_eq!(plugin_id, "release-nyaa");
                assert_eq!(chapter, Some(143.0));
                assert_eq!(language, "en");
            }
            other => panic!("unexpected event: {:?}", other),
        }

        // Re-recording the same release dedups; no new event should fire.
        let req_clone = req.clone();
        let handler_clone = handler.clone();
        let second = with_recording_broadcaster(broadcaster.clone(), async move {
            handler_clone.handle_request(&req_clone).await
        })
        .await;
        let body: RecordResponse = serde_json::from_value(second.result.unwrap()).unwrap();
        assert!(body.deduped);
        assert!(
            rx.try_recv().is_err(),
            "dedup must not emit a new ReleaseAnnounced event"
        );
    }

    /// Without a task-local recording broadcaster in scope, `releases/record`
    /// completes successfully but emits no event (the operation is logged
    /// at debug; we don't surface a fake "live" emit anywhere).
    #[tokio::test]
    async fn record_skips_emit_when_no_broadcaster_in_scope() {
        let (db, _t) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let (series_id, source_id) = setup(conn, "release-nyaa").await;

        let handler = ReleasesRequestHandler::new(
            conn.clone(),
            "release-nyaa".to_string(),
            make_capability(false, vec![]),
        );

        let cand = good_candidate(series_id);
        let req = make_request(
            methods::RELEASES_RECORD,
            json!({"sourceId": source_id, "candidate": cand}),
        );

        let resp = handler.handle_request(&req).await;
        assert!(!resp.is_error(), "unexpected error: {:?}", resp.error);
        let body: RecordResponse = serde_json::from_value(resp.result.unwrap()).unwrap();
        assert!(!body.deduped, "ledger row still inserted");
    }

    #[tokio::test]
    async fn record_inserts_then_dedups() {
        let (db, _t) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let (series_id, source_id) = setup(conn, "release-nyaa").await;

        let handler = ReleasesRequestHandler::new(
            conn.clone(),
            "release-nyaa".to_string(),
            make_capability(false, vec![]),
        );
        let cand = good_candidate(series_id);
        let req = make_request(
            methods::RELEASES_RECORD,
            json!({"sourceId": source_id, "candidate": cand}),
        );

        let first = handler.handle_request(&req).await;
        assert!(!first.is_error(), "unexpected error: {:?}", first.error);
        let body: RecordResponse = serde_json::from_value(first.result.unwrap()).unwrap();
        assert!(!body.deduped);

        let second = handler.handle_request(&req).await;
        let body: RecordResponse = serde_json::from_value(second.result.unwrap()).unwrap();
        assert!(body.deduped, "second insert should dedup");
    }

    #[tokio::test]
    async fn record_drops_below_threshold_candidate() {
        let (db, _t) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let (series_id, source_id) = setup(conn, "release-nyaa").await;

        let handler = ReleasesRequestHandler::new(
            conn.clone(),
            "release-nyaa".to_string(),
            make_capability(false, vec![]),
        );
        let mut cand = good_candidate(series_id);
        cand.series_match.confidence = 0.5;
        let req = make_request(
            methods::RELEASES_RECORD,
            json!({"sourceId": source_id, "candidate": cand}),
        );
        let resp = handler.handle_request(&req).await;
        assert!(resp.is_error());
        let err = resp.error.unwrap();
        assert_eq!(err.code, error_codes::INVALID_PARAMS);
        assert!(err.message.contains("below threshold"));
    }

    #[tokio::test]
    async fn record_honors_per_series_threshold_override() {
        let (db, _t) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let (series_id, source_id) = setup(conn, "release-nyaa").await;
        // Lower threshold for this series only.
        SeriesTrackingRepository::upsert(
            conn,
            series_id,
            TrackingUpdate {
                confidence_threshold_override: Some(Some(0.4)),
                ..Default::default()
            },
        )
        .await
        .unwrap();

        let handler = ReleasesRequestHandler::new(
            conn.clone(),
            "release-nyaa".to_string(),
            make_capability(false, vec![]),
        );
        let mut cand = good_candidate(series_id);
        cand.series_match.confidence = 0.5;
        let req = make_request(
            methods::RELEASES_RECORD,
            json!({"sourceId": source_id, "candidate": cand}),
        );
        let resp = handler.handle_request(&req).await;
        assert!(!resp.is_error(), "override should accept 0.5 candidate");
    }

    #[tokio::test]
    async fn record_rejects_source_owned_by_other_plugin() {
        let (db, _t) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let (series_id, source_id) = setup(conn, "release-nyaa").await;

        let handler = ReleasesRequestHandler::new(
            conn.clone(),
            "release-other".to_string(),
            make_capability(false, vec![]),
        );
        let cand = good_candidate(series_id);
        let req = make_request(
            methods::RELEASES_RECORD,
            json!({"sourceId": source_id, "candidate": cand}),
        );
        let resp = handler.handle_request(&req).await;
        assert!(resp.is_error());
        assert_eq!(resp.error.unwrap().code, error_codes::AUTH_FAILED);
    }

    #[tokio::test]
    async fn source_state_get_returns_view() {
        let (db, _t) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let (_series, source_id) = setup(conn, "release-nyaa").await;
        ReleaseSourceRepository::record_poll_success(
            conn,
            source_id,
            Utc::now(),
            Some("etag-123".to_string()),
            None,
        )
        .await
        .unwrap();

        let handler = ReleasesRequestHandler::new(
            conn.clone(),
            "release-nyaa".to_string(),
            make_capability(false, vec![]),
        );
        let req = make_request(
            methods::RELEASES_SOURCE_STATE_GET,
            json!({"sourceId": source_id}),
        );
        let resp = handler.handle_request(&req).await;
        assert!(!resp.is_error());
        let body: SourceStateView = serde_json::from_value(resp.result.unwrap()).unwrap();
        assert_eq!(body.etag.as_deref(), Some("etag-123"));
        assert!(body.last_polled_at.is_some());
    }

    #[tokio::test]
    async fn source_state_set_writes_etag() {
        let (db, _t) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let (_series, source_id) = setup(conn, "release-nyaa").await;

        let handler = ReleasesRequestHandler::new(
            conn.clone(),
            "release-nyaa".to_string(),
            make_capability(false, vec![]),
        );
        let req = make_request(
            methods::RELEASES_SOURCE_STATE_SET,
            json!({"sourceId": source_id, "etag": "\"abc\""}),
        );
        let resp = handler.handle_request(&req).await;
        assert!(!resp.is_error());

        let row = ReleaseSourceRepository::get_by_id(conn, source_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(row.etag.as_deref(), Some("\"abc\""));
    }

    #[tokio::test]
    async fn source_state_set_rejects_when_no_writable_field() {
        let (db, _t) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let (_series, source_id) = setup(conn, "release-nyaa").await;

        let handler = ReleasesRequestHandler::new(
            conn.clone(),
            "release-nyaa".to_string(),
            make_capability(false, vec![]),
        );
        let req = make_request(
            methods::RELEASES_SOURCE_STATE_SET,
            json!({"sourceId": source_id}),
        );
        let resp = handler.handle_request(&req).await;
        assert!(resp.is_error());
        assert_eq!(resp.error.unwrap().code, error_codes::INVALID_PARAMS);
    }

    #[tokio::test]
    async fn unknown_method_returns_method_not_found() {
        let (db, _t) = create_test_db().await;
        let conn = db.sea_orm_connection();

        let handler = ReleasesRequestHandler::new(
            conn.clone(),
            "release-nyaa".to_string(),
            make_capability(false, vec![]),
        );
        let req = make_request("releases/unknown", json!({}));
        let resp = handler.handle_request(&req).await;
        assert!(resp.is_error());
        assert_eq!(resp.error.unwrap().code, error_codes::METHOD_NOT_FOUND);
    }

    #[test]
    fn is_releases_method_detects_namespace() {
        assert!(is_releases_method(methods::RELEASES_LIST_TRACKED));
        assert!(is_releases_method(methods::RELEASES_RECORD));
        assert!(is_releases_method(methods::RELEASES_SOURCE_STATE_GET));
        assert!(is_releases_method(methods::RELEASES_SOURCE_STATE_SET));
        assert!(is_releases_method(methods::RELEASES_REGISTER_SOURCES));
        assert!(!is_releases_method("releases/poll"));
        assert!(!is_releases_method("storage/get"));
    }

    // -------------------------------------------------------------------------
    // latest_known_* advancement tests (Phase 6)
    // -------------------------------------------------------------------------

    async fn record_candidate(
        handler: &ReleasesRequestHandler,
        source_id: Uuid,
        cand: ReleaseCandidate,
    ) -> RecordResponse {
        let req = make_request(
            methods::RELEASES_RECORD,
            json!({"sourceId": source_id, "candidate": cand}),
        );
        let resp = handler.handle_request(&req).await;
        assert!(!resp.is_error(), "unexpected error: {:?}", resp.error);
        serde_json::from_value(resp.result.unwrap()).unwrap()
    }

    fn candidate_with(
        series_id: Uuid,
        external_release_id: &str,
        chapter: Option<f64>,
        volume: Option<i32>,
        language: &str,
    ) -> ReleaseCandidate {
        ReleaseCandidate {
            series_match: SeriesMatch {
                codex_series_id: series_id,
                confidence: 0.95,
                reason: "test".to_string(),
            },
            external_release_id: external_release_id.to_string(),
            chapter,
            volume,
            language: language.to_string(),
            format_hints: None,
            group_or_uploader: Some("group-x".to_string()),
            payload_url: format!("https://example.com/{}", external_release_id),
            media_url: None,
            media_url_kind: None,
            info_hash: None,
            metadata: None,
            observed_at: Utc::now(),
        }
    }

    #[tokio::test]
    async fn record_advances_latest_known_chapter() {
        let (db, _t) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let (series_id, source_id) = setup(conn, "release-mu").await;

        // Seed tracking with chapter=142.
        SeriesTrackingRepository::upsert(
            conn,
            series_id,
            TrackingUpdate {
                tracked: Some(true),
                latest_known_chapter: Some(Some(142.0)),
                ..Default::default()
            },
        )
        .await
        .unwrap();

        let handler = ReleasesRequestHandler::new(
            conn.clone(),
            "release-mu".to_string(),
            make_capability(false, vec![]),
        );

        record_candidate(
            &handler,
            source_id,
            candidate_with(series_id, "rel-143", Some(143.0), None, "en"),
        )
        .await;

        let row = SeriesTrackingRepository::get(conn, series_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(row.latest_known_chapter, Some(143.0));
    }

    #[tokio::test]
    async fn record_does_not_advance_for_stale_chapter() {
        let (db, _t) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let (series_id, source_id) = setup(conn, "release-mu").await;

        SeriesTrackingRepository::upsert(
            conn,
            series_id,
            TrackingUpdate {
                tracked: Some(true),
                latest_known_chapter: Some(Some(143.0)),
                ..Default::default()
            },
        )
        .await
        .unwrap();

        let handler = ReleasesRequestHandler::new(
            conn.clone(),
            "release-mu".to_string(),
            make_capability(false, vec![]),
        );

        record_candidate(
            &handler,
            source_id,
            candidate_with(series_id, "rel-140", Some(140.0), None, "en"),
        )
        .await;

        let row = SeriesTrackingRepository::get(conn, series_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            row.latest_known_chapter,
            Some(143.0),
            "stale candidate must not move the high-water mark backwards"
        );
    }

    #[tokio::test]
    async fn record_skips_chapter_advance_when_track_chapters_false() {
        let (db, _t) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let (series_id, source_id) = setup(conn, "release-mu").await;

        SeriesTrackingRepository::upsert(
            conn,
            series_id,
            TrackingUpdate {
                tracked: Some(true),
                track_chapters: Some(false),
                track_volumes: Some(true),
                latest_known_chapter: Some(Some(140.0)),
                ..Default::default()
            },
        )
        .await
        .unwrap();

        let handler = ReleasesRequestHandler::new(
            conn.clone(),
            "release-mu".to_string(),
            make_capability(false, vec![]),
        );

        record_candidate(
            &handler,
            source_id,
            candidate_with(series_id, "rel-143", Some(143.0), None, "en"),
        )
        .await;

        let row = SeriesTrackingRepository::get(conn, series_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            row.latest_known_chapter,
            Some(140.0),
            "track_chapters=false must suppress chapter advance"
        );
    }

    #[tokio::test]
    async fn record_advances_volume_independently_of_chapter() {
        let (db, _t) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let (series_id, source_id) = setup(conn, "release-mu").await;

        SeriesTrackingRepository::upsert(
            conn,
            series_id,
            TrackingUpdate {
                tracked: Some(true),
                latest_known_volume: Some(Some(14)),
                ..Default::default()
            },
        )
        .await
        .unwrap();

        let handler = ReleasesRequestHandler::new(
            conn.clone(),
            "release-mu".to_string(),
            make_capability(false, vec![]),
        );

        record_candidate(
            &handler,
            source_id,
            candidate_with(series_id, "rel-vol-15", None, Some(15), "en"),
        )
        .await;

        let row = SeriesTrackingRepository::get(conn, series_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(row.latest_known_volume, Some(15));
    }

    #[tokio::test]
    async fn record_skips_advance_when_language_outside_effective_list() {
        let (db, _t) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let (series_id, source_id) = setup(conn, "release-mu").await;

        // Per-series languages = ["en"]; candidate is "id" (Indonesian).
        SeriesTrackingRepository::upsert(
            conn,
            series_id,
            TrackingUpdate {
                tracked: Some(true),
                languages: Some(Some(serde_json::json!(["en"]))),
                latest_known_chapter: Some(Some(142.0)),
                ..Default::default()
            },
        )
        .await
        .unwrap();

        let handler = ReleasesRequestHandler::new(
            conn.clone(),
            "release-mu".to_string(),
            make_capability(false, vec![]),
        );

        let resp = record_candidate(
            &handler,
            source_id,
            candidate_with(series_id, "rel-id-145", Some(145.0), None, "id"),
        )
        .await;
        // Ledger row is still created — language filtering is the plugin's
        // job. The handler only enforces that out-of-language records don't
        // move the high-water mark.
        assert!(!resp.deduped);

        let row = SeriesTrackingRepository::get(conn, series_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            row.latest_known_chapter,
            Some(142.0),
            "out-of-language candidate must not move latest_known_chapter"
        );
    }

    #[tokio::test]
    async fn record_dedup_does_not_re_advance_latest_known() {
        let (db, _t) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let (series_id, source_id) = setup(conn, "release-mu").await;

        SeriesTrackingRepository::upsert(
            conn,
            series_id,
            TrackingUpdate {
                tracked: Some(true),
                ..Default::default()
            },
        )
        .await
        .unwrap();

        let handler = ReleasesRequestHandler::new(
            conn.clone(),
            "release-mu".to_string(),
            make_capability(false, vec![]),
        );

        let cand = candidate_with(series_id, "rel-143", Some(143.0), None, "en");
        let first = record_candidate(&handler, source_id, cand.clone()).await;
        assert!(!first.deduped);

        // Manually wind back latest_known_chapter to detect a spurious advance
        // on the dedup path.
        SeriesTrackingRepository::upsert(
            conn,
            series_id,
            TrackingUpdate {
                latest_known_chapter: Some(Some(100.0)),
                ..Default::default()
            },
        )
        .await
        .unwrap();

        let second = record_candidate(&handler, source_id, cand).await;
        assert!(second.deduped);

        let row = SeriesTrackingRepository::get(conn, series_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            row.latest_known_chapter,
            Some(100.0),
            "dedup path must not re-tick latest_known_chapter"
        );
    }

    #[tokio::test]
    async fn record_does_not_create_tracking_row_for_untracked_series() {
        let (db, _t) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let (series_id, source_id) = setup(conn, "release-mu").await;

        // Flip the tracking row off so the series is not being tracked.
        SeriesTrackingRepository::upsert(
            conn,
            series_id,
            TrackingUpdate {
                tracked: Some(false),
                ..Default::default()
            },
        )
        .await
        .unwrap();

        let handler = ReleasesRequestHandler::new(
            conn.clone(),
            "release-mu".to_string(),
            make_capability(false, vec![]),
        );

        record_candidate(
            &handler,
            source_id,
            candidate_with(series_id, "rel-143", Some(143.0), None, "en"),
        )
        .await;

        let row = SeriesTrackingRepository::get(conn, series_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            row.latest_known_chapter, None,
            "untracked series must not have its high-water mark moved"
        );
    }

    // -------------------------------------------------------------------------
    // register_sources
    // -------------------------------------------------------------------------

    fn register_request(sources: Value) -> JsonRpcRequest {
        make_request(
            methods::RELEASES_REGISTER_SOURCES,
            json!({ "sources": sources }),
        )
    }

    #[tokio::test]
    async fn register_sources_creates_rows_for_a_fresh_plugin() {
        let (db, _t) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let handler = ReleasesRequestHandler::new(
            conn.clone(),
            "release-nyaa".to_string(),
            make_capability(false, vec![]),
        );

        let req = register_request(json!([
            {
                "sourceKey": "user:tsuna69",
                "displayName": "Nyaa: tsuna69",
                "kind": "rss-uploader",
                "config": { "subscription": { "kind": "user", "identifier": "tsuna69" } }
            },
            {
                "sourceKey": "query:LuminousScans",
                "displayName": "Nyaa search: LuminousScans",
                "kind": "rss-uploader"
            }
        ]));
        let resp = handler.handle_request(&req).await;
        assert!(!resp.is_error(), "unexpected error: {:?}", resp.error);
        let body: Value = resp.result.unwrap();
        assert_eq!(body["registered"], 2);
        assert_eq!(body["pruned"], 0);

        let rows = ReleaseSourceRepository::list_by_plugin(conn, "release-nyaa")
            .await
            .unwrap();
        assert_eq!(rows.len(), 2);
        let by_key: HashMap<&str, &release_sources::Model> =
            rows.iter().map(|r| (r.source_key.as_str(), r)).collect();
        assert!(by_key.contains_key("user:tsuna69"));
        assert!(by_key.contains_key("query:LuminousScans"));
        assert!(
            by_key["user:tsuna69"].enabled,
            "new rows default to enabled"
        );
    }

    #[tokio::test]
    async fn register_sources_prunes_rows_no_longer_declared() {
        let (db, _t) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let handler = ReleasesRequestHandler::new(
            conn.clone(),
            "release-nyaa".to_string(),
            make_capability(false, vec![]),
        );

        // First call creates two rows.
        let _ = handler
            .handle_request(&register_request(json!([
                { "sourceKey": "user:a", "displayName": "A", "kind": "rss-uploader" },
                { "sourceKey": "user:b", "displayName": "B", "kind": "rss-uploader" }
            ])))
            .await;

        // Second call drops `user:b` and adds `user:c`.
        let resp = handler
            .handle_request(&register_request(json!([
                { "sourceKey": "user:a", "displayName": "A", "kind": "rss-uploader" },
                { "sourceKey": "user:c", "displayName": "C", "kind": "rss-uploader" }
            ])))
            .await;
        assert!(!resp.is_error());
        let body: Value = resp.result.unwrap();
        assert_eq!(body["registered"], 2);
        assert_eq!(body["pruned"], 1);

        let rows = ReleaseSourceRepository::list_by_plugin(conn, "release-nyaa")
            .await
            .unwrap();
        let keys: Vec<&str> = rows.iter().map(|r| r.source_key.as_str()).collect();
        assert!(keys.contains(&"user:a"));
        assert!(keys.contains(&"user:c"));
        assert!(!keys.contains(&"user:b"), "stale source must be pruned");
    }

    #[tokio::test]
    async fn register_sources_with_empty_list_wipes_plugins_rows() {
        let (db, _t) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let handler = ReleasesRequestHandler::new(
            conn.clone(),
            "release-nyaa".to_string(),
            make_capability(false, vec![]),
        );

        let _ = handler
            .handle_request(&register_request(json!([
                { "sourceKey": "user:a", "displayName": "A", "kind": "rss-uploader" }
            ])))
            .await;

        let resp = handler.handle_request(&register_request(json!([]))).await;
        assert!(!resp.is_error());
        let body: Value = resp.result.unwrap();
        assert_eq!(body["registered"], 0);
        assert_eq!(body["pruned"], 1);

        let rows = ReleaseSourceRepository::list_by_plugin(conn, "release-nyaa")
            .await
            .unwrap();
        assert!(rows.is_empty());
    }

    #[tokio::test]
    async fn register_sources_preserves_user_managed_fields_on_re_register() {
        let (db, _t) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let handler = ReleasesRequestHandler::new(
            conn.clone(),
            "release-nyaa".to_string(),
            make_capability(false, vec![]),
        );

        // Initial register.
        let _ = handler
            .handle_request(&register_request(json!([
                { "sourceKey": "user:tsuna69", "displayName": "Nyaa: tsuna69", "kind": "rss-uploader" }
            ])))
            .await;

        // Admin disables it and pins a custom interval.
        let row = ReleaseSourceRepository::find_by_key(conn, "release-nyaa", "user:tsuna69")
            .await
            .unwrap()
            .unwrap();
        ReleaseSourceRepository::update(
            conn,
            row.id,
            ReleaseSourceUpdate {
                enabled: Some(false),
                cron_schedule: Some(Some("0 */6 * * *".to_string())),
                ..Default::default()
            },
        )
        .await
        .unwrap();

        // Plugin re-registers (e.g., after restart) with a refreshed display name + new config.
        let _ = handler
            .handle_request(&register_request(json!([
                {
                    "sourceKey": "user:tsuna69",
                    "displayName": "Nyaa: tsuna69 (refreshed)",
                    "kind": "rss-uploader",
                    "config": { "subscription": "fresh" }
                }
            ])))
            .await;

        let after = ReleaseSourceRepository::find_by_key(conn, "release-nyaa", "user:tsuna69")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(after.display_name, "Nyaa: tsuna69 (refreshed)");
        assert_eq!(after.config, Some(json!({ "subscription": "fresh" })));
        assert!(!after.enabled, "user-set disabled must survive re-register");
        assert_eq!(
            after.cron_schedule.as_deref(),
            Some("0 */6 * * *"),
            "user-set cron_schedule must survive re-register"
        );
    }

    #[tokio::test]
    async fn register_sources_does_not_touch_other_plugins_rows() {
        let (db, _t) = create_test_db().await;
        let conn = db.sea_orm_connection();

        // Pre-existing source from a different plugin.
        ReleaseSourceRepository::create(
            conn,
            NewReleaseSource {
                plugin_id: "release-mangaupdates".to_string(),
                source_key: "default".to_string(),
                display_name: "MangaUpdates".to_string(),
                kind: kind::RSS_SERIES.to_string(),
                enabled: None,
                config: None,
            },
        )
        .await
        .unwrap();

        let handler = ReleasesRequestHandler::new(
            conn.clone(),
            "release-nyaa".to_string(),
            make_capability(false, vec![]),
        );
        // Empty register from nyaa — must not nuke mangaupdates' row.
        let _ = handler.handle_request(&register_request(json!([]))).await;

        let mu_rows = ReleaseSourceRepository::list_by_plugin(conn, "release-mangaupdates")
            .await
            .unwrap();
        assert_eq!(mu_rows.len(), 1);
    }

    #[tokio::test]
    async fn register_sources_rejects_kind_outside_capability() {
        let (db, _t) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let handler = ReleasesRequestHandler::new(
            conn.clone(),
            "release-nyaa".to_string(),
            // Only declares rss-uploader.
            make_capability(false, vec![]),
        );

        let resp = handler
            .handle_request(&register_request(json!([
                { "sourceKey": "x", "displayName": "X", "kind": "rss-series" }
            ])))
            .await;
        assert!(resp.is_error());
        assert!(resp.error.unwrap().message.contains("not declared"));

        // Nothing was written.
        let rows = ReleaseSourceRepository::list_by_plugin(conn, "release-nyaa")
            .await
            .unwrap();
        assert!(rows.is_empty());
    }

    #[tokio::test]
    async fn register_sources_rejects_invalid_kind_string() {
        let (db, _t) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let handler = ReleasesRequestHandler::new(
            conn.clone(),
            "release-nyaa".to_string(),
            make_capability(false, vec![]),
        );

        let resp = handler
            .handle_request(&register_request(json!([
                { "sourceKey": "x", "displayName": "X", "kind": "frobnicate" }
            ])))
            .await;
        assert!(resp.is_error());
        assert!(resp.error.unwrap().message.contains("invalid kind"));
    }

    #[tokio::test]
    async fn register_sources_rejects_duplicate_keys_in_request() {
        let (db, _t) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let handler = ReleasesRequestHandler::new(
            conn.clone(),
            "release-nyaa".to_string(),
            make_capability(false, vec![]),
        );

        let resp = handler
            .handle_request(&register_request(json!([
                { "sourceKey": "dup", "displayName": "A", "kind": "rss-uploader" },
                { "sourceKey": "dup", "displayName": "B", "kind": "rss-uploader" }
            ])))
            .await;
        assert!(resp.is_error());
        assert!(resp.error.unwrap().message.contains("duplicate"));
        let rows = ReleaseSourceRepository::list_by_plugin(conn, "release-nyaa")
            .await
            .unwrap();
        assert!(rows.is_empty(), "validation must run before any write");
    }

    #[tokio::test]
    async fn register_sources_rejects_empty_source_key_or_display_name() {
        let (db, _t) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let handler = ReleasesRequestHandler::new(
            conn.clone(),
            "release-nyaa".to_string(),
            make_capability(false, vec![]),
        );

        let resp1 = handler
            .handle_request(&register_request(json!([
                { "sourceKey": "  ", "displayName": "X", "kind": "rss-uploader" }
            ])))
            .await;
        assert!(resp1.is_error());

        let resp2 = handler
            .handle_request(&register_request(json!([
                { "sourceKey": "x", "displayName": "  ", "kind": "rss-uploader" }
            ])))
            .await;
        assert!(resp2.is_error());
    }
}
