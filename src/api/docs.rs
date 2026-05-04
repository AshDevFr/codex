use crate::api::{
    error::ErrorResponse,
    routes::{komga, opds, opds2, v1},
};
use utoipa::OpenApi;

/// OpenAPI documentation for Codex REST API
///
/// This struct aggregates all API endpoints, DTOs, and security schemes
/// for automatic API documentation generation (Scalar).
#[derive(OpenApi)]
#[openapi(
    info(
        title = "Codex API",
        version = env!("CARGO_PKG_VERSION"),
        description = r#"REST API for Codex, a digital library server for comics, manga, and ebooks.

## Interactive API Documentation

You can explore and test this API interactively:

- **Hosted Documentation**: Visit [codex.4sh.dev/docs/api/codex-api](https://codex.4sh.dev/docs/api/codex-api) for the full API reference
- **Your Instance**: If you have Scalar UI enabled, access `/api/docs` on your Codex server

## Authentication

Most endpoints require authentication. Codex supports two authentication methods:

1. **JWT Bearer Token**: Obtain a token via `POST /api/v1/auth/login`, then include it as `Authorization: Bearer <token>`
2. **API Key**: Generate an API key in the web UI or via the API, then include it as `X-API-Key: <key>` header

## OPDS Support

Codex provides OPDS catalog feeds for e-reader applications:

- **OPDS 1.2** (Atom XML): `/opds/v1/catalog` - Compatible with most e-readers
- **OPDS 2.0** (JSON): `/opds/v2` - Modern JSON-based format with enhanced features

## Komga-Compatible API

Codex provides an optional Komga-compatible API for third-party apps like Komic:

- **Disabled by default** - Enable via `komga_api.enabled: true` in config
- **Configurable prefix** - Default path: `/{prefix}/api/v1/` where prefix defaults to `komga`
- **Same authentication** - Supports JWT, API keys, and Basic Auth

Note: The `{prefix}` path parameter in Komga endpoints is configurable at runtime.

## Pagination

List endpoints support pagination with the following conventions:

### Query Parameters

All endpoints (GET and POST) use query parameters for pagination:

| Parameter | Default | Max | Description |
|-----------|---------|-----|-------------|
| `page` | `1` | - | Page number (1-indexed) |
| `pageSize` | `50` | `500` | Items per page |

Example GET: `GET /api/v1/books?page=2&pageSize=25`

Example POST: `POST /api/v1/series/list?page=1&pageSize=25&sort=name,asc`

For POST endpoints like `/api/v1/books/list` and `/api/v1/series/list`:
- Pagination parameters (`page`, `pageSize`, `sort`) go in the **query string**
- Filter criteria (`condition`, `fullTextSearch`) go in the **request body**

### Response Format

All paginated responses use camelCase and include HATEOAS navigation links:

```json
{
  "data": [...],
  "page": 1,
  "pageSize": 25,
  "total": 150,
  "totalPages": 6,
  "links": {
    "self": "/api/v1/series/list?page=1&pageSize=25",
    "first": "/api/v1/series/list?page=1&pageSize=25",
    "next": "/api/v1/series/list?page=2&pageSize=25",
    "last": "/api/v1/series/list?page=6&pageSize=25"
  }
}
```

## Rate Limiting

All API endpoints are protected by rate limiting (enabled by default). Rate limits use a token bucket algorithm with separate limits for anonymous and authenticated users.

### Limits

| Client Type | Requests/Second | Burst Size |
|-------------|-----------------|------------|
| Anonymous (by IP) | 10 | 50 |
| Authenticated (by user) | 50 | 200 |

### Response Headers

All responses include rate limit information:

| Header | Description |
|--------|-------------|
| `X-RateLimit-Limit` | Maximum requests allowed (burst size) |
| `X-RateLimit-Remaining` | Requests remaining |
| `X-RateLimit-Reset` | Seconds until a token is available |

### 429 Too Many Requests

When rate limited, the API returns HTTP 429 with a `Retry-After` header:

```json
{
  "error": "rate_limit_exceeded",
  "message": "Too many requests. Please retry after 30 seconds.",
  "retry_after": 30
}
```

### Exempt Paths

The following paths are exempt from rate limiting:
- `/health` - Health check endpoint
- `/api/v1/events` - SSE event stream"#,
        license(
            name = "MIT",
            url = "https://opensource.org/licenses/MIT"
        )
    ),
    paths(
        // Health check and info
        v1::handlers::health_check,
        v1::handlers::info::get_app_info,

        // Setup endpoints
        v1::handlers::setup::setup_status,
        v1::handlers::setup::initialize_setup,
        v1::handlers::setup::configure_initial_settings,

        // Auth endpoints
        v1::handlers::login,
        v1::handlers::logout,
        v1::handlers::register,
        v1::handlers::verify_email,
        v1::handlers::resend_verification,
        v1::handlers::get_me,

        // OIDC auth endpoints
        v1::handlers::oidc::list_providers,
        v1::handlers::oidc::login,
        v1::handlers::oidc::callback,

        // Library endpoints
        v1::handlers::list_libraries,
        v1::handlers::create_library,
        v1::handlers::preview_scan,
        v1::handlers::get_library,
        v1::handlers::update_library,
        v1::handlers::delete_library,
        v1::handlers::purge_deleted_books,

        // Series endpoints
        v1::handlers::list_series,
        v1::handlers::search_series,
        v1::handlers::list_series_filtered,
        v1::handlers::list_series_alphabetical_groups,
        v1::handlers::get_series,
        v1::handlers::patch_series,
        v1::handlers::get_series_books,
        v1::handlers::purge_series_deleted_books,
        v1::handlers::get_series_thumbnail,
        v1::handlers::upload_series_cover,
        v1::handlers::set_series_cover_source,
        v1::handlers::trigger_series_analysis,
        v1::handlers::renumber_series,
        v1::handlers::list_in_progress_series,
        v1::handlers::list_recently_added_series,
        v1::handlers::list_recently_updated_series,
        v1::handlers::list_library_series,
        v1::handlers::list_library_in_progress_series,
        v1::handlers::list_library_recently_added_series,
        v1::handlers::list_library_recently_updated_series,
        v1::handlers::download_series,
        v1::handlers::replace_series_metadata,
        v1::handlers::patch_series_metadata,
        v1::handlers::reset_series_metadata,
        v1::handlers::get_series_metadata,
        v1::handlers::get_metadata_locks,
        v1::handlers::update_metadata_locks,

        // Genre endpoints
        v1::handlers::list_genres,
        v1::handlers::get_series_genres,
        v1::handlers::set_series_genres,
        v1::handlers::add_series_genre,
        v1::handlers::remove_series_genre,
        v1::handlers::delete_genre,
        v1::handlers::cleanup_genres,

        // Tag endpoints
        v1::handlers::list_tags,
        v1::handlers::get_series_tags,
        v1::handlers::set_series_tags,
        v1::handlers::add_series_tag,
        v1::handlers::remove_series_tag,
        v1::handlers::delete_tag,
        v1::handlers::cleanup_tags,

        // Current user endpoint
        v1::handlers::get_current_user,

        // User rating endpoints
        v1::handlers::get_series_rating,
        v1::handlers::set_series_rating,
        v1::handlers::delete_series_rating,
        v1::handlers::list_user_ratings,

        // User preferences endpoints
        v1::handlers::user_preferences::get_all_preferences,
        v1::handlers::user_preferences::get_preference,
        v1::handlers::user_preferences::set_preference,
        v1::handlers::user_preferences::set_bulk_preferences,
        v1::handlers::user_preferences::delete_preference,

        // Series Exports
        v1::handlers::series_exports::create_export,
        v1::handlers::series_exports::list_exports,
        v1::handlers::series_exports::get_export,
        v1::handlers::series_exports::download_export,
        v1::handlers::series_exports::delete_export,
        v1::handlers::series_exports::get_field_catalog,

        // Alternate title endpoints
        v1::handlers::get_series_alternate_titles,
        v1::handlers::create_alternate_title,
        v1::handlers::update_alternate_title,
        v1::handlers::delete_alternate_title,

        // External rating endpoints
        v1::handlers::get_series_external_ratings,
        v1::handlers::create_external_rating,
        v1::handlers::delete_external_rating,

        // Average rating endpoint
        v1::handlers::get_series_average_rating,

        // External link endpoints
        v1::handlers::get_series_external_links,
        v1::handlers::create_external_link,
        v1::handlers::delete_external_link,
        // Series external ID endpoints
        v1::handlers::list_series_external_ids,
        v1::handlers::create_series_external_id,
        v1::handlers::delete_series_external_id,

        // Cover management endpoints
        v1::handlers::list_series_covers,
        v1::handlers::get_series_cover_image,
        v1::handlers::select_series_cover,
        v1::handlers::reset_series_cover,
        v1::handlers::delete_series_cover,

        // Book endpoints
        v1::handlers::list_books,
        v1::handlers::list_books_filtered,
        v1::handlers::get_book,
        v1::handlers::patch_book,
        v1::handlers::get_adjacent_books,
        v1::handlers::get_book_file,
        v1::handlers::get_book_thumbnail,
        v1::handlers::trigger_book_analysis,
        v1::handlers::list_library_books,
        v1::handlers::list_in_progress_books,
        v1::handlers::list_library_in_progress_books,
        v1::handlers::list_on_deck_books,
        v1::handlers::list_library_on_deck_books,
        v1::handlers::list_recently_added_books,
        v1::handlers::list_library_recently_added_books,
        v1::handlers::list_recently_read_books,
        v1::handlers::list_library_recently_read_books,
        v1::handlers::list_books_with_errors,
        v1::handlers::retry_book_errors,
        v1::handlers::retry_all_book_errors,
        v1::handlers::replace_book_metadata,
        v1::handlers::patch_book_metadata,
        v1::handlers::get_book_metadata_locks,
        v1::handlers::update_book_metadata_locks,
        v1::handlers::upload_book_cover,

        // Book external IDs endpoints
        v1::handlers::list_book_external_ids,
        v1::handlers::create_book_external_id,
        v1::handlers::delete_book_external_id,
        // Book external links endpoints
        v1::handlers::list_book_external_links,
        v1::handlers::create_book_external_link,
        v1::handlers::delete_book_external_link,

        // Book cover management endpoints
        v1::handlers::list_book_covers,
        v1::handlers::select_book_cover,
        v1::handlers::reset_book_cover,
        v1::handlers::get_book_cover_image,
        v1::handlers::delete_book_cover,

        // Page endpoints
        v1::handlers::list_book_pages,
        v1::handlers::get_page_image,

        // Reading progress endpoints
        v1::handlers::update_reading_progress,
        v1::handlers::get_reading_progress,
        v1::handlers::delete_reading_progress,
        v1::handlers::get_user_progress,
        v1::handlers::mark_book_as_read,
        v1::handlers::mark_book_as_unread,
        v1::handlers::mark_series_as_read,
        v1::handlers::mark_series_as_unread,

        // Bulk operations endpoints
        v1::handlers::bulk_mark_books_as_read,
        v1::handlers::bulk_mark_books_as_unread,
        v1::handlers::bulk_analyze_books,
        v1::handlers::bulk_generate_book_thumbnails,
        v1::handlers::bulk_mark_series_as_read,
        v1::handlers::bulk_mark_series_as_unread,
        v1::handlers::bulk_analyze_series,
        v1::handlers::bulk_renumber_series,
        v1::handlers::bulk_generate_series_thumbnails,
        v1::handlers::bulk_generate_series_book_thumbnails,
        v1::handlers::bulk_reprocess_series_titles,
        v1::handlers::bulk_reset_series_metadata,
        v1::handlers::bulk_patch_series_metadata,
        v1::handlers::bulk_patch_book_metadata,
        v1::handlers::bulk_modify_series_tags,
        v1::handlers::bulk_modify_series_genres,
        v1::handlers::bulk_modify_book_tags,
        v1::handlers::bulk_modify_book_genres,
        v1::handlers::bulk_update_series_locks,
        v1::handlers::bulk_update_book_locks,

        // User endpoints
        v1::handlers::list_users,
        v1::handlers::create_user,
        v1::handlers::get_user,
        v1::handlers::update_user,
        v1::handlers::delete_user,

        // API key endpoints
        v1::handlers::api_keys::list_api_keys,
        v1::handlers::api_keys::create_api_key,
        v1::handlers::api_keys::get_api_key,
        v1::handlers::api_keys::update_api_key,
        v1::handlers::api_keys::delete_api_key,

        // Metrics endpoints
        v1::handlers::get_inventory_metrics,
        v1::handlers::get_plugin_metrics,
        v1::handlers::task_metrics::get_task_metrics,
        v1::handlers::task_metrics::get_task_metrics_history,
        v1::handlers::task_metrics::trigger_metrics_cleanup,
        v1::handlers::task_metrics::nuke_task_metrics,

        // Scan endpoints
        v1::handlers::trigger_scan,
        v1::handlers::get_scan_status,
        v1::handlers::cancel_scan,
        v1::handlers::list_active_scans,
        v1::handlers::scan_progress_stream,
        v1::handlers::trigger_library_analysis,
        v1::handlers::trigger_library_unanalyzed_analysis,
        v1::handlers::trigger_series_unanalyzed_analysis,
        v1::handlers::trigger_book_unanalyzed_analysis,

        // Task Queue endpoints
        v1::handlers::task_queue::list_tasks,
        v1::handlers::task_queue::create_task,
        v1::handlers::task_queue::get_task,
        v1::handlers::task_queue::cancel_task,
        v1::handlers::task_queue::unlock_task,
        v1::handlers::task_queue::retry_task,
        v1::handlers::task_queue::get_task_stats,
        v1::handlers::task_queue::purge_old_tasks,
        v1::handlers::task_queue::nuke_all_tasks,
        // Book thumbnail endpoints
        v1::handlers::task_queue::generate_book_thumbnail,
        v1::handlers::task_queue::generate_book_thumbnails,
        v1::handlers::task_queue::generate_library_book_thumbnails,
        // Series thumbnail endpoints
        v1::handlers::task_queue::generate_series_thumbnail,
        v1::handlers::task_queue::generate_series_thumbnails,
        v1::handlers::task_queue::generate_library_series_thumbnails,
        // Reprocess title endpoints
        v1::handlers::task_queue::reprocess_series_title,
        v1::handlers::task_queue::reprocess_series_titles,
        v1::handlers::task_queue::reprocess_library_series_titles,

        // Filesystem endpoints
        v1::handlers::browse_filesystem,
        v1::handlers::list_drives,

        // Settings endpoints
        v1::handlers::settings::get_branding_settings,
        v1::handlers::settings::get_public_settings,
        v1::handlers::settings::list_settings,
        v1::handlers::settings::get_setting,
        v1::handlers::settings::update_setting,
        v1::handlers::settings::bulk_update_settings,
        v1::handlers::settings::reset_setting,
        v1::handlers::settings::get_setting_history,

        // Plugins endpoints
        v1::handlers::plugins::list_plugins,
        v1::handlers::plugins::create_plugin,
        v1::handlers::plugins::get_plugin,
        v1::handlers::plugins::update_plugin,
        v1::handlers::plugins::delete_plugin,
        v1::handlers::plugins::enable_plugin,
        v1::handlers::plugins::disable_plugin,
        v1::handlers::plugins::test_plugin,
        v1::handlers::plugins::get_plugin_health,
        v1::handlers::plugins::reset_plugin_failures,
        v1::handlers::plugins::get_plugin_failures,

        // Plugin Actions endpoints
        v1::handlers::plugin_actions::get_plugin_actions,
        v1::handlers::plugin_actions::execute_plugin,
        v1::handlers::plugin_actions::get_series_search_title,
        v1::handlers::plugin_actions::preview_series_metadata,
        v1::handlers::plugin_actions::apply_series_metadata,
        v1::handlers::plugin_actions::auto_match_series_metadata,
        v1::handlers::plugin_actions::enqueue_auto_match_task,
        v1::handlers::plugin_actions::enqueue_bulk_auto_match_tasks,
        v1::handlers::plugin_actions::enqueue_library_auto_match_tasks,

        // User Plugin endpoints
        v1::handlers::user_plugins::list_user_plugins,
        v1::handlers::user_plugins::enable_plugin,
        v1::handlers::user_plugins::disable_plugin,
        v1::handlers::user_plugins::disconnect_plugin,
        v1::handlers::user_plugins::get_user_plugin,
        v1::handlers::user_plugins::update_user_plugin_config,
        v1::handlers::user_plugins::oauth_start,
        v1::handlers::user_plugins::oauth_callback,
        v1::handlers::user_plugins::set_user_credentials,
        v1::handlers::user_plugins::trigger_sync,
        v1::handlers::user_plugins::get_sync_status,
        v1::handlers::user_plugins::get_plugin_tasks,

        // Recommendation endpoints
        v1::handlers::recommendations::get_recommendations,
        v1::handlers::recommendations::refresh_recommendations,
        v1::handlers::recommendations::dismiss_recommendation,

        // Sharing Tags endpoints
        v1::handlers::sharing_tags::list_sharing_tags,
        v1::handlers::sharing_tags::get_sharing_tag,
        v1::handlers::sharing_tags::create_sharing_tag,
        v1::handlers::sharing_tags::update_sharing_tag,
        v1::handlers::sharing_tags::delete_sharing_tag,
        v1::handlers::sharing_tags::get_series_sharing_tags,
        v1::handlers::sharing_tags::set_series_sharing_tags,
        v1::handlers::sharing_tags::add_series_sharing_tag,
        v1::handlers::sharing_tags::remove_series_sharing_tag,
        v1::handlers::sharing_tags::get_user_sharing_tags,
        v1::handlers::sharing_tags::set_user_sharing_tag,
        v1::handlers::sharing_tags::remove_user_sharing_tag,
        v1::handlers::sharing_tags::get_my_sharing_tags,

        // Cleanup endpoints
        v1::handlers::cleanup::get_orphan_stats,
        v1::handlers::cleanup::trigger_cleanup,
        v1::handlers::cleanup::delete_orphans,

        // PDF cache management endpoints
        v1::handlers::pdf_cache::get_pdf_cache_stats,
        v1::handlers::pdf_cache::trigger_pdf_cache_cleanup,
        v1::handlers::pdf_cache::clear_pdf_cache,

        // Plugin storage management endpoints
        v1::handlers::plugin_storage::get_all_plugin_storage_stats,
        v1::handlers::plugin_storage::get_plugin_storage_stats,
        v1::handlers::plugin_storage::cleanup_plugin_storage,

        // Duplicates endpoints
        v1::handlers::duplicates::list_duplicates,
        v1::handlers::duplicates::trigger_duplicate_scan,
        v1::handlers::duplicates::delete_duplicate_group,

        // SSE streaming endpoints
        v1::handlers::events::entity_events_stream,
        v1::handlers::events::task_progress_stream,

        // OPDS 1.2 catalog endpoints (XML format)
        opds::handlers::catalog::root_catalog,
        opds::handlers::catalog::list_libraries,
        opds::handlers::catalog::library_series,
        opds::handlers::catalog::series_books,
        opds::handlers::search::opensearch_descriptor,
        opds::handlers::search::search,
        opds::handlers::pse::book_pages,
        opds::handlers::pse::book_page_image,

        // OPDS 2.0 catalog endpoints (JSON format)
        opds2::handlers::catalog::root,
        opds2::handlers::catalog::libraries,
        opds2::handlers::catalog::library_series,
        opds2::handlers::catalog::series_books,
        opds2::handlers::catalog::recent,
        opds2::handlers::search::search,

        // Komga-compatible API endpoints (for third-party apps)
        komga::handlers::list_libraries,
        komga::handlers::get_library,
        komga::handlers::get_library_thumbnail,
        komga::handlers::list_series,
        komga::handlers::get_series_new,
        komga::handlers::get_series_updated,
        komga::handlers::get_series,
        komga::handlers::get_series_thumbnail,
        komga::handlers::get_series_books,
        komga::handlers::get_book,
        komga::handlers::get_book_thumbnail,
        komga::handlers::get_books_ondeck,
        komga::handlers::search_books,
        komga::handlers::get_next_book,
        komga::handlers::get_previous_book,
        komga::handlers::download_book_file,
        komga::handlers::list_pages,
        komga::handlers::get_page,
        komga::handlers::get_page_thumbnail,
        komga::handlers::update_progress,
        komga::handlers::delete_progress,
        komga::handlers::mark_series_as_read,
        komga::handlers::mark_series_as_unread,
        komga::handlers::get_current_user,
        komga::handlers::search_series,

        // Komga stub endpoints (empty responses for third-party app compatibility)
        komga::handlers::list_collections,
        komga::handlers::list_readlists,
        komga::handlers::list_genres,
        komga::handlers::list_tags,
        komga::handlers::list_authors_v2,
        komga::handlers::list_languages,
        komga::handlers::list_publishers,
        komga::handlers::list_age_ratings,
        komga::handlers::list_series_release_dates,
    ),
    components(
        schemas(
            // App info
            v1::dto::AppInfoDto,

            // DTOs
            v1::dto::LoginRequest,
            v1::dto::LoginResponse,
            v1::dto::RegisterRequest,
            v1::dto::RegisterResponse,
            v1::dto::VerifyEmailRequest,
            v1::dto::VerifyEmailResponse,
            v1::dto::ResendVerificationRequest,
            v1::dto::ResendVerificationResponse,
            v1::dto::TokenResponse,

            // OIDC DTOs
            v1::dto::OidcProviderInfo,
            v1::dto::OidcProvidersResponse,
            v1::dto::OidcLoginResponse,
            v1::dto::OidcCallbackResponse,
            v1::dto::OidcErrorResponse,

            // Setup DTOs
            v1::dto::SetupStatusResponse,
            v1::dto::InitializeSetupRequest,
            v1::dto::InitializeSetupResponse,
            v1::dto::ConfigureSettingsRequest,
            v1::dto::ConfigureSettingsResponse,
            v1::dto::LibraryDto,
            v1::dto::CreateLibraryRequest,
            v1::dto::UpdateLibraryRequest,
            v1::dto::PreviewScanRequest,
            v1::dto::PreviewScanResponse,
            v1::dto::DetectedSeriesDto,
            v1::dto::DetectedSeriesMetadataDto,

            // Strategy types
            crate::models::SeriesStrategy,
            crate::models::BookStrategy,
            crate::models::FlatStrategyConfig,
            crate::models::PublisherHierarchyConfig,
            crate::models::CalibreStrategyConfig,
            crate::models::CalibreSeriesMode,
            crate::models::CustomStrategyConfig,
            crate::models::SmartBookConfig,
            v1::dto::SeriesDto,
            v1::dto::SeriesListResponse,
            v1::dto::SearchSeriesRequest,
            v1::dto::SeriesListRequest,
            v1::dto::SeriesCondition,
            v1::dto::AlphabeticalGroupDto,
            v1::dto::PatchSeriesRequest,
            v1::dto::SeriesUpdateResponse,
            v1::dto::BookListRequest,
            v1::dto::BookCondition,
            v1::dto::FieldOperator,
            v1::dto::UuidOperator,
            v1::dto::BoolOperator,
            v1::dto::ReplaceSeriesMetadataRequest,
            v1::dto::PatchSeriesMetadataRequest,
            v1::dto::SeriesMetadataResponse,
            v1::dto::FullSeriesMetadataResponse,
            v1::dto::SeriesFullMetadata,
            v1::dto::FullSeriesResponse,
            v1::dto::MetadataLocks,
            v1::dto::UpdateMetadataLocksRequest,
            v1::dto::ReprocessTitleRequest,
            v1::dto::ReprocessTitleResult,
            v1::dto::ReprocessLibraryTitlesResponse,
            v1::dto::EnqueueReprocessTitleRequest,
            v1::dto::ReprocessSeriesTitlesRequest,
            v1::dto::EnqueueReprocessTitleResponse,

            // Series Context DTOs (for template evaluation)
            v1::dto::SeriesContextDto,
            v1::dto::MetadataContextDto,
            v1::dto::ExternalIdContextDto,
            v1::dto::AlternateTitleContextDto,
            v1::dto::AuthorContextDto,
            v1::dto::ExternalRatingContextDto,
            v1::dto::ExternalLinkContextDto,

            // Book Context DTOs (for template evaluation)
            v1::dto::BookContextDto,
            v1::dto::BookMetadataContextDto,
            v1::dto::BookAwardContextDto,

            // Genre DTOs
            v1::dto::GenreDto,
            v1::dto::GenreListResponse,
            v1::dto::SetSeriesGenresRequest,
            v1::dto::AddSeriesGenreRequest,
            v1::dto::TaxonomyCleanupResponse,

            // Tag DTOs
            v1::dto::TagDto,
            v1::dto::TagListResponse,
            v1::dto::SetSeriesTagsRequest,
            v1::dto::AddSeriesTagRequest,

            // User Rating DTOs
            v1::dto::UserSeriesRatingDto,
            v1::dto::UserRatingsListResponse,
            v1::dto::SetUserRatingRequest,

            // User Preferences DTOs
            v1::dto::UserPreferenceDto,
            v1::dto::UserPreferencesResponse,
            v1::dto::SetPreferenceRequest,
            v1::dto::BulkSetPreferencesRequest,
            v1::dto::SetPreferencesResponse,
            v1::dto::DeletePreferenceResponse,

            // Series Export DTOs
            v1::dto::series_export::CreateSeriesExportRequest,
            v1::dto::series_export::SeriesExportDto,
            v1::dto::series_export::SeriesExportListResponse,
            v1::dto::series_export::ExportFieldDto,
            v1::dto::series_export::ExportFieldCatalogResponse,
            v1::dto::series_export::ExportPresetsDto,

            // Alternate Title DTOs
            v1::dto::AlternateTitleDto,
            v1::dto::AlternateTitleListResponse,
            v1::dto::CreateAlternateTitleRequest,
            v1::dto::UpdateAlternateTitleRequest,

            // External Rating DTOs
            v1::dto::ExternalRatingDto,
            v1::dto::ExternalRatingListResponse,
            v1::dto::CreateExternalRatingRequest,

            // Average Rating DTOs
            v1::dto::SeriesAverageRatingResponse,

            // External Link DTOs
            v1::dto::ExternalLinkDto,
            v1::dto::ExternalLinkListResponse,
            v1::dto::CreateExternalLinkRequest,

            // Cover DTOs
            v1::dto::SeriesCoverDto,
            v1::dto::SeriesCoverListResponse,
            v1::dto::BookCoverDto,
            v1::dto::BookCoverListResponse,

            // Book External ID DTOs
            v1::dto::BookExternalIdDto,
            v1::dto::BookExternalIdListResponse,
            v1::dto::CreateBookExternalIdRequest,

            // Book External Link DTOs
            v1::dto::BookExternalLinkDto,
            v1::dto::BookExternalLinkListResponse,
            v1::dto::CreateBookExternalLinkRequest,

            // Sharing Tag DTOs
            v1::dto::SharingTagDto,
            v1::dto::SharingTagSummaryDto,
            v1::dto::SharingTagListResponse,
            v1::dto::CreateSharingTagRequest,
            v1::dto::UpdateSharingTagRequest,
            v1::dto::SetSeriesSharingTagsRequest,
            v1::dto::ModifySeriesSharingTagRequest,
            v1::dto::UserSharingTagGrantDto,
            v1::dto::SetUserSharingTagGrantRequest,
            v1::dto::UserSharingTagGrantsResponse,
            crate::db::entities::user_sharing_tags::AccessMode,

            v1::dto::BookDto,
            v1::dto::BookListResponse,
            v1::dto::BookDetailResponse,
            v1::dto::FullBookResponse,
            v1::dto::FullBookListResponse,
            v1::dto::BookFullMetadata,
            v1::dto::BookMetadataLocks,
            v1::dto::UpdateBookMetadataLocksRequest,
            v1::dto::PatchBookRequest,
            v1::dto::BookUpdateResponse,
            v1::dto::AdjacentBooksResponse,
            v1::dto::BookMetadataDto,
            v1::dto::ReplaceBookMetadataRequest,
            v1::dto::PatchBookMetadataRequest,
            v1::dto::BookMetadataResponse,
            v1::dto::BookErrorTypeDto,
            v1::dto::BookErrorDto,
            v1::dto::BookWithErrorsDto,
            v1::dto::ErrorGroupDto,
            v1::dto::BooksWithErrorsResponse,
            v1::dto::RetryBookErrorsRequest,
            v1::dto::RetryAllErrorsRequest,
            v1::dto::RetryErrorsResponse,
            v1::dto::PageDto,
            v1::dto::UserDto,
            v1::dto::UserDetailDto,
            v1::dto::CreateUserRequest,
            v1::dto::UpdateUserRequest,
            v1::dto::ApiKeyDto,
            v1::dto::CreateApiKeyRequest,
            v1::dto::CreateApiKeyResponse,
            v1::dto::UpdateApiKeyRequest,
            v1::dto::PaginatedResponse<v1::dto::SeriesDto>,
            v1::dto::PaginatedResponse<v1::dto::BookDto>,
            v1::dto::PaginatedResponse<v1::dto::UserDto>,

            // Metrics DTOs
            v1::dto::MetricsDto,
            v1::dto::LibraryMetricsDto,

            // Plugin Metrics DTOs
            v1::dto::PluginMetricsResponse,
            v1::dto::PluginMetricsSummaryDto,
            v1::dto::PluginMetricsDto,
            v1::dto::PluginMethodMetricsDto,

            // Task Metrics DTOs
            v1::dto::TaskMetricsResponse,
            v1::dto::TaskMetricsSummaryDto,
            v1::dto::TaskTypeMetricsDto,
            v1::dto::QueueHealthMetricsDto,
            v1::dto::TaskMetricsHistoryResponse,
            v1::dto::TaskMetricsDataPointDto,
            v1::dto::MetricsCleanupResponse,
            v1::dto::MetricsNukeResponse,

            // Scan DTOs
            v1::dto::ScanStatusDto,
            v1::dto::TriggerScanQuery,
            v1::dto::ScanningConfigDto,
            v1::dto::AnalysisResult,

            // Reading progress DTOs
            v1::dto::UpdateProgressRequest,
            v1::dto::ReadProgressResponse,
            v1::dto::ReadProgressListResponse,
            v1::dto::MarkReadResponse,

            // Bulk operations DTOs
            v1::dto::BulkBooksRequest,
            v1::dto::BulkAnalyzeBooksRequest,
            v1::dto::BulkSeriesRequest,
            v1::dto::BulkAnalyzeSeriesRequest,
            v1::dto::BulkAnalyzeResponse,
            v1::dto::BulkRenumberSeriesRequest,
            v1::dto::BulkGenerateBookThumbnailsRequest,
            v1::dto::BulkGenerateSeriesBookThumbnailsRequest,
            v1::dto::BulkGenerateSeriesThumbnailsRequest,
            v1::dto::BulkReprocessSeriesTitlesRequest,
            v1::dto::BulkTaskResponse,
            v1::dto::BulkMetadataResetResponse,
            v1::dto::BulkPatchSeriesMetadataRequest,
            v1::dto::BulkPatchBookMetadataRequest,
            v1::dto::BulkModifySeriesTagsRequest,
            v1::dto::BulkModifySeriesGenresRequest,
            v1::dto::BulkModifyBookTagsRequest,
            v1::dto::BulkModifyBookGenresRequest,
            v1::dto::BulkUpdateSeriesLocksRequest,
            v1::dto::BulkUpdateBookLocksRequest,
            v1::dto::BulkMetadataUpdateResponse,

            // Filesystem DTOs
            v1::handlers::filesystem::FileSystemEntry,
            v1::handlers::filesystem::BrowseResponse,

            // Settings DTOs
            v1::dto::SettingDto,
            v1::dto::PublicSettingDto,
            v1::dto::BrandingSettingsDto,
            v1::dto::UpdateSettingRequest,
            v1::dto::BulkUpdateSettingsRequest,
            v1::dto::BulkSettingUpdate,
            v1::dto::SettingHistoryDto,
            v1::dto::ListSettingsQuery,

            // Plugin DTOs
            v1::dto::PluginDto,
            v1::dto::PluginsListResponse,
            v1::dto::CreatePluginRequest,
            v1::dto::UpdatePluginRequest,
            v1::dto::EnvVarDto,
            v1::dto::PluginManifestDto,
            v1::dto::OAuthConfigDto,
            v1::dto::PluginCapabilitiesDto,
            v1::dto::CredentialFieldDto,
            v1::dto::PluginTestResult,
            v1::dto::PluginStatusResponse,
            v1::dto::PluginHealthDto,
            v1::dto::PluginHealthResponse,
            v1::dto::PluginFailureDto,
            v1::dto::PluginFailuresResponse,

            // User Plugin DTOs
            v1::dto::UserPluginDto,
            v1::dto::AvailablePluginDto,
            v1::dto::UserPluginCapabilitiesDto,
            v1::dto::UserPluginsListResponse,
            v1::dto::OAuthStartResponse,
            v1::dto::UpdateUserPluginConfigRequest,
            v1::dto::SetUserCredentialsRequest,
            v1::dto::SyncTriggerResponse,
            v1::dto::SyncStatusDto,
            v1::dto::SyncStatusQuery,
            v1::dto::UserPluginTaskDto,
            v1::dto::UserPluginTasksQuery,

            // Recommendation DTOs
            v1::dto::recommendations::RecommendationDto,
            v1::dto::recommendations::RecommendationsResponse,
            v1::dto::recommendations::RecommendationsRefreshResponse,
            v1::dto::recommendations::DismissRecommendationRequest,
            v1::dto::recommendations::DismissRecommendationResponse,

            // Plugin Actions DTOs
            v1::dto::PluginActionDto,
            v1::dto::PluginActionsResponse,
            v1::dto::ExecutePluginRequest,
            v1::dto::ExecutePluginResponse,
            v1::dto::PluginSearchResultDto,
            v1::dto::SearchResultPreviewDto,
            v1::dto::PluginSearchResponse,
            v1::dto::MetadataPreviewRequest,
            v1::dto::MetadataPreviewResponse,
            v1::dto::MetadataFieldPreview,
            v1::dto::FieldApplyStatus,
            v1::dto::PreviewSummary,
            v1::dto::MetadataApplyRequest,
            v1::dto::MetadataApplyResponse,
            v1::dto::FieldChangeDto,
            v1::dto::DryRunReportDto,
            v1::dto::SkippedField,
            v1::dto::MetadataAutoMatchRequest,
            v1::dto::MetadataAutoMatchResponse,
            v1::dto::SearchTitleResponse,
            v1::dto::EnqueueAutoMatchRequest,
            v1::dto::EnqueueAutoMatchResponse,
            v1::dto::EnqueueBulkAutoMatchRequest,
            v1::dto::EnqueueLibraryAutoMatchRequest,

            // Library Jobs DTOs (Phase 9)
            v1::dto::LibraryJobDto,
            v1::dto::LibraryJobConfigDto,
            v1::dto::MetadataRefreshJobConfigDto,
            v1::dto::CreateLibraryJobRequest,
            v1::dto::PatchLibraryJobRequest,
            v1::dto::ListLibraryJobsResponse,
            v1::dto::RunNowResponse,
            v1::dto::DryRunRequest,
            v1::dto::DryRunResponse,
            v1::dto::DryRunSeriesDelta,
            v1::dto::DryRunSkippedFieldDto,
            v1::dto::FieldGroupDto,
            v1::dto::PluginCapabilitiesDto,

            // Task Queue DTOs
            v1::handlers::task_queue::CreateTaskRequest,
            v1::handlers::task_queue::CreateTaskResponse,
            v1::handlers::task_queue::TaskResponse,
            v1::handlers::task_queue::PurgeTasksResponse,
            v1::handlers::task_queue::MessageResponse,
            v1::handlers::task_queue::GenerateBookThumbnailsRequest,
            v1::handlers::task_queue::GenerateSeriesThumbnailsRequest,
            v1::handlers::task_queue::ForceRequest,
            crate::tasks::types::TaskStats,
            crate::tasks::types::TaskTypeStats,
            crate::tasks::types::TaskType,

            // Duplicates DTOs
            v1::dto::DuplicateGroup,
            v1::dto::ListDuplicatesResponse,
            v1::dto::TriggerDuplicateScanResponse,

            // Cleanup DTOs
            v1::dto::OrphanStatsDto,
            v1::dto::OrphanedFileDto,
            v1::dto::CleanupResultDto,
            v1::dto::TriggerCleanupResponse,
            v1::dto::OrphanStatsQuery,

            // PDF Cache DTOs
            v1::dto::PdfCacheStatsDto,
            v1::dto::PdfCacheCleanupResultDto,
            v1::dto::TriggerPdfCacheCleanupResponse,

            // Plugin Storage DTOs
            v1::dto::AllPluginStorageStatsDto,
            v1::dto::PluginStorageStatsDto,
            v1::dto::PluginCleanupResultDto,

            // SSE Event DTOs
            crate::events::EntityChangeEvent,
            crate::events::EntityEvent,
            crate::events::TaskProgressEvent,

            // Error responses
            ErrorResponse,

            // OPDS 2.0 DTOs
            opds2::dto::Opds2Feed,
            opds2::dto::Opds2Link,
            opds2::dto::LinkProperties,
            opds2::dto::FeedMetadata,
            opds2::dto::PublicationMetadata,
            opds2::dto::Publication,
            opds2::dto::ImageLink,
            opds2::dto::Contributor,
            opds2::dto::BelongsTo,
            opds2::dto::SeriesInfo,
            opds2::dto::Group,
            opds2::dto::ReadingProgress,

            // Komga-compatible API DTOs
            komga::dto::KomgaLibraryDto,
            komga::dto::KomgaSeriesDto,
            komga::dto::KomgaSeriesMetadataDto,
            komga::dto::KomgaBooksMetadataAggregationDto,
            komga::dto::KomgaAuthorDto,
            komga::dto::KomgaWebLinkDto,
            komga::dto::KomgaAlternateTitleDto,
            komga::dto::KomgaBookDto,
            komga::dto::KomgaMediaDto,
            komga::dto::KomgaBookMetadataDto,
            komga::dto::KomgaBookLinkDto,
            komga::dto::KomgaReadProgressDto,
            komga::dto::KomgaReadProgressUpdateDto,
            komga::dto::KomgaBooksSearchRequestDto,
            komga::dto::KomgaPageDto,
            komga::dto::KomgaSort,
            komga::dto::KomgaPageable,
            komga::dto::KomgaUserDto,
            komga::dto::KomgaContentRestrictionsDto,
            komga::dto::KomgaAgeRestrictionDto,
            komga::dto::KomgaSeriesSearchRequestDto,
            komga::dto::KomgaCollectionDto,
            komga::dto::KomgaReadListDto,
            komga::handlers::series::SeriesPaginationQuery,
            komga::handlers::books::BooksPaginationQuery,
        )
    ),
    tags(
        // Getting Started
        (name = "Health", description = "Health check endpoints"),
        (name = "Info", description = "Application info endpoints"),
        (name = "Setup", description = "Initial server setup and configuration"),

        // Authentication & Security
        (name = "Auth", description = "Authentication endpoints (login, logout, registration)"),
        (name = "API Keys", description = "API key management for programmatic access"),

        // Library Content
        (name = "Libraries", description = "Library management endpoints"),
        (name = "Series", description = "Series browsing and search endpoints"),
        (name = "Books", description = "Book details and metadata endpoints"),
        (name = "Pages", description = "Page image serving endpoints"),

        // Metadata & Taxonomy
        (name = "Genres", description = "Genre taxonomy for categorizing series"),
        (name = "Tags", description = "Tag taxonomy for labeling series"),
        (name = "Ratings", description = "User series ratings"),

        // User Features
        (name = "Users", description = "User management (admin only)"),
        (name = "User Preferences", description = "Per-user settings and preferences"),
        (name = "Reading Progress", description = "Reading progress tracking"),

        // Background Jobs
        (name = "Task Queue", description = "Background job queue management"),
        (name = "Scans", description = "Library scanning and analysis"),
        (name = "Thumbnails", description = "Thumbnail generation"),

        // System Administration
        (name = "Admin", description = "Administrative operations (cleanup, maintenance)"),
        (name = "Settings", description = "Runtime configuration settings (admin only)"),
        (name = "Plugins", description = "Admin-managed external plugin processes"),
        (name = "Plugin Actions", description = "Plugin action discovery and execution for metadata fetching"),
        (name = "Library Jobs", description = "Per-library scheduled jobs (metadata refresh today; future: scan, cleanup). Supports CRUD, run-now, and dry-run preview."),
        (name = "User Plugins", description = "User-facing plugin management, OAuth, and configuration"),
        (name = "Recommendations", description = "Personalized recommendation endpoints"),
        (name = "Metrics", description = "Application metrics and statistics"),
        (name = "Filesystem", description = "Filesystem browsing for library paths"),
        (name = "Duplicates", description = "Duplicate book detection and management"),
        (name = "Sharing Tags", description = "Content access control tags (admin only)"),

        // Real-time Events
        (name = "Events", description = "Server-Sent Events for real-time updates"),

        // OPDS Catalog Feeds
        (name = "OPDS", description = "OPDS 1.2 catalog (Atom XML) - Compatible with most e-readers"),
        (name = "OPDS 2.0", description = "OPDS 2.0 catalog (JSON) - Modern JSON-based format"),

        // Third-Party Compatibility
        (name = "Komga", description = "Komga-compatible API for third-party apps (Komic, etc.)"),
    ),
    modifiers(&SecurityAddon, &OperationIdPrefixer, &TagGroupsModifier),
)]
pub struct ApiDoc;

/// Security scheme definitions for JWT and API Key authentication
struct SecurityAddon;

impl utoipa::Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        if let Some(components) = openapi.components.as_mut() {
            components.add_security_scheme(
                "jwt_bearer",
                utoipa::openapi::security::SecurityScheme::Http(
                    utoipa::openapi::security::HttpBuilder::new()
                        .scheme(utoipa::openapi::security::HttpAuthScheme::Bearer)
                        .bearer_format("JWT")
                        .description(Some("JWT token obtained from /api/v1/auth/login"))
                        .build(),
                ),
            );

            components.add_security_scheme(
                "api_key",
                utoipa::openapi::security::SecurityScheme::ApiKey(
                    utoipa::openapi::security::ApiKey::Header(
                        utoipa::openapi::security::ApiKeyValue::new("X-API-Key"),
                    ),
                ),
            );
        }
    }
}

/// Modifier that prefixes operation IDs for non-v1 APIs to avoid duplicates.
///
/// This automatically prefixes operation IDs based on the path:
/// - `/opds/v1/` paths get `opds_` prefix
/// - `/opds/v2/` paths get `opds2_` prefix
/// - `/{prefix}/api/v1/` paths (Komga) get `komga_` prefix
struct OperationIdPrefixer;

impl utoipa::Modify for OperationIdPrefixer {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        for (path, item) in openapi.paths.paths.iter_mut() {
            let prefix = if path.starts_with("/opds/v2") {
                Some("opds2_")
            } else if path.starts_with("/opds/") {
                Some("opds_")
            } else if path.starts_with("/{prefix}") && path.contains("/api/v1/") {
                // Komga paths have /{prefix}/api/v1/ pattern
                Some("komga_")
            } else {
                None
            };

            if let Some(prefix) = prefix {
                // Prefix operation IDs for all HTTP methods
                for operation in [
                    &mut item.get,
                    &mut item.put,
                    &mut item.post,
                    &mut item.delete,
                    &mut item.options,
                    &mut item.head,
                    &mut item.patch,
                    &mut item.trace,
                ]
                .into_iter()
                .flatten()
                {
                    if let Some(ref op_id) = operation.operation_id {
                        operation.operation_id = Some(format!("{}{}", prefix, op_id));
                    }
                }
            }
        }
    }
}

/// Modifier that adds x-tagGroups extension for better API documentation organization.
///
/// This groups related tags together in the Scalar UI sidebar, making the API
/// easier to navigate. Supported by Scalar, Redoc, and other OpenAPI viewers.
struct TagGroupsModifier;

impl utoipa::Modify for TagGroupsModifier {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        use serde_json::json;

        // Define tag groups for organized navigation
        let tag_groups = json!([
            {
                "name": "Getting Started",
                "tags": ["Health", "Setup"]
            },
            {
                "name": "Authentication",
                "tags": ["Auth", "API Keys"]
            },
            {
                "name": "Library Content",
                "tags": ["Libraries", "Series", "Books", "Pages"]
            },
            {
                "name": "Metadata & Taxonomy",
                "tags": ["Genres", "Tags", "Ratings"]
            },
            {
                "name": "User Features",
                "tags": ["Users", "User Preferences", "User Plugins", "Recommendations", "Reading Progress"]
            },
            {
                "name": "Background Jobs",
                "tags": ["Task Queue", "Scans", "Thumbnails"]
            },
            {
                "name": "Administration",
                "tags": ["Admin", "Settings", "Plugins", "Plugin Actions", "Metrics", "Filesystem", "Duplicates", "Sharing Tags"]
            },
            {
                "name": "Real-time Events",
                "tags": ["Events"]
            },
            {
                "name": "OPDS Catalog",
                "tags": ["OPDS", "OPDS 2.0"]
            },
            {
                "name": "Third-Party Compatibility",
                "tags": ["Komga"]
            }
        ]);

        // Add x-tagGroups extension to the OpenAPI spec
        if openapi.extensions.is_none() {
            openapi.extensions = Some(Default::default());
        }
        if let Some(extensions) = openapi.extensions.as_mut() {
            extensions.insert("x-tagGroups".to_string(), tag_groups);
        }
    }
}
