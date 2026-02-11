//! Plugin Actions API handlers (Phase 4)
//!
//! Provides endpoints for plugin action discovery and execution:
//! - GET /api/v1/plugins/actions - Get available plugin actions for a scope
//! - POST /api/v1/plugins/:id/execute - Execute a plugin method
//!
//! And metadata operations via plugins:
//! - POST /api/v1/series/:id/metadata/preview - Preview metadata from a plugin
//! - POST /api/v1/series/:id/metadata/apply - Apply metadata from a plugin
//! - POST /api/v1/books/:id/metadata/preview - Preview metadata for a book
//! - POST /api/v1/books/:id/metadata/apply - Apply metadata for a book

use super::super::dto::{
    EnqueueAutoMatchRequest, EnqueueAutoMatchResponse, EnqueueBulkAutoMatchRequest,
    EnqueueLibraryAutoMatchRequest, ExecutePluginRequest, ExecutePluginResponse, FieldApplyStatus,
    MetadataAction, MetadataApplyRequest, MetadataApplyResponse, MetadataAutoMatchRequest,
    MetadataAutoMatchResponse, MetadataFieldPreview, MetadataPreviewRequest,
    MetadataPreviewResponse, PluginActionDto, PluginActionRequest, PluginActionsResponse,
    PluginSearchResponse, PluginSearchResultDto, PreviewSummary, SearchTitleResponse, SkippedField,
    parse_scope,
};
use crate::api::{AppState, error::ApiError, extractors::AuthContext, permissions::Permission};
use crate::db::entities::plugins::PluginPermission;
use crate::db::repositories::{
    AlternateTitleRepository, BookExternalIdRepository, BookMetadataRepository, BookRepository,
    ExternalLinkRepository, ExternalRatingRepository, GenreRepository, LibraryRepository,
    PluginsRepository, SeriesExternalIdRepository, SeriesMetadataRepository, SeriesRepository,
    TagRepository, TaskRepository,
};
use crate::services::metadata::preprocessing::{
    PreprocessingRule, SeriesContextBuilder, apply_rules, render_template,
};
use crate::services::metadata::{
    ApplyOptions, BookApplyOptions, BookMetadataApplier, MetadataApplier,
};
use crate::services::plugin::PluginManagerError;
use crate::services::plugin::protocol::{
    BookMatchParams, BookSearchParams, MetadataContentType, MetadataGetParams, MetadataMatchParams,
    MetadataSearchParams, PluginScope,
};
use crate::tasks::types::TaskType;
use axum::{
    Json,
    extract::{Path, Query, State},
};
use sea_orm::prelude::Decimal;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Instant;
use utoipa::OpenApi;
use uuid::Uuid;

#[derive(OpenApi)]
#[openapi(
    paths(
        get_plugin_actions,
        execute_plugin,
        preview_series_metadata,
        apply_series_metadata,
        auto_match_series_metadata,
        get_series_search_title,
        enqueue_auto_match_task,
        enqueue_bulk_auto_match_tasks,
        enqueue_library_auto_match_tasks,
        preview_book_metadata,
        apply_book_metadata,
    ),
    components(schemas(
        PluginActionDto,
        PluginActionsResponse,
        MetadataAction,
        PluginActionRequest,
        ExecutePluginRequest,
        ExecutePluginResponse,
        PluginSearchResponse,
        PluginSearchResultDto,
        MetadataPreviewRequest,
        MetadataPreviewResponse,
        MetadataFieldPreview,
        FieldApplyStatus,
        PreviewSummary,
        MetadataApplyRequest,
        MetadataApplyResponse,
        SkippedField,
        MetadataAutoMatchRequest,
        MetadataAutoMatchResponse,
        SearchTitleResponse,
        EnqueueAutoMatchRequest,
        EnqueueAutoMatchResponse,
        EnqueueBulkAutoMatchRequest,
        EnqueueLibraryAutoMatchRequest,
    )),
    tags(
        (name = "Plugin Actions", description = "Plugin action discovery and execution")
    )
)]
#[allow(dead_code)]
pub struct PluginActionsApi;

/// Query parameters for getting plugin actions
#[derive(Debug, Deserialize, utoipa::IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct PluginActionsQuery {
    /// Scope to filter actions by (e.g., "series:detail", "series:bulk")
    pub scope: String,

    /// Optional library ID to filter plugins by. When provided, only plugins that
    /// apply to this library (or all libraries) will be returned.
    #[serde(default)]
    pub library_id: Option<Uuid>,
}

/// Get available plugin actions for a scope
///
/// Returns a list of available plugin actions for the specified scope.
/// This is used by the UI to populate dropdown menus with available plugins.
#[utoipa::path(
    get,
    path = "/api/v1/plugins/actions",
    params(PluginActionsQuery),
    responses(
        (status = 200, description = "Plugin actions retrieved", body = PluginActionsResponse),
        (status = 400, description = "Invalid scope"),
        (status = 401, description = "Unauthorized"),
    ),
    security(
        ("bearer_auth" = []),
        ("api_key" = [])
    ),
    tag = "Plugin Actions"
)]
pub async fn get_plugin_actions(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    Query(query): Query<PluginActionsQuery>,
) -> Result<Json<PluginActionsResponse>, ApiError> {
    // Require LibrariesRead permission to view plugin actions.
    // This prevents users without library access from enumerating plugins.
    auth.require_permission(&Permission::LibrariesRead)?;

    // Get user's effective permissions for filtering plugins by capability
    let user_permissions = auth.effective_permissions();

    // Parse and validate scope
    let scope = parse_scope(&query.scope).ok_or_else(|| {
        ApiError::BadRequest(format!(
            "Invalid scope '{}'. Valid scopes: series:detail, series:bulk, book:detail, book:bulk, library:detail, library:scan",
            query.scope
        ))
    })?;

    // If library_id is provided, verify the library exists
    if let Some(library_id) = query.library_id {
        let library_exists = LibraryRepository::get_by_id(&state.db, library_id)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to check library: {}", e)))?
            .is_some();

        if !library_exists {
            return Err(ApiError::NotFound("Library not found".to_string()));
        }
    }

    // Get plugins that support this scope, optionally filtered by library
    let plugins = match query.library_id {
        Some(library_id) => {
            state
                .plugin_manager
                .plugins_by_scope_and_library(&scope, library_id)
                .await
        }
        None => state.plugin_manager.plugins_by_scope(&scope).await,
    };

    // Build actions list, filtering by user permissions
    let mut actions = Vec::new();

    for plugin in plugins {
        // Skip disabled plugins
        if !plugin.enabled {
            continue;
        }

        // Check if plugin has metadata provider capability from its cached manifest
        let manifest = match plugin.cached_manifest() {
            Some(m) => m,
            None => continue, // No manifest = can't determine capabilities
        };

        // Get the content types this plugin supports
        let supported_content_types = &manifest.capabilities.metadata_provider;

        // Skip plugins the user doesn't have permission to use
        // User needs write permission for at least one of the plugin's content types
        if !user_can_use_plugin(supported_content_types, &user_permissions) {
            continue;
        }

        // Check if the plugin can provide metadata for the requested scope's content type
        let can_provide = match scope {
            PluginScope::SeriesDetail | PluginScope::SeriesBulk => {
                manifest.capabilities.can_provide_series_metadata()
            }
            PluginScope::BookDetail | PluginScope::BookBulk => {
                manifest.capabilities.can_provide_book_metadata()
            }
            PluginScope::LibraryDetail | PluginScope::LibraryScan => {
                // Library-level scopes: include plugin if it provides either content type
                manifest.capabilities.can_provide_series_metadata()
                    || manifest.capabilities.can_provide_book_metadata()
            }
        };

        if can_provide {
            // Add metadata search action
            actions.push(PluginActionDto {
                plugin_id: plugin.id,
                plugin_name: plugin.name.clone(),
                plugin_display_name: plugin.display_name.clone(),
                action_type: "metadata_search".to_string(),
                label: format!("Fetch from {}", plugin.display_name),
                description: plugin.description.clone(),
                icon: Some("search".to_string()),
                library_ids: plugin.library_ids_vec(),
            });
        }
    }

    Ok(Json(PluginActionsResponse {
        actions,
        scope: query.scope,
    }))
}

/// Execute a plugin action
///
/// Invokes a plugin action and returns the result. Actions are typed by plugin type:
/// - `metadata`: search, get, match (requires write permission for the content_type)
/// - `ping`: health check (requires PluginsManage permission)
#[utoipa::path(
    post,
    path = "/api/v1/plugins/{id}/execute",
    params(
        ("id" = Uuid, Path, description = "Plugin ID")
    ),
    request_body = ExecutePluginRequest,
    responses(
        (status = 200, description = "Action executed", body = ExecutePluginResponse),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Insufficient permission for this action"),
        (status = 404, description = "Plugin not found"),
    ),
    security(
        ("bearer_auth" = []),
        ("api_key" = [])
    ),
    tag = "Plugin Actions"
)]
pub async fn execute_plugin(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    Path(plugin_id): Path<Uuid>,
    Json(request): Json<ExecutePluginRequest>,
) -> Result<Json<ExecutePluginResponse>, ApiError> {
    let start = Instant::now();

    // Check permission based on action type
    match &request.action {
        PluginActionRequest::Metadata { content_type, .. } => {
            // Metadata actions require write permission for the content type
            let required_permission = permission_for_content_type(content_type);
            auth.require_permission(&required_permission)?;
        }
        PluginActionRequest::Ping => {
            // Ping is an admin operation (health check)
            auth.require_permission(&Permission::PluginsManage)?;
        }
    }

    // Get plugin from database to verify it exists
    let plugin = PluginsRepository::get_by_id(&state.db, plugin_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to get plugin: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Plugin not found".to_string()))?;

    if !plugin.enabled {
        return Ok(Json(ExecutePluginResponse {
            success: false,
            result: None,
            error: Some("Plugin is disabled".to_string()),
            latency_ms: start.elapsed().as_millis() as u64,
        }));
    }

    // Read internal config for server-side settings
    let internal_config = plugin.internal_config_parsed();

    // Execute based on action type
    // Backend owns the protocol method strings - frontend only knows about typed actions
    match request.action {
        PluginActionRequest::Metadata {
            action,
            content_type,
            params,
        } => {
            execute_metadata_action(
                &state,
                plugin_id,
                action,
                content_type,
                params,
                internal_config.search_results_limit,
                start,
            )
            .await
        }
        PluginActionRequest::Ping => match state.plugin_manager.ping(plugin_id).await {
            Ok(()) => Ok(Json(ExecutePluginResponse {
                success: true,
                result: Some(serde_json::json!("pong")),
                error: None,
                latency_ms: start.elapsed().as_millis() as u64,
            })),
            Err(e) => Ok(Json(ExecutePluginResponse {
                success: false,
                result: None,
                error: Some(sanitize_plugin_error(&e)),
                latency_ms: start.elapsed().as_millis() as u64,
            })),
        },
    }
}

/// Execute a metadata plugin action
async fn execute_metadata_action(
    state: &Arc<AppState>,
    plugin_id: Uuid,
    action: MetadataAction,
    content_type: MetadataContentType,
    params: serde_json::Value,
    search_results_limit: Option<u32>,
    start: Instant,
) -> Result<Json<ExecutePluginResponse>, ApiError> {
    match (action, content_type) {
        (MetadataAction::Search, MetadataContentType::Series) => {
            let mut params: MetadataSearchParams = serde_json::from_value(params)
                .map_err(|e| ApiError::BadRequest(format!("Invalid search params: {}", e)))?;

            // Apply server-side search results limit if client didn't set one
            if params.limit.is_none() {
                params.limit = search_results_limit;
            }

            match state.plugin_manager.search_series(plugin_id, params).await {
                Ok(response) => {
                    let result = serde_json::to_value(&response)
                        .map_err(|e| ApiError::Internal(format!("Failed to serialize: {}", e)))?;

                    Ok(Json(ExecutePluginResponse {
                        success: true,
                        result: Some(result),
                        error: None,
                        latency_ms: start.elapsed().as_millis() as u64,
                    }))
                }
                Err(e) => Ok(Json(ExecutePluginResponse {
                    success: false,
                    result: None,
                    error: Some(sanitize_plugin_error(&e)),
                    latency_ms: start.elapsed().as_millis() as u64,
                })),
            }
        }
        (MetadataAction::Get, MetadataContentType::Series) => {
            let params: MetadataGetParams = serde_json::from_value(params)
                .map_err(|e| ApiError::BadRequest(format!("Invalid get params: {}", e)))?;

            match state
                .plugin_manager
                .get_series_metadata(plugin_id, params)
                .await
            {
                Ok(metadata) => {
                    let result = serde_json::to_value(&metadata)
                        .map_err(|e| ApiError::Internal(format!("Failed to serialize: {}", e)))?;

                    Ok(Json(ExecutePluginResponse {
                        success: true,
                        result: Some(result),
                        error: None,
                        latency_ms: start.elapsed().as_millis() as u64,
                    }))
                }
                Err(e) => Ok(Json(ExecutePluginResponse {
                    success: false,
                    result: None,
                    error: Some(sanitize_plugin_error(&e)),
                    latency_ms: start.elapsed().as_millis() as u64,
                })),
            }
        }
        (MetadataAction::Match, MetadataContentType::Series) => {
            let params: MetadataMatchParams = serde_json::from_value(params)
                .map_err(|e| ApiError::BadRequest(format!("Invalid match params: {}", e)))?;

            match state.plugin_manager.match_series(plugin_id, params).await {
                Ok(result) => {
                    let result = serde_json::to_value(&result)
                        .map_err(|e| ApiError::Internal(format!("Failed to serialize: {}", e)))?;

                    Ok(Json(ExecutePluginResponse {
                        success: true,
                        result: Some(result),
                        error: None,
                        latency_ms: start.elapsed().as_millis() as u64,
                    }))
                }
                Err(e) => Ok(Json(ExecutePluginResponse {
                    success: false,
                    result: None,
                    error: Some(sanitize_plugin_error(&e)),
                    latency_ms: start.elapsed().as_millis() as u64,
                })),
            }
        }
        // Book metadata actions
        (MetadataAction::Search, MetadataContentType::Book) => {
            let mut params: BookSearchParams = serde_json::from_value(params)
                .map_err(|e| ApiError::BadRequest(format!("Invalid book search params: {}", e)))?;

            // Apply server-side search results limit if client didn't set one
            if params.limit.is_none() {
                params.limit = search_results_limit;
            }

            // Validate that at least one of isbn or query is provided
            if !params.is_valid() {
                return Err(ApiError::BadRequest(
                    "Either 'isbn' or 'query' must be provided for book search".to_string(),
                ));
            }

            match state.plugin_manager.search_book(plugin_id, params).await {
                Ok(response) => {
                    let result = serde_json::to_value(&response)
                        .map_err(|e| ApiError::Internal(format!("Failed to serialize: {}", e)))?;

                    Ok(Json(ExecutePluginResponse {
                        success: true,
                        result: Some(result),
                        error: None,
                        latency_ms: start.elapsed().as_millis() as u64,
                    }))
                }
                Err(e) => Ok(Json(ExecutePluginResponse {
                    success: false,
                    result: None,
                    error: Some(sanitize_plugin_error(&e)),
                    latency_ms: start.elapsed().as_millis() as u64,
                })),
            }
        }
        (MetadataAction::Get, MetadataContentType::Book) => {
            let params: MetadataGetParams = serde_json::from_value(params)
                .map_err(|e| ApiError::BadRequest(format!("Invalid get params: {}", e)))?;

            match state
                .plugin_manager
                .get_book_metadata(plugin_id, params)
                .await
            {
                Ok(metadata) => {
                    let result = serde_json::to_value(&metadata)
                        .map_err(|e| ApiError::Internal(format!("Failed to serialize: {}", e)))?;

                    Ok(Json(ExecutePluginResponse {
                        success: true,
                        result: Some(result),
                        error: None,
                        latency_ms: start.elapsed().as_millis() as u64,
                    }))
                }
                Err(e) => Ok(Json(ExecutePluginResponse {
                    success: false,
                    result: None,
                    error: Some(sanitize_plugin_error(&e)),
                    latency_ms: start.elapsed().as_millis() as u64,
                })),
            }
        }
        (MetadataAction::Match, MetadataContentType::Book) => {
            let params: BookMatchParams = serde_json::from_value(params)
                .map_err(|e| ApiError::BadRequest(format!("Invalid book match params: {}", e)))?;

            match state.plugin_manager.match_book(plugin_id, params).await {
                Ok(result) => {
                    let result = serde_json::to_value(&result)
                        .map_err(|e| ApiError::Internal(format!("Failed to serialize: {}", e)))?;

                    Ok(Json(ExecutePluginResponse {
                        success: true,
                        result: Some(result),
                        error: None,
                        latency_ms: start.elapsed().as_millis() as u64,
                    }))
                }
                Err(e) => Ok(Json(ExecutePluginResponse {
                    success: false,
                    result: None,
                    error: Some(sanitize_plugin_error(&e)),
                    latency_ms: start.elapsed().as_millis() as u64,
                })),
            }
        }
    }
}

/// Query parameters for getting the search title
#[derive(Debug, Deserialize, utoipa::IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct SearchTitleQuery {
    /// Plugin ID to get preprocessing rules from
    pub plugin_id: Uuid,
}

/// Get the preprocessed search title for a series
///
/// Returns the series title after applying plugin and library preprocessing rules.
/// Use this to get the correct search query before opening the metadata search modal.
#[utoipa::path(
    get,
    path = "/api/v1/series/{id}/metadata/search-title",
    params(
        ("id" = Uuid, Path, description = "Series ID"),
        SearchTitleQuery
    ),
    responses(
        (status = 200, description = "Preprocessed search title", body = SearchTitleResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Series or plugin not found"),
    ),
    security(
        ("bearer_auth" = []),
        ("api_key" = [])
    ),
    tag = "Plugin Actions"
)]
pub async fn get_series_search_title(
    State(state): State<Arc<AppState>>,
    _auth: AuthContext,
    Path(series_id): Path<Uuid>,
    Query(query): Query<SearchTitleQuery>,
) -> Result<Json<SearchTitleResponse>, ApiError> {
    // Get the series
    let series = SeriesRepository::get_by_id(&state.db, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to get series: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Series not found".to_string()))?;

    // Get the plugin
    let plugin = PluginsRepository::get_by_id(&state.db, query.plugin_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to get plugin: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Plugin not found".to_string()))?;

    // Get the library for library-level preprocessing rules
    let library = LibraryRepository::get_by_id(&state.db, series.library_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to get library: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Library not found".to_string()))?;

    // Build the full series context using the new builder
    // This includes metadata, genres, tags, book count, external IDs, and custom metadata
    let series_context = SeriesContextBuilder::new(series_id)
        .build(&state.db)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to build series context: {}", e)))?;

    // Get original title from context
    let original_title = series_context
        .metadata
        .title
        .clone()
        .unwrap_or_else(|| series.name.clone());

    // Step 1: Apply search query template if configured
    let templated_title =
        if let Some(template) = PluginsRepository::get_search_query_template(&plugin) {
            // Convert series context to JSON for template rendering
            let context_json =
                serde_json::to_value(&series_context).unwrap_or_else(|_| serde_json::json!({}));
            match render_template(template, &context_json) {
                Ok(rendered) => rendered,
                Err(_) => original_title.clone(), // Fall back to original on template error
            }
        } else {
            original_title.clone()
        };

    // Step 2: Apply preprocessing rules (plugin rules first, then library rules)
    let plugin_rules = PluginsRepository::get_search_preprocessing_rules(&plugin);
    let library_rules = LibraryRepository::get_preprocessing_rules(&library);
    let total_rules = plugin_rules.len() + library_rules.len();
    let search_title = apply_preprocessing_rules(&templated_title, &plugin_rules, &library_rules);

    Ok(Json(SearchTitleResponse {
        original_title,
        search_title,
        rules_applied: total_rules,
    }))
}

/// Apply preprocessing rules to a query string
///
/// Plugin rules are applied first, then library rules.
fn apply_preprocessing_rules(
    query: &str,
    plugin_rules: &[PreprocessingRule],
    library_rules: &[PreprocessingRule],
) -> String {
    let mut result = query.to_string();

    // Apply plugin rules first
    if !plugin_rules.is_empty() {
        result = apply_rules(&result, plugin_rules);
    }

    // Then apply library rules
    if !library_rules.is_empty() {
        result = apply_rules(&result, library_rules);
    }

    result
}

/// Preview metadata from a plugin for a series
///
/// Fetches metadata from a plugin and computes a field-by-field diff with the current
/// series metadata, showing which fields will be applied, locked, or denied by RBAC.
#[utoipa::path(
    post,
    path = "/api/v1/series/{id}/metadata/preview",
    params(
        ("id" = Uuid, Path, description = "Series ID")
    ),
    request_body = MetadataPreviewRequest,
    responses(
        (status = 200, description = "Preview computed", body = MetadataPreviewResponse),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "No permission to edit series"),
        (status = 404, description = "Series or plugin not found"),
    ),
    security(
        ("bearer_auth" = []),
        ("api_key" = [])
    ),
    tag = "Plugin Actions"
)]
pub async fn preview_series_metadata(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    Path(series_id): Path<Uuid>,
    Json(request): Json<MetadataPreviewRequest>,
) -> Result<Json<MetadataPreviewResponse>, ApiError> {
    // Check permission to edit series metadata
    auth.require_permission(&Permission::SeriesWrite)?;

    // Get the series (verify it exists)
    let series = SeriesRepository::get_by_id(&state.db, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to get series: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Series not found".to_string()))?;

    // Get the plugin
    let plugin = PluginsRepository::get_by_id(&state.db, request.plugin_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to get plugin: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Plugin not found".to_string()))?;

    if !plugin.enabled {
        return Err(ApiError::BadRequest("Plugin is disabled".to_string()));
    }

    // Check if plugin applies to this series' library
    if !plugin.applies_to_library(series.library_id) {
        return Err(ApiError::BadRequest(format!(
            "Plugin '{}' is not configured to apply to this series' library",
            plugin.display_name
        )));
    }

    // Fetch metadata from plugin
    let params = MetadataGetParams {
        external_id: request.external_id.clone(),
    };

    let plugin_metadata = state
        .plugin_manager
        .get_series_metadata(request.plugin_id, params)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch metadata from plugin: {}", e)))?;

    // Get current series metadata
    let current_metadata = SeriesMetadataRepository::get_by_series_id(&state.db, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to get current metadata: {}", e)))?;

    // Get current genres, tags, and alternate titles
    let current_genres = GenreRepository::get_genres_for_series(&state.db, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to get genres: {}", e)))?;
    let current_tags = TagRepository::get_tags_for_series(&state.db, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to get tags: {}", e)))?;
    let current_alternate_titles = AlternateTitleRepository::get_for_series(&state.db, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to get alternate titles: {}", e)))?;

    // Get plugin permissions (used via has_permission closure)
    let _plugin_permissions = plugin.permissions_vec();

    // Build field-by-field preview
    let mut fields = Vec::new();
    let mut will_apply = 0;
    let mut locked = 0;
    let mut no_permission = 0;
    let mut unchanged = 0;
    let mut not_provided = 0;

    // Helper to check permission
    let has_permission = |perm: PluginPermission| -> bool { plugin.has_permission(&perm) };

    // Title
    let title_will_change = plugin_metadata.title.is_some()
        && !current_metadata
            .as_ref()
            .map(|m| m.title_lock)
            .unwrap_or(false)
        && has_permission(PluginPermission::MetadataWriteTitle)
        && current_metadata
            .as_ref()
            .map(|m| Some(&m.title) != plugin_metadata.title.as_ref())
            .unwrap_or(true);

    fields.push(build_field_preview(
        "title",
        current_metadata
            .as_ref()
            .map(|m| serde_json::json!(m.title.clone())),
        plugin_metadata.title.as_ref().map(|v| serde_json::json!(v)),
        current_metadata
            .as_ref()
            .map(|m| m.title_lock)
            .unwrap_or(false),
        has_permission(PluginPermission::MetadataWriteTitle),
        &mut will_apply,
        &mut locked,
        &mut no_permission,
        &mut unchanged,
        &mut not_provided,
    ));

    // title_sort - automatically updated when title changes (unless locked)
    // This is a derived field: if title changes and title_sort is not locked,
    // title_sort will be updated to match the new title
    let title_sort_locked = current_metadata
        .as_ref()
        .map(|m| m.title_sort_lock)
        .unwrap_or(false);
    let proposed_title_sort = if title_will_change && !title_sort_locked {
        plugin_metadata.title.as_ref().map(|v| serde_json::json!(v))
    } else {
        None
    };
    fields.push(build_field_preview(
        "titleSort",
        current_metadata
            .as_ref()
            .and_then(|m| m.title_sort.as_ref().map(|v| serde_json::json!(v))),
        proposed_title_sort,
        title_sort_locked,
        true, // title_sort update is automatic, no separate permission needed
        &mut will_apply,
        &mut locked,
        &mut no_permission,
        &mut unchanged,
        &mut not_provided,
    ));

    // Alternate Titles
    let mut current_alt_titles: Vec<serde_json::Value> = current_alternate_titles
        .iter()
        .map(|t| serde_json::json!({"label": t.label, "title": t.title}))
        .collect();
    current_alt_titles.sort_by(|a, b| {
        let a_title = a["title"].as_str().unwrap_or("");
        let b_title = b["title"].as_str().unwrap_or("");
        a_title.cmp(b_title)
    });
    fields.push(build_field_preview(
        "alternateTitles",
        if current_alt_titles.is_empty() {
            None
        } else {
            Some(serde_json::json!(current_alt_titles))
        },
        if plugin_metadata.alternate_titles.is_empty() {
            None
        } else {
            // Generate labels matching the apply logic: language > title_type > "alternate",
            // with dedup suffixes for duplicates (e.g., "en", "en-2", "en-3")
            let mut label_counts: HashMap<String, u32> = HashMap::new();
            let mut proposed_alt_titles: Vec<serde_json::Value> = plugin_metadata
                .alternate_titles
                .iter()
                .map(|t| {
                    let base_label = t
                        .language
                        .clone()
                        .or_else(|| t.title_type.clone())
                        .unwrap_or_else(|| "alternate".to_string());
                    let count = label_counts.entry(base_label.clone()).or_insert(0);
                    *count += 1;
                    let label = if *count == 1 {
                        base_label
                    } else {
                        format!("{}-{}", base_label, count)
                    };
                    serde_json::json!({
                        "label": label,
                        "title": t.title
                    })
                })
                .collect();
            proposed_alt_titles.sort_by(|a, b| {
                let a_title = a["title"].as_str().unwrap_or("");
                let b_title = b["title"].as_str().unwrap_or("");
                a_title.cmp(b_title)
            });
            Some(serde_json::json!(proposed_alt_titles))
        },
        current_metadata
            .as_ref()
            .map(|m| m.title_lock) // Use title_lock to control alternate titles too
            .unwrap_or(false),
        has_permission(PluginPermission::MetadataWriteTitle), // Use title permission
        &mut will_apply,
        &mut locked,
        &mut no_permission,
        &mut unchanged,
        &mut not_provided,
    ));

    // Summary
    fields.push(build_field_preview(
        "summary",
        current_metadata
            .as_ref()
            .and_then(|m| m.summary.as_ref().map(|v| serde_json::json!(v))),
        plugin_metadata
            .summary
            .as_ref()
            .map(|v| serde_json::json!(v)),
        current_metadata
            .as_ref()
            .map(|m| m.summary_lock)
            .unwrap_or(false),
        has_permission(PluginPermission::MetadataWriteSummary),
        &mut will_apply,
        &mut locked,
        &mut no_permission,
        &mut unchanged,
        &mut not_provided,
    ));

    // Year
    fields.push(build_field_preview(
        "year",
        current_metadata
            .as_ref()
            .and_then(|m| m.year.map(|v| serde_json::json!(v))),
        plugin_metadata.year.map(|v| serde_json::json!(v)),
        current_metadata
            .as_ref()
            .map(|m| m.year_lock)
            .unwrap_or(false),
        has_permission(PluginPermission::MetadataWriteYear),
        &mut will_apply,
        &mut locked,
        &mut no_permission,
        &mut unchanged,
        &mut not_provided,
    ));

    // Status
    fields.push(build_field_preview(
        "status",
        current_metadata
            .as_ref()
            .and_then(|m| m.status.as_ref().map(|v| serde_json::json!(v))),
        plugin_metadata
            .status
            .as_ref()
            .map(|v| serde_json::json!(v.to_string())),
        current_metadata
            .as_ref()
            .map(|m| m.status_lock)
            .unwrap_or(false),
        has_permission(PluginPermission::MetadataWriteStatus),
        &mut will_apply,
        &mut locked,
        &mut no_permission,
        &mut unchanged,
        &mut not_provided,
    ));

    // Publisher
    fields.push(build_field_preview(
        "publisher",
        current_metadata
            .as_ref()
            .and_then(|m| m.publisher.as_ref().map(|v| serde_json::json!(v))),
        plugin_metadata
            .publisher
            .as_ref()
            .map(|v| serde_json::json!(v)),
        current_metadata
            .as_ref()
            .map(|m| m.publisher_lock)
            .unwrap_or(false),
        has_permission(PluginPermission::MetadataWritePublisher),
        &mut will_apply,
        &mut locked,
        &mut no_permission,
        &mut unchanged,
        &mut not_provided,
    ));

    // Genres
    let mut current_genre_names: Vec<String> =
        current_genres.iter().map(|g| g.name.clone()).collect();
    current_genre_names.sort();
    fields.push(build_field_preview(
        "genres",
        Some(serde_json::json!(current_genre_names)),
        if plugin_metadata.genres.is_empty() {
            None
        } else {
            let mut proposed_genres = plugin_metadata.genres.clone();
            proposed_genres.sort();
            Some(serde_json::json!(proposed_genres))
        },
        current_metadata
            .as_ref()
            .map(|m| m.genres_lock)
            .unwrap_or(false),
        has_permission(PluginPermission::MetadataWriteGenres),
        &mut will_apply,
        &mut locked,
        &mut no_permission,
        &mut unchanged,
        &mut not_provided,
    ));

    // Tags
    let mut current_tag_names: Vec<String> = current_tags.iter().map(|t| t.name.clone()).collect();
    current_tag_names.sort();
    fields.push(build_field_preview(
        "tags",
        Some(serde_json::json!(current_tag_names)),
        if plugin_metadata.tags.is_empty() {
            None
        } else {
            let mut proposed_tags = plugin_metadata.tags.clone();
            proposed_tags.sort();
            Some(serde_json::json!(proposed_tags))
        },
        current_metadata
            .as_ref()
            .map(|m| m.tags_lock)
            .unwrap_or(false),
        has_permission(PluginPermission::MetadataWriteTags),
        &mut will_apply,
        &mut locked,
        &mut no_permission,
        &mut unchanged,
        &mut not_provided,
    ));

    // Age Rating
    fields.push(build_field_preview(
        "ageRating",
        current_metadata
            .as_ref()
            .and_then(|m| m.age_rating.map(|v| serde_json::json!(v))),
        plugin_metadata.age_rating.map(|v| serde_json::json!(v)),
        current_metadata
            .as_ref()
            .map(|m| m.age_rating_lock)
            .unwrap_or(false),
        has_permission(PluginPermission::MetadataWriteAgeRating),
        &mut will_apply,
        &mut locked,
        &mut no_permission,
        &mut unchanged,
        &mut not_provided,
    ));

    // Language
    fields.push(build_field_preview(
        "language",
        current_metadata
            .as_ref()
            .and_then(|m| m.language.as_ref().map(|v| serde_json::json!(v))),
        plugin_metadata
            .language
            .as_ref()
            .map(|v| serde_json::json!(v)),
        current_metadata
            .as_ref()
            .map(|m| m.language_lock)
            .unwrap_or(false),
        has_permission(PluginPermission::MetadataWriteLanguage),
        &mut will_apply,
        &mut locked,
        &mut no_permission,
        &mut unchanged,
        &mut not_provided,
    ));

    // Reading Direction
    fields.push(build_field_preview(
        "readingDirection",
        current_metadata
            .as_ref()
            .and_then(|m| m.reading_direction.as_ref().map(|v| serde_json::json!(v))),
        plugin_metadata
            .reading_direction
            .as_ref()
            .map(|v| serde_json::json!(v)),
        current_metadata
            .as_ref()
            .map(|m| m.reading_direction_lock)
            .unwrap_or(false),
        has_permission(PluginPermission::MetadataWriteReadingDirection),
        &mut will_apply,
        &mut locked,
        &mut no_permission,
        &mut unchanged,
        &mut not_provided,
    ));

    // Total Book Count
    fields.push(build_field_preview(
        "totalBookCount",
        current_metadata
            .as_ref()
            .and_then(|m| m.total_book_count.map(|v| serde_json::json!(v))),
        plugin_metadata
            .total_book_count
            .map(|v| serde_json::json!(v)),
        current_metadata
            .as_ref()
            .map(|m| m.total_book_count_lock)
            .unwrap_or(false),
        has_permission(PluginPermission::MetadataWriteTotalBookCount),
        &mut will_apply,
        &mut locked,
        &mut no_permission,
        &mut unchanged,
        &mut not_provided,
    ));

    // External Links (preview only - shows what links would be added/updated)
    // Get current external links
    let current_links = ExternalLinkRepository::get_for_series(&state.db, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to get external links: {}", e)))?;
    // Build set of normalized source names the plugin provides, to filter current links
    let proposed_link_sources: HashSet<String> = plugin_metadata
        .external_links
        .iter()
        .map(|l| l.label.to_lowercase().trim().to_string())
        .collect();
    // Filter current links to only include sources the plugin provides
    let mut current_link_values: Vec<serde_json::Value> = current_links
        .iter()
        .filter(|l| proposed_link_sources.contains(&l.source_name))
        .map(|l| serde_json::json!({"label": l.source_name.clone(), "url": l.url.clone()}))
        .collect();
    current_link_values.sort_by(|a, b| {
        let a_label = a["label"].as_str().unwrap_or("");
        let b_label = b["label"].as_str().unwrap_or("");
        a_label.cmp(b_label)
    });
    fields.push(build_field_preview(
        "externalLinks",
        if current_link_values.is_empty() {
            None
        } else {
            Some(serde_json::json!(current_link_values))
        },
        if plugin_metadata.external_links.is_empty() {
            None
        } else {
            let mut proposed_links: Vec<serde_json::Value> = plugin_metadata
                .external_links
                .iter()
                .map(|l| {
                    // Normalize label to lowercase to match DB storage
                    serde_json::json!({"label": l.label.to_lowercase().trim(), "url": l.url.trim()})
                })
                .collect();
            proposed_links.sort_by(|a, b| {
                let a_label = a["label"].as_str().unwrap_or("");
                let b_label = b["label"].as_str().unwrap_or("");
                a_label.cmp(b_label)
            });
            Some(serde_json::json!(proposed_links))
        },
        false, // Links don't have a lock field
        has_permission(PluginPermission::MetadataWriteLinks),
        &mut will_apply,
        &mut locked,
        &mut no_permission,
        &mut unchanged,
        &mut not_provided,
    ));

    // External Rating (primary rating - preview only)
    let current_ratings = ExternalRatingRepository::get_for_series(&state.db, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to get external ratings: {}", e)))?;
    let current_rating_info: Option<serde_json::Value> = current_ratings
        .iter()
        .find(|r| r.source_name == plugin.name.to_lowercase())
        .map(|r| {
            let score: f64 = Decimal::to_string(&r.rating).parse().unwrap_or(0.0);
            serde_json::json!({"score": score, "voteCount": r.vote_count, "source": r.source_name})
        });
    fields.push(build_field_preview(
        "rating",
        current_rating_info,
        plugin_metadata.rating.as_ref().map(|r| {
            serde_json::json!({
                "score": r.score,
                "voteCount": r.vote_count,
                "source": r.source
            })
        }),
        false, // Ratings don't have a lock field
        has_permission(PluginPermission::MetadataWriteRatings),
        &mut will_apply,
        &mut locked,
        &mut no_permission,
        &mut unchanged,
        &mut not_provided,
    ));

    // External Ratings array (multiple sources like AniList, MAL, etc.)
    if !plugin_metadata.external_ratings.is_empty() {
        // Build set of sources the plugin is providing, so we only compare those
        let proposed_sources: HashSet<&str> = plugin_metadata
            .external_ratings
            .iter()
            .map(|r| r.source.as_str())
            .collect();
        // Filter current ratings to only include sources the plugin provides
        let mut current_ext_ratings: Vec<serde_json::Value> = current_ratings
            .iter()
            .filter(|r| proposed_sources.contains(r.source_name.as_str()))
            .map(|r| {
                let score: f64 = Decimal::to_string(&r.rating).parse().unwrap_or(0.0);
                serde_json::json!({"score": score, "voteCount": r.vote_count, "source": r.source_name})
            })
            .collect();
        current_ext_ratings.sort_by(|a, b| {
            let a_src = a["source"].as_str().unwrap_or("");
            let b_src = b["source"].as_str().unwrap_or("");
            a_src.cmp(b_src)
        });
        let mut proposed_ext_ratings: Vec<serde_json::Value> = plugin_metadata
            .external_ratings
            .iter()
            .map(|r| {
                serde_json::json!({
                    "score": r.score,
                    "voteCount": r.vote_count,
                    "source": r.source
                })
            })
            .collect();
        proposed_ext_ratings.sort_by(|a, b| {
            let a_src = a["source"].as_str().unwrap_or("");
            let b_src = b["source"].as_str().unwrap_or("");
            a_src.cmp(b_src)
        });
        fields.push(build_field_preview(
            "externalRatings",
            if current_ext_ratings.is_empty() {
                None
            } else {
                Some(serde_json::json!(current_ext_ratings))
            },
            Some(serde_json::json!(proposed_ext_ratings)),
            false, // Ratings don't have a lock field
            has_permission(PluginPermission::MetadataWriteRatings),
            &mut will_apply,
            &mut locked,
            &mut no_permission,
            &mut unchanged,
            &mut not_provided,
        ));
    }

    // Cover URL (preview only - shows if a cover would be downloaded)
    fields.push(build_field_preview(
        "coverUrl",
        None, // We don't show the current cover URL in preview
        plugin_metadata
            .cover_url
            .as_ref()
            .map(|v| serde_json::json!(v)),
        current_metadata
            .as_ref()
            .map(|m| m.cover_lock)
            .unwrap_or(false),
        has_permission(PluginPermission::MetadataWriteCovers),
        &mut will_apply,
        &mut locked,
        &mut no_permission,
        &mut unchanged,
        &mut not_provided,
    ));

    // External IDs (cross-reference IDs from other services like AniList, MAL)
    if !plugin_metadata.external_ids.is_empty() {
        let current_ext_ids = SeriesExternalIdRepository::get_for_series(&state.db, series_id)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to get external IDs: {}", e)))?;
        // Filter current external IDs to only include sources the plugin provides
        let proposed_ext_id_sources: HashSet<&str> = plugin_metadata
            .external_ids
            .iter()
            .map(|e| e.source.as_str())
            .collect();
        let mut current_ext_id_sources: Vec<serde_json::Value> = current_ext_ids
            .iter()
            .filter(|e| proposed_ext_id_sources.contains(e.source.as_str()))
            .map(|e| {
                serde_json::json!({
                    "source": e.source,
                    "externalId": e.external_id
                })
            })
            .collect();
        current_ext_id_sources.sort_by(|a, b| {
            let a_src = a["source"].as_str().unwrap_or("");
            let b_src = b["source"].as_str().unwrap_or("");
            a_src.cmp(b_src)
        });
        let mut proposed_ext_ids: Vec<serde_json::Value> = plugin_metadata
            .external_ids
            .iter()
            .map(|e| {
                serde_json::json!({
                    "source": e.source,
                    "externalId": e.external_id
                })
            })
            .collect();
        proposed_ext_ids.sort_by(|a, b| {
            let a_src = a["source"].as_str().unwrap_or("");
            let b_src = b["source"].as_str().unwrap_or("");
            a_src.cmp(b_src)
        });
        fields.push(build_field_preview(
            "externalIds",
            if current_ext_id_sources.is_empty() {
                None
            } else {
                Some(serde_json::json!(current_ext_id_sources))
            },
            Some(serde_json::json!(proposed_ext_ids)),
            false, // External IDs don't have a lock field
            has_permission(PluginPermission::MetadataWriteExternalIds),
            &mut will_apply,
            &mut locked,
            &mut no_permission,
            &mut unchanged,
            &mut not_provided,
        ));
    }

    Ok(Json(MetadataPreviewResponse {
        fields,
        summary: PreviewSummary {
            will_apply,
            locked,
            no_permission,
            unchanged,
            not_provided,
        },
        plugin_id: plugin.id,
        plugin_name: plugin.display_name,
        external_id: request.external_id,
        external_url: Some(plugin_metadata.external_url),
    }))
}

/// Apply metadata from a plugin to a series
///
/// Fetches metadata from a plugin and applies it to the series, respecting
/// RBAC permissions and field locks.
#[utoipa::path(
    post,
    path = "/api/v1/series/{id}/metadata/apply",
    params(
        ("id" = Uuid, Path, description = "Series ID")
    ),
    request_body = MetadataApplyRequest,
    responses(
        (status = 200, description = "Metadata applied", body = MetadataApplyResponse),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "No permission to edit series"),
        (status = 404, description = "Series or plugin not found"),
    ),
    security(
        ("bearer_auth" = []),
        ("api_key" = [])
    ),
    tag = "Plugin Actions"
)]
pub async fn apply_series_metadata(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    Path(series_id): Path<Uuid>,
    Json(request): Json<MetadataApplyRequest>,
) -> Result<Json<MetadataApplyResponse>, ApiError> {
    // Check permission to edit series metadata
    auth.require_permission(&Permission::SeriesWrite)?;

    // Get the series (verify it exists and get library_id for events)
    let series = SeriesRepository::get_by_id(&state.db, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to get series: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Series not found".to_string()))?;

    // Get the plugin
    let plugin = PluginsRepository::get_by_id(&state.db, request.plugin_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to get plugin: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Plugin not found".to_string()))?;

    if !plugin.enabled {
        return Err(ApiError::BadRequest("Plugin is disabled".to_string()));
    }

    // Check if plugin applies to this series' library
    if !plugin.applies_to_library(series.library_id) {
        return Err(ApiError::BadRequest(format!(
            "Plugin '{}' is not configured to apply to this series' library",
            plugin.display_name
        )));
    }

    // Fetch metadata from plugin
    let params = MetadataGetParams {
        external_id: request.external_id.clone(),
    };

    let plugin_metadata = state
        .plugin_manager
        .get_series_metadata(request.plugin_id, params)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch metadata from plugin: {}", e)))?;

    // Get current series metadata
    let current_metadata = SeriesMetadataRepository::get_by_series_id(&state.db, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to get current metadata: {}", e)))?;

    // Build apply options
    let options = ApplyOptions {
        fields_filter: request.fields.map(|f| f.into_iter().collect()),
        thumbnail_service: Some(state.thumbnail_service.clone()),
        event_broadcaster: Some(state.event_broadcaster.clone()),
    };

    // Apply metadata using the shared service
    let result = MetadataApplier::apply(
        &state.db,
        series_id,
        series.library_id,
        &plugin,
        &plugin_metadata,
        current_metadata.as_ref(),
        &options,
    )
    .await
    .map_err(|e| ApiError::Internal(format!("Failed to apply metadata: {}", e)))?;

    // Store/update external ID for future lookups (matches auto-match behavior)
    if let Err(e) = SeriesExternalIdRepository::upsert_for_plugin(
        &state.db,
        series_id,
        &plugin.name,
        &plugin_metadata.external_id,
        Some(&plugin_metadata.external_url),
        None, // metadata_hash - could be computed if needed
    )
    .await
    {
        tracing::warn!(
            "Failed to store external ID for series {}: {}",
            series_id,
            e
        );
        // Don't fail the request - metadata was still applied successfully
    }

    // Convert SkippedField types
    let skipped_fields: Vec<SkippedField> = result
        .skipped_fields
        .into_iter()
        .map(|sf| SkippedField {
            field: sf.field,
            reason: sf.reason,
        })
        .collect();

    let message = if result.applied_fields.is_empty() {
        "No fields were applied".to_string()
    } else {
        format!("Applied {} field(s)", result.applied_fields.len())
    };

    Ok(Json(MetadataApplyResponse {
        success: !result.applied_fields.is_empty(),
        applied_fields: result.applied_fields,
        skipped_fields,
        message,
    }))
}

/// Auto-match and apply metadata from a plugin to a series
///
/// Searches for the series using the plugin's metadata search, picks the best match,
/// and applies the metadata in one step. This is a convenience endpoint for quick
/// metadata updates without user intervention.
#[utoipa::path(
    post,
    path = "/api/v1/series/{id}/metadata/auto-match",
    params(
        ("id" = Uuid, Path, description = "Series ID")
    ),
    request_body = MetadataAutoMatchRequest,
    responses(
        (status = 200, description = "Auto-match completed", body = MetadataAutoMatchResponse),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "No permission to edit series"),
        (status = 404, description = "Series or plugin not found or no match found"),
    ),
    security(
        ("bearer_auth" = []),
        ("api_key" = [])
    ),
    tag = "Plugin Actions"
)]
pub async fn auto_match_series_metadata(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    Path(series_id): Path<Uuid>,
    Json(request): Json<MetadataAutoMatchRequest>,
) -> Result<Json<MetadataAutoMatchResponse>, ApiError> {
    // Check permission to edit series metadata
    auth.require_permission(&Permission::SeriesWrite)?;

    // Get the series (verify it exists and get its title)
    let series = SeriesRepository::get_by_id(&state.db, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to get series: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Series not found".to_string()))?;

    // Get the current series metadata for title
    let series_metadata = SeriesMetadataRepository::get_by_series_id(&state.db, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to get series metadata: {}", e)))?;

    // Use the provided query or fall back to series title
    let search_query = request.query.unwrap_or_else(|| {
        series_metadata
            .map(|m| m.title)
            .unwrap_or_else(|| series.name.clone())
    });

    // Get the plugin
    let plugin = PluginsRepository::get_by_id(&state.db, request.plugin_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to get plugin: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Plugin not found".to_string()))?;

    if !plugin.enabled {
        return Err(ApiError::BadRequest("Plugin is disabled".to_string()));
    }

    // Check if plugin applies to this series' library
    if !plugin.applies_to_library(series.library_id) {
        return Err(ApiError::BadRequest(format!(
            "Plugin '{}' is not configured to apply to this series' library",
            plugin.display_name
        )));
    }

    // Search for metadata using the plugin
    let auto_match_limit = plugin
        .internal_config_parsed()
        .search_results_limit
        .unwrap_or(10);
    let search_params = MetadataSearchParams {
        query: search_query.clone(),
        limit: Some(auto_match_limit),
        cursor: None,
    };

    let search_response = state
        .plugin_manager
        .search_series(request.plugin_id, search_params)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to search for metadata: {}", e)))?;

    // Check if we got any results
    if search_response.results.is_empty() {
        return Ok(Json(MetadataAutoMatchResponse {
            success: false,
            matched_result: None,
            applied_fields: vec![],
            skipped_fields: vec![],
            message: format!("No matches found for '{}'", search_query),
            external_url: None,
        }));
    }

    // Pick the best result - use relevance_score if available, otherwise take first result
    // (APIs typically return results in relevance order already)
    let best_match = search_response
        .results
        .into_iter()
        .enumerate()
        .max_by(|(i, a), (j, b)| {
            match (a.relevance_score, b.relevance_score) {
                (Some(a_score), Some(b_score)) => a_score
                    .partial_cmp(&b_score)
                    .unwrap_or(std::cmp::Ordering::Equal),
                // If no scores, prefer earlier results (lower index = higher relevance)
                _ => j.cmp(i),
            }
        })
        .map(|(_, result)| result)
        .unwrap(); // Safe: we checked results is non-empty

    let external_id = best_match.external_id.clone();
    let matched_result_dto = PluginSearchResultDto::from(best_match);

    // Fetch full metadata for the best match
    let params = MetadataGetParams {
        external_id: external_id.clone(),
    };

    let plugin_metadata = state
        .plugin_manager
        .get_series_metadata(request.plugin_id, params)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch metadata from plugin: {}", e)))?;

    let external_url = plugin_metadata.external_url.clone();

    // Get current series metadata
    let current_metadata = SeriesMetadataRepository::get_by_series_id(&state.db, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to get current metadata: {}", e)))?;

    // Build apply options (no field filtering for auto-match)
    let options = ApplyOptions {
        fields_filter: None, // Apply all fields
        thumbnail_service: Some(state.thumbnail_service.clone()),
        event_broadcaster: Some(state.event_broadcaster.clone()),
    };

    // Apply metadata using the shared service
    let result = MetadataApplier::apply(
        &state.db,
        series_id,
        series.library_id,
        &plugin,
        &plugin_metadata,
        current_metadata.as_ref(),
        &options,
    )
    .await
    .map_err(|e| ApiError::Internal(format!("Failed to apply metadata: {}", e)))?;

    // Convert SkippedField types
    let skipped_fields: Vec<SkippedField> = result
        .skipped_fields
        .into_iter()
        .map(|sf| SkippedField {
            field: sf.field,
            reason: sf.reason,
        })
        .collect();

    let message = if result.applied_fields.is_empty() {
        format!(
            "Matched '{}' but no fields were applied",
            matched_result_dto.title
        )
    } else {
        format!(
            "Matched '{}' and applied {} field(s)",
            matched_result_dto.title,
            result.applied_fields.len()
        )
    };

    Ok(Json(MetadataAutoMatchResponse {
        success: !result.applied_fields.is_empty(),
        matched_result: Some(matched_result_dto),
        applied_fields: result.applied_fields,
        skipped_fields,
        message,
        external_url: Some(external_url),
    }))
}

// =============================================================================
// Book Metadata Preview and Apply Endpoints
// =============================================================================

/// Preview metadata from a plugin for a book
///
/// Fetches metadata from a plugin and computes a field-by-field diff with the current
/// book metadata, showing which fields will be applied, locked, or denied by RBAC.
#[utoipa::path(
    post,
    path = "/api/v1/books/{id}/metadata/preview",
    params(
        ("id" = Uuid, Path, description = "Book ID")
    ),
    request_body = MetadataPreviewRequest,
    responses(
        (status = 200, description = "Preview computed", body = MetadataPreviewResponse),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "No permission to edit books"),
        (status = 404, description = "Book or plugin not found"),
    ),
    security(
        ("bearer_auth" = []),
        ("api_key" = [])
    ),
    tag = "Plugin Actions"
)]
pub async fn preview_book_metadata(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    Path(book_id): Path<Uuid>,
    Json(request): Json<MetadataPreviewRequest>,
) -> Result<Json<MetadataPreviewResponse>, ApiError> {
    // Check permission to edit book metadata
    auth.require_permission(&Permission::BooksWrite)?;

    // Get the book (verify it exists and get library_id)
    let book = BookRepository::get_by_id(&state.db, book_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to get book: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Book not found".to_string()))?;

    // Get the plugin
    let plugin = PluginsRepository::get_by_id(&state.db, request.plugin_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to get plugin: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Plugin not found".to_string()))?;

    if !plugin.enabled {
        return Err(ApiError::BadRequest("Plugin is disabled".to_string()));
    }

    // Check if plugin applies to this book's library
    if !plugin.applies_to_library(book.library_id) {
        return Err(ApiError::BadRequest(format!(
            "Plugin '{}' is not configured to apply to this book's library",
            plugin.display_name
        )));
    }

    // Fetch metadata from plugin
    let params = MetadataGetParams {
        external_id: request.external_id.clone(),
    };

    let plugin_metadata = state
        .plugin_manager
        .get_book_metadata(request.plugin_id, params)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch metadata from plugin: {}", e)))?;

    // Get current book metadata
    let current_metadata = BookMetadataRepository::get_by_book_id(&state.db, book_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to get current metadata: {}", e)))?;

    // Helper to check permission
    let has_permission = |perm: PluginPermission| -> bool { plugin.has_permission(&perm) };

    // Build field-by-field preview
    let mut fields = Vec::new();
    let mut will_apply = 0;
    let mut locked = 0;
    let mut no_permission = 0;
    let mut unchanged = 0;
    let mut not_provided = 0;

    // Title
    fields.push(build_field_preview(
        "title",
        current_metadata
            .as_ref()
            .and_then(|m| m.title.as_ref().map(|v| serde_json::json!(v))),
        plugin_metadata.title.as_ref().map(|v| serde_json::json!(v)),
        current_metadata
            .as_ref()
            .map(|m| m.title_lock)
            .unwrap_or(false),
        has_permission(PluginPermission::MetadataWriteTitle),
        &mut will_apply,
        &mut locked,
        &mut no_permission,
        &mut unchanged,
        &mut not_provided,
    ));

    // Summary
    fields.push(build_field_preview(
        "summary",
        current_metadata
            .as_ref()
            .and_then(|m| m.summary.as_ref().map(|v| serde_json::json!(v))),
        plugin_metadata
            .summary
            .as_ref()
            .map(|v| serde_json::json!(v)),
        current_metadata
            .as_ref()
            .map(|m| m.summary_lock)
            .unwrap_or(false),
        has_permission(PluginPermission::MetadataWriteSummary),
        &mut will_apply,
        &mut locked,
        &mut no_permission,
        &mut unchanged,
        &mut not_provided,
    ));

    // Book Type
    fields.push(build_field_preview(
        "bookType",
        current_metadata
            .as_ref()
            .and_then(|m| m.book_type.as_ref().map(|v| serde_json::json!(v))),
        plugin_metadata
            .book_type
            .as_ref()
            .map(|v| serde_json::json!(v)),
        current_metadata
            .as_ref()
            .map(|m| m.book_type_lock)
            .unwrap_or(false),
        has_permission(PluginPermission::MetadataWriteBookType),
        &mut will_apply,
        &mut locked,
        &mut no_permission,
        &mut unchanged,
        &mut not_provided,
    ));

    // Subtitle
    fields.push(build_field_preview(
        "subtitle",
        current_metadata
            .as_ref()
            .and_then(|m| m.subtitle.as_ref().map(|v| serde_json::json!(v))),
        plugin_metadata
            .subtitle
            .as_ref()
            .map(|v| serde_json::json!(v)),
        current_metadata
            .as_ref()
            .map(|m| m.subtitle_lock)
            .unwrap_or(false),
        has_permission(PluginPermission::MetadataWriteSubtitle),
        &mut will_apply,
        &mut locked,
        &mut no_permission,
        &mut unchanged,
        &mut not_provided,
    ));

    // Publisher
    fields.push(build_field_preview(
        "publisher",
        current_metadata
            .as_ref()
            .and_then(|m| m.publisher.as_ref().map(|v| serde_json::json!(v))),
        plugin_metadata
            .publisher
            .as_ref()
            .map(|v| serde_json::json!(v)),
        current_metadata
            .as_ref()
            .map(|m| m.publisher_lock)
            .unwrap_or(false),
        has_permission(PluginPermission::MetadataWritePublisher),
        &mut will_apply,
        &mut locked,
        &mut no_permission,
        &mut unchanged,
        &mut not_provided,
    ));

    // Year
    fields.push(build_field_preview(
        "year",
        current_metadata
            .as_ref()
            .and_then(|m| m.year.map(|v| serde_json::json!(v))),
        plugin_metadata.year.map(|v| serde_json::json!(v)),
        current_metadata
            .as_ref()
            .map(|m| m.year_lock)
            .unwrap_or(false),
        has_permission(PluginPermission::MetadataWriteYear),
        &mut will_apply,
        &mut locked,
        &mut no_permission,
        &mut unchanged,
        &mut not_provided,
    ));

    // Authors
    fields.push(build_field_preview(
        "authors",
        current_metadata.as_ref().and_then(|m| {
            m.authors_json
                .as_ref()
                .and_then(|v| serde_json::from_str(v).ok())
        }),
        if plugin_metadata.authors.is_empty() {
            None
        } else {
            Some(serde_json::json!(plugin_metadata.authors))
        },
        current_metadata
            .as_ref()
            .map(|m| m.authors_json_lock)
            .unwrap_or(false),
        has_permission(PluginPermission::MetadataWriteAuthors),
        &mut will_apply,
        &mut locked,
        &mut no_permission,
        &mut unchanged,
        &mut not_provided,
    ));

    // Translator
    fields.push(build_field_preview(
        "translator",
        current_metadata
            .as_ref()
            .and_then(|m| m.translator.as_ref().map(|v| serde_json::json!(v))),
        plugin_metadata
            .translator
            .as_ref()
            .map(|v| serde_json::json!(v)),
        current_metadata
            .as_ref()
            .map(|m| m.translator_lock)
            .unwrap_or(false),
        has_permission(PluginPermission::MetadataWriteTranslator),
        &mut will_apply,
        &mut locked,
        &mut no_permission,
        &mut unchanged,
        &mut not_provided,
    ));

    // Edition
    fields.push(build_field_preview(
        "edition",
        current_metadata
            .as_ref()
            .and_then(|m| m.edition.as_ref().map(|v| serde_json::json!(v))),
        plugin_metadata
            .edition
            .as_ref()
            .map(|v| serde_json::json!(v)),
        current_metadata
            .as_ref()
            .map(|m| m.edition_lock)
            .unwrap_or(false),
        has_permission(PluginPermission::MetadataWriteEdition),
        &mut will_apply,
        &mut locked,
        &mut no_permission,
        &mut unchanged,
        &mut not_provided,
    ));

    // Original Title
    fields.push(build_field_preview(
        "originalTitle",
        current_metadata
            .as_ref()
            .and_then(|m| m.original_title.as_ref().map(|v| serde_json::json!(v))),
        plugin_metadata
            .original_title
            .as_ref()
            .map(|v| serde_json::json!(v)),
        current_metadata
            .as_ref()
            .map(|m| m.original_title_lock)
            .unwrap_or(false),
        has_permission(PluginPermission::MetadataWriteOriginalTitle),
        &mut will_apply,
        &mut locked,
        &mut no_permission,
        &mut unchanged,
        &mut not_provided,
    ));

    // Original Year
    fields.push(build_field_preview(
        "originalYear",
        current_metadata
            .as_ref()
            .and_then(|m| m.original_year.map(|v| serde_json::json!(v))),
        plugin_metadata.original_year.map(|v| serde_json::json!(v)),
        current_metadata
            .as_ref()
            .map(|m| m.original_year_lock)
            .unwrap_or(false),
        has_permission(PluginPermission::MetadataWriteOriginalYear),
        &mut will_apply,
        &mut locked,
        &mut no_permission,
        &mut unchanged,
        &mut not_provided,
    ));

    // Language
    fields.push(build_field_preview(
        "language",
        current_metadata
            .as_ref()
            .and_then(|m| m.language_iso.as_ref().map(|v| serde_json::json!(v))),
        plugin_metadata
            .language
            .as_ref()
            .map(|v| serde_json::json!(v)),
        current_metadata
            .as_ref()
            .map(|m| m.language_iso_lock)
            .unwrap_or(false),
        has_permission(PluginPermission::MetadataWriteLanguage),
        &mut will_apply,
        &mut locked,
        &mut no_permission,
        &mut unchanged,
        &mut not_provided,
    ));

    // ISBNs - normalize current value from comma-separated string to array for comparison
    let current_isbns: Option<serde_json::Value> = current_metadata.as_ref().and_then(|m| {
        m.isbns.as_ref().map(|v| {
            let isbn_vec: Vec<&str> = v
                .split(',')
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .collect();
            serde_json::json!(isbn_vec)
        })
    });
    fields.push(build_field_preview(
        "isbns",
        current_isbns,
        if plugin_metadata.isbns.is_empty() {
            None
        } else {
            Some(serde_json::json!(plugin_metadata.isbns))
        },
        current_metadata
            .as_ref()
            .map(|m| m.isbns_lock)
            .unwrap_or(false),
        has_permission(PluginPermission::MetadataWriteIsbn),
        &mut will_apply,
        &mut locked,
        &mut no_permission,
        &mut unchanged,
        &mut not_provided,
    ));

    // Series Position
    fields.push(build_field_preview(
        "seriesPosition",
        current_metadata
            .as_ref()
            .and_then(|m| m.series_position.map(|v| serde_json::json!(v.to_string()))),
        plugin_metadata
            .series_position
            .map(|v| serde_json::json!(v)),
        current_metadata
            .as_ref()
            .map(|m| m.series_position_lock)
            .unwrap_or(false),
        has_permission(PluginPermission::MetadataWriteSeriesPosition),
        &mut will_apply,
        &mut locked,
        &mut no_permission,
        &mut unchanged,
        &mut not_provided,
    ));

    // Series Total
    fields.push(build_field_preview(
        "seriesTotal",
        current_metadata
            .as_ref()
            .and_then(|m| m.series_total.map(|v| serde_json::json!(v))),
        plugin_metadata.series_total.map(|v| serde_json::json!(v)),
        current_metadata
            .as_ref()
            .map(|m| m.series_total_lock)
            .unwrap_or(false),
        has_permission(PluginPermission::MetadataWriteSeriesPosition),
        &mut will_apply,
        &mut locked,
        &mut no_permission,
        &mut unchanged,
        &mut not_provided,
    ));

    // Subjects
    fields.push(build_field_preview(
        "subjects",
        current_metadata.as_ref().and_then(|m| {
            m.subjects
                .as_ref()
                .and_then(|v| serde_json::from_str(v).ok())
        }),
        if plugin_metadata.subjects.is_empty() {
            None
        } else {
            Some(serde_json::json!(plugin_metadata.subjects))
        },
        current_metadata
            .as_ref()
            .map(|m| m.subjects_lock)
            .unwrap_or(false),
        has_permission(PluginPermission::MetadataWriteSubjects),
        &mut will_apply,
        &mut locked,
        &mut no_permission,
        &mut unchanged,
        &mut not_provided,
    ));

    // Awards
    fields.push(build_field_preview(
        "awards",
        current_metadata.as_ref().and_then(|m| {
            m.awards_json
                .as_ref()
                .and_then(|v| serde_json::from_str(v).ok())
        }),
        if plugin_metadata.awards.is_empty() {
            None
        } else {
            Some(serde_json::json!(plugin_metadata.awards))
        },
        current_metadata
            .as_ref()
            .map(|m| m.awards_json_lock)
            .unwrap_or(false),
        has_permission(PluginPermission::MetadataWriteAwards),
        &mut will_apply,
        &mut locked,
        &mut no_permission,
        &mut unchanged,
        &mut not_provided,
    ));

    // Cover URL
    fields.push(build_field_preview(
        "coverUrl",
        None,
        plugin_metadata
            .cover_url
            .as_ref()
            .map(|v| serde_json::json!(v)),
        current_metadata
            .as_ref()
            .map(|m| m.cover_lock)
            .unwrap_or(false),
        has_permission(PluginPermission::MetadataWriteCovers),
        &mut will_apply,
        &mut locked,
        &mut no_permission,
        &mut unchanged,
        &mut not_provided,
    ));

    Ok(Json(MetadataPreviewResponse {
        fields,
        summary: PreviewSummary {
            will_apply,
            locked,
            no_permission,
            unchanged,
            not_provided,
        },
        plugin_id: plugin.id,
        plugin_name: plugin.display_name,
        external_id: request.external_id,
        external_url: Some(plugin_metadata.external_url),
    }))
}

/// Apply metadata from a plugin to a book
///
/// Fetches metadata from a plugin and applies it to the book, respecting
/// RBAC permissions and field locks.
#[utoipa::path(
    post,
    path = "/api/v1/books/{id}/metadata/apply",
    params(
        ("id" = Uuid, Path, description = "Book ID")
    ),
    request_body = MetadataApplyRequest,
    responses(
        (status = 200, description = "Metadata applied", body = MetadataApplyResponse),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "No permission to edit books"),
        (status = 404, description = "Book or plugin not found"),
    ),
    security(
        ("bearer_auth" = []),
        ("api_key" = [])
    ),
    tag = "Plugin Actions"
)]
pub async fn apply_book_metadata(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    Path(book_id): Path<Uuid>,
    Json(request): Json<MetadataApplyRequest>,
) -> Result<Json<MetadataApplyResponse>, ApiError> {
    // Check permission to edit book metadata
    auth.require_permission(&Permission::BooksWrite)?;

    // Get the book (verify it exists)
    let book = BookRepository::get_by_id(&state.db, book_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to get book: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Book not found".to_string()))?;

    // Get the plugin
    let plugin = PluginsRepository::get_by_id(&state.db, request.plugin_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to get plugin: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Plugin not found".to_string()))?;

    if !plugin.enabled {
        return Err(ApiError::BadRequest("Plugin is disabled".to_string()));
    }

    // Check if plugin applies to this book's library
    if !plugin.applies_to_library(book.library_id) {
        return Err(ApiError::BadRequest(format!(
            "Plugin '{}' is not configured to apply to this book's library",
            plugin.display_name
        )));
    }

    // Fetch metadata from plugin
    let params = MetadataGetParams {
        external_id: request.external_id.clone(),
    };

    let plugin_metadata = state
        .plugin_manager
        .get_book_metadata(request.plugin_id, params)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch metadata from plugin: {}", e)))?;

    // Get current book metadata
    let current_metadata = BookMetadataRepository::get_by_book_id(&state.db, book_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to get current metadata: {}", e)))?;

    // Build apply options
    let options = BookApplyOptions {
        fields_filter: request.fields.map(|f| f.into_iter().collect()),
        thumbnail_service: Some(state.thumbnail_service.clone()),
        event_broadcaster: Some(state.event_broadcaster.clone()),
        library_id: Some(book.library_id),
    };

    // Apply metadata using the book metadata applier
    let result = BookMetadataApplier::apply(
        &state.db,
        book_id,
        &plugin,
        &plugin_metadata,
        current_metadata.as_ref(),
        &options,
    )
    .await
    .map_err(|e| ApiError::Internal(format!("Failed to apply metadata: {}", e)))?;

    // Store/update external ID for future lookups
    if let Err(e) = BookExternalIdRepository::upsert_for_plugin(
        &state.db,
        book_id,
        &plugin.name,
        &plugin_metadata.external_id,
        Some(&plugin_metadata.external_url),
        None, // metadata_hash
    )
    .await
    {
        tracing::warn!("Failed to store external ID for book {}: {}", book_id, e);
    }

    // Convert SkippedField types
    let skipped_fields: Vec<SkippedField> = result
        .skipped_fields
        .into_iter()
        .map(|sf| SkippedField {
            field: sf.field,
            reason: sf.reason,
        })
        .collect();

    let message = if result.applied_fields.is_empty() {
        "No fields were applied".to_string()
    } else {
        format!("Applied {} field(s)", result.applied_fields.len())
    };

    Ok(Json(MetadataApplyResponse {
        success: !result.applied_fields.is_empty(),
        applied_fields: result.applied_fields,
        skipped_fields,
        message,
    }))
}

// =============================================================================
// Helper Functions
// =============================================================================

/// Build a field preview entry
#[allow(clippy::too_many_arguments)]
fn build_field_preview(
    field: &str,
    current_value: Option<serde_json::Value>,
    proposed_value: Option<serde_json::Value>,
    is_locked: bool,
    has_permission: bool,
    will_apply: &mut usize,
    locked: &mut usize,
    no_permission: &mut usize,
    unchanged: &mut usize,
    not_provided: &mut usize,
) -> MetadataFieldPreview {
    let (status, reason) = if proposed_value.is_none() {
        *not_provided += 1;
        (
            FieldApplyStatus::NotProvided,
            Some("Not provided by plugin".to_string()),
        )
    } else if is_locked {
        *locked += 1;
        (
            FieldApplyStatus::Locked,
            Some("Field is locked".to_string()),
        )
    } else if !has_permission {
        *no_permission += 1;
        (
            FieldApplyStatus::NoPermission,
            Some("Plugin lacks permission".to_string()),
        )
    } else if current_value == proposed_value {
        *unchanged += 1;
        (
            FieldApplyStatus::Unchanged,
            Some("Value unchanged".to_string()),
        )
    } else {
        *will_apply += 1;
        (FieldApplyStatus::WillApply, None)
    };

    MetadataFieldPreview {
        field: field.to_string(),
        current_value,
        proposed_value,
        status,
        reason,
    }
}

// =============================================================================
// Permission Helpers for Plugin Visibility
// =============================================================================

/// Map a metadata content type to the required permission.
///
/// This determines what permission a user needs to use a plugin for a given content type:
/// - Series metadata plugins require `SeriesWrite`
/// - Book metadata plugins require `BooksWrite`
/// - Library metadata plugins require `LibrariesWrite` (future)
fn permission_for_content_type(content_type: &MetadataContentType) -> Permission {
    match content_type {
        MetadataContentType::Series => Permission::SeriesWrite,
        MetadataContentType::Book => Permission::BooksWrite,
    }
}

/// Check if a user has permission to use a plugin based on its metadata capabilities.
///
/// A user can use a plugin if they have the required write permission for at least
/// one of the content types the plugin supports.
fn user_can_use_plugin(
    plugin_capabilities: &[MetadataContentType],
    user_permissions: &HashSet<Permission>,
) -> bool {
    plugin_capabilities
        .iter()
        .map(permission_for_content_type)
        .any(|perm| user_permissions.contains(&perm))
}

/// Sanitize plugin error messages for client responses.
///
/// This prevents exposing internal error details to clients while preserving
/// user-actionable information. The full error is logged server-side.
fn sanitize_plugin_error(error: &PluginManagerError) -> String {
    // Log the full error server-side for debugging
    tracing::warn!(error = %error, "Plugin operation failed");

    match error {
        // User-actionable errors - return sanitized messages
        PluginManagerError::PluginNotFound(id) => format!("Plugin {} not found", id),
        PluginManagerError::PluginNotEnabled(id) => format!("Plugin {} is not enabled", id),
        PluginManagerError::RateLimited { .. } => {
            "Plugin rate limit exceeded, please try again later".to_string()
        }

        // Nested plugin errors - extract and sanitize
        PluginManagerError::Plugin(plugin_error) => sanitize_nested_plugin_error(plugin_error),

        // User plugin errors
        PluginManagerError::UserPluginNotFound { .. } => {
            "User plugin connection not found".to_string()
        }

        // OAuth token errors - user-actionable
        PluginManagerError::ReauthRequired(_) => {
            "OAuth session expired. Please reconnect the plugin.".to_string()
        }
        PluginManagerError::TokenRefreshFailed(_) => {
            "Failed to refresh OAuth token. Please try again or reconnect.".to_string()
        }

        // Internal errors - don't expose details
        PluginManagerError::Database(_) | PluginManagerError::Encryption(_) => {
            "An internal plugin error occurred".to_string()
        }
    }
}

/// Sanitize nested PluginError messages
///
/// Since the nested error types (PluginError, RpcError) are not part of the public API,
/// we pattern match on the error string to provide user-friendly messages.
fn sanitize_nested_plugin_error(error: &crate::services::plugin::handle::PluginError) -> String {
    use crate::services::plugin::handle::PluginError;
    use crate::services::plugin::rpc::RpcError;

    match error {
        PluginError::NotInitialized => "Plugin is not ready, please try again".to_string(),
        PluginError::Disabled { .. } => "Plugin is disabled".to_string(),
        PluginError::SpawnFailed(_) => {
            "Failed to start plugin, please contact an administrator".to_string()
        }

        // RPC errors - these may contain more detail
        PluginError::Rpc(rpc_error) => match rpc_error {
            RpcError::Timeout(_) => "Plugin request timed out, please try again".to_string(),
            RpcError::PluginError { code, message, .. } => {
                // JSON-RPC errors from the plugin are user-visible
                // but we sanitize the data field which may contain internal details
                format!("Plugin error ({}): {}", code, message)
            }
            RpcError::RateLimited { .. } => {
                "Plugin rate limit exceeded, please try again later".to_string()
            }
            RpcError::Cancelled => "Plugin request was cancelled".to_string(),
            // User-friendly errors from plugin - pass through the message
            RpcError::NotFound(msg) => format!("Not found: {}", msg),
            RpcError::AuthFailed(_) => {
                "Plugin authentication failed, please check credentials".to_string()
            }
            RpcError::ApiError(msg) => format!("External API error: {}", msg),
            RpcError::ConfigError(_) => "Plugin configuration error".to_string(),
            // Internal RPC errors - don't expose details
            RpcError::Serialization(_) | RpcError::InvalidResponse(_) | RpcError::Process(_) => {
                "Plugin communication error, please try again".to_string()
            }
        },

        // Process errors - don't expose command details
        PluginError::Process(_) => "Plugin communication error, please try again".to_string(),
    }
}

// =============================================================================
// Task-based Auto-Match Endpoints (Background Processing)
// =============================================================================

/// Maximum number of series that can be enqueued in a single bulk request.
/// This prevents worker queue overload through excessive task creation.
const MAX_BULK_SERIES_COUNT: usize = 100;

/// Maximum number of series that can be enqueued for a library auto-match.
/// Libraries with more series will be rejected with an error suggesting
/// to use the bulk endpoint in batches instead.
const MAX_LIBRARY_SERIES_COUNT: usize = 1000;

/// Enqueue a plugin auto-match task for a single series
///
/// Creates a background task to auto-match metadata for a series using the specified plugin.
/// The task runs asynchronously in a worker process and emits a SeriesMetadataUpdated event
/// when complete.
#[utoipa::path(
    post,
    path = "/api/v1/series/{id}/metadata/auto-match/task",
    params(
        ("id" = Uuid, Path, description = "Series ID")
    ),
    request_body = EnqueueAutoMatchRequest,
    responses(
        (status = 200, description = "Task enqueued", body = EnqueueAutoMatchResponse),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "No permission to edit series"),
        (status = 404, description = "Series or plugin not found"),
    ),
    security(
        ("bearer_auth" = []),
        ("api_key" = [])
    ),
    tag = "Plugin Actions"
)]
pub async fn enqueue_auto_match_task(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    Path(series_id): Path<Uuid>,
    Json(request): Json<EnqueueAutoMatchRequest>,
) -> Result<Json<EnqueueAutoMatchResponse>, ApiError> {
    // Check permission to edit series metadata
    auth.require_permission(&Permission::SeriesWrite)?;

    // Verify series exists
    let series = SeriesRepository::get_by_id(&state.db, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to get series: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Series not found".to_string()))?;

    // Verify plugin exists and is enabled
    let plugin = PluginsRepository::get_by_id(&state.db, request.plugin_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to get plugin: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Plugin not found".to_string()))?;

    if !plugin.enabled {
        return Err(ApiError::BadRequest("Plugin is disabled".to_string()));
    }

    // Check if plugin applies to this series' library
    if !plugin.applies_to_library(series.library_id) {
        return Err(ApiError::BadRequest(format!(
            "Plugin '{}' is not configured to apply to this series' library",
            plugin.display_name
        )));
    }

    // Create the task
    let task_type = TaskType::PluginAutoMatch {
        series_id,
        plugin_id: request.plugin_id,
        source_scope: Some("series:detail".to_string()),
    };

    let task_id = TaskRepository::enqueue(&state.db, task_type, 0, None)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to enqueue task: {}", e)))?;

    Ok(Json(EnqueueAutoMatchResponse {
        success: true,
        tasks_enqueued: 1,
        task_ids: vec![task_id],
        message: "Auto-match task enqueued".to_string(),
    }))
}

/// Enqueue plugin auto-match tasks for multiple series (bulk operation)
///
/// Creates background tasks to auto-match metadata for multiple series using the specified plugin.
/// Each series gets its own task that runs asynchronously in a worker process.
#[utoipa::path(
    post,
    path = "/api/v1/series/metadata/auto-match/task/bulk",
    request_body = EnqueueBulkAutoMatchRequest,
    responses(
        (status = 200, description = "Tasks enqueued", body = EnqueueAutoMatchResponse),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "No permission to edit series"),
        (status = 404, description = "Plugin not found"),
    ),
    security(
        ("bearer_auth" = []),
        ("api_key" = [])
    ),
    tag = "Plugin Actions"
)]
pub async fn enqueue_bulk_auto_match_tasks(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    Json(request): Json<EnqueueBulkAutoMatchRequest>,
) -> Result<Json<EnqueueAutoMatchResponse>, ApiError> {
    // Check permission to edit series metadata
    auth.require_permission(&Permission::SeriesWrite)?;

    if request.series_ids.is_empty() {
        return Err(ApiError::BadRequest(
            "At least one series ID is required".to_string(),
        ));
    }

    // Limit bulk request size to prevent worker queue DoS
    if request.series_ids.len() > MAX_BULK_SERIES_COUNT {
        return Err(ApiError::BadRequest(format!(
            "Too many series in bulk request. Maximum is {}, got {}. \
             Please split into smaller batches.",
            MAX_BULK_SERIES_COUNT,
            request.series_ids.len()
        )));
    }

    // Verify plugin exists and is enabled
    let plugin = PluginsRepository::get_by_id(&state.db, request.plugin_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to get plugin: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Plugin not found".to_string()))?;

    if !plugin.enabled {
        return Err(ApiError::BadRequest("Plugin is disabled".to_string()));
    }

    // Create tasks for each series
    let mut task_ids = Vec::new();
    let mut enqueued = 0;
    let mut skipped_not_found = 0;
    let mut skipped_library_mismatch = 0;

    for series_id in &request.series_ids {
        // Verify series exists (skip if not)
        let series = match SeriesRepository::get_by_id(&state.db, *series_id).await {
            Ok(Some(s)) => s,
            Ok(None) => {
                skipped_not_found += 1;
                continue;
            }
            Err(e) => {
                tracing::warn!("Failed to get series {}: {}", series_id, e);
                skipped_not_found += 1;
                continue;
            }
        };

        // Check if plugin applies to this series' library
        if !plugin.applies_to_library(series.library_id) {
            skipped_library_mismatch += 1;
            continue;
        }

        let task_type = TaskType::PluginAutoMatch {
            series_id: *series_id,
            plugin_id: request.plugin_id,
            source_scope: Some("series:bulk".to_string()),
        };

        match TaskRepository::enqueue(&state.db, task_type, 0, None).await {
            Ok(task_id) => {
                task_ids.push(task_id);
                enqueued += 1;
            }
            Err(e) => {
                tracing::warn!(
                    "Failed to enqueue auto-match task for series {}: {}",
                    series_id,
                    e
                );
            }
        }
    }

    let message = if enqueued == request.series_ids.len() {
        format!("Enqueued {} auto-match task(s)", enqueued)
    } else {
        let mut parts = vec![format!(
            "Enqueued {} of {} task(s)",
            enqueued,
            request.series_ids.len()
        )];
        if skipped_library_mismatch > 0 {
            parts.push(format!(
                "{} skipped (plugin doesn't apply to library)",
                skipped_library_mismatch
            ));
        }
        if skipped_not_found > 0 {
            parts.push(format!("{} skipped (series not found)", skipped_not_found));
        }
        parts.join(", ")
    };

    Ok(Json(EnqueueAutoMatchResponse {
        success: enqueued > 0,
        tasks_enqueued: enqueued,
        task_ids,
        message,
    }))
}

/// Enqueue plugin auto-match tasks for all series in a library
///
/// Creates background tasks to auto-match metadata for all series in a library using
/// the specified plugin. Each series gets its own task that runs asynchronously.
#[utoipa::path(
    post,
    path = "/api/v1/libraries/{id}/metadata/auto-match/task",
    params(
        ("id" = Uuid, Path, description = "Library ID")
    ),
    request_body = EnqueueLibraryAutoMatchRequest,
    responses(
        (status = 200, description = "Tasks enqueued", body = EnqueueAutoMatchResponse),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "No permission to edit series"),
        (status = 404, description = "Library or plugin not found"),
    ),
    security(
        ("bearer_auth" = []),
        ("api_key" = [])
    ),
    tag = "Plugin Actions"
)]
pub async fn enqueue_library_auto_match_tasks(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    Path(library_id): Path<Uuid>,
    Json(request): Json<EnqueueLibraryAutoMatchRequest>,
) -> Result<Json<EnqueueAutoMatchResponse>, ApiError> {
    // Check permission to edit series metadata
    auth.require_permission(&Permission::SeriesWrite)?;

    // Verify library exists
    let _library = LibraryRepository::get_by_id(&state.db, library_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to get library: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Library not found".to_string()))?;

    // Verify plugin exists and is enabled
    let plugin = PluginsRepository::get_by_id(&state.db, request.plugin_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to get plugin: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Plugin not found".to_string()))?;

    if !plugin.enabled {
        return Err(ApiError::BadRequest("Plugin is disabled".to_string()));
    }

    // Check if plugin applies to this library
    if !plugin.applies_to_library(library_id) {
        return Err(ApiError::BadRequest(format!(
            "Plugin '{}' is not configured to apply to this library",
            plugin.display_name
        )));
    }

    // Get all series in the library
    let series_list = SeriesRepository::list_by_library(&state.db, library_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to get series: {}", e)))?;

    if series_list.is_empty() {
        return Ok(Json(EnqueueAutoMatchResponse {
            success: true,
            tasks_enqueued: 0,
            task_ids: vec![],
            message: "No series in library".to_string(),
        }));
    }

    // Limit library auto-match to prevent worker queue DoS
    if series_list.len() > MAX_LIBRARY_SERIES_COUNT {
        return Err(ApiError::BadRequest(format!(
            "Library has too many series ({}) for auto-match. Maximum is {}. \
             Please use the bulk endpoint to process in batches.",
            series_list.len(),
            MAX_LIBRARY_SERIES_COUNT
        )));
    }

    // Create tasks for each series
    let mut task_ids = Vec::new();
    let mut enqueued = 0;

    for series in &series_list {
        let task_type = TaskType::PluginAutoMatch {
            series_id: series.id,
            plugin_id: request.plugin_id,
            source_scope: Some("library:detail".to_string()),
        };

        match TaskRepository::enqueue(&state.db, task_type, 0, None).await {
            Ok(task_id) => {
                task_ids.push(task_id);
                enqueued += 1;
            }
            Err(e) => {
                tracing::warn!(
                    "Failed to enqueue auto-match task for series {}: {}",
                    series.id,
                    e
                );
            }
        }
    }

    let message = if enqueued == series_list.len() {
        format!("Enqueued {} auto-match task(s) for library", enqueued)
    } else {
        format!(
            "Enqueued {} of {} auto-match task(s) for library",
            enqueued,
            series_list.len()
        )
    };

    Ok(Json(EnqueueAutoMatchResponse {
        success: enqueued > 0,
        tasks_enqueued: enqueued,
        task_ids,
        message,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_field_preview_will_apply() {
        let mut will_apply = 0;
        let mut locked = 0;
        let mut no_permission = 0;
        let mut unchanged = 0;
        let mut not_provided = 0;

        let preview = build_field_preview(
            "title",
            Some(serde_json::json!("Old Title")),
            Some(serde_json::json!("New Title")),
            false,
            true,
            &mut will_apply,
            &mut locked,
            &mut no_permission,
            &mut unchanged,
            &mut not_provided,
        );

        assert_eq!(preview.status, FieldApplyStatus::WillApply);
        assert!(preview.reason.is_none());
        assert_eq!(will_apply, 1);
    }

    #[test]
    fn test_build_field_preview_locked() {
        let mut will_apply = 0;
        let mut locked = 0;
        let mut no_permission = 0;
        let mut unchanged = 0;
        let mut not_provided = 0;

        let preview = build_field_preview(
            "title",
            Some(serde_json::json!("Old Title")),
            Some(serde_json::json!("New Title")),
            true, // locked
            true,
            &mut will_apply,
            &mut locked,
            &mut no_permission,
            &mut unchanged,
            &mut not_provided,
        );

        assert_eq!(preview.status, FieldApplyStatus::Locked);
        assert_eq!(locked, 1);
    }

    #[test]
    fn test_build_field_preview_no_permission() {
        let mut will_apply = 0;
        let mut locked = 0;
        let mut no_permission = 0;
        let mut unchanged = 0;
        let mut not_provided = 0;

        let preview = build_field_preview(
            "title",
            Some(serde_json::json!("Old Title")),
            Some(serde_json::json!("New Title")),
            false,
            false, // no permission
            &mut will_apply,
            &mut locked,
            &mut no_permission,
            &mut unchanged,
            &mut not_provided,
        );

        assert_eq!(preview.status, FieldApplyStatus::NoPermission);
        assert_eq!(no_permission, 1);
    }

    #[test]
    fn test_build_field_preview_unchanged() {
        let mut will_apply = 0;
        let mut locked = 0;
        let mut no_permission = 0;
        let mut unchanged = 0;
        let mut not_provided = 0;

        let preview = build_field_preview(
            "title",
            Some(serde_json::json!("Same Title")),
            Some(serde_json::json!("Same Title")),
            false,
            true,
            &mut will_apply,
            &mut locked,
            &mut no_permission,
            &mut unchanged,
            &mut not_provided,
        );

        assert_eq!(preview.status, FieldApplyStatus::Unchanged);
        assert_eq!(unchanged, 1);
    }

    #[test]
    fn test_build_field_preview_not_provided() {
        let mut will_apply = 0;
        let mut locked = 0;
        let mut no_permission = 0;
        let mut unchanged = 0;
        let mut not_provided = 0;

        let preview = build_field_preview(
            "title",
            Some(serde_json::json!("Old Title")),
            None, // not provided
            false,
            true,
            &mut will_apply,
            &mut locked,
            &mut no_permission,
            &mut unchanged,
            &mut not_provided,
        );

        assert_eq!(preview.status, FieldApplyStatus::NotProvided);
        assert_eq!(not_provided, 1);
    }

    #[test]
    fn test_build_field_preview_sorted_arrays_are_unchanged() {
        let mut will_apply = 0;
        let mut locked = 0;
        let mut no_permission = 0;
        let mut unchanged = 0;
        let mut not_provided = 0;

        // Simulate what happens after sorting: same genres in same order should match
        let mut current = vec!["Drama".to_string(), "Action".to_string()];
        current.sort();
        let mut proposed = vec!["Action".to_string(), "Drama".to_string()];
        proposed.sort();

        let preview = build_field_preview(
            "genres",
            Some(serde_json::json!(current)),
            Some(serde_json::json!(proposed)),
            false,
            true,
            &mut will_apply,
            &mut locked,
            &mut no_permission,
            &mut unchanged,
            &mut not_provided,
        );

        assert_eq!(preview.status, FieldApplyStatus::Unchanged);
        assert_eq!(unchanged, 1);
    }

    #[test]
    fn test_build_field_preview_sorted_objects_are_unchanged() {
        let mut will_apply = 0;
        let mut locked = 0;
        let mut no_permission = 0;
        let mut unchanged = 0;
        let mut not_provided = 0;

        // Simulate sorted external links with same structure
        let mut current = vec![
            serde_json::json!({"label": "Kitsu", "url": "https://kitsu.app/manga/123"}),
            serde_json::json!({"label": "AniList", "url": "https://anilist.co/manga/456"}),
        ];
        current.sort_by(|a, b| {
            a["label"]
                .as_str()
                .unwrap_or("")
                .cmp(b["label"].as_str().unwrap_or(""))
        });

        let mut proposed = vec![
            serde_json::json!({"label": "AniList", "url": "https://anilist.co/manga/456"}),
            serde_json::json!({"label": "Kitsu", "url": "https://kitsu.app/manga/123"}),
        ];
        proposed.sort_by(|a, b| {
            a["label"]
                .as_str()
                .unwrap_or("")
                .cmp(b["label"].as_str().unwrap_or(""))
        });

        let preview = build_field_preview(
            "externalLinks",
            Some(serde_json::json!(current)),
            Some(serde_json::json!(proposed)),
            false,
            true,
            &mut will_apply,
            &mut locked,
            &mut no_permission,
            &mut unchanged,
            &mut not_provided,
        );

        assert_eq!(preview.status, FieldApplyStatus::Unchanged);
        assert_eq!(unchanged, 1);
    }

    #[test]
    fn test_build_field_preview_ratings_with_same_structure_are_unchanged() {
        let mut will_apply = 0;
        let mut locked = 0;
        let mut no_permission = 0;
        let mut unchanged = 0;
        let mut not_provided = 0;

        // Both current and proposed use the same structure (score as f64, voteCount, source)
        let current = serde_json::json!({"score": 79.5, "voteCount": 100, "source": "mangabaka"});
        let proposed = serde_json::json!({"score": 79.5, "voteCount": 100, "source": "mangabaka"});

        let preview = build_field_preview(
            "rating",
            Some(current),
            Some(proposed),
            false,
            true,
            &mut will_apply,
            &mut locked,
            &mut no_permission,
            &mut unchanged,
            &mut not_provided,
        );

        assert_eq!(preview.status, FieldApplyStatus::Unchanged);
        assert_eq!(unchanged, 1);
    }

    #[test]
    fn test_build_field_preview_mismatched_structure_shows_will_apply() {
        let mut will_apply = 0;
        let mut locked = 0;
        let mut no_permission = 0;
        let mut unchanged = 0;
        let mut not_provided = 0;

        // Old bug: current lacked voteCount, proposed had it — this should detect the difference
        let current = serde_json::json!({"score": 79.5, "source": "mangabaka"});
        let proposed = serde_json::json!({"score": 79.5, "voteCount": 100, "source": "mangabaka"});

        let preview = build_field_preview(
            "rating",
            Some(current),
            Some(proposed),
            false,
            true,
            &mut will_apply,
            &mut locked,
            &mut no_permission,
            &mut unchanged,
            &mut not_provided,
        );

        // These are structurally different, so it should detect a change
        assert_eq!(preview.status, FieldApplyStatus::WillApply);
        assert_eq!(will_apply, 1);
    }
}
