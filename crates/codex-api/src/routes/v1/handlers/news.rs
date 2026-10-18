//! News handlers: Codex release and app announcements for admins.

use std::sync::Arc;

use axum::{Json, extract::State, http::StatusCode};
use chrono::Utc;

use super::super::dto::{NewsEntryDto, NewsResponse, NewsUpdateAvailableDto};
use crate::error::ApiError;
use crate::extractors::{AppState, AuthContext};
use codex_services::news;

/// Get news for the calling admin
///
/// Codex releases and Codex Reader announcements, with what is unread for the
/// caller and whether a newer Codex release than the running one exists.
///
/// # Permission Required
/// - Admin access required
#[utoipa::path(
    get,
    path = "/api/v1/news",
    responses(
        (status = 200, description = "News for the calling admin", body = NewsResponse),
        (status = 403, description = "Admin access required"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "News"
)]
pub async fn get_news(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
) -> Result<Json<NewsResponse>, ApiError> {
    auth.require_admin()?;

    if !state.news_enabled {
        return Ok(Json(NewsResponse {
            enabled: false,
            last_fetched_at: None,
            unread_count: 0,
            update_available: None,
            entries: Vec::new(),
        }));
    }

    let view = news::load_news_view(&state.db, auth.user_id, state.app_version)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to load news: {e}")))?;

    Ok(Json(NewsResponse {
        enabled: true,
        last_fetched_at: view.last_fetched_at,
        unread_count: view.unread_count as u32,
        update_available: view.update_available.map(|latest| NewsUpdateAvailableDto {
            current_version: state.app_version.to_string(),
            latest_version: latest.version.unwrap_or_default(),
            url: latest.url,
        }),
        entries: view
            .entries
            .into_iter()
            .map(|(entry, unread)| NewsEntryDto {
                id: entry.id,
                project: entry.project,
                kind: entry.kind,
                title: entry.title,
                version: entry.version,
                summary: entry.summary,
                body: entry.body,
                url: entry.url,
                published_at: entry.published_at,
                unread,
            })
            .collect(),
    }))
}

/// Mark news as seen
///
/// Everything published so far stops counting as unread for the caller.
/// The web UI calls this when the news drawer opens.
///
/// # Permission Required
/// - Admin access required
#[utoipa::path(
    post,
    path = "/api/v1/news/seen",
    responses(
        (status = 204, description = "News marked as seen"),
        (status = 403, description = "Admin access required"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "News"
)]
pub async fn mark_news_seen(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
) -> Result<StatusCode, ApiError> {
    auth.require_admin()?;
    news::mark_seen(&state.db, auth.user_id, Utc::now())
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to mark news as seen: {e}")))?;
    Ok(StatusCode::NO_CONTENT)
}
