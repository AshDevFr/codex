//! Application info DTOs

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Application information response
#[derive(Debug, Serialize, Deserialize, ToSchema, Clone)]
pub struct AppInfoDto {
    /// Application version from Cargo.toml
    #[schema(example = "1.0.0")]
    pub version: String,

    /// Application name
    #[schema(example = "codex")]
    pub name: String,
}
