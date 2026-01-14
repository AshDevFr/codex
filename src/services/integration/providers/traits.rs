//! Integration Provider Traits
//!
//! Defines the traits for metadata providers and user sync providers.
//!
//! TODO: Remove allow(dead_code) once integration features are fully implemented

#![allow(dead_code)]

use anyhow::Result;
use axum::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Health status for an integration provider
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HealthStatus {
    /// Provider is working correctly
    Healthy,
    /// Provider is partially working
    Degraded { message: String },
    /// Provider is not working
    Unhealthy { message: String },
    /// Provider status is unknown
    Unknown,
}

impl HealthStatus {
    pub fn as_str(&self) -> &str {
        match self {
            HealthStatus::Healthy => "healthy",
            HealthStatus::Degraded { .. } => "degraded",
            HealthStatus::Unhealthy { .. } => "unhealthy",
            HealthStatus::Unknown => "unknown",
        }
    }
}

/// Error type for provider operations
#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    #[error("Authentication failed: {0}")]
    AuthenticationFailed(String),

    #[error("Rate limited: retry after {retry_after_seconds} seconds")]
    RateLimited { retry_after_seconds: u64 },

    #[error("Resource not found: {0}")]
    NotFound(String),

    #[error("Invalid request: {0}")]
    InvalidRequest(String),

    #[error("Network error: {0}")]
    NetworkError(String),

    #[error("Provider error: {0}")]
    Internal(String),

    #[error("Serialization error: {0}")]
    SerializationError(String),
}

/// Result of a metadata search
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetadataSearchResult {
    /// External ID from the provider
    pub external_id: String,
    /// Primary title
    pub title: String,
    /// Alternative titles
    pub alternate_titles: Vec<String>,
    /// Year of publication/release
    pub year: Option<i32>,
    /// Cover image URL
    pub cover_url: Option<String>,
    /// Relevance score (higher is more relevant)
    pub score: f64,
}

/// Full series metadata from a provider
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeriesMetadata {
    /// External ID from the provider
    pub external_id: String,
    /// Primary title
    pub title: String,
    /// Alternative titles with labels (e.g., ("English", "Naruto"))
    pub alternate_titles: Vec<(String, String)>,
    /// Summary/description
    pub summary: Option<String>,
    /// Publication status (e.g., "ongoing", "completed")
    pub status: Option<String>,
    /// Year of publication
    pub year: Option<i32>,
    /// Genres
    pub genres: Vec<String>,
    /// Tags
    pub tags: Vec<String>,
    /// Publisher name
    pub publisher: Option<String>,
    /// Cover image URL
    pub cover_url: Option<String>,
    /// URL to the series on the provider's website
    pub external_url: String,
    /// Average rating (0.0 - 10.0)
    pub rating: Option<f64>,
    /// Number of ratings
    pub rating_count: Option<i32>,
}

/// OAuth tokens for user authentication
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthTokens {
    /// Access token for API calls
    pub access_token: String,
    /// Refresh token for obtaining new access tokens
    pub refresh_token: Option<String>,
    /// Token type (usually "Bearer")
    pub token_type: String,
    /// When the access token expires
    pub expires_at: Option<DateTime<Utc>>,
    /// OAuth scope granted
    pub scope: Option<String>,
}

/// Reading progress for sync
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadingProgress {
    /// External ID of the series
    pub external_id: String,
    /// Number of chapters read
    pub chapters_read: Option<i32>,
    /// Number of volumes read
    pub volumes_read: Option<i32>,
    /// Reading status: "reading", "completed", "on_hold", "dropped", "plan_to_read"
    pub status: String,
    /// User's score (0.0 - 10.0)
    pub score: Option<f64>,
    /// When the user started reading
    pub started_at: Option<DateTime<Utc>>,
    /// When the user completed reading
    pub completed_at: Option<DateTime<Utc>>,
}

/// Trait for metadata provider integrations (MangaUpdates, AniList, etc.)
///
/// These providers fetch metadata for series at the system level.
#[async_trait]
pub trait MetadataProvider: Send + Sync {
    /// Provider name (e.g., "mangaupdates", "anilist")
    fn name(&self) -> &'static str;

    /// Display name for UI
    fn display_name(&self) -> &'static str;

    /// Search for series by title
    async fn search_series(&self, query: &str) -> Result<Vec<MetadataSearchResult>, ProviderError>;

    /// Get full metadata for a series by external ID
    async fn get_series_metadata(&self, external_id: &str)
        -> Result<SeriesMetadata, ProviderError>;

    /// Get cover image URL for a series
    async fn get_cover_url(&self, external_id: &str) -> Result<Option<String>, ProviderError>;

    /// Test the connection to the provider
    async fn test_connection(&self) -> Result<bool, ProviderError>;

    /// Check health status of the provider
    async fn health_check(&self) -> HealthStatus;
}

/// Trait for user sync integrations (AniList personal, MAL, etc.)
///
/// These providers sync reading progress and ratings for individual users.
#[async_trait]
pub trait UserSyncProvider: Send + Sync {
    /// Provider name
    fn name(&self) -> &'static str;

    /// Display name for UI
    fn display_name(&self) -> &'static str;

    /// Get OAuth authorization URL
    fn get_auth_url(&self, state: &str, redirect_uri: &str) -> String;

    /// Exchange OAuth authorization code for tokens
    async fn exchange_code(
        &self,
        code: &str,
        redirect_uri: &str,
    ) -> Result<OAuthTokens, ProviderError>;

    /// Refresh access token
    async fn refresh_token(&self, refresh_token: &str) -> Result<OAuthTokens, ProviderError>;

    /// Get user info from the provider
    async fn get_user_info(&self, tokens: &OAuthTokens) -> Result<(String, String), ProviderError>; // (user_id, username)

    /// Push reading progress to external service
    async fn push_progress(
        &self,
        tokens: &OAuthTokens,
        progress: &ReadingProgress,
    ) -> Result<(), ProviderError>;

    /// Pull reading progress from external service
    async fn pull_progress(
        &self,
        tokens: &OAuthTokens,
        external_id: &str,
    ) -> Result<Option<ReadingProgress>, ProviderError>;

    /// Push rating to external service
    async fn push_rating(
        &self,
        tokens: &OAuthTokens,
        external_id: &str,
        rating: f64,
    ) -> Result<(), ProviderError>;

    /// Import user's library/lists
    async fn import_library(
        &self,
        tokens: &OAuthTokens,
    ) -> Result<Vec<ReadingProgress>, ProviderError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_status_as_str() {
        assert_eq!(HealthStatus::Healthy.as_str(), "healthy");
        assert_eq!(
            HealthStatus::Degraded {
                message: "slow".to_string()
            }
            .as_str(),
            "degraded"
        );
        assert_eq!(
            HealthStatus::Unhealthy {
                message: "down".to_string()
            }
            .as_str(),
            "unhealthy"
        );
        assert_eq!(HealthStatus::Unknown.as_str(), "unknown");
    }

    #[test]
    fn test_provider_error_display() {
        let err = ProviderError::RateLimited {
            retry_after_seconds: 60,
        };
        assert!(err.to_string().contains("60 seconds"));

        let err = ProviderError::NotFound("Series not found".to_string());
        assert!(err.to_string().contains("Series not found"));
    }
}
