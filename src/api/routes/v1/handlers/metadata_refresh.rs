//! HTTP handlers for the scheduled metadata-refresh feature (Phase 6).
//!
//! Endpoints:
//! - `GET    /api/v1/libraries/{id}/metadata-refresh`         — read config
//! - `PATCH  /api/v1/libraries/{id}/metadata-refresh`         — partial update
//! - `POST   /api/v1/libraries/{id}/metadata-refresh/run-now` — enqueue task
//! - `POST   /api/v1/libraries/{id}/metadata-refresh/dry-run` — preview
//! - `GET    /api/v1/metadata-refresh/field-groups`           — group catalog
//!
//! Permissions: read endpoints require `libraries:read`; mutating and
//! action endpoints require `libraries:write`.

use crate::api::{AppState, error::ApiError, extractors::AuthContext, permissions::Permission};
use crate::db::repositories::{
    LibraryRepository, PluginsRepository, SeriesMetadataRepository, SeriesRepository,
    TaskRepository,
};
use crate::services::metadata::{
    ApplyOptions, FieldGroup, MatchingStrategy, MetadataApplier, MetadataRefreshConfig,
    PlannedRefresh, ProviderOverride, RefreshPlan, RefreshPlanner, fields_filter_for_provider,
    fields_for_group,
};
use crate::services::plugin::protocol::{MetadataGetParams, MetadataMatchParams};
use crate::tasks::types::TaskType;
use crate::utils::cron::{validate_cron_expression, validate_timezone};

use super::super::dto::{
    DryRunRequest, DryRunResponse, DryRunSeriesDelta, DryRunSkippedFieldDto, FieldChangeDto,
    FieldGroupDto, MetadataRefreshConfigDto, MetadataRefreshConfigPatchDto, RunNowResponse,
};

use axum::{
    Json,
    extract::{Path, State},
};
use sea_orm::DatabaseConnection;
use std::collections::BTreeMap;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use uuid::Uuid;

/// Per-pair timeout when fetching metadata for the dry-run preview. Kept
/// short because the user is waiting on the HTTP response — a slow plugin
/// shouldn't block the modal indefinitely.
const DRY_RUN_PER_PAIR_TIMEOUT: Duration = Duration::from_secs(20);

/// Default sample size when the request omits `sample_size`. Mirrors the
/// plan's "show me 5 series" UX.
const DEFAULT_SAMPLE_SIZE: u32 = 5;

/// Hard cap on `sample_size`. Prevents the dry-run from doing the full task
/// over HTTP.
const MAX_SAMPLE_SIZE: u32 = 20;

/// Friendly label for a [`FieldGroup`] used in the public field-group catalog.
fn field_group_label(group: FieldGroup) -> &'static str {
    match group {
        FieldGroup::Identifiers => "Identifiers",
        FieldGroup::Descriptive => "Descriptive",
        FieldGroup::Status => "Status",
        FieldGroup::Counts => "Counts",
        FieldGroup::Ratings => "Ratings",
        FieldGroup::Cover => "Cover",
        FieldGroup::Tags => "Tags",
        FieldGroup::Genres => "Genres",
        FieldGroup::AgeRating => "Age Rating",
        FieldGroup::Classification => "Classification",
        FieldGroup::Publisher => "Publisher",
        FieldGroup::ExternalRefs => "External References",
    }
}

/// `GET /api/v1/libraries/{library_id}/metadata-refresh`
#[utoipa::path(
    get,
    path = "/api/v1/libraries/{library_id}/metadata-refresh",
    params(
        ("library_id" = Uuid, Path, description = "Library ID"),
    ),
    responses(
        (status = 200, description = "Current scheduled refresh config (defaults if none persisted)", body = MetadataRefreshConfigDto),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Library not found"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = []),
    ),
    tag = "Metadata Refresh",
)]
pub async fn get_refresh_config(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    Path(library_id): Path<Uuid>,
) -> Result<Json<MetadataRefreshConfigDto>, ApiError> {
    auth.require_permission(&Permission::LibrariesRead)?;
    ensure_library_exists(&state.db, library_id).await?;

    let cfg = LibraryRepository::get_metadata_refresh_config(&state.db, library_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to load refresh config: {e}")))?;

    Ok(Json(cfg.into()))
}

/// `PATCH /api/v1/libraries/{library_id}/metadata-refresh`
#[utoipa::path(
    patch,
    path = "/api/v1/libraries/{library_id}/metadata-refresh",
    params(
        ("library_id" = Uuid, Path, description = "Library ID"),
    ),
    request_body = MetadataRefreshConfigPatchDto,
    responses(
        (status = 200, description = "Updated config", body = MetadataRefreshConfigDto),
        (status = 400, description = "Invalid cron, timezone, field group, or provider"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Library not found"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = []),
    ),
    tag = "Metadata Refresh",
)]
pub async fn patch_refresh_config(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    Path(library_id): Path<Uuid>,
    Json(patch): Json<MetadataRefreshConfigPatchDto>,
) -> Result<Json<MetadataRefreshConfigDto>, ApiError> {
    auth.require_permission(&Permission::LibrariesWrite)?;
    ensure_library_exists(&state.db, library_id).await?;

    let mut cfg = LibraryRepository::get_metadata_refresh_config(&state.db, library_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to load refresh config: {e}")))?;

    apply_patch_with_validation(&mut cfg, patch, &state.db).await?;

    LibraryRepository::set_metadata_refresh_config(&state.db, library_id, &cfg)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to persist refresh config: {e}")))?;

    // Reload the scheduler so the change picks up immediately. Best-effort —
    // a failure here doesn't undo the config write because the next reload
    // (or restart) will pick it up.
    if let Some(scheduler) = &state.scheduler
        && let Err(e) = scheduler.lock().await.reload_schedules().await
    {
        tracing::warn!(
            "Failed to reload scheduler after metadata-refresh config update: {}",
            e
        );
    }

    Ok(Json(cfg.into()))
}

/// `POST /api/v1/libraries/{library_id}/metadata-refresh/run-now`
#[utoipa::path(
    post,
    path = "/api/v1/libraries/{library_id}/metadata-refresh/run-now",
    params(
        ("library_id" = Uuid, Path, description = "Library ID"),
    ),
    responses(
        (status = 200, description = "Task enqueued", body = RunNowResponse),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Library not found"),
        (status = 409, description = "A refresh task is already running"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = []),
    ),
    tag = "Metadata Refresh",
)]
pub async fn run_refresh_now(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    Path(library_id): Path<Uuid>,
) -> Result<Json<RunNowResponse>, ApiError> {
    auth.require_permission(&Permission::LibrariesWrite)?;
    ensure_library_exists(&state.db, library_id).await?;

    if has_active_refresh_task(&state.db, library_id).await? {
        return Err(ApiError::Conflict(format!(
            "A metadata refresh is already in progress for library {library_id}"
        )));
    }

    let task_id = TaskRepository::enqueue(
        &state.db,
        TaskType::RefreshLibraryMetadata { library_id },
        None,
    )
    .await
    .map_err(|e| ApiError::Internal(format!("Failed to enqueue refresh task: {e}")))?;

    Ok(Json(RunNowResponse { task_id }))
}

/// `POST /api/v1/libraries/{library_id}/metadata-refresh/dry-run`
///
/// Synchronous over HTTP. Bounded by `sampleSize` (default 5, capped at 20).
#[utoipa::path(
    post,
    path = "/api/v1/libraries/{library_id}/metadata-refresh/dry-run",
    params(
        ("library_id" = Uuid, Path, description = "Library ID"),
    ),
    request_body = DryRunRequest,
    responses(
        (status = 200, description = "Sample of would-be changes", body = DryRunResponse),
        (status = 400, description = "Invalid override config"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Library not found"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = []),
    ),
    tag = "Metadata Refresh",
)]
pub async fn dry_run_refresh(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    Path(library_id): Path<Uuid>,
    Json(req): Json<DryRunRequest>,
) -> Result<Json<DryRunResponse>, ApiError> {
    auth.require_permission(&Permission::LibrariesWrite)?;
    ensure_library_exists(&state.db, library_id).await?;

    let config = if let Some(override_dto) = req.config_override {
        let mut cfg = MetadataRefreshConfig::default();
        apply_override_dto(&mut cfg, override_dto, &state.db).await?;
        cfg
    } else {
        LibraryRepository::get_metadata_refresh_config(&state.db, library_id)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to load refresh config: {e}")))?
    };

    let sample_size = req
        .sample_size
        .unwrap_or(DEFAULT_SAMPLE_SIZE)
        .clamp(1, MAX_SAMPLE_SIZE);

    let plan = RefreshPlanner::plan(&state.db, library_id, &config)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to plan refresh: {e}")))?;

    let totals = plan_totals(&plan);

    let matching_strategy = if config.existing_source_ids_only {
        MatchingStrategy::ExistingExternalIdOnly
    } else {
        MatchingStrategy::AllowReMatch
    };

    let sample_pairs: Vec<&PlannedRefresh> =
        plan.planned.iter().take(sample_size as usize).collect();

    let mut sample = Vec::with_capacity(sample_pairs.len());
    for planned in sample_pairs {
        let provider_key = format!("plugin:{}", planned.plugin.name);
        let pair_fields_filter = fields_filter_for_provider(&config, &provider_key);

        match dry_run_one_pair(
            &state.db,
            library_id,
            planned,
            pair_fields_filter.as_ref(),
            matching_strategy,
            &state,
        )
        .await
        {
            Ok(delta) => sample.push(delta),
            Err(e) => {
                tracing::warn!(
                    "Dry-run preview failed for series {} / plugin {}: {:#}",
                    planned.series_id,
                    planned.plugin.name,
                    e
                );
                // Surface the failure as a single-skip row so the UI can
                // show "this series couldn't be previewed" instead of
                // silently dropping it.
                sample.push(DryRunSeriesDelta {
                    series_id: planned.series_id,
                    series_title: lookup_series_title(&state.db, planned.series_id).await,
                    provider: format!("plugin:{}", planned.plugin.name),
                    changes: Vec::new(),
                    skipped: vec![DryRunSkippedFieldDto {
                        field: "_preview".to_string(),
                        reason: format!("Preview failed: {e}"),
                    }],
                });
            }
        }
    }

    Ok(Json(DryRunResponse {
        sample,
        total_eligible: totals.total_eligible,
        est_skipped_no_id: totals.skipped_no_id,
        est_skipped_recently_synced: totals.skipped_recently_synced,
        unresolved_providers: plan.unresolved_providers,
    }))
}

/// `GET /api/v1/metadata-refresh/field-groups`
#[utoipa::path(
    get,
    path = "/api/v1/metadata-refresh/field-groups",
    responses(
        (status = 200, description = "All field groups in display order", body = [FieldGroupDto]),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = []),
    ),
    tag = "Metadata Refresh",
)]
pub async fn list_field_groups(auth: AuthContext) -> Result<Json<Vec<FieldGroupDto>>, ApiError> {
    auth.require_permission(&Permission::LibrariesRead)?;

    let out: Vec<FieldGroupDto> = FieldGroup::all()
        .iter()
        .map(|g| FieldGroupDto {
            id: g.as_str().to_string(),
            label: field_group_label(*g).to_string(),
            fields: fields_for_group(*g)
                .iter()
                .map(|s| (*s).to_string())
                .collect(),
        })
        .collect();

    Ok(Json(out))
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

async fn ensure_library_exists(db: &DatabaseConnection, library_id: Uuid) -> Result<(), ApiError> {
    LibraryRepository::get_by_id(db, library_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to load library: {e}")))?
        .ok_or_else(|| ApiError::NotFound(format!("Library {library_id} not found")))
        .map(|_| ())
}

/// Detect whether a `RefreshLibraryMetadata` task for this library is still
/// in flight. Mirrors the scheduler's skip-if-already-running guard so the
/// HTTP "run now" path doesn't pile work onto an existing run.
async fn has_active_refresh_task(
    db: &DatabaseConnection,
    library_id: Uuid,
) -> Result<bool, ApiError> {
    use crate::db::entities::{prelude::Tasks, tasks};
    use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter};

    let count = Tasks::find()
        .filter(tasks::Column::TaskType.eq("refresh_library_metadata"))
        .filter(tasks::Column::LibraryId.eq(library_id))
        .filter(tasks::Column::Status.is_in(["pending", "processing"]))
        .count(db)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to check active refresh tasks: {e}")))?;

    Ok(count > 0)
}

/// Apply a PATCH to the in-memory config, validating each field as we go.
///
/// Validation strategy:
/// - cron: parse via `validate_cron_expression` (rejects nonsense up front).
///   Stores the *original* (pre-normalization) string so the user sees what
///   they typed; the scheduler normalizes again at register time.
/// - timezone: IANA name check via `validate_timezone`.
/// - field_groups / extra_fields: every group string must parse to a known
///   `FieldGroup`. Extras pass through (power-user hatch).
/// - providers: each `"plugin:<name>"` must resolve to an existing plugin.
///   Disabled plugins are accepted (so the user can persist a "wait for
///   admin to enable this" state) but missing plugins are rejected.
/// - per_provider_overrides: same group validation; unknown groups rejected.
async fn apply_patch_with_validation(
    cfg: &mut MetadataRefreshConfig,
    patch: MetadataRefreshConfigPatchDto,
    db: &DatabaseConnection,
) -> Result<(), ApiError> {
    if let Some(v) = patch.enabled {
        cfg.enabled = v;
    }
    if let Some(v) = patch.cron_schedule {
        validate_cron_expression(&v)
            .map_err(|e| ApiError::BadRequest(format!("Invalid cron schedule: {e}")))?;
        cfg.cron_schedule = v;
    }
    if let Some(maybe_tz) = patch.timezone.into_nested_option() {
        match maybe_tz {
            None => cfg.timezone = None,
            Some(tz) => {
                validate_timezone(&tz)
                    .map_err(|e| ApiError::BadRequest(format!("Invalid timezone: {e}")))?;
                cfg.timezone = Some(tz);
            }
        }
    }
    if let Some(groups) = patch.field_groups {
        validate_field_groups(&groups)?;
        cfg.field_groups = groups;
    }
    if let Some(extras) = patch.extra_fields {
        cfg.extra_fields = extras;
    }
    if let Some(providers) = patch.providers {
        validate_providers(db, &providers).await?;
        cfg.providers = providers;
    }
    if let Some(v) = patch.existing_source_ids_only {
        cfg.existing_source_ids_only = v;
    }
    if let Some(v) = patch.skip_recently_synced_within_s {
        cfg.skip_recently_synced_within_s = v;
    }
    if let Some(v) = patch.max_concurrency {
        if v == 0 {
            return Err(ApiError::BadRequest(
                "max_concurrency must be at least 1".to_string(),
            ));
        }
        cfg.max_concurrency = v;
    }
    if let Some(maybe_overrides) = patch.per_provider_overrides.into_nested_option() {
        match maybe_overrides {
            None => cfg.per_provider_overrides = None,
            Some(map) => {
                let provider_keys: Vec<String> = map.keys().cloned().collect();
                validate_providers(db, &provider_keys).await?;
                for (provider, ovr) in &map {
                    validate_field_groups(&ovr.field_groups).map_err(|e| match e {
                        ApiError::BadRequest(msg) => ApiError::BadRequest(format!(
                            "Invalid override for provider '{provider}': {msg}"
                        )),
                        other => other,
                    })?;
                }
                let mut converted: BTreeMap<String, ProviderOverride> = BTreeMap::new();
                for (k, v) in map {
                    converted.insert(k, v.into());
                }
                cfg.per_provider_overrides = Some(converted);
            }
        }
    }
    Ok(())
}

/// Apply a complete config DTO (used by dry-run's `configOverride`) onto a
/// fresh default config, with the same validation as PATCH.
async fn apply_override_dto(
    cfg: &mut MetadataRefreshConfig,
    dto: MetadataRefreshConfigDto,
    db: &DatabaseConnection,
) -> Result<(), ApiError> {
    cfg.enabled = dto.enabled;
    validate_cron_expression(&dto.cron_schedule)
        .map_err(|e| ApiError::BadRequest(format!("Invalid cron schedule: {e}")))?;
    cfg.cron_schedule = dto.cron_schedule;

    if let Some(tz) = dto.timezone {
        validate_timezone(&tz)
            .map_err(|e| ApiError::BadRequest(format!("Invalid timezone: {e}")))?;
        cfg.timezone = Some(tz);
    }
    validate_field_groups(&dto.field_groups)?;
    cfg.field_groups = dto.field_groups;
    cfg.extra_fields = dto.extra_fields;
    validate_providers(db, &dto.providers).await?;
    cfg.providers = dto.providers;
    cfg.existing_source_ids_only = dto.existing_source_ids_only;
    cfg.skip_recently_synced_within_s = dto.skip_recently_synced_within_s;
    if dto.max_concurrency == 0 {
        return Err(ApiError::BadRequest(
            "max_concurrency must be at least 1".to_string(),
        ));
    }
    cfg.max_concurrency = dto.max_concurrency;

    if let Some(map) = dto.per_provider_overrides {
        let provider_keys: Vec<String> = map.keys().cloned().collect();
        validate_providers(db, &provider_keys).await?;
        for (provider, ovr) in &map {
            validate_field_groups(&ovr.field_groups).map_err(|e| match e {
                ApiError::BadRequest(msg) => ApiError::BadRequest(format!(
                    "Invalid override for provider '{provider}': {msg}"
                )),
                other => other,
            })?;
        }
        let mut converted: BTreeMap<String, ProviderOverride> = BTreeMap::new();
        for (k, v) in map {
            converted.insert(k, v.into());
        }
        cfg.per_provider_overrides = Some(converted);
    }
    Ok(())
}

fn validate_field_groups(groups: &[String]) -> Result<(), ApiError> {
    for g in groups {
        FieldGroup::from_str(g)
            .map_err(|_| ApiError::BadRequest(format!("Unknown field group '{g}'")))?;
    }
    Ok(())
}

/// Each provider must be `"plugin:<name>"` and point to an installed plugin.
/// Disabled plugins are allowed: the user may persist them in anticipation
/// of enabling later, and the planner already records them as
/// `unresolved_providers` at run time.
async fn validate_providers(db: &DatabaseConnection, providers: &[String]) -> Result<(), ApiError> {
    for p in providers {
        let Some(name) = p.strip_prefix("plugin:") else {
            return Err(ApiError::BadRequest(format!(
                "Invalid provider '{p}': expected 'plugin:<name>'"
            )));
        };
        let plugin = PluginsRepository::get_by_name(db, name)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to look up plugin '{name}': {e}")))?;
        if plugin.is_none() {
            return Err(ApiError::BadRequest(format!(
                "Unknown plugin '{name}' in provider '{p}'"
            )));
        }
    }
    Ok(())
}

struct PlanTotals {
    total_eligible: u32,
    skipped_no_id: u32,
    skipped_recently_synced: u32,
}

fn plan_totals(plan: &RefreshPlan) -> PlanTotals {
    let counts = plan.skipped_by_reason();
    PlanTotals {
        total_eligible: plan.planned.len() as u32,
        skipped_no_id: counts.get("no_external_id").copied().unwrap_or(0) as u32,
        skipped_recently_synced: counts.get("recently_synced").copied().unwrap_or(0) as u32,
    }
}

/// Lookup a series' display name. Best-effort; falls back to the UUID string
/// so the dry-run response stays well-formed even if the row was deleted
/// between planning and rendering.
async fn lookup_series_title(db: &DatabaseConnection, series_id: Uuid) -> String {
    match SeriesRepository::get_by_id(db, series_id).await {
        Ok(Some(s)) => {
            // Prefer the metadata title (canonical display name) when present.
            match SeriesMetadataRepository::get_by_series_id(db, series_id).await {
                Ok(Some(meta)) if !meta.title.is_empty() => meta.title,
                _ => s.name,
            }
        }
        _ => series_id.to_string(),
    }
}

/// Fetch metadata for one planned pair, run a dry-run apply, and convert the
/// result to a [`DryRunSeriesDelta`].
///
/// Re-uses the task handler's matching semantics (strict vs. loose) so the
/// preview reflects what a real run would do.
async fn dry_run_one_pair(
    db: &DatabaseConnection,
    library_id: Uuid,
    planned: &PlannedRefresh,
    fields_filter: Option<&std::collections::HashSet<String>>,
    matching_strategy: MatchingStrategy,
    state: &AppState,
) -> Result<DryRunSeriesDelta, anyhow::Error> {
    let plugin = &planned.plugin;

    // 1. External ID resolution mirrors the task handler's process_pair.
    let external_id = if let Some(record) = planned.existing_external_id.as_ref() {
        record.external_id.clone()
    } else {
        match matching_strategy {
            MatchingStrategy::ExistingExternalIdOnly => {
                anyhow::bail!("No stored external ID for series; strict mode")
            }
            MatchingStrategy::AllowReMatch => rematch_for_dry_run(db, planned, state).await?,
        }
    };

    let get_params = MetadataGetParams {
        external_id: external_id.clone(),
    };
    let plugin_metadata = tokio::time::timeout(
        DRY_RUN_PER_PAIR_TIMEOUT,
        state
            .plugin_manager
            .get_series_metadata(plugin.id, get_params),
    )
    .await
    .map_err(|_| anyhow::anyhow!("Plugin call timed out"))?
    .map_err(|e| anyhow::anyhow!("Plugin call failed: {e}"))?;

    let current_metadata = SeriesMetadataRepository::get_by_series_id(db, planned.series_id)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to load current metadata: {e:#}"))?;

    let options = ApplyOptions {
        fields_filter: fields_filter.cloned(),
        thumbnail_service: Some(state.thumbnail_service.clone()),
        event_broadcaster: None, // Don't emit events for previews.
        dry_run: true,
        matching_strategy,
    };

    let result = MetadataApplier::apply(
        db,
        planned.series_id,
        library_id,
        plugin,
        &plugin_metadata,
        current_metadata.as_ref(),
        &options,
    )
    .await
    .map_err(|e| anyhow::anyhow!("Dry-run apply failed: {e:#}"))?;

    let changes: Vec<FieldChangeDto> = result
        .dry_run_report
        .map(|r| {
            r.changes
                .into_iter()
                .map(|c| FieldChangeDto {
                    field: c.field,
                    before: c.before,
                    after: c.after,
                })
                .collect()
        })
        .unwrap_or_default();

    let skipped: Vec<DryRunSkippedFieldDto> = result
        .skipped_fields
        .into_iter()
        .map(|sf| DryRunSkippedFieldDto {
            field: sf.field,
            reason: sf.reason,
        })
        .collect();

    Ok(DryRunSeriesDelta {
        series_id: planned.series_id,
        series_title: lookup_series_title(db, planned.series_id).await,
        provider: format!("plugin:{}", plugin.name),
        changes,
        skipped,
    })
}

/// Loose-mode rematch helper for dry-run. Mirrors `process_pair`'s rematch
/// path but stops at returning the candidate external ID — the dry-run
/// caller does the rest.
async fn rematch_for_dry_run(
    db: &DatabaseConnection,
    planned: &PlannedRefresh,
    state: &AppState,
) -> Result<String, anyhow::Error> {
    let series = SeriesRepository::get_by_id(db, planned.series_id)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to load series for re-match: {e:#}"))?
        .ok_or_else(|| anyhow::anyhow!("Series {} not found", planned.series_id))?;
    let metadata = SeriesMetadataRepository::get_by_series_id(db, planned.series_id)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to load metadata for re-match: {e:#}"))?;
    let title = metadata
        .as_ref()
        .map(|m| m.title.clone())
        .filter(|t| !t.is_empty())
        .unwrap_or_else(|| series.name.clone());
    let year = metadata.as_ref().and_then(|m| m.year);

    let match_params = MetadataMatchParams {
        title,
        year,
        author: None,
    };

    let candidate = tokio::time::timeout(
        DRY_RUN_PER_PAIR_TIMEOUT,
        state
            .plugin_manager
            .match_series(planned.plugin.id, match_params),
    )
    .await
    .map_err(|_| anyhow::anyhow!("Re-match timed out"))?
    .map_err(|e| anyhow::anyhow!("Re-match failed: {e}"))?
    .ok_or_else(|| anyhow::anyhow!("No match candidate"))?;

    Ok(candidate.external_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn field_group_label_covers_every_variant() {
        // Smoke-test that all enum variants are mapped — adding a variant
        // without a label would otherwise pass clippy but produce a blank
        // label in the API response.
        for g in FieldGroup::all() {
            let label = field_group_label(*g);
            assert!(!label.is_empty(), "field group {:?} has empty label", g);
        }
    }

    #[test]
    fn validate_field_groups_accepts_known() {
        let groups = vec![
            "ratings".to_string(),
            "status".to_string(),
            "counts".to_string(),
        ];
        assert!(validate_field_groups(&groups).is_ok());
    }

    #[test]
    fn validate_field_groups_rejects_unknown() {
        let groups = vec!["ratings".to_string(), "made_up".to_string()];
        let err = validate_field_groups(&groups).unwrap_err();
        match err {
            ApiError::BadRequest(msg) => assert!(msg.contains("made_up")),
            _ => panic!("expected BadRequest"),
        }
    }
}
