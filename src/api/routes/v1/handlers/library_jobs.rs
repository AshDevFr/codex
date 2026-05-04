//! Library jobs CRUD + run-now + dry-run handlers (Phase 9).

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use std::sync::Arc;
use uuid::Uuid;

use crate::api::{
    error::ApiError,
    extractors::{AppState, AuthContext},
    permissions::Permission,
};
use crate::db::entities::library_jobs;
use crate::db::repositories::{
    CreateLibraryJobParams, LibraryJobRepository, LibraryRepository, SeriesRepository,
};
use crate::require_permission;
use crate::services::library_jobs::{
    LibraryJobConfig, MetadataRefreshJobConfig, parse_job_config, validation,
};
use crate::services::metadata::{FieldGroup, RefreshPlanner, fields_for_group};
use crate::tasks::types::TaskType;

use super::super::dto::patch::PatchValue;
use super::super::dto::{
    CreateLibraryJobRequest, DryRunFieldChange, DryRunRequest, DryRunResponse, DryRunSeriesDelta,
    FieldGroupDto, LibraryJobConfigDto, LibraryJobDto, ListLibraryJobsResponse,
    PatchLibraryJobRequest, RunNowResponse,
};

// =============================================================================
// Helpers
// =============================================================================

fn validation_to_api_error(e: validation::ValidationError) -> ApiError {
    ApiError::BadRequest(e.to_string())
}

fn anyhow_to_api_error(e: anyhow::Error, ctx: &str) -> ApiError {
    ApiError::Internal(format!("{ctx}: {e}"))
}

async fn ensure_library_exists(state: &AppState, library_id: Uuid) -> Result<(), ApiError> {
    LibraryRepository::get_by_id(&state.db, library_id)
        .await
        .map_err(|e| anyhow_to_api_error(e, "Failed to look up library"))?
        .ok_or_else(|| ApiError::NotFound("Library not found".to_string()))?;
    Ok(())
}

fn row_to_dto(row: library_jobs::Model) -> Result<LibraryJobDto, ApiError> {
    let cfg = parse_job_config(&row.r#type, &row.config)
        .map_err(|e| anyhow_to_api_error(e, "Failed to decode job config"))?;
    Ok(LibraryJobDto {
        id: row.id,
        library_id: row.library_id,
        name: row.name,
        enabled: row.enabled,
        cron_schedule: row.cron_schedule,
        timezone: row.timezone,
        config: cfg.into(),
        last_run_at: row.last_run_at,
        last_run_status: row.last_run_status,
        last_run_message: row.last_run_message,
        created_at: row.created_at,
        updated_at: row.updated_at,
    })
}

/// Auto-suggest a name when the create body omits one.
fn auto_name(cfg: &MetadataRefreshJobConfig) -> String {
    let provider = cfg
        .provider
        .strip_prefix("plugin:")
        .unwrap_or(&cfg.provider);
    let groups: Vec<&str> = cfg.field_groups.iter().map(String::as_str).collect();
    if groups.is_empty() {
        provider.to_string()
    } else {
        format!("{provider} — {}", groups.join(", "))
    }
}

async fn validate_and_normalise_create(
    state: &AppState,
    name: &str,
    cron: &str,
    tz: Option<&str>,
    cfg: &LibraryJobConfigDto,
) -> Result<(String, String, Option<String>), ApiError> {
    let domain_cfg: LibraryJobConfig = cfg.clone().into();
    let validated = match &domain_cfg {
        LibraryJobConfig::MetadataRefresh(c) => {
            validation::validate_metadata_refresh_config(&state.db, name, cron, tz, c)
                .await
                .map_err(validation_to_api_error)?
        }
    };
    let normalised_name = name.to_string();
    Ok((normalised_name, validated.cron_schedule, validated.timezone))
}

// =============================================================================
// CRUD
// =============================================================================

pub async fn list_jobs(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    Path(library_id): Path<Uuid>,
) -> Result<Json<ListLibraryJobsResponse>, ApiError> {
    require_permission!(auth, Permission::LibrariesRead)?;
    ensure_library_exists(&state, library_id).await?;
    let rows = LibraryJobRepository::list_for_library(&state.db, library_id)
        .await
        .map_err(|e| anyhow_to_api_error(e, "Failed to list library jobs"))?;
    let jobs = rows
        .into_iter()
        .map(row_to_dto)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Json(ListLibraryJobsResponse { jobs }))
}

pub async fn get_job(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    Path((library_id, job_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<LibraryJobDto>, ApiError> {
    require_permission!(auth, Permission::LibrariesRead)?;
    let row = LibraryJobRepository::get_by_id(&state.db, job_id)
        .await
        .map_err(|e| anyhow_to_api_error(e, "Failed to load job"))?
        .ok_or_else(|| ApiError::NotFound("Job not found".to_string()))?;
    if row.library_id != library_id {
        return Err(ApiError::NotFound("Job not found".to_string()));
    }
    Ok(Json(row_to_dto(row)?))
}

pub async fn create_job(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    Path(library_id): Path<Uuid>,
    Json(body): Json<CreateLibraryJobRequest>,
) -> Result<(StatusCode, Json<LibraryJobDto>), ApiError> {
    require_permission!(auth, Permission::LibrariesWrite)?;
    ensure_library_exists(&state, library_id).await?;

    // Determine name (auto-generate when blank).
    let name = match body.name.as_deref().map(str::trim) {
        Some(s) if !s.is_empty() => s.to_string(),
        _ => match &body.config {
            LibraryJobConfigDto::MetadataRefresh(c) => {
                let mr: MetadataRefreshJobConfig = c.clone().into();
                auto_name(&mr)
            }
        },
    };

    let (name, cron, tz) = validate_and_normalise_create(
        &state,
        &name,
        &body.cron_schedule,
        body.timezone.as_deref(),
        &body.config,
    )
    .await?;

    // Determine the type discriminator from the config variant.
    let domain_cfg: LibraryJobConfig = body.config.into();
    let job_type = domain_cfg.job_type().as_str().to_string();
    let config_json = serde_json::to_string(&domain_cfg)
        .map_err(|e| anyhow_to_api_error(e.into(), "Failed to serialize config"))?;

    let row = LibraryJobRepository::create(
        &state.db,
        CreateLibraryJobParams {
            library_id,
            job_type,
            name,
            enabled: body.enabled,
            cron_schedule: cron,
            timezone: tz,
            config: config_json,
        },
    )
    .await
    .map_err(|e| anyhow_to_api_error(e, "Failed to create job"))?;

    if let Some(scheduler) = state.scheduler.as_ref() {
        let mut s = scheduler.lock().await;
        if let Err(e) = s.reload_schedules().await {
            tracing::warn!("Scheduler reload after job create failed: {e:#}");
        }
    }

    Ok((StatusCode::CREATED, Json(row_to_dto(row)?)))
}

pub async fn patch_job(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    Path((library_id, job_id)): Path<(Uuid, Uuid)>,
    Json(patch): Json<PatchLibraryJobRequest>,
) -> Result<Json<LibraryJobDto>, ApiError> {
    require_permission!(auth, Permission::LibrariesWrite)?;
    let mut row = LibraryJobRepository::get_by_id(&state.db, job_id)
        .await
        .map_err(|e| anyhow_to_api_error(e, "Failed to load job"))?
        .ok_or_else(|| ApiError::NotFound("Job not found".to_string()))?;
    if row.library_id != library_id {
        return Err(ApiError::NotFound("Job not found".to_string()));
    }

    if let Some(n) = patch.name.as_ref() {
        if n.trim().is_empty() {
            return Err(ApiError::BadRequest("Name cannot be empty".to_string()));
        }
        row.name = n.clone();
    }
    if let Some(e) = patch.enabled {
        row.enabled = e;
    }
    if let Some(cron) = patch.cron_schedule.as_ref() {
        row.cron_schedule = cron.clone();
    }
    match patch.timezone {
        PatchValue::Absent => {}
        PatchValue::Null => row.timezone = None,
        PatchValue::Value(tz) => row.timezone = Some(tz),
    }
    if let Some(new_cfg_dto) = patch.config {
        let new_cfg: LibraryJobConfig = new_cfg_dto.into();
        if new_cfg.job_type().as_str() != row.r#type {
            return Err(ApiError::BadRequest(
                "Config 'type' must match the existing job's type".to_string(),
            ));
        }
        row.config = serde_json::to_string(&new_cfg)
            .map_err(|e| anyhow_to_api_error(e.into(), "Failed to serialize config"))?;
    }

    // Re-validate the whole row's worth of typed fields.
    let cfg = parse_job_config(&row.r#type, &row.config)
        .map_err(|e| anyhow_to_api_error(e, "Failed to decode job config"))?;
    match &cfg {
        LibraryJobConfig::MetadataRefresh(c) => {
            validation::validate_metadata_refresh_config(
                &state.db,
                &row.name,
                &row.cron_schedule,
                row.timezone.as_deref(),
                c,
            )
            .await
            .map_err(validation_to_api_error)?;
        }
    }

    LibraryJobRepository::update(&state.db, &row)
        .await
        .map_err(|e| anyhow_to_api_error(e, "Failed to update job"))?;

    if let Some(scheduler) = state.scheduler.as_ref() {
        let mut s = scheduler.lock().await;
        if let Err(e) = s.reload_schedules().await {
            tracing::warn!("Scheduler reload after job patch failed: {e:#}");
        }
    }

    let updated = LibraryJobRepository::get_by_id(&state.db, job_id)
        .await
        .map_err(|e| anyhow_to_api_error(e, "Failed to reload job"))?
        .ok_or_else(|| ApiError::NotFound("Job vanished after update".to_string()))?;
    Ok(Json(row_to_dto(updated)?))
}

pub async fn delete_job(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    Path((library_id, job_id)): Path<(Uuid, Uuid)>,
) -> Result<Response, ApiError> {
    require_permission!(auth, Permission::LibrariesWrite)?;
    let row = LibraryJobRepository::get_by_id(&state.db, job_id)
        .await
        .map_err(|e| anyhow_to_api_error(e, "Failed to load job"))?
        .ok_or_else(|| ApiError::NotFound("Job not found".to_string()))?;
    if row.library_id != library_id {
        return Err(ApiError::NotFound("Job not found".to_string()));
    }
    LibraryJobRepository::delete(&state.db, job_id)
        .await
        .map_err(|e| anyhow_to_api_error(e, "Failed to delete job"))?;

    if let Some(scheduler) = state.scheduler.as_ref() {
        let mut s = scheduler.lock().await;
        if let Err(e) = s.reload_schedules().await {
            tracing::warn!("Scheduler reload after job delete failed: {e:#}");
        }
    }
    Ok(StatusCode::NO_CONTENT.into_response())
}

// =============================================================================
// Run-now
// =============================================================================

pub async fn run_job_now(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    Path((library_id, job_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<RunNowResponse>, ApiError> {
    require_permission!(auth, Permission::LibrariesWrite)?;
    let row = LibraryJobRepository::get_by_id(&state.db, job_id)
        .await
        .map_err(|e| anyhow_to_api_error(e, "Failed to load job"))?
        .ok_or_else(|| ApiError::NotFound("Job not found".to_string()))?;
    if row.library_id != library_id {
        return Err(ApiError::NotFound("Job not found".to_string()));
    }

    if crate::scheduler::has_active_refresh_for_job(&state.db, job_id)
        .await
        .map_err(|e| anyhow_to_api_error(e, "Failed to check in-flight task"))?
    {
        return Err(ApiError::Conflict(
            "A refresh task for this job is already running".to_string(),
        ));
    }

    let task_id = crate::db::repositories::TaskRepository::enqueue(
        &state.db,
        TaskType::RefreshLibraryMetadata { job_id },
        None,
    )
    .await
    .map_err(|e| anyhow_to_api_error(e, "Failed to enqueue task"))?;

    Ok(Json(RunNowResponse { task_id }))
}

// =============================================================================
// Dry-run
// =============================================================================

const DRY_RUN_DEFAULT_SAMPLE: u32 = 5;
const DRY_RUN_MAX_SAMPLE: u32 = 20;

pub async fn dry_run_job(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    Path((library_id, job_id)): Path<(Uuid, Uuid)>,
    Json(body): Json<DryRunRequest>,
) -> Result<Json<DryRunResponse>, ApiError> {
    require_permission!(auth, Permission::LibrariesWrite)?;
    let row = LibraryJobRepository::get_by_id(&state.db, job_id)
        .await
        .map_err(|e| anyhow_to_api_error(e, "Failed to load job"))?
        .ok_or_else(|| ApiError::NotFound("Job not found".to_string()))?;
    if row.library_id != library_id {
        return Err(ApiError::NotFound("Job not found".to_string()));
    }

    // Resolve the config to plan against.
    let cfg = if let Some(override_dto) = body.config_override {
        let domain: LibraryJobConfig = override_dto.into();
        if domain.job_type().as_str() != row.r#type {
            return Err(ApiError::BadRequest(
                "Override config 'type' must match the job's type".to_string(),
            ));
        }
        match domain {
            LibraryJobConfig::MetadataRefresh(c) => {
                validation::validate_metadata_refresh_config(
                    &state.db,
                    &row.name,
                    &row.cron_schedule,
                    row.timezone.as_deref(),
                    &c,
                )
                .await
                .map_err(validation_to_api_error)?;
                c
            }
        }
    } else {
        let parsed = parse_job_config(&row.r#type, &row.config)
            .map_err(|e| anyhow_to_api_error(e, "Failed to decode job config"))?;
        let LibraryJobConfig::MetadataRefresh(c) = parsed;
        c
    };

    let plan = RefreshPlanner::plan(&state.db, library_id, &cfg)
        .await
        .map_err(|e| anyhow_to_api_error(e, "Failed to build refresh plan"))?;

    // Surface plan-level failures cleanly.
    if let Some(failure) = plan.failure.as_ref() {
        return Ok(Json(DryRunResponse {
            total_eligible: 0,
            sample: vec![],
            est_skipped_no_id: 0,
            est_skipped_recently_synced: 0,
            plan_failure: Some(failure.as_str().to_string()),
        }));
    }

    let total_eligible = plan.planned.len() as u32;
    let mut est_skipped_no_id = 0u32;
    let mut est_skipped_recently = 0u32;
    for s in &plan.skipped {
        match s.reason {
            crate::services::metadata::SkipReason::NoExternalId => est_skipped_no_id += 1,
            crate::services::metadata::SkipReason::RecentlySynced { .. } => {
                est_skipped_recently += 1
            }
        }
    }

    let sample_size = body
        .sample_size
        .unwrap_or(DRY_RUN_DEFAULT_SAMPLE)
        .min(DRY_RUN_MAX_SAMPLE) as usize;

    // For Phase 9 we return a planner-only sample (no plugin call). Phase 6
    // executed plugin calls per pair; the downsides (slow, brittle) outweigh
    // the marginal benefit when the user is previewing a single provider.
    let mut sample = Vec::new();
    for planned in plan.planned.iter().take(sample_size) {
        let series = SeriesRepository::get_by_id(&state.db, planned.series_id)
            .await
            .map_err(|e| anyhow_to_api_error(e, "Failed to load series"))?;
        let series_name = series
            .map(|s| s.name)
            .unwrap_or_else(|| "(unknown)".to_string());
        sample.push(DryRunSeriesDelta {
            series_id: planned.series_id,
            series_name,
            // Actual `before/after` requires a plugin call; we surface a
            // single placeholder row so the UI can render "this series is
            // a candidate" without lying about specifics.
            changes: std::collections::HashMap::from([(
                "_preview".to_string(),
                DryRunFieldChange {
                    before: serde_json::Value::Null,
                    after: serde_json::Value::String(format!(
                        "would refresh via {}",
                        planned.plugin.name
                    )),
                },
            )]),
            skipped: vec![],
        });
    }

    Ok(Json(DryRunResponse {
        total_eligible,
        sample,
        est_skipped_no_id,
        est_skipped_recently_synced: est_skipped_recently,
        plan_failure: None,
    }))
}

// =============================================================================
// Field-groups catalog
// =============================================================================

pub async fn list_field_groups(auth: AuthContext) -> Result<Json<Vec<FieldGroupDto>>, ApiError> {
    require_permission!(auth, Permission::LibrariesRead)?;
    let mut out = Vec::with_capacity(FieldGroup::all().len());
    for g in FieldGroup::all() {
        out.push(FieldGroupDto {
            id: g.as_str().to_string(),
            label: human_label(*g).to_string(),
            fields: fields_for_group(*g)
                .iter()
                .map(|s| (*s).to_string())
                .collect(),
        });
    }
    Ok(Json(out))
}

fn human_label(g: FieldGroup) -> &'static str {
    match g {
        FieldGroup::Identifiers => "Identifiers",
        FieldGroup::Descriptive => "Descriptive",
        FieldGroup::Status => "Status",
        FieldGroup::Counts => "Counts",
        FieldGroup::Ratings => "Ratings",
        FieldGroup::Cover => "Cover",
        FieldGroup::Tags => "Tags",
        FieldGroup::Genres => "Genres",
        FieldGroup::AgeRating => "Age rating",
        FieldGroup::Classification => "Classification",
        FieldGroup::Publisher => "Publisher",
        FieldGroup::ExternalRefs => "External refs",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::library_jobs::RefreshScope;

    #[test]
    fn auto_name_uses_provider_and_groups() {
        let cfg = MetadataRefreshJobConfig {
            provider: "plugin:mb".to_string(),
            scope: RefreshScope::SeriesOnly,
            field_groups: vec!["ratings".to_string(), "status".to_string()],
            extra_fields: vec![],
            book_field_groups: vec![],
            book_extra_fields: vec![],
            existing_source_ids_only: true,
            skip_recently_synced_within_s: 0,
            max_concurrency: 4,
        };
        assert_eq!(auto_name(&cfg), "mb — ratings, status");
    }

    #[test]
    fn auto_name_handles_no_groups() {
        let cfg = MetadataRefreshJobConfig {
            provider: "plugin:foo".to_string(),
            field_groups: vec![],
            ..MetadataRefreshJobConfig::default()
        };
        assert_eq!(auto_name(&cfg), "foo");
    }

    #[test]
    fn human_labels_cover_every_group() {
        for g in FieldGroup::all() {
            let lbl = human_label(*g);
            assert!(!lbl.is_empty());
        }
    }
}
