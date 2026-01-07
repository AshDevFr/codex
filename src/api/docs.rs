use crate::api::{dto, error::ErrorResponse, handlers};
use utoipa::OpenApi;

/// OpenAPI documentation for Codex REST API
///
/// This struct aggregates all API endpoints, DTOs, and security schemes
/// for automatic Swagger UI generation.
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

        // Book endpoints
        handlers::list_books,
        handlers::get_book,
        handlers::get_book_thumbnail,
        handlers::trigger_book_analysis,

        // Page endpoints
        handlers::get_page_image,

        // Reading progress endpoints
        handlers::update_reading_progress,
        handlers::get_reading_progress,
        handlers::delete_reading_progress,
        handlers::get_user_progress,
        handlers::get_currently_reading,

        // User endpoints
        handlers::list_users,
        handlers::create_user,
        handlers::get_user,
        handlers::update_user,
        handlers::delete_user,

        // Metrics endpoints
        handlers::get_metrics,

        // Scan endpoints
        handlers::trigger_scan,
        handlers::get_scan_status,
        handlers::cancel_scan,
        handlers::trigger_analysis,
        handlers::list_active_scans,
        handlers::scan_progress_stream,

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

            // Filesystem DTOs
            handlers::filesystem::FileSystemEntry,
            handlers::filesystem::BrowseResponse,

            // Task Queue DTOs
            handlers::task_queue::CreateTaskRequest,
            handlers::task_queue::CreateTaskResponse,
            handlers::task_queue::TaskResponse,
            handlers::task_queue::PurgeTasksResponse,
            handlers::task_queue::MessageResponse,
            crate::tasks::types::TaskStats,
            crate::tasks::types::TaskTypeStats,
            crate::tasks::types::TaskType,

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
        (name = "Metrics", description = "Application metrics and statistics"),
        (name = "Scans", description = "Library scanning and analysis endpoints"),
        (name = "Task Queue", description = "Distributed task queue for background jobs (analysis, thumbnails, scans)"),
        (name = "filesystem", description = "Filesystem browsing for library path selection"),
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
