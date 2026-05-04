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

use chrono::{DateTime, Utc};
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::{debug, error, warn};
use uuid::Uuid;

use super::protocol::{
    JsonRpcError, JsonRpcRequest, JsonRpcResponse, ReleaseSourceCapability, RequestId, error_codes,
    methods,
};
use crate::db::repositories::{
    ReleaseLedgerRepository, ReleaseSourceRepository, SeriesAliasRepository,
    SeriesExternalIdRepository, SeriesTrackingRepository, TrackingUpdate,
};
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
    /// Optional event broadcaster used to emit `ReleaseAnnounced` events on
    /// successful (non-deduped) `releases/record` inserts.
    event_broadcaster: Option<std::sync::Arc<crate::events::EventBroadcaster>>,
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
            event_broadcaster: None,
        }
    }

    /// Attach an event broadcaster so the handler emits `ReleaseAnnounced`
    /// events on inserts. Builder-style.
    pub fn with_event_broadcaster(
        mut self,
        broadcaster: std::sync::Arc<crate::events::EventBroadcaster>,
    ) -> Self {
        self.event_broadcaster = Some(broadcaster);
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
                        // Source naming convention: `plugin:<name>` for
                        // plugin-provided IDs; we accept either bare source
                        // names (e.g. "mangaupdates") or the prefixed form.
                        let mut by_source: HashMap<String, String> = HashMap::new();
                        for row in rows {
                            let normalized = row
                                .source
                                .strip_prefix("plugin:")
                                .unwrap_or(&row.source)
                                .to_string();
                            if self
                                .capability
                                .requires_external_ids
                                .iter()
                                .any(|req| req == &normalized)
                            {
                                by_source.insert(normalized, row.external_id);
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

        // 4. Hand off to the ledger (which is itself idempotent).
        let entry = accepted.into_ledger_entry(params.source_id);
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

            if let Some(ref broadcaster) = self.event_broadcaster {
                let _ = broadcaster.emit(crate::events::EntityChangeEvent::release_announced(
                    &outcome.row,
                    &self.plugin_name,
                ));
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
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::ScanningStrategy;
    use crate::db::entities::release_sources::kind;
    use crate::db::repositories::{
        LibraryRepository, NewReleaseSource, ReleaseSourceRepository, SeriesAliasRepository,
        SeriesExternalIdRepository, SeriesRepository, SeriesTrackingRepository, TrackingUpdate,
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
            default_poll_interval_s: 3600,
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
                poll_interval_s: 3600,
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

    /// `releases/record` emits a `ReleaseAnnounced` event on insert and
    /// suppresses it on dedup.
    #[tokio::test]
    async fn record_emits_release_announced_on_insert_only() {
        use crate::events::{EntityEvent, EventBroadcaster};

        let (db, _t) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let (series_id, source_id) = setup(conn, "release-nyaa").await;

        let broadcaster = std::sync::Arc::new(EventBroadcaster::new(8));
        let mut rx = broadcaster.subscribe();

        let handler = ReleasesRequestHandler::new(
            conn.clone(),
            "release-nyaa".to_string(),
            make_capability(false, vec![]),
        )
        .with_event_broadcaster(broadcaster.clone());

        let cand = good_candidate(series_id);
        let req = make_request(
            methods::RELEASES_RECORD,
            json!({"sourceId": source_id, "candidate": cand}),
        );

        let first = handler.handle_request(&req).await;
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
        let second = handler.handle_request(&req).await;
        let body: RecordResponse = serde_json::from_value(second.result.unwrap()).unwrap();
        assert!(body.deduped);
        assert!(
            rx.try_recv().is_err(),
            "dedup must not emit a new ReleaseAnnounced event"
        );
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
}
