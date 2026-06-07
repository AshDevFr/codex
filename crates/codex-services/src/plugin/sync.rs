//! Sync Provider Protocol Types
//!
//! Defines the JSON-RPC request/response types for sync provider operations.
//! Sync providers push and pull reading progress between Codex and external
//! services like AniList and MyAnimeList.
//!
//! ## Architecture
//!
//! Sync operations are initiated by the host (Codex) and sent to the plugin.
//! The plugin communicates with the external service using user credentials
//! provided during initialization.
//!
//! ## Methods
//!
//! - `sync/getUserInfo` - Get user info from external service
//! - `sync/pushProgress` - Push reading progress to external service
//! - `sync/pullProgress` - Pull reading progress from external service
//! - `sync/status` - Get sync status/diff between Codex and external

use serde::{Deserialize, Serialize};

// =============================================================================
// Reading Status
// =============================================================================

/// Reading status for sync entries
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncReadingStatus {
    /// Currently reading
    Reading,
    /// Finished reading
    Completed,
    /// Paused / On hold
    OnHold,
    /// Dropped / Abandoned
    Dropped,
    /// Planning to read
    PlanToRead,
}

// =============================================================================
// User Info
// =============================================================================

/// Response from `sync/getUserInfo` method
///
/// Returns the user's identity on the external service.
/// Used to display the connected account in the UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalUserInfo {
    /// User ID on the external service
    pub external_id: String,
    /// Display name / username
    pub username: String,
    /// Avatar/profile image URL
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub avatar_url: Option<String>,
    /// Profile URL on the external service
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile_url: Option<String>,
}

// =============================================================================
// Sync Entry (shared between push and pull)
// =============================================================================

/// A single reading progress entry for sync
///
/// Represents one series/book's reading state that can be pushed to
/// or pulled from an external service.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncEntry {
    /// External ID on the target service (e.g., AniList media ID)
    pub external_id: String,
    /// Reading status
    pub status: SyncReadingStatus,
    /// Reading progress
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub progress: Option<SyncProgress>,
    /// User's score/rating (service-specific scale, e.g., 1-10 or 1-100)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub score: Option<f64>,
    /// When the user started reading (ISO 8601)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started_at: Option<String>,
    /// When the user completed reading (ISO 8601)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<String>,
    /// User notes
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    /// When the series was most recently updated (ISO 8601).
    /// Populated from the most recent read_progress.updated_at for the series.
    /// Plugins can use this for time-based logic (e.g., pause/drop stale series).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_updated_at: Option<String>,
    /// Series title (for plugins that support title-based search fallback).
    /// Populated when the backend knows the series name. Plugins can use this
    /// to search the external service by title when no external ID is present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// ID of the library the series belongs to. Populated on push so plugins
    /// can scope behaviour per library. Empty on pulled entries (the external
    /// service does not send it back).
    #[serde(default)]
    pub library_id: String,
    /// Human-readable name of the library the series belongs to. Populated on
    /// push; empty on pulled entries.
    #[serde(default)]
    pub library_name: String,
    /// Series genres (top-level taxonomy). Populated on push only when the user
    /// enables `sendGenres` for a `wantsFullMetadata` plugin; empty otherwise and
    /// on pulled entries.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub genres: Vec<String>,
    /// Series tags (top-level taxonomy). Populated on push only when the user
    /// enables `sendTags` for a `wantsFullMetadata` plugin; empty otherwise and
    /// on pulled entries.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    /// Bibliographic metadata, attached on push only when the plugin declares
    /// `wantsFullMetadata` and the user enables the `sendMetadata` toggle. Lets
    /// rule-based plugins act on summary/authors/age-rating/etc. Empty on pulled
    /// entries.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<crate::plugin::protocol::SeriesMetadata>,
    /// User-defined custom metadata (parsed `series_metadata.custom_metadata`),
    /// attached on push only when the plugin declares `wantsFullMetadata` and the
    /// user enables the separate `sendCustomMetadata` toggle. Empty on pulled
    /// entries.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_metadata: Option<serde_json::Value>,
}

/// Reading progress details
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncProgress {
    /// Number of chapters read
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chapters: Option<i32>,
    /// Number of volumes read.
    ///
    /// This is the **relative** count of books the user has read in the series
    /// (each file = 1 book), not an absolute volume number. Retained for
    /// backward compatibility; consumers that want accurate progress for
    /// libraries with gaps should prefer `max_volume`/`max_chapter`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub volumes: Option<i32>,
    /// Number of pages read (for single-volume works)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pages: Option<i32>,
    /// Total number of chapters in the series (if known)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total_chapters: Option<i32>,
    /// Total number of volumes in the series (if known)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total_volumes: Option<i32>,

    /// Highest **read** volume number, derived from Codex's per-book volume
    /// detection. Unlike `volumes` (a count), this is the absolute highest
    /// volume the user has reached, so it stays correct for libraries that do
    /// not start at volume 1 or have gaps. Computed over the same set of books
    /// that feeds `volumes` (completed always; in-progress when the user's
    /// `countPartialProgress` setting is on). `None` when no counted book has a
    /// detected volume number.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_volume: Option<i32>,
    /// Highest **read** chapter number, derived from per-book chapter detection.
    /// `f32` because chapters can be fractional (e.g. 47.5 for side chapters).
    /// Same set/semantics as `max_volume`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_chapter: Option<f32>,
    /// Per-book reading-progress breakdown, one entry per book that has reading
    /// progress (completed or in-progress). Attached on push only when the
    /// plugin declares the `wantsDetailedProgress` capability, so authors of
    /// custom sync targets can map progress however their service expects.
    /// `None` (and the key omitted) otherwise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub read_books: Option<Vec<SyncBookProgress>>,
}

/// Per-book reading progress, the unit of `SyncProgress::read_books`.
///
/// Carries reading *position* (detected volume/chapter plus page progress), not
/// bibliographic metadata. All fields except `completed` are optional because
/// detection and page tracking may be absent for a given book.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncBookProgress {
    /// Detected volume number for this book, if known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub volume: Option<i32>,
    /// Detected chapter number for this book, if known (fractional allowed).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chapter: Option<f32>,
    /// Whether the user has finished this book.
    pub completed: bool,
    /// Current page within the book, if tracked.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_page: Option<i32>,
    /// Fractional progress within the book (0.0-1.0 or a percentage, as stored
    /// in `read_progress.progress_percentage`), if tracked.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub progress_percentage: Option<f64>,
}

// =============================================================================
// Push Progress
// =============================================================================

/// Parameters for `sync/pushProgress` method
///
/// Sends reading progress from Codex to the external service.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncPushRequest {
    /// Entries to push to the external service
    pub entries: Vec<SyncEntry>,
}

/// Response from `sync/pushProgress` method
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncPushResponse {
    /// Successfully synced entries
    pub success: Vec<SyncEntryResult>,
    /// Failed entries
    pub failed: Vec<SyncEntryResult>,
}

/// Result for a single sync entry (push or pull)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncEntryResult {
    /// External ID of the entry
    pub external_id: String,
    /// Result status
    pub status: SyncEntryResultStatus,
    /// Error message if failed
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Status of a single sync entry operation
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncEntryResultStatus {
    /// New entry created on external service
    Created,
    /// Existing entry updated
    Updated,
    /// No changes needed
    Unchanged,
    /// Operation failed
    Failed,
}

// =============================================================================
// Pull Progress
// =============================================================================

/// Parameters for `sync/pullProgress` method
///
/// Requests reading progress from the external service.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncPullRequest {
    /// Only pull entries updated after this timestamp (ISO 8601)
    /// If not set, pulls all entries
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub since: Option<String>,
    /// Maximum number of entries to pull
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    /// Pagination cursor for continuing a previous pull
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
}

/// Response from `sync/pullProgress` method
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncPullResponse {
    /// Entries pulled from the external service
    pub entries: Vec<SyncEntry>,
    /// Cursor for next page (if more entries available)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
    /// Whether there are more entries to pull
    #[serde(default)]
    pub has_more: bool,
}

// =============================================================================
// Sync Status
// =============================================================================

/// Response from `sync/status` method
///
/// Provides an overview of the sync state between Codex and the external service.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncStatusResponse {
    /// Last successful sync timestamp (ISO 8601)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_sync_at: Option<String>,
    /// Number of entries on the external service
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub external_count: Option<u32>,
    /// Number of entries that need to be pushed
    #[serde(default)]
    pub pending_push: u32,
    /// Number of entries that need to be pulled
    #[serde(default)]
    pub pending_pull: u32,
    /// Entries with conflicts (different on both sides)
    #[serde(default)]
    pub conflicts: u32,
}

// =============================================================================
// Permission Check
// =============================================================================

/// Check if a method name is a sync method
#[allow(dead_code)] // Protocol contract: mirrors is_storage_method() for sync methods
pub fn is_sync_method(method: &str) -> bool {
    matches!(
        method,
        "sync/getUserInfo" | "sync/pushProgress" | "sync/pullProgress" | "sync/status"
    )
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // =========================================================================
    // Reading Status Tests
    // =========================================================================

    #[test]
    fn test_reading_status_serialization() {
        assert_eq!(
            serde_json::to_value(SyncReadingStatus::Reading).unwrap(),
            json!("reading")
        );
        assert_eq!(
            serde_json::to_value(SyncReadingStatus::Completed).unwrap(),
            json!("completed")
        );
        assert_eq!(
            serde_json::to_value(SyncReadingStatus::OnHold).unwrap(),
            json!("on_hold")
        );
        assert_eq!(
            serde_json::to_value(SyncReadingStatus::Dropped).unwrap(),
            json!("dropped")
        );
        assert_eq!(
            serde_json::to_value(SyncReadingStatus::PlanToRead).unwrap(),
            json!("plan_to_read")
        );
    }

    #[test]
    fn test_reading_status_deserialization() {
        let reading: SyncReadingStatus = serde_json::from_value(json!("reading")).unwrap();
        assert_eq!(reading, SyncReadingStatus::Reading);

        let on_hold: SyncReadingStatus = serde_json::from_value(json!("on_hold")).unwrap();
        assert_eq!(on_hold, SyncReadingStatus::OnHold);

        let plan: SyncReadingStatus = serde_json::from_value(json!("plan_to_read")).unwrap();
        assert_eq!(plan, SyncReadingStatus::PlanToRead);
    }

    // =========================================================================
    // External User Info Tests
    // =========================================================================

    #[test]
    fn test_external_user_info_serialization() {
        let info = ExternalUserInfo {
            external_id: "12345".to_string(),
            username: "manga_reader".to_string(),
            avatar_url: Some("https://anilist.co/img/avatar.jpg".to_string()),
            profile_url: Some("https://anilist.co/user/manga_reader".to_string()),
        };
        let json = serde_json::to_value(&info).unwrap();
        assert_eq!(json["externalId"], "12345");
        assert_eq!(json["username"], "manga_reader");
        assert_eq!(json["avatarUrl"], "https://anilist.co/img/avatar.jpg");
        assert_eq!(json["profileUrl"], "https://anilist.co/user/manga_reader");
    }

    #[test]
    fn test_external_user_info_minimal() {
        let json = json!({
            "externalId": "99",
            "username": "user99"
        });
        let info: ExternalUserInfo = serde_json::from_value(json).unwrap();
        assert_eq!(info.external_id, "99");
        assert_eq!(info.username, "user99");
        assert!(info.avatar_url.is_none());
        assert!(info.profile_url.is_none());
    }

    #[test]
    fn test_external_user_info_skips_none_fields() {
        let info = ExternalUserInfo {
            external_id: "1".to_string(),
            username: "test".to_string(),
            avatar_url: None,
            profile_url: None,
        };
        let json = serde_json::to_value(&info).unwrap();
        let obj = json.as_object().unwrap();
        assert!(!obj.contains_key("avatarUrl"));
        assert!(!obj.contains_key("profileUrl"));
    }

    // =========================================================================
    // Sync Entry Tests
    // =========================================================================

    #[test]
    fn test_sync_entry_full_serialization() {
        let entry = SyncEntry {
            external_id: "12345".to_string(),
            status: SyncReadingStatus::Reading,
            progress: Some(SyncProgress {
                chapters: Some(42),
                volumes: Some(5),
                pages: None,
                total_chapters: None,
                total_volumes: None,
                ..Default::default()
            }),
            score: Some(8.5),
            started_at: Some("2026-01-15T00:00:00Z".to_string()),
            completed_at: None,
            notes: Some("Great series!".to_string()),
            latest_updated_at: Some("2026-02-01T12:00:00Z".to_string()),
            title: None,
            library_id: String::new(),
            library_name: String::new(),
            metadata: None,
            custom_metadata: None,
            genres: Vec::new(),
            tags: Vec::new(),
        };
        let json = serde_json::to_value(&entry).unwrap();
        assert_eq!(json["externalId"], "12345");
        assert_eq!(json["status"], "reading");
        assert_eq!(json["progress"]["chapters"], 42);
        assert_eq!(json["progress"]["volumes"], 5);
        assert!(!json["progress"].as_object().unwrap().contains_key("pages"));
        assert_eq!(json["score"], 8.5);
        assert_eq!(json["startedAt"], "2026-01-15T00:00:00Z");
        assert!(!json.as_object().unwrap().contains_key("completedAt"));
        assert_eq!(json["notes"], "Great series!");
        assert_eq!(json["latestUpdatedAt"], "2026-02-01T12:00:00Z");
    }

    #[test]
    fn test_sync_entry_minimal() {
        let json = json!({
            "externalId": "99",
            "status": "completed"
        });
        let entry: SyncEntry = serde_json::from_value(json).unwrap();
        assert_eq!(entry.external_id, "99");
        assert_eq!(entry.status, SyncReadingStatus::Completed);
        assert!(entry.progress.is_none());
        assert!(entry.score.is_none());
        assert!(entry.started_at.is_none());
        assert!(entry.completed_at.is_none());
        assert!(entry.notes.is_none());
    }

    #[test]
    fn test_sync_progress_serialization() {
        let progress = SyncProgress {
            chapters: Some(100),
            volumes: Some(10),
            pages: Some(3200),
            total_chapters: None,
            total_volumes: None,
            ..Default::default()
        };
        let json = serde_json::to_value(&progress).unwrap();
        assert_eq!(json["chapters"], 100);
        assert_eq!(json["volumes"], 10);
        assert_eq!(json["pages"], 3200);
    }

    #[test]
    fn test_sync_progress_partial() {
        let progress = SyncProgress {
            chapters: Some(50),
            volumes: None,
            pages: None,
            total_chapters: None,
            total_volumes: None,
            ..Default::default()
        };
        let json = serde_json::to_value(&progress).unwrap();
        assert_eq!(json["chapters"], 50);
        assert!(!json.as_object().unwrap().contains_key("volumes"));
        assert!(!json.as_object().unwrap().contains_key("pages"));
    }

    #[test]
    fn test_sync_progress_with_totals() {
        let progress = SyncProgress {
            chapters: Some(42),
            volumes: Some(5),
            pages: None,
            total_chapters: Some(200),
            total_volumes: Some(20),
            ..Default::default()
        };
        let json = serde_json::to_value(&progress).unwrap();
        assert_eq!(json["chapters"], 42);
        assert_eq!(json["volumes"], 5);
        assert_eq!(json["totalChapters"], 200);
        assert_eq!(json["totalVolumes"], 20);
        assert!(!json.as_object().unwrap().contains_key("pages"));
    }

    #[test]
    fn test_sync_progress_totals_deserialization() {
        let json = json!({
            "chapters": 10,
            "totalChapters": 100,
            "totalVolumes": 10
        });
        let progress: SyncProgress = serde_json::from_value(json).unwrap();
        assert_eq!(progress.chapters, Some(10));
        assert_eq!(progress.total_chapters, Some(100));
        assert_eq!(progress.total_volumes, Some(10));
        assert!(progress.volumes.is_none());
        assert!(progress.pages.is_none());
    }

    #[test]
    fn test_sync_progress_detailed_fields_omitted_when_none() {
        // Backward compatibility: progress without the detailed fields must not
        // emit the new keys, so existing plugins see byte-identical output.
        let progress = SyncProgress {
            volumes: Some(4),
            ..Default::default()
        };
        let json = serde_json::to_value(&progress).unwrap();
        let obj = json.as_object().unwrap();
        assert_eq!(json["volumes"], 4);
        assert!(!obj.contains_key("maxVolume"));
        assert!(!obj.contains_key("maxChapter"));
        assert!(!obj.contains_key("readBooks"));
    }

    #[test]
    fn test_sync_progress_detailed_fields_serialization() {
        let progress = SyncProgress {
            volumes: Some(4),
            max_volume: Some(8),
            max_chapter: Some(123.5),
            read_books: Some(vec![
                SyncBookProgress {
                    volume: Some(1),
                    chapter: None,
                    completed: true,
                    current_page: Some(200),
                    progress_percentage: Some(1.0),
                },
                SyncBookProgress {
                    volume: None,
                    chapter: Some(47.5),
                    completed: false,
                    current_page: Some(10),
                    progress_percentage: Some(0.25),
                },
            ]),
            ..Default::default()
        };
        let json = serde_json::to_value(&progress).unwrap();
        assert_eq!(json["volumes"], 4);
        assert_eq!(json["maxVolume"], 8);
        assert_eq!(json["maxChapter"], 123.5);

        let books = json["readBooks"].as_array().unwrap();
        assert_eq!(books.len(), 2);
        assert_eq!(books[0]["volume"], 1);
        assert_eq!(books[0]["completed"], true);
        assert_eq!(books[0]["currentPage"], 200);
        assert_eq!(books[0]["progressPercentage"], 1.0);
        // Fractional chapter survives; absent volume key is omitted.
        assert_eq!(books[1]["chapter"], 47.5);
        assert_eq!(books[1]["completed"], false);
        assert!(!books[1].as_object().unwrap().contains_key("volume"));
    }

    #[test]
    fn test_sync_progress_detailed_round_trip() {
        let json = json!({
            "volumes": 4,
            "maxVolume": 8,
            "maxChapter": 123.5,
            "readBooks": [
                {"volume": 1, "completed": true, "currentPage": 200},
                {"chapter": 47.5, "completed": false}
            ]
        });
        let progress: SyncProgress = serde_json::from_value(json).unwrap();
        assert_eq!(progress.volumes, Some(4));
        assert_eq!(progress.max_volume, Some(8));
        assert_eq!(progress.max_chapter, Some(123.5));

        let books = progress.read_books.unwrap();
        assert_eq!(books.len(), 2);
        assert_eq!(books[0].volume, Some(1));
        assert!(books[0].completed);
        assert_eq!(books[0].current_page, Some(200));
        assert_eq!(books[1].chapter, Some(47.5));
        assert!(!books[1].completed);
        assert!(books[1].volume.is_none());
    }

    #[test]
    fn test_sync_book_progress_omits_none_fields() {
        let book = SyncBookProgress {
            volume: Some(2),
            chapter: None,
            completed: true,
            current_page: None,
            progress_percentage: None,
        };
        let json = serde_json::to_value(&book).unwrap();
        let obj = json.as_object().unwrap();
        assert_eq!(json["volume"], 2);
        assert_eq!(json["completed"], true);
        assert!(!obj.contains_key("chapter"));
        assert!(!obj.contains_key("currentPage"));
        assert!(!obj.contains_key("progressPercentage"));
    }

    // =========================================================================
    // Push Progress Tests
    // =========================================================================

    #[test]
    fn test_sync_push_request_serialization() {
        let req = SyncPushRequest {
            entries: vec![
                SyncEntry {
                    external_id: "1".to_string(),
                    status: SyncReadingStatus::Reading,
                    progress: Some(SyncProgress {
                        chapters: Some(10),
                        volumes: None,
                        pages: None,
                        total_chapters: None,
                        total_volumes: None,
                        ..Default::default()
                    }),
                    score: None,
                    started_at: None,
                    completed_at: None,
                    notes: None,
                    latest_updated_at: None,
                    title: None,
                    library_id: String::new(),
                    library_name: String::new(),
                    metadata: None,
                    custom_metadata: None,
                    genres: Vec::new(),
                    tags: Vec::new(),
                },
                SyncEntry {
                    external_id: "2".to_string(),
                    status: SyncReadingStatus::Completed,
                    progress: None,
                    score: Some(9.0),
                    started_at: None,
                    completed_at: Some("2026-02-01T00:00:00Z".to_string()),
                    notes: None,
                    latest_updated_at: None,
                    title: None,
                    library_id: String::new(),
                    library_name: String::new(),
                    metadata: None,
                    custom_metadata: None,
                    genres: Vec::new(),
                    tags: Vec::new(),
                },
            ],
        };
        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["entries"].as_array().unwrap().len(), 2);
        assert_eq!(json["entries"][0]["externalId"], "1");
        assert_eq!(json["entries"][1]["status"], "completed");
    }

    #[test]
    fn test_sync_push_response_serialization() {
        let resp = SyncPushResponse {
            success: vec![
                SyncEntryResult {
                    external_id: "1".to_string(),
                    status: SyncEntryResultStatus::Updated,
                    error: None,
                },
                SyncEntryResult {
                    external_id: "2".to_string(),
                    status: SyncEntryResultStatus::Created,
                    error: None,
                },
            ],
            failed: vec![SyncEntryResult {
                external_id: "3".to_string(),
                status: SyncEntryResultStatus::Failed,
                error: Some("Rate limited".to_string()),
            }],
        };
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["success"].as_array().unwrap().len(), 2);
        assert_eq!(json["success"][0]["status"], "updated");
        assert_eq!(json["success"][1]["status"], "created");
        assert_eq!(json["failed"].as_array().unwrap().len(), 1);
        assert_eq!(json["failed"][0]["status"], "failed");
        assert_eq!(json["failed"][0]["error"], "Rate limited");
    }

    #[test]
    fn test_sync_entry_result_status_serialization() {
        assert_eq!(
            serde_json::to_value(SyncEntryResultStatus::Created).unwrap(),
            json!("created")
        );
        assert_eq!(
            serde_json::to_value(SyncEntryResultStatus::Updated).unwrap(),
            json!("updated")
        );
        assert_eq!(
            serde_json::to_value(SyncEntryResultStatus::Unchanged).unwrap(),
            json!("unchanged")
        );
        assert_eq!(
            serde_json::to_value(SyncEntryResultStatus::Failed).unwrap(),
            json!("failed")
        );
    }

    #[test]
    fn test_sync_entry_result_skips_none_error() {
        let result = SyncEntryResult {
            external_id: "1".to_string(),
            status: SyncEntryResultStatus::Updated,
            error: None,
        };
        let json = serde_json::to_value(&result).unwrap();
        assert!(!json.as_object().unwrap().contains_key("error"));
    }

    // =========================================================================
    // Pull Progress Tests
    // =========================================================================

    #[test]
    fn test_sync_pull_request_serialization() {
        let req = SyncPullRequest {
            since: Some("2026-02-01T00:00:00Z".to_string()),
            limit: Some(50),
            cursor: None,
        };
        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["since"], "2026-02-01T00:00:00Z");
        assert_eq!(json["limit"], 50);
        assert!(!json.as_object().unwrap().contains_key("cursor"));
    }

    #[test]
    fn test_sync_pull_request_minimal() {
        let json = json!({});
        let req: SyncPullRequest = serde_json::from_value(json).unwrap();
        assert!(req.since.is_none());
        assert!(req.limit.is_none());
        assert!(req.cursor.is_none());
    }

    #[test]
    fn test_sync_pull_request_with_cursor() {
        let req = SyncPullRequest {
            since: None,
            limit: None,
            cursor: Some("next_page_token".to_string()),
        };
        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["cursor"], "next_page_token");
    }

    #[test]
    fn test_sync_pull_response_serialization() {
        let resp = SyncPullResponse {
            entries: vec![SyncEntry {
                external_id: "42".to_string(),
                status: SyncReadingStatus::OnHold,
                progress: Some(SyncProgress {
                    chapters: Some(25),
                    volumes: None,
                    pages: None,
                    total_chapters: None,
                    total_volumes: None,
                    ..Default::default()
                }),
                score: Some(7.0),
                started_at: None,
                completed_at: None,
                notes: None,
                latest_updated_at: None,
                title: None,
                library_id: String::new(),
                library_name: String::new(),
                metadata: None,
                custom_metadata: None,
                genres: Vec::new(),
                tags: Vec::new(),
            }],
            next_cursor: Some("page2".to_string()),
            has_more: true,
        };
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["entries"].as_array().unwrap().len(), 1);
        assert_eq!(json["entries"][0]["status"], "on_hold");
        assert_eq!(json["nextCursor"], "page2");
        assert!(json["hasMore"].as_bool().unwrap());
    }

    #[test]
    fn test_sync_pull_response_last_page() {
        let resp = SyncPullResponse {
            entries: vec![],
            next_cursor: None,
            has_more: false,
        };
        let json = serde_json::to_value(&resp).unwrap();
        assert!(json["entries"].as_array().unwrap().is_empty());
        assert!(!json.as_object().unwrap().contains_key("nextCursor"));
        assert!(!json["hasMore"].as_bool().unwrap());
    }

    // =========================================================================
    // Sync Status Tests
    // =========================================================================

    #[test]
    fn test_sync_status_response_full() {
        let resp = SyncStatusResponse {
            last_sync_at: Some("2026-02-06T12:00:00Z".to_string()),
            external_count: Some(150),
            pending_push: 5,
            pending_pull: 3,
            conflicts: 1,
        };
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["lastSyncAt"], "2026-02-06T12:00:00Z");
        assert_eq!(json["externalCount"], 150);
        assert_eq!(json["pendingPush"], 5);
        assert_eq!(json["pendingPull"], 3);
        assert_eq!(json["conflicts"], 1);
    }

    #[test]
    fn test_sync_status_response_minimal() {
        let json = json!({});
        let resp: SyncStatusResponse = serde_json::from_value(json).unwrap();
        assert!(resp.last_sync_at.is_none());
        assert!(resp.external_count.is_none());
        assert_eq!(resp.pending_push, 0);
        assert_eq!(resp.pending_pull, 0);
        assert_eq!(resp.conflicts, 0);
    }

    #[test]
    fn test_sync_status_skips_none_fields() {
        let resp = SyncStatusResponse {
            last_sync_at: None,
            external_count: None,
            pending_push: 0,
            pending_pull: 0,
            conflicts: 0,
        };
        let json = serde_json::to_value(&resp).unwrap();
        let obj = json.as_object().unwrap();
        assert!(!obj.contains_key("lastSyncAt"));
        assert!(!obj.contains_key("externalCount"));
    }

    // =========================================================================
    // SyncEntry title field Tests
    // =========================================================================

    #[test]
    fn test_sync_entry_with_title() {
        let entry = SyncEntry {
            external_id: "".to_string(),
            status: SyncReadingStatus::Reading,
            progress: Some(SyncProgress {
                chapters: None,
                volumes: Some(3),
                pages: None,
                total_chapters: None,
                total_volumes: None,
                ..Default::default()
            }),
            score: None,
            started_at: None,
            completed_at: None,
            notes: None,
            latest_updated_at: None,
            title: Some("Berserk".to_string()),
            library_id: String::new(),
            library_name: String::new(),
            metadata: None,
            custom_metadata: None,
            genres: Vec::new(),
            tags: Vec::new(),
        };
        let json = serde_json::to_value(&entry).unwrap();
        assert_eq!(json["title"], "Berserk");
        assert_eq!(json["externalId"], "");
    }

    #[test]
    fn test_sync_entry_title_omitted_when_none() {
        let entry = SyncEntry {
            external_id: "42".to_string(),
            status: SyncReadingStatus::Reading,
            progress: None,
            score: None,
            started_at: None,
            completed_at: None,
            notes: None,
            latest_updated_at: None,
            title: None,
            library_id: String::new(),
            library_name: String::new(),
            metadata: None,
            custom_metadata: None,
            genres: Vec::new(),
            tags: Vec::new(),
        };
        let json = serde_json::to_value(&entry).unwrap();
        assert!(!json.as_object().unwrap().contains_key("title"));
    }

    #[test]
    fn test_sync_entry_title_deserialization() {
        let json = json!({
            "externalId": "",
            "status": "reading",
            "title": "One Piece"
        });
        let entry: SyncEntry = serde_json::from_value(json).unwrap();
        assert_eq!(entry.title, Some("One Piece".to_string()));
        assert_eq!(entry.external_id, "");
    }

    #[test]
    fn test_sync_entry_title_absent_deserializes_to_none() {
        let json = json!({
            "externalId": "42",
            "status": "completed"
        });
        let entry: SyncEntry = serde_json::from_value(json).unwrap();
        assert!(entry.title.is_none());
    }

    #[test]
    fn test_sync_entry_library_fields_serialize_as_camel_case() {
        let entry = SyncEntry {
            external_id: "42".to_string(),
            status: SyncReadingStatus::Reading,
            progress: None,
            score: None,
            started_at: None,
            completed_at: None,
            notes: None,
            latest_updated_at: None,
            title: None,
            library_id: "11111111-1111-1111-1111-111111111111".to_string(),
            library_name: "Manga".to_string(),
            metadata: None,
            custom_metadata: None,
            genres: Vec::new(),
            tags: Vec::new(),
        };
        let json = serde_json::to_value(&entry).unwrap();
        assert_eq!(json["libraryId"], "11111111-1111-1111-1111-111111111111");
        assert_eq!(json["libraryName"], "Manga");
    }

    #[test]
    fn test_sync_entry_library_fields_default_when_absent() {
        // Pulled entries (from the plugin) omit library context; must not fail.
        let json = json!({
            "externalId": "42",
            "status": "completed"
        });
        let entry: SyncEntry = serde_json::from_value(json).unwrap();
        assert_eq!(entry.library_id, "");
        assert_eq!(entry.library_name, "");
    }

    // =========================================================================
    // is_sync_method Tests
    // =========================================================================

    #[test]
    fn test_is_sync_method() {
        assert!(is_sync_method("sync/getUserInfo"));
        assert!(is_sync_method("sync/pushProgress"));
        assert!(is_sync_method("sync/pullProgress"));
        assert!(is_sync_method("sync/status"));
        assert!(!is_sync_method("storage/get"));
        assert!(!is_sync_method("metadata/series/search"));
        assert!(!is_sync_method("initialize"));
        assert!(!is_sync_method("sync/unknown"));
    }
}
