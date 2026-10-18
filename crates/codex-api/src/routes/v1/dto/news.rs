//! News DTOs: Codex release and app announcements shown to admins.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// The news drawer for the calling admin.
#[derive(Debug, Serialize, Deserialize, ToSchema, Clone)]
#[serde(rename_all = "camelCase")]
pub struct NewsResponse {
    /// False when the server has news turned off (`news.enabled`). The other
    /// fields are then empty and the UI should hide itself.
    pub enabled: bool,

    /// Most recent successful fetch of the news feeds, if any.
    #[schema(example = "2026-10-18T04:17:00Z")]
    pub last_fetched_at: Option<DateTime<Utc>>,

    /// Entries unread by the calling admin.
    #[schema(example = 1)]
    pub unread_count: u32,

    /// Set when a Codex release newer than the running one exists. Absent,
    /// not null, otherwise.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub update_available: Option<NewsUpdateAvailableDto>,

    /// Newest first, at most 50.
    pub entries: Vec<NewsEntryDto>,
}

/// A newer Codex release than the one this server runs.
#[derive(Debug, Serialize, Deserialize, ToSchema, Clone)]
#[serde(rename_all = "camelCase")]
pub struct NewsUpdateAvailableDto {
    #[schema(example = "2.6.3")]
    pub current_version: String,
    #[schema(example = "2.7.0")]
    pub latest_version: String,
    /// Release notes for the latest version, when the feed links them.
    #[schema(example = "https://github.com/AshDevFr/codex/releases/tag/v2.7.0")]
    pub url: Option<String>,
}

/// One news entry.
#[derive(Debug, Serialize, Deserialize, ToSchema, Clone)]
#[serde(rename_all = "camelCase")]
pub struct NewsEntryDto {
    /// Stable feed id.
    #[schema(example = "codex-v2.7.0")]
    pub id: String,
    /// Feed the entry came from: `codex` or `codex-ios`.
    #[schema(example = "codex")]
    pub project: String,
    /// `release` or `post`.
    #[schema(example = "release")]
    pub kind: String,
    #[schema(example = "Codex 2.7.0")]
    pub title: String,
    #[schema(example = "2.7.0")]
    pub version: Option<String>,
    /// Plain text, a few hundred characters at most.
    pub summary: String,
    /// Markdown. Render without raw HTML.
    pub body: String,
    pub url: Option<String>,
    pub published_at: DateTime<Utc>,
    /// Unread by the calling admin.
    pub unread: bool,
}
