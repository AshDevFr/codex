use crate::api::{error::ErrorResponse, routes::v1::dto, routes::v1::handlers};
use utoipa::OpenApi;

/// OpenAPI documentation for Codex REST API
///
/// This struct aggregates all API endpoints, DTOs, and security schemes
/// for automatic API documentation generation (Scalar).
#[derive(OpenApi)]
#[openapi(
    info(
        title = "Codex API",
        version = "1.0.0",
        description = r#"REST API for Codex, a digital library server for comics, manga, and ebooks.

## Interactive API Documentation

You can explore and test this API interactively:

- **Hosted Documentation**: Visit [codex.4sh.dev/docs/api](https://codex.4sh.dev/docs/api) for the full API reference
- **Your Instance**: If you have Scalar UI enabled, access `/api/docs` on your Codex server

## Authentication

Most endpoints require authentication. Codex supports two authentication methods:

1. **JWT Bearer Token**: Obtain a token via `POST /api/v1/auth/login`, then include it as `Authorization: Bearer <token>`
2. **API Key**: Generate an API key in the web UI or via the API, then include it as `X-API-Key: <key>` header

## OPDS Support

Codex provides OPDS catalog feeds for e-reader applications:

- **OPDS 1.2** (Atom XML): `/opds/v1/catalog` - Compatible with most e-readers
- **OPDS 2.0** (JSON): `/opds/v2` - Modern JSON-based format with enhanced features"#,
        license(
            name = "MIT",
            url = "https://opensource.org/licenses/MIT"
        )
    ),
    paths(
        // Health check
        handlers::health_check,

        // Setup endpoints
        handlers::setup::setup_status,
        handlers::setup::initialize_setup,
        handlers::setup::configure_initial_settings,

        // Auth endpoints
        handlers::login,
        handlers::logout,
        handlers::register,
        handlers::verify_email,
        handlers::resend_verification,

        // Library endpoints
        handlers::list_libraries,
        handlers::create_library,
        handlers::preview_scan,
        handlers::get_library,
        handlers::update_library,
        handlers::delete_library,
        handlers::purge_deleted_books,

        // Series endpoints
        handlers::list_series,
        handlers::search_series,
        handlers::list_series_filtered,
        handlers::get_series,
        handlers::get_series_books,
        handlers::purge_series_deleted_books,
        handlers::get_series_thumbnail,
        handlers::upload_series_cover,
        handlers::set_series_cover_source,
        handlers::trigger_series_analysis,
        handlers::list_in_progress_series,
        handlers::list_recently_added_series,
        handlers::list_recently_updated_series,
        handlers::list_library_series,
        handlers::list_library_in_progress_series,
        handlers::list_library_recently_added_series,
        handlers::list_library_recently_updated_series,
        handlers::download_series,
        handlers::replace_series_metadata,
        handlers::patch_series_metadata,
        handlers::get_full_series_metadata,
        handlers::get_metadata_locks,
        handlers::update_metadata_locks,

        // Genre endpoints
        handlers::list_genres,
        handlers::get_series_genres,
        handlers::set_series_genres,
        handlers::add_series_genre,
        handlers::remove_series_genre,
        handlers::delete_genre,
        handlers::cleanup_genres,

        // Tag endpoints
        handlers::list_tags,
        handlers::get_series_tags,
        handlers::set_series_tags,
        handlers::add_series_tag,
        handlers::remove_series_tag,
        handlers::delete_tag,
        handlers::cleanup_tags,

        // User rating endpoints
        handlers::get_series_rating,
        handlers::set_series_rating,
        handlers::delete_series_rating,
        handlers::list_user_ratings,

        // User preferences endpoints
        handlers::user_preferences::get_all_preferences,
        handlers::user_preferences::get_preference,
        handlers::user_preferences::set_preference,
        handlers::user_preferences::set_bulk_preferences,
        handlers::user_preferences::delete_preference,

        // User integrations endpoints
        handlers::user_integrations::list_user_integrations,
        handlers::user_integrations::get_user_integration,
        handlers::user_integrations::connect_integration,
        handlers::user_integrations::oauth_callback,
        handlers::user_integrations::update_integration_settings,
        handlers::user_integrations::disconnect_integration,
        handlers::user_integrations::trigger_sync,

        // Alternate title endpoints
        handlers::get_series_alternate_titles,
        handlers::create_alternate_title,
        handlers::update_alternate_title,
        handlers::delete_alternate_title,

        // External rating endpoints
        handlers::get_series_external_ratings,
        handlers::create_external_rating,
        handlers::delete_external_rating,

        // Average rating endpoint
        handlers::get_series_average_rating,

        // External link endpoints
        handlers::get_series_external_links,
        handlers::create_external_link,
        handlers::delete_external_link,

        // Cover management endpoints
        handlers::list_series_covers,
        handlers::get_series_cover_image,
        handlers::select_series_cover,
        handlers::reset_series_cover,
        handlers::delete_series_cover,

        // Book endpoints
        handlers::list_books,
        handlers::list_books_filtered,
        handlers::get_book,
        handlers::get_adjacent_books,
        handlers::get_book_file,
        handlers::get_book_thumbnail,
        handlers::trigger_book_analysis,
        handlers::list_library_books,
        handlers::list_in_progress_books,
        handlers::list_library_in_progress_books,
        handlers::list_on_deck_books,
        handlers::list_library_on_deck_books,
        handlers::list_recently_added_books,
        handlers::list_library_recently_added_books,
        handlers::list_recently_read_books,
        handlers::list_library_recently_read_books,
        handlers::list_books_with_errors,
        handlers::list_library_books_with_errors,
        handlers::list_series_books_with_errors,
        handlers::replace_book_metadata,
        handlers::patch_book_metadata,

        // Page endpoints
        handlers::get_page_image,

        // Reading progress endpoints
        handlers::update_reading_progress,
        handlers::get_reading_progress,
        handlers::delete_reading_progress,
        handlers::get_user_progress,
        handlers::mark_book_as_read,
        handlers::mark_book_as_unread,
        handlers::mark_series_as_read,
        handlers::mark_series_as_unread,

        // User endpoints
        handlers::list_users,
        handlers::create_user,
        handlers::get_user,
        handlers::update_user,
        handlers::delete_user,

        // API key endpoints
        handlers::api_keys::list_api_keys,
        handlers::api_keys::create_api_key,
        handlers::api_keys::get_api_key,
        handlers::api_keys::update_api_key,
        handlers::api_keys::delete_api_key,

        // Metrics endpoints
        handlers::get_inventory_metrics,
        handlers::task_metrics::get_task_metrics,
        handlers::task_metrics::get_task_metrics_history,
        handlers::task_metrics::trigger_metrics_cleanup,
        handlers::task_metrics::nuke_task_metrics,

        // Scan endpoints
        handlers::trigger_scan,
        handlers::get_scan_status,
        handlers::cancel_scan,
        handlers::list_active_scans,
        handlers::scan_progress_stream,
        handlers::trigger_library_analysis,
        handlers::trigger_library_unanalyzed_analysis,
        handlers::trigger_series_unanalyzed_analysis,
        handlers::trigger_book_unanalyzed_analysis,

        // Task Queue endpoints
        handlers::task_queue::list_tasks,
        handlers::task_queue::create_task,
        handlers::task_queue::get_task,
        handlers::task_queue::cancel_task,
        handlers::task_queue::unlock_task,
        handlers::task_queue::retry_task,
        handlers::task_queue::get_task_stats,
        handlers::task_queue::purge_old_tasks,
        handlers::task_queue::nuke_all_tasks,
        handlers::task_queue::generate_thumbnails,
        handlers::task_queue::generate_library_thumbnails,
        handlers::task_queue::generate_series_thumbnails,
        handlers::task_queue::generate_book_thumbnail,

        // Filesystem endpoints
        handlers::browse_filesystem,
        handlers::list_drives,

        // Settings endpoints
        handlers::settings::get_branding_settings,
        handlers::settings::get_public_settings,
        handlers::settings::list_settings,
        handlers::settings::get_setting,
        handlers::settings::update_setting,
        handlers::settings::bulk_update_settings,
        handlers::settings::reset_setting,
        handlers::settings::get_setting_history,

        // System integrations endpoints
        handlers::system_integrations::list_system_integrations,
        handlers::system_integrations::create_system_integration,
        handlers::system_integrations::get_system_integration,
        handlers::system_integrations::update_system_integration,
        handlers::system_integrations::delete_system_integration,
        handlers::system_integrations::enable_system_integration,
        handlers::system_integrations::disable_system_integration,
        handlers::system_integrations::test_system_integration,

        // Sharing Tags endpoints
        handlers::sharing_tags::list_sharing_tags,
        handlers::sharing_tags::get_sharing_tag,
        handlers::sharing_tags::create_sharing_tag,
        handlers::sharing_tags::update_sharing_tag,
        handlers::sharing_tags::delete_sharing_tag,
        handlers::sharing_tags::get_series_sharing_tags,
        handlers::sharing_tags::set_series_sharing_tags,
        handlers::sharing_tags::add_series_sharing_tag,
        handlers::sharing_tags::remove_series_sharing_tag,
        handlers::sharing_tags::get_user_sharing_tags,
        handlers::sharing_tags::set_user_sharing_tag,
        handlers::sharing_tags::remove_user_sharing_tag,
        handlers::sharing_tags::get_my_sharing_tags,

        // Cleanup endpoints
        handlers::cleanup::get_orphan_stats,
        handlers::cleanup::trigger_cleanup,
        handlers::cleanup::delete_orphans,

        // PDF cache management endpoints
        handlers::pdf_cache::get_pdf_cache_stats,
        handlers::pdf_cache::trigger_pdf_cache_cleanup,
        handlers::pdf_cache::clear_pdf_cache,

        // Duplicates endpoints
        handlers::duplicates::list_duplicates,
        handlers::duplicates::trigger_duplicate_scan,
        handlers::duplicates::delete_duplicate_group,

        // SSE streaming endpoints
        handlers::events::entity_events_stream,
        handlers::events::task_progress_stream,

        // OPDS 1.2 catalog endpoints (XML format)
        crate::api::routes::opds::handlers::catalog::root_catalog,
        crate::api::routes::opds::handlers::catalog::opds_list_libraries,
        crate::api::routes::opds::handlers::catalog::opds_library_series,
        crate::api::routes::opds::handlers::catalog::opds_series_books,
        crate::api::routes::opds::handlers::search::opensearch_descriptor,
        crate::api::routes::opds::handlers::search::opds_search,
        crate::api::routes::opds::handlers::pse::opds_book_pages,

        // OPDS 2.0 catalog endpoints (JSON format)
        crate::api::routes::opds2::handlers::catalog::opds2_root,
        crate::api::routes::opds2::handlers::catalog::opds2_libraries,
        crate::api::routes::opds2::handlers::catalog::opds2_library_series,
        crate::api::routes::opds2::handlers::catalog::opds2_series_books,
        crate::api::routes::opds2::handlers::catalog::opds2_recent,
        crate::api::routes::opds2::handlers::search::opds2_search,
    ),
    components(
        schemas(
            // DTOs
            dto::LoginRequest,
            dto::LoginResponse,
            dto::RegisterRequest,
            dto::RegisterResponse,
            dto::VerifyEmailRequest,
            dto::VerifyEmailResponse,
            dto::ResendVerificationRequest,
            dto::ResendVerificationResponse,
            dto::TokenResponse,

            // Setup DTOs
            dto::SetupStatusResponse,
            dto::InitializeSetupRequest,
            dto::InitializeSetupResponse,
            dto::ConfigureSettingsRequest,
            dto::ConfigureSettingsResponse,
            dto::LibraryDto,
            dto::CreateLibraryRequest,
            dto::UpdateLibraryRequest,
            dto::PreviewScanRequest,
            dto::PreviewScanResponse,
            dto::DetectedSeriesDto,
            dto::DetectedSeriesMetadataDto,

            // Strategy types
            crate::models::SeriesStrategy,
            crate::models::BookStrategy,
            crate::models::FlatStrategyConfig,
            crate::models::PublisherHierarchyConfig,
            crate::models::CalibreStrategyConfig,
            crate::models::CalibreSeriesMode,
            crate::models::CustomStrategyConfig,
            crate::models::SmartBookConfig,
            dto::SeriesDto,
            dto::SeriesListResponse,
            dto::SearchSeriesRequest,
            dto::SeriesListRequest,
            dto::SeriesCondition,
            dto::BookListRequest,
            dto::BookCondition,
            dto::FieldOperator,
            dto::UuidOperator,
            dto::BoolOperator,
            dto::ReplaceSeriesMetadataRequest,
            dto::PatchSeriesMetadataRequest,
            dto::SeriesMetadataResponse,
            dto::FullSeriesMetadataResponse,
            dto::MetadataLocks,
            dto::UpdateMetadataLocksRequest,

            // Genre DTOs
            dto::GenreDto,
            dto::GenreListResponse,
            dto::SetSeriesGenresRequest,
            dto::AddSeriesGenreRequest,
            dto::TaxonomyCleanupResponse,

            // Tag DTOs
            dto::TagDto,
            dto::TagListResponse,
            dto::SetSeriesTagsRequest,
            dto::AddSeriesTagRequest,

            // User Rating DTOs
            dto::UserSeriesRatingDto,
            dto::UserRatingsListResponse,
            dto::SetUserRatingRequest,

            // User Preferences DTOs
            dto::UserPreferenceDto,
            dto::UserPreferencesResponse,
            dto::SetPreferenceRequest,
            dto::BulkSetPreferencesRequest,
            dto::SetPreferencesResponse,
            dto::DeletePreferenceResponse,

            // Alternate Title DTOs
            dto::AlternateTitleDto,
            dto::AlternateTitleListResponse,
            dto::CreateAlternateTitleRequest,
            dto::UpdateAlternateTitleRequest,

            // External Rating DTOs
            dto::ExternalRatingDto,
            dto::ExternalRatingListResponse,
            dto::CreateExternalRatingRequest,

            // Average Rating DTOs
            dto::SeriesAverageRatingResponse,

            // External Link DTOs
            dto::ExternalLinkDto,
            dto::ExternalLinkListResponse,
            dto::CreateExternalLinkRequest,

            // Cover DTOs
            dto::SeriesCoverDto,
            dto::SeriesCoverListResponse,

            // Sharing Tag DTOs
            dto::SharingTagDto,
            dto::SharingTagSummaryDto,
            dto::SharingTagListResponse,
            dto::CreateSharingTagRequest,
            dto::UpdateSharingTagRequest,
            dto::SetSeriesSharingTagsRequest,
            dto::ModifySeriesSharingTagRequest,
            dto::UserSharingTagGrantDto,
            dto::SetUserSharingTagGrantRequest,
            dto::UserSharingTagGrantsResponse,
            crate::db::entities::user_sharing_tags::AccessMode,

            dto::BookDto,
            dto::BookListResponse,
            dto::BookDetailResponse,
            dto::AdjacentBooksResponse,
            dto::BookMetadataDto,
            dto::ReplaceBookMetadataRequest,
            dto::PatchBookMetadataRequest,
            dto::BookMetadataResponse,
            dto::PageDto,
            dto::UserDto,
            dto::UserDetailDto,
            dto::CreateUserRequest,
            dto::UpdateUserRequest,
            dto::ApiKeyDto,
            dto::CreateApiKeyRequest,
            dto::CreateApiKeyResponse,
            dto::UpdateApiKeyRequest,
            dto::PaginatedResponse<dto::SeriesDto>,
            dto::PaginatedResponse<dto::BookDto>,
            dto::PaginatedResponse<dto::UserDto>,

            // Metrics DTOs
            dto::MetricsDto,
            dto::LibraryMetricsDto,

            // Task Metrics DTOs
            dto::TaskMetricsResponse,
            dto::TaskMetricsSummaryDto,
            dto::TaskTypeMetricsDto,
            dto::QueueHealthMetricsDto,
            dto::TaskMetricsHistoryResponse,
            dto::TaskMetricsDataPointDto,
            dto::MetricsCleanupResponse,
            dto::MetricsNukeResponse,

            // Scan DTOs
            dto::ScanStatusDto,
            dto::TriggerScanQuery,
            dto::ScanningConfigDto,
            dto::AnalysisResult,

            // Reading progress DTOs
            dto::UpdateProgressRequest,
            dto::ReadProgressResponse,
            dto::ReadProgressListResponse,
            dto::MarkReadResponse,

            // Filesystem DTOs
            handlers::filesystem::FileSystemEntry,
            handlers::filesystem::BrowseResponse,

            // Settings DTOs
            dto::SettingDto,
            dto::PublicSettingDto,
            dto::BrandingSettingsDto,
            dto::UpdateSettingRequest,
            dto::BulkUpdateSettingsRequest,
            dto::BulkSettingUpdate,
            dto::SettingHistoryDto,
            dto::ListSettingsQuery,

            // System Integrations DTOs
            dto::SystemIntegrationDto,
            dto::SystemIntegrationsListResponse,
            dto::CreateSystemIntegrationRequest,
            dto::UpdateSystemIntegrationRequest,
            dto::IntegrationTestResult,
            dto::IntegrationStatusResponse,

            // Task Queue DTOs
            handlers::task_queue::CreateTaskRequest,
            handlers::task_queue::CreateTaskResponse,
            handlers::task_queue::TaskResponse,
            handlers::task_queue::PurgeTasksResponse,
            handlers::task_queue::MessageResponse,
            handlers::task_queue::GenerateThumbnailsRequest,
            handlers::task_queue::ForceRequest,
            crate::tasks::types::TaskStats,
            crate::tasks::types::TaskTypeStats,
            crate::tasks::types::TaskType,

            // Duplicates DTOs
            dto::DuplicateGroup,
            dto::ListDuplicatesResponse,
            dto::TriggerDuplicateScanResponse,

            // Cleanup DTOs
            dto::OrphanStatsDto,
            dto::OrphanedFileDto,
            dto::CleanupResultDto,
            dto::TriggerCleanupResponse,
            dto::OrphanStatsQuery,

            // PDF Cache DTOs
            dto::PdfCacheStatsDto,
            dto::PdfCacheCleanupResultDto,
            dto::TriggerPdfCacheCleanupResponse,

            // SSE Event DTOs
            crate::events::EntityChangeEvent,
            crate::events::EntityEvent,
            crate::events::TaskProgressEvent,

            // Error responses
            ErrorResponse,

            // OPDS 2.0 DTOs
            crate::api::routes::opds2::dto::Opds2Feed,
            crate::api::routes::opds2::dto::Opds2Link,
            crate::api::routes::opds2::dto::LinkProperties,
            crate::api::routes::opds2::dto::FeedMetadata,
            crate::api::routes::opds2::dto::PublicationMetadata,
            crate::api::routes::opds2::dto::Publication,
            crate::api::routes::opds2::dto::ImageLink,
            crate::api::routes::opds2::dto::Contributor,
            crate::api::routes::opds2::dto::BelongsTo,
            crate::api::routes::opds2::dto::SeriesInfo,
            crate::api::routes::opds2::dto::Group,
            crate::api::routes::opds2::dto::ReadingProgress,
        )
    ),
    tags(
        (name = "health", description = "Health check endpoints"),
        (name = "auth", description = "Authentication endpoints"),
        (name = "libraries", description = "Library management endpoints"),
        (name = "series", description = "Series browsing and search endpoints"),
        (name = "genres", description = "Genre taxonomy endpoints for categorizing series"),
        (name = "tags", description = "Tag taxonomy endpoints for labeling series"),
        (name = "ratings", description = "User series rating endpoints"),
        (name = "User Preferences", description = "Per-user settings and preferences"),
        (name = "books", description = "Book details and metadata endpoints"),
        (name = "pages", description = "Page image serving endpoints"),
        (name = "Reading Progress", description = "Reading progress tracking endpoints"),
        (name = "users", description = "User management endpoints (admin only)"),
        (name = "api-keys", description = "API key management endpoints"),
        (name = "Metrics", description = "Application metrics and statistics"),
        (name = "Scans", description = "Library scanning and analysis endpoints"),
        (name = "Task Queue", description = "Distributed task queue for background jobs (analysis, thumbnails, scans)"),
        (name = "Thumbnails", description = "Thumbnail generation and management"),
        (name = "filesystem", description = "Filesystem browsing for library path selection"),
        (name = "settings", description = "Runtime configuration settings management (admin only)"),
        (name = "System Integrations", description = "Admin-managed external service integrations"),
        (name = "duplicates", description = "Duplicate book detection and management"),
        (name = "Admin", description = "Administrative operations (cleanup, maintenance)"),
        (name = "sharing-tags", description = "Sharing tags for content access control (admin only)"),
        (name = "events", description = "Server-Sent Events for real-time updates"),
        (name = "opds", description = "OPDS 1.2 catalog feed (Atom XML format)"),
        (name = "opds2", description = "OPDS 2.0 catalog feed (JSON format) - Modern JSON-based OPDS specification"),
    ),
    modifiers(&SecurityAddon),
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
