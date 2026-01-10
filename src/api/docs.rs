use crate::api::{dto, error::ErrorResponse, handlers};
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
        description = "REST API for Codex digital library server",
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
        handlers::get_library,
        handlers::update_library,
        handlers::delete_library,
        handlers::purge_deleted_books,

        // Series endpoints
        handlers::list_series,
        handlers::search_series,
        handlers::get_series,
        handlers::get_series_books,
        handlers::purge_series_deleted_books,
        handlers::get_series_thumbnail,
        handlers::upload_series_cover,
        handlers::set_series_cover_source,
        handlers::trigger_series_analysis,
        handlers::list_started_series,
        handlers::list_library_series,
        handlers::list_library_started_series,

        // Book endpoints
        handlers::list_books,
        handlers::get_book,
        handlers::get_book_thumbnail,
        handlers::trigger_book_analysis,
        handlers::list_library_books,
        handlers::list_in_progress_books,
        handlers::list_library_in_progress_books,
        handlers::list_recently_added_books,
        handlers::list_library_recently_added_books,

        // Page endpoints
        handlers::get_page_image,

        // Reading progress endpoints
        handlers::update_reading_progress,
        handlers::get_reading_progress,
        handlers::delete_reading_progress,
        handlers::get_user_progress,
        handlers::get_currently_reading,
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
        handlers::get_metrics,

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

        // Filesystem endpoints
        handlers::browse_filesystem,
        handlers::list_drives,

        // Settings endpoints
        handlers::settings::list_settings,
        handlers::settings::get_setting,
        handlers::settings::update_setting,
        handlers::settings::bulk_update_settings,
        handlers::settings::reset_setting,
        handlers::settings::get_setting_history,

        // Duplicates endpoints
        handlers::duplicates::list_duplicates,
        handlers::duplicates::trigger_duplicate_scan,
        handlers::duplicates::delete_duplicate_group,

        // SSE streaming endpoints
        handlers::events::entity_events_stream,
        handlers::events::task_progress_stream,

        // OPDS catalog endpoints
        handlers::opds::catalog::root_catalog,
        handlers::opds::catalog::opds_list_libraries,
        handlers::opds::catalog::opds_library_series,
        handlers::opds::catalog::opds_series_books,
        handlers::opds::search::opensearch_descriptor,
        handlers::opds::search::opds_search,
        handlers::opds::pse::opds_book_pages,
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
            dto::SeriesDto,
            dto::SeriesListResponse,
            dto::SearchSeriesRequest,
            dto::BookDto,
            dto::BookListResponse,
            dto::BookDetailResponse,
            dto::BookMetadataDto,
            dto::PageDto,
            dto::UserDto,
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
            dto::UpdateSettingRequest,
            dto::BulkUpdateSettingsRequest,
            dto::BulkSettingUpdate,
            dto::SettingHistoryDto,
            dto::ListSettingsQuery,

            // Task Queue DTOs
            handlers::task_queue::CreateTaskRequest,
            handlers::task_queue::CreateTaskResponse,
            handlers::task_queue::TaskResponse,
            handlers::task_queue::PurgeTasksResponse,
            handlers::task_queue::MessageResponse,
            crate::tasks::types::TaskStats,
            crate::tasks::types::TaskTypeStats,
            crate::tasks::types::TaskType,

            // Duplicates DTOs
            dto::DuplicateGroup,
            dto::ListDuplicatesResponse,
            dto::TriggerDuplicateScanResponse,

            // SSE Event DTOs
            crate::events::EntityChangeEvent,
            crate::events::EntityEvent,
            crate::events::TaskProgressEvent,

            // Error responses
            ErrorResponse,
        )
    ),
    tags(
        (name = "health", description = "Health check endpoints"),
        (name = "auth", description = "Authentication endpoints"),
        (name = "libraries", description = "Library management endpoints"),
        (name = "series", description = "Series browsing and search endpoints"),
        (name = "books", description = "Book details and metadata endpoints"),
        (name = "pages", description = "Page image serving endpoints"),
        (name = "Reading Progress", description = "Reading progress tracking endpoints"),
        (name = "users", description = "User management endpoints (admin only)"),
        (name = "api-keys", description = "API key management endpoints"),
        (name = "Metrics", description = "Application metrics and statistics"),
        (name = "Scans", description = "Library scanning and analysis endpoints"),
        (name = "Task Queue", description = "Distributed task queue for background jobs (analysis, thumbnails, scans)"),
        (name = "filesystem", description = "Filesystem browsing for library path selection"),
        (name = "settings", description = "Runtime configuration settings management (admin only)"),
        (name = "duplicates", description = "Duplicate book detection and management"),
        (name = "events", description = "Server-Sent Events for real-time updates"),
        (name = "opds", description = "OPDS catalog feed for e-reader applications"),
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
