use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Type of entity that was changed
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EntityType {
    Book,
    Series,
    Library,
}

/// Specific event types for entity changes
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EntityEvent {
    /// A book was created
    BookCreated {
        book_id: Uuid,
        series_id: Uuid,
        library_id: Uuid,
    },
    /// A book was updated
    BookUpdated {
        book_id: Uuid,
        series_id: Uuid,
        library_id: Uuid,
        #[serde(skip_serializing_if = "Option::is_none")]
        fields: Option<Vec<String>>,
    },
    /// A book was deleted
    BookDeleted {
        book_id: Uuid,
        series_id: Uuid,
        library_id: Uuid,
    },
    /// A series was created
    SeriesCreated { series_id: Uuid, library_id: Uuid },
    /// A series was updated
    SeriesUpdated {
        series_id: Uuid,
        library_id: Uuid,
        #[serde(skip_serializing_if = "Option::is_none")]
        fields: Option<Vec<String>>,
    },
    /// A series was deleted
    SeriesDeleted { series_id: Uuid, library_id: Uuid },
    /// A cover image was updated
    CoverUpdated {
        entity_type: EntityType,
        entity_id: Uuid,
        #[serde(skip_serializing_if = "Option::is_none")]
        library_id: Option<Uuid>,
    },
    /// A library was updated
    LibraryUpdated { library_id: Uuid },
    /// A library was deleted
    LibraryDeleted { library_id: Uuid },
}

/// Complete entity change event with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityChangeEvent {
    /// The specific event that occurred
    #[serde(flatten)]
    pub event: EntityEvent,
    /// When the event occurred
    pub timestamp: DateTime<Utc>,
    /// User who triggered the change (if applicable)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_id: Option<Uuid>,
}

impl EntityChangeEvent {
    /// Create a new entity change event
    pub fn new(event: EntityEvent, user_id: Option<Uuid>) -> Self {
        Self {
            event,
            timestamp: Utc::now(),
            user_id,
        }
    }

    /// Get the library ID associated with this event (if any)
    pub fn library_id(&self) -> Option<Uuid> {
        match &self.event {
            EntityEvent::BookCreated { library_id, .. }
            | EntityEvent::BookUpdated { library_id, .. }
            | EntityEvent::BookDeleted { library_id, .. }
            | EntityEvent::SeriesCreated { library_id, .. }
            | EntityEvent::SeriesUpdated { library_id, .. }
            | EntityEvent::SeriesDeleted { library_id, .. }
            | EntityEvent::LibraryUpdated { library_id }
            | EntityEvent::LibraryDeleted { library_id } => Some(*library_id),
            EntityEvent::CoverUpdated { library_id, .. } => *library_id,
        }
    }
}
