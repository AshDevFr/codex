//! `SeaORM` Entity for book_external_ids table
//!
//! Stores external provider IDs for books, enabling:
//! - Tracking which source a book was matched from (plugin:openlibrary, epub, pdf, manual)
//! - Efficient re-fetching without search when external ID is known
//! - Metadata change detection via metadata_hash
//!
//! ## Source Naming Convention
//!
//! - `plugin:<name>` - External ID from a metadata plugin (e.g., "plugin:openlibrary")
//! - `epub` - External ID extracted from EPUB metadata (e.g., ISBN from dc:identifier)
//! - `pdf` - External ID extracted from PDF metadata
//! - `manual` - Manually entered by user

use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "book_external_ids")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub book_id: Uuid,
    /// Source identifier: 'plugin:openlibrary', 'epub', 'pdf', 'manual'
    pub source: String,
    /// ID in the external system (ISBN, OLID, etc.)
    pub external_id: String,
    /// Full URL to the source page (optional convenience)
    pub external_url: Option<String>,
    /// Hash of last fetched metadata for change detection
    pub metadata_hash: Option<String>,
    /// When metadata was last synced from this source
    pub last_synced_at: Option<DateTime<Utc>>,
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
    /// Check if this external ID is from a plugin
    pub fn is_plugin_source(&self) -> bool {
        self.source.starts_with("plugin:")
    }

    /// Get the plugin name if this is a plugin source
    pub fn plugin_name(&self) -> Option<&str> {
        self.source.strip_prefix("plugin:")
    }

    /// Check if this external ID is from EPUB metadata
    pub fn is_epub_source(&self) -> bool {
        self.source == "epub"
    }

    /// Check if this external ID is from PDF metadata
    pub fn is_pdf_source(&self) -> bool {
        self.source == "pdf"
    }

    /// Check if this external ID was manually entered
    pub fn is_manual_source(&self) -> bool {
        self.source == "manual"
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
            external_id: "12345".to_string(),
            external_url: None,
            metadata_hash: None,
            last_synced_at: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn test_is_plugin_source() {
        let model = create_test_model("plugin:openlibrary");
        assert!(model.is_plugin_source());

        let model = create_test_model("epub");
        assert!(!model.is_plugin_source());
    }

    #[test]
    fn test_plugin_name() {
        let model = create_test_model("plugin:openlibrary");
        assert_eq!(model.plugin_name(), Some("openlibrary"));

        let model = create_test_model("plugin:googlebooks");
        assert_eq!(model.plugin_name(), Some("googlebooks"));

        let model = create_test_model("epub");
        assert_eq!(model.plugin_name(), None);
    }

    #[test]
    fn test_is_epub_source() {
        let model = create_test_model("epub");
        assert!(model.is_epub_source());

        let model = create_test_model("plugin:openlibrary");
        assert!(!model.is_epub_source());
    }

    #[test]
    fn test_is_pdf_source() {
        let model = create_test_model("pdf");
        assert!(model.is_pdf_source());

        let model = create_test_model("epub");
        assert!(!model.is_pdf_source());
    }

    #[test]
    fn test_is_manual_source() {
        let model = create_test_model("manual");
        assert!(model.is_manual_source());

        let model = create_test_model("epub");
        assert!(!model.is_manual_source());
    }

    #[test]
    fn test_plugin_source_helper() {
        assert_eq!(Model::plugin_source("openlibrary"), "plugin:openlibrary");
        assert_eq!(Model::plugin_source("googlebooks"), "plugin:googlebooks");
    }
}
