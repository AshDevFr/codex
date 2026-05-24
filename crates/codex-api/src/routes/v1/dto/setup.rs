use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use utoipa::ToSchema;

use super::auth::UserInfo;

/// Setup status response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SetupStatusResponse {
    /// Whether initial setup is required
    pub setup_required: bool,

    /// Whether any users exist in the database
    pub has_users: bool,

    /// Whether user registration is enabled
    pub registration_enabled: bool,
}

/// Initialize setup request - creates first admin user
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct InitializeSetupRequest {
    /// Username for the first admin user
    pub username: String,

    /// Email address for the first admin user
    pub email: String,

    /// Password for the first admin user
    pub password: String,
}

/// Initialize setup response - returns user and JWT token
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct InitializeSetupResponse {
    /// Created user information
    pub user: UserInfo,

    /// JWT access token
    pub access_token: String,

    /// Token type (always "Bearer")
    pub token_type: String,

    /// Token expiry in seconds
    pub expires_in: u64,

    /// Success message
    pub message: String,
}

/// Configure initial settings request
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ConfigureSettingsRequest {
    /// Settings to configure (key-value pairs)
    pub settings: HashMap<String, String>,

    /// Whether to skip settings configuration
    pub skip_configuration: bool,
}

/// Configure settings response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ConfigureSettingsResponse {
    /// Success message
    pub message: String,

    /// Number of settings configured
    pub settings_configured: usize,
}
