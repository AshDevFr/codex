//! Application info handler

use std::sync::Arc;

use axum::{Json, extract::State};

use super::super::dto::AppInfoDto;
use crate::extractors::AppState;

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
pub async fn get_app_info(State(state): State<Arc<AppState>>) -> Json<AppInfoDto> {
    Json(AppInfoDto {
        version: state.app_version.to_string(),
        name: state.app_name.to_string(),
    })
}
