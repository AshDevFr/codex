//! Application info handler

use axum::Json;

use super::super::dto::AppInfoDto;

/// Get application information
///
/// Returns the application name and version.
/// This endpoint is public (no authentication required).
#[utoipa::path(
    get,
    path = "/api/v1/info",
    responses(
        (status = 200, description = "Application info", body = AppInfoDto),
    ),
    tag = "Info"
)]
pub async fn get_app_info() -> Json<AppInfoDto> {
    Json(AppInfoDto {
        version: env!("CARGO_PKG_VERSION").to_string(),
        name: env!("CARGO_PKG_NAME").to_string(),
    })
}
