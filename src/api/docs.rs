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

        // Library endpoints
        handlers::list_libraries,
        handlers::create_library,
        handlers::get_library,
        handlers::update_library,
        handlers::delete_library,

        // Series endpoints
        handlers::list_series,
        handlers::search_series,
        handlers::get_series,

        // Book endpoints
        handlers::list_books,
        handlers::get_book,

        // Page endpoints
        handlers::get_page_image,

        // User endpoints
        handlers::list_users,
        handlers::create_user,
        handlers::get_user,
        handlers::update_user,
        handlers::delete_user,

        // Metrics endpoints
        handlers::get_metrics,

        // Task management endpoints
        handlers::list_tasks,
        handlers::get_task,
        handlers::cancel_task,
    ),
    components(
        schemas(
            // DTOs
            dto::LoginRequest,
            dto::LoginResponse,
            dto::TokenResponse,
            dto::LibraryDto,
            dto::CreateLibraryRequest,
            dto::UpdateLibraryRequest,
            dto::SeriesDto,
            dto::SeriesListResponse,
            dto::SearchSeriesRequest,
            dto::BookDto,
            dto::BookListResponse,
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

            // Task DTOs
            dto::TaskDto,
            dto::TaskProgressDto,

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
        (name = "users", description = "User management endpoints (admin only)"),
        (name = "Metrics", description = "Application metrics and statistics"),
        (name = "Tasks", description = "Background task management and monitoring"),
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
