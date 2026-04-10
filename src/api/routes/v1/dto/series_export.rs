//! DTOs for series export endpoints

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

/// Request body for creating a new series export
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateSeriesExportRequest {
    /// Export format: "json" or "csv"
    pub format: String,
    /// Library IDs to include in the export
    pub library_ids: Vec<Uuid>,
    /// Field keys to include (from the field catalog)
    pub fields: Vec<String>,
}

/// Response DTO for a series export record
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SeriesExportDto {
    pub id: Uuid,
    pub format: String,
    pub status: String,
    pub library_ids: Vec<Uuid>,
    pub fields: Vec<String>,
    pub file_size_bytes: Option<i64>,
    pub row_count: Option<i32>,
    pub error: Option<String>,
    pub created_at: String,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub expires_at: String,
}

impl SeriesExportDto {
    pub fn from_model(m: &crate::db::entities::series_exports::Model) -> Self {
        let library_ids: Vec<Uuid> =
            serde_json::from_value(m.library_ids.clone()).unwrap_or_default();
        let fields: Vec<String> = serde_json::from_value(m.fields.clone()).unwrap_or_default();

        Self {
            id: m.id,
            format: m.format.clone(),
            status: m.status.clone(),
            library_ids,
            fields,
            file_size_bytes: m.file_size_bytes,
            row_count: m.row_count,
            error: m.error.clone(),
            created_at: m.created_at.to_rfc3339(),
            started_at: m.started_at.map(|t| t.to_rfc3339()),
            completed_at: m.completed_at.map(|t| t.to_rfc3339()),
            expires_at: m.expires_at.to_rfc3339(),
        }
    }
}

/// Response for listing exports
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SeriesExportListResponse {
    pub exports: Vec<SeriesExportDto>,
}

/// DTO describing a single exportable field
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ExportFieldDto {
    pub key: String,
    pub label: String,
    pub multi_value: bool,
    pub user_specific: bool,
}

/// Response for the field catalog
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ExportFieldCatalogResponse {
    pub fields: Vec<ExportFieldDto>,
}
