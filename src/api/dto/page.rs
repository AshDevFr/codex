use serde::Serialize;
use utoipa::ToSchema;

/// Page data transfer object
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PageDto {
    pub id: uuid::Uuid,
    pub book_id: uuid::Uuid,
    pub page_number: i32,
    pub file_name: String,
    pub file_format: String,
    pub file_size: i64,
    pub width: Option<i32>,
    pub height: Option<i32>,
}
