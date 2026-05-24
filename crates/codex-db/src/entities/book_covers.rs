//! `SeaORM` Entity for book_covers table
//!
//! Stores multiple cover images per book with one selected as primary.
//!
//! ## Source Naming Convention
//!
//! - `extracted` - Cover extracted from the book file (EPUB cover, PDF first page, CBZ first image)
//! - `plugin:<name>` - Cover downloaded from a metadata plugin (e.g., "plugin:openlibrary")
//! - `custom` - User-uploaded custom cover
//! - `url` - Cover downloaded from a user-provided URL
//!
//! ## Storage Path Convention
//!
//! Covers are stored at: `uploads/covers/books/{book_uuid}/{cover_uuid}.{ext}`

use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "book_covers")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub book_id: Uuid,
    /// Source: "extracted", "plugin:openlibrary", "custom", "url"
    pub source: String,
    /// Local file path to cover image
    pub path: String,
    /// Whether this cover is currently selected as the primary cover
    pub is_selected: bool,
    /// Image width in pixels (optional, for display optimization)
    pub width: Option<i32>,
    /// Image height in pixels (optional, for display optimization)
    pub height: Option<i32>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::books::Entity",
        from = "Column::BookId",
        to = "super::books::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    Books,
}

impl Related<super::books::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Books.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

// =============================================================================
// Helper Methods
// =============================================================================

#[allow(dead_code)]
impl Model {
    /// Check if this cover is from a plugin
    pub fn is_plugin_source(&self) -> bool {
        self.source.starts_with("plugin:")
    }

    /// Get the plugin name if this is a plugin source
    pub fn plugin_name(&self) -> Option<&str> {
        self.source.strip_prefix("plugin:")
    }

    /// Check if this cover was extracted from the book file
    pub fn is_extracted(&self) -> bool {
        self.source == "extracted"
    }

    /// Check if this cover was user-uploaded
    pub fn is_custom(&self) -> bool {
        self.source == "custom"
    }

    /// Check if this cover was downloaded from a URL
    pub fn is_url_source(&self) -> bool {
        self.source == "url"
    }

    /// Create a plugin source string from a plugin name
    pub fn plugin_source(plugin_name: &str) -> String {
        format!("plugin:{}", plugin_name)
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_model(source: &str) -> Model {
        Model {
            id: Uuid::new_v4(),
            book_id: Uuid::new_v4(),
            source: source.to_string(),
            path: "/uploads/covers/books/test/cover.jpg".to_string(),
            is_selected: false,
            width: Some(300),
            height: Some(450),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn test_is_plugin_source() {
        let model = create_test_model("plugin:openlibrary");
        assert!(model.is_plugin_source());

        let model = create_test_model("extracted");
        assert!(!model.is_plugin_source());
    }

    #[test]
    fn test_plugin_name() {
        let model = create_test_model("plugin:openlibrary");
        assert_eq!(model.plugin_name(), Some("openlibrary"));

        let model = create_test_model("plugin:googlebooks");
        assert_eq!(model.plugin_name(), Some("googlebooks"));

        let model = create_test_model("extracted");
        assert_eq!(model.plugin_name(), None);
    }

    #[test]
    fn test_is_extracted() {
        let model = create_test_model("extracted");
        assert!(model.is_extracted());

        let model = create_test_model("plugin:openlibrary");
        assert!(!model.is_extracted());
    }

    #[test]
    fn test_is_custom() {
        let model = create_test_model("custom");
        assert!(model.is_custom());

        let model = create_test_model("extracted");
        assert!(!model.is_custom());
    }

    #[test]
    fn test_is_url_source() {
        let model = create_test_model("url");
        assert!(model.is_url_source());

        let model = create_test_model("custom");
        assert!(!model.is_url_source());
    }

    #[test]
    fn test_plugin_source_helper() {
        assert_eq!(Model::plugin_source("openlibrary"), "plugin:openlibrary");
        assert_eq!(Model::plugin_source("googlebooks"), "plugin:googlebooks");
    }
}
