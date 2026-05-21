use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

/// Operators for string and equality comparisons
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(tag = "operator", rename_all = "camelCase")]
pub enum FieldOperator {
    /// Exact match
    Is { value: String },
    /// Not equal
    IsNot { value: String },
    /// Field is null/empty
    IsNull,
    /// Field is not null/empty
    IsNotNull,
    /// String contains (case-insensitive)
    Contains { value: String },
    /// String does not contain (case-insensitive)
    DoesNotContain { value: String },
    /// String starts with (case-insensitive)
    BeginsWith { value: String },
    /// String ends with (case-insensitive)
    EndsWith { value: String },
}

/// Operators for UUID comparisons (library_id, series_id, etc.)
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(tag = "operator", rename_all = "camelCase")]
pub enum UuidOperator {
    /// Exact match
    Is { value: Uuid },
    /// Not equal
    IsNot { value: Uuid },
}

/// Operators for boolean comparisons
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(tag = "operator", rename_all = "camelCase")]
pub enum BoolOperator {
    /// Is true
    IsTrue,
    /// Is false
    IsFalse,
}

/// Operators for numeric comparisons (year, page count, etc.).
///
/// Values are deserialized as `i64` so the same operator can target either
/// `INTEGER` or `BIGINT` columns. Implementations downcast as needed.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(tag = "operator", rename_all = "camelCase")]
pub enum NumberOperator {
    /// Equal to value
    Eq { value: i64 },
    /// Not equal to value
    Ne { value: i64 },
    /// Greater than value (strict)
    Gt { value: i64 },
    /// Greater than or equal to value
    Gte { value: i64 },
    /// Less than value (strict)
    Lt { value: i64 },
    /// Less than or equal to value
    Lte { value: i64 },
    /// Inclusive range, `min <= field <= max`. Either bound may be omitted to
    /// model open-ended ranges (e.g. "year >= 2000").
    Between {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        min: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        max: Option<i64>,
    },
    /// Field is null
    IsNull,
    /// Field is not null
    IsNotNull,
}

/// Operators for date/timestamp comparisons.
///
/// Values are RFC 3339 / ISO 8601 timestamps. For range comparisons either
/// bound may be omitted to express an open-ended range.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(tag = "operator", rename_all = "camelCase")]
pub enum DateOperator {
    /// Strictly after the given timestamp
    After { value: DateTime<Utc> },
    /// Strictly before the given timestamp
    Before { value: DateTime<Utc> },
    /// On or after the given timestamp
    OnOrAfter { value: DateTime<Utc> },
    /// On or before the given timestamp
    OnOrBefore { value: DateTime<Utc> },
    /// Inclusive between range. Either bound may be omitted.
    Between {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        start: Option<DateTime<Utc>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        end: Option<DateTime<Utc>>,
    },
    /// Field is null
    IsNull,
    /// Field is not null
    IsNotNull,
}

/// Series-level search conditions
///
/// Conditions can be composed using `allOf` (AND) and `anyOf` (OR).
/// Uses untagged enum for cleaner JSON without explicit type field.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(untagged)]
pub enum SeriesCondition {
    /// All conditions must match (AND)
    AllOf {
        #[serde(rename = "allOf")]
        #[schema(no_recursion)]
        all_of: Vec<SeriesCondition>,
    },
    /// Any condition must match (OR)
    AnyOf {
        #[serde(rename = "anyOf")]
        #[schema(no_recursion)]
        any_of: Vec<SeriesCondition>,
    },
    /// Filter by library ID
    LibraryId {
        #[serde(rename = "libraryId")]
        library_id: UuidOperator,
    },
    /// Filter by genre name
    Genre { genre: FieldOperator },
    /// Filter by tag name
    Tag { tag: FieldOperator },
    /// Filter by series status (ongoing, ended, hiatus, etc.)
    Status { status: FieldOperator },
    /// Filter by publisher
    Publisher { publisher: FieldOperator },
    /// Filter by language
    Language { language: FieldOperator },
    /// Filter by series title (`series_metadata.title`)
    Title { title: FieldOperator },
    /// Filter by series title_sort field (used for alphabetical filtering)
    TitleSort {
        #[serde(rename = "titleSort")]
        title_sort: FieldOperator,
    },
    /// Filter by read status (unread, in_progress, read)
    ReadStatus {
        #[serde(rename = "readStatus")]
        read_status: FieldOperator,
    },
    /// Filter by sharing tag name
    SharingTag {
        #[serde(rename = "sharingTag")]
        sharing_tag: FieldOperator,
    },
    /// Filter by series completion status (complete/incomplete based on book_count vs total_volume_count)
    Completion { completion: BoolOperator },
    /// Filter by whether the series has an external source ID linked
    HasExternalSourceId {
        #[serde(rename = "hasExternalSourceId")]
        has_external_source_id: BoolOperator,
    },
    /// Filter by whether the series has a rating from the current user
    HasUserRating {
        #[serde(rename = "hasUserRating")]
        has_user_rating: BoolOperator,
    },
    /// Filter by whether release tracking is enabled for the series.
    ///
    /// `IsTrue` returns only series whose `series_tracking.tracked` flag is
    /// `true`. `IsFalse` returns everything else, including series with no
    /// `series_tracking` row at all (the common case for a fresh library).
    IsTracked {
        #[serde(rename = "isTracked")]
        is_tracked: BoolOperator,
    },
    /// Filter by release year (from `series_metadata.year`).
    Year { year: NumberOperator },
    /// Filter by author (substring match on `series_metadata.authors_json`).
    ///
    /// The match is performed against the raw JSON text. It is tolerant of
    /// both string-list and object-list shapes but may incidentally match
    /// other fields (e.g. `role`); callers wanting strict matching should
    /// pre-quote the value.
    Author { author: FieldOperator },
    /// Filter by the series' folder path (`series.path`). Useful for matching
    /// series under a given directory.
    Path { path: FieldOperator },
    /// Filter by date the series was added to the library
    /// (`series.created_at`).
    DateAdded {
        #[serde(rename = "dateAdded")]
        date_added: DateOperator,
    },
}

/// Book-level search conditions
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(untagged)]
pub enum BookCondition {
    /// All conditions must match (AND)
    AllOf {
        #[serde(rename = "allOf")]
        #[schema(no_recursion)]
        all_of: Vec<BookCondition>,
    },
    /// Any condition must match (OR)
    AnyOf {
        #[serde(rename = "anyOf")]
        #[schema(no_recursion)]
        any_of: Vec<BookCondition>,
    },
    /// Filter by library ID
    LibraryId {
        #[serde(rename = "libraryId")]
        library_id: UuidOperator,
    },
    /// Filter by series ID
    SeriesId {
        #[serde(rename = "seriesId")]
        series_id: UuidOperator,
    },
    /// Filter by genre name (from parent series)
    Genre { genre: FieldOperator },
    /// Filter by tag name (from parent series)
    Tag { tag: FieldOperator },
    /// Filter by book title (`book_metadata.title`)
    Title { title: FieldOperator },
    /// Filter by book title_sort field (`book_metadata.title_sort`,
    /// used for alphabetical filtering)
    TitleSort {
        #[serde(rename = "titleSort")]
        title_sort: FieldOperator,
    },
    /// Filter by read status (unread, in_progress, read)
    ReadStatus {
        #[serde(rename = "readStatus")]
        read_status: FieldOperator,
    },
    /// Filter by books with analysis errors
    HasError {
        #[serde(rename = "hasError")]
        has_error: BoolOperator,
    },
    /// Filter by book type (comic, manga, novel, etc.)
    BookType {
        #[serde(rename = "bookType")]
        book_type: FieldOperator,
    },
    /// Filter by the book's file path (`books.path`). Useful for matching
    /// books under a given directory or with a specific filename fragment.
    Path { path: FieldOperator },
    /// Filter by file format (`books.format`, e.g. `cbz`, `cbr`, `epub`,
    /// `pdf`). Distinct from `BookType`, which classifies content (comic,
    /// manga, novel, ...).
    Format { format: FieldOperator },
    /// Filter by page count (`books.page_count`).
    PageCount {
        #[serde(rename = "pageCount")]
        page_count: NumberOperator,
    },
    /// Filter by date the book was added to the library (`books.created_at`).
    DateAdded {
        #[serde(rename = "dateAdded")]
        date_added: DateOperator,
    },
}

/// Request body for POST /series/list
///
/// Pagination parameters (page, pageSize, sort) are passed as query parameters,
/// not in the request body. This enables proper HATEOAS links.
#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SeriesListRequest {
    /// Filter condition (optional - no condition returns all)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Option<Object>)]
    pub condition: Option<SeriesCondition>,

    /// Full-text search query (case-insensitive search on series name)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub full_text_search: Option<String>,
}

/// Request body for POST /books/list
///
/// Pagination parameters (page, pageSize, sort) are passed as query parameters,
/// not in the request body. This enables proper HATEOAS links.
#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BookListRequest {
    /// Filter condition (optional - no condition returns all)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Option<Object>)]
    pub condition: Option<BookCondition>,

    /// Full-text search query (case-insensitive search on book title)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub full_text_search: Option<String>,

    /// Include soft-deleted books in results (default: false)
    #[serde(default)]
    pub include_deleted: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_genre_condition_serialization() {
        let condition = SeriesCondition::Genre {
            genre: FieldOperator::Is {
                value: "Action".to_string(),
            },
        };

        let json = serde_json::to_string(&condition).unwrap();
        assert_eq!(json, r#"{"genre":{"operator":"is","value":"Action"}}"#);
    }

    #[test]
    fn test_simple_genre_condition_deserialization() {
        let json = r#"{"genre":{"operator":"is","value":"Action"}}"#;
        let condition: SeriesCondition = serde_json::from_str(json).unwrap();

        match condition {
            SeriesCondition::Genre {
                genre: FieldOperator::Is { value },
            } => {
                assert_eq!(value, "Action");
            }
            _ => panic!("Expected Genre condition"),
        }
    }

    #[test]
    fn test_all_of_condition_serialization() {
        let condition = SeriesCondition::AllOf {
            all_of: vec![
                SeriesCondition::Genre {
                    genre: FieldOperator::Is {
                        value: "Action".to_string(),
                    },
                },
                SeriesCondition::Genre {
                    genre: FieldOperator::IsNot {
                        value: "Horror".to_string(),
                    },
                },
            ],
        };

        let json = serde_json::to_string(&condition).unwrap();
        assert!(json.contains(r#""allOf""#));
        assert!(json.contains(r#""operator":"is""#));
        assert!(json.contains(r#""operator":"isNot""#));
    }

    #[test]
    fn test_all_of_condition_deserialization() {
        let json = r#"{
            "allOf": [
                {"genre": {"operator": "is", "value": "Action"}},
                {"genre": {"operator": "isNot", "value": "Horror"}}
            ]
        }"#;

        let condition: SeriesCondition = serde_json::from_str(json).unwrap();

        match condition {
            SeriesCondition::AllOf { all_of } => {
                assert_eq!(all_of.len(), 2);
            }
            _ => panic!("Expected AllOf condition"),
        }
    }

    #[test]
    fn test_nested_condition() {
        // (Action AND NOT Comedy) OR (Fantasy AND NOT Horror)
        let json = r#"{
            "anyOf": [
                {
                    "allOf": [
                        {"genre": {"operator": "is", "value": "Action"}},
                        {"genre": {"operator": "isNot", "value": "Comedy"}}
                    ]
                },
                {
                    "allOf": [
                        {"genre": {"operator": "is", "value": "Fantasy"}},
                        {"genre": {"operator": "isNot", "value": "Horror"}}
                    ]
                }
            ]
        }"#;

        let condition: SeriesCondition = serde_json::from_str(json).unwrap();

        match condition {
            SeriesCondition::AnyOf { any_of } => {
                assert_eq!(any_of.len(), 2);
                // Each should be an AllOf
                for inner in &any_of {
                    match inner {
                        SeriesCondition::AllOf { all_of } => {
                            assert_eq!(all_of.len(), 2);
                        }
                        _ => panic!("Expected AllOf inside AnyOf"),
                    }
                }
            }
            _ => panic!("Expected AnyOf condition"),
        }
    }

    #[test]
    fn test_library_id_condition() {
        let uuid = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let condition = SeriesCondition::LibraryId {
            library_id: UuidOperator::Is { value: uuid },
        };

        let json = serde_json::to_string(&condition).unwrap();
        assert!(json.contains(r#""libraryId""#));
        assert!(json.contains(r#""operator":"is""#));
        assert!(json.contains("550e8400-e29b-41d4-a716-446655440000"));
    }

    #[test]
    fn test_string_operators() {
        let operators = vec![
            (
                FieldOperator::Contains {
                    value: "test".to_string(),
                },
                "contains",
            ),
            (
                FieldOperator::DoesNotContain {
                    value: "test".to_string(),
                },
                "doesNotContain",
            ),
            (
                FieldOperator::BeginsWith {
                    value: "test".to_string(),
                },
                "beginsWith",
            ),
            (
                FieldOperator::EndsWith {
                    value: "test".to_string(),
                },
                "endsWith",
            ),
            (FieldOperator::IsNull, "isNull"),
            (FieldOperator::IsNotNull, "isNotNull"),
        ];

        for (op, expected_name) in operators {
            let json = serde_json::to_string(&op).unwrap();
            assert!(
                json.contains(&format!(r#""operator":"{}""#, expected_name)),
                "Expected operator {} in {}",
                expected_name,
                json
            );
        }
    }

    #[test]
    fn test_series_list_request() {
        let request = SeriesListRequest {
            condition: Some(SeriesCondition::Genre {
                genre: FieldOperator::Is {
                    value: "Action".to_string(),
                },
            }),
            full_text_search: Some("test".to_string()),
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains(r#""condition""#));
        assert!(json.contains(r#""fullTextSearch":"test""#));
        // Pagination fields should NOT be in the body (they're query params now)
        assert!(!json.contains(r#""page""#));
        assert!(!json.contains(r#""pageSize""#));
        assert!(!json.contains(r#""sort""#));
    }

    #[test]
    fn test_series_list_request_empty() {
        let request = SeriesListRequest::default();

        let json = serde_json::to_string(&request).unwrap();
        // Empty optional fields should be omitted
        assert!(!json.contains(r#""condition""#));
        assert!(!json.contains(r#""fullTextSearch""#));
        // Body should be empty JSON object
        assert_eq!(json, "{}");
    }

    #[test]
    fn test_series_list_request_defaults() {
        let request = SeriesListRequest::default();
        assert!(request.condition.is_none());
        assert!(request.full_text_search.is_none());
    }

    #[test]
    fn test_book_list_request_defaults() {
        let request = BookListRequest::default();
        assert!(request.condition.is_none());
        assert!(request.full_text_search.is_none());
        assert!(!request.include_deleted);
    }

    #[test]
    fn test_series_list_request_deserialization_with_defaults() {
        // Test that deserialization uses correct defaults
        let json = r#"{}"#;
        let request: SeriesListRequest = serde_json::from_str(json).unwrap();
        assert!(request.condition.is_none());
        assert!(request.full_text_search.is_none());
    }

    #[test]
    fn test_book_list_request_deserialization_with_defaults() {
        // Test that deserialization uses correct defaults
        let json = r#"{}"#;
        let request: BookListRequest = serde_json::from_str(json).unwrap();
        assert!(request.condition.is_none());
        assert!(request.full_text_search.is_none());
        assert!(!request.include_deleted);
    }

    #[test]
    fn test_book_list_request_with_include_deleted() {
        let request = BookListRequest {
            include_deleted: true,
            ..Default::default()
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains(r#""includeDeleted":true"#));
    }

    #[test]
    fn test_book_condition() {
        let condition = BookCondition::AllOf {
            all_of: vec![
                BookCondition::SeriesId {
                    series_id: UuidOperator::Is {
                        value: Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap(),
                    },
                },
                BookCondition::ReadStatus {
                    read_status: FieldOperator::Is {
                        value: "unread".to_string(),
                    },
                },
            ],
        };

        let json = serde_json::to_string(&condition).unwrap();
        assert!(json.contains(r#""allOf""#));
        assert!(json.contains(r#""seriesId""#));
        assert!(json.contains(r#""readStatus""#));
    }

    #[test]
    fn test_book_has_error_condition() {
        let condition = BookCondition::HasError {
            has_error: BoolOperator::IsTrue,
        };

        let json = serde_json::to_string(&condition).unwrap();
        assert!(json.contains(r#""hasError""#));
        assert!(json.contains(r#""operator":"isTrue""#));
    }

    #[test]
    fn test_title_sort_condition_begins_with() {
        let condition = SeriesCondition::TitleSort {
            title_sort: FieldOperator::BeginsWith {
                value: "A".to_string(),
            },
        };

        let json = serde_json::to_string(&condition).unwrap();
        assert!(json.contains(r#""titleSort""#));
        assert!(json.contains(r#""operator":"beginsWith""#));
        assert!(json.contains(r#""value":"A""#));
    }

    #[test]
    fn test_title_sort_condition_deserialization() {
        let json = r#"{"titleSort":{"operator":"beginsWith","value":"B"}}"#;
        let condition: SeriesCondition = serde_json::from_str(json).unwrap();

        match condition {
            SeriesCondition::TitleSort {
                title_sort: FieldOperator::BeginsWith { value },
            } => {
                assert_eq!(value, "B");
            }
            _ => panic!("Expected TitleSort condition with BeginsWith operator"),
        }
    }

    #[test]
    fn test_title_sort_combined_with_other_filters() {
        // Combined condition: titleSort begins with "A" AND genre is "Action"
        let condition = SeriesCondition::AllOf {
            all_of: vec![
                SeriesCondition::TitleSort {
                    title_sort: FieldOperator::BeginsWith {
                        value: "A".to_string(),
                    },
                },
                SeriesCondition::Genre {
                    genre: FieldOperator::Is {
                        value: "Action".to_string(),
                    },
                },
            ],
        };

        let json = serde_json::to_string(&condition).unwrap();
        assert!(json.contains(r#""allOf""#));
        assert!(json.contains(r#""titleSort""#));
        assert!(json.contains(r#""genre""#));
    }

    #[test]
    fn test_completion_condition_is_true() {
        let condition = SeriesCondition::Completion {
            completion: BoolOperator::IsTrue,
        };

        let json = serde_json::to_string(&condition).unwrap();
        assert!(json.contains(r#""completion""#));
        assert!(json.contains(r#""operator":"isTrue""#));
    }

    #[test]
    fn test_completion_condition_is_false() {
        let condition = SeriesCondition::Completion {
            completion: BoolOperator::IsFalse,
        };

        let json = serde_json::to_string(&condition).unwrap();
        assert!(json.contains(r#""completion""#));
        assert!(json.contains(r#""operator":"isFalse""#));
    }

    #[test]
    fn test_completion_condition_deserialization() {
        let json = r#"{"completion":{"operator":"isTrue"}}"#;
        let condition: SeriesCondition = serde_json::from_str(json).unwrap();

        match condition {
            SeriesCondition::Completion {
                completion: BoolOperator::IsTrue,
            } => {}
            _ => panic!("Expected Completion condition with IsTrue operator"),
        }
    }

    #[test]
    fn test_has_external_source_id_condition_is_true() {
        let condition = SeriesCondition::HasExternalSourceId {
            has_external_source_id: BoolOperator::IsTrue,
        };

        let json = serde_json::to_string(&condition).unwrap();
        assert!(json.contains(r#""hasExternalSourceId""#));
        assert!(json.contains(r#""operator":"isTrue""#));
    }

    #[test]
    fn test_has_external_source_id_condition_is_false() {
        let condition = SeriesCondition::HasExternalSourceId {
            has_external_source_id: BoolOperator::IsFalse,
        };

        let json = serde_json::to_string(&condition).unwrap();
        assert!(json.contains(r#""hasExternalSourceId""#));
        assert!(json.contains(r#""operator":"isFalse""#));
    }

    #[test]
    fn test_has_external_source_id_condition_deserialization() {
        let json = r#"{"hasExternalSourceId":{"operator":"isTrue"}}"#;
        let condition: SeriesCondition = serde_json::from_str(json).unwrap();

        match condition {
            SeriesCondition::HasExternalSourceId {
                has_external_source_id: BoolOperator::IsTrue,
            } => {}
            _ => panic!("Expected HasExternalSourceId condition with IsTrue operator"),
        }
    }

    #[test]
    fn test_has_user_rating_condition_is_true() {
        let condition = SeriesCondition::HasUserRating {
            has_user_rating: BoolOperator::IsTrue,
        };

        let json = serde_json::to_string(&condition).unwrap();
        assert!(json.contains(r#""hasUserRating""#));
        assert!(json.contains(r#""operator":"isTrue""#));
    }

    #[test]
    fn test_has_user_rating_condition_is_false() {
        let condition = SeriesCondition::HasUserRating {
            has_user_rating: BoolOperator::IsFalse,
        };

        let json = serde_json::to_string(&condition).unwrap();
        assert!(json.contains(r#""hasUserRating""#));
        assert!(json.contains(r#""operator":"isFalse""#));
    }

    #[test]
    fn test_has_user_rating_condition_deserialization() {
        let json = r#"{"hasUserRating":{"operator":"isTrue"}}"#;
        let condition: SeriesCondition = serde_json::from_str(json).unwrap();

        match condition {
            SeriesCondition::HasUserRating {
                has_user_rating: BoolOperator::IsTrue,
            } => {}
            _ => panic!("Expected HasUserRating condition with IsTrue operator"),
        }
    }

    #[test]
    fn test_is_tracked_condition_is_true() {
        let condition = SeriesCondition::IsTracked {
            is_tracked: BoolOperator::IsTrue,
        };

        let json = serde_json::to_string(&condition).unwrap();
        assert!(json.contains(r#""isTracked""#));
        assert!(json.contains(r#""operator":"isTrue""#));
    }

    #[test]
    fn test_is_tracked_condition_is_false() {
        let condition = SeriesCondition::IsTracked {
            is_tracked: BoolOperator::IsFalse,
        };

        let json = serde_json::to_string(&condition).unwrap();
        assert!(json.contains(r#""isTracked""#));
        assert!(json.contains(r#""operator":"isFalse""#));
    }

    #[test]
    fn test_is_tracked_condition_deserialization() {
        let json = r#"{"isTracked":{"operator":"isTrue"}}"#;
        let condition: SeriesCondition = serde_json::from_str(json).unwrap();

        match condition {
            SeriesCondition::IsTracked {
                is_tracked: BoolOperator::IsTrue,
            } => {}
            _ => panic!("Expected IsTracked condition with IsTrue operator"),
        }
    }

    #[test]
    fn test_number_operator_eq_serialization() {
        let op = NumberOperator::Eq { value: 2024 };
        let json = serde_json::to_string(&op).unwrap();
        assert_eq!(json, r#"{"operator":"eq","value":2024}"#);
    }

    #[test]
    fn test_number_operator_between_serialization() {
        let op = NumberOperator::Between {
            min: Some(1980),
            max: Some(1989),
        };
        let json = serde_json::to_string(&op).unwrap();
        assert!(json.contains(r#""operator":"between""#));
        assert!(json.contains(r#""min":1980"#));
        assert!(json.contains(r#""max":1989"#));
    }

    #[test]
    fn test_number_operator_between_open_ended() {
        // No max bound: "year >= 2000"
        let op = NumberOperator::Between {
            min: Some(2000),
            max: None,
        };
        let json = serde_json::to_string(&op).unwrap();
        assert!(json.contains(r#""min":2000"#));
        assert!(!json.contains(r#""max""#));
    }

    #[test]
    fn test_year_condition_round_trip() {
        let condition = SeriesCondition::Year {
            year: NumberOperator::Gte { value: 2000 },
        };
        let json = serde_json::to_string(&condition).unwrap();
        let parsed: SeriesCondition = serde_json::from_str(&json).unwrap();
        match parsed {
            SeriesCondition::Year {
                year: NumberOperator::Gte { value },
            } => assert_eq!(value, 2000),
            _ => panic!("Expected Year/Gte condition"),
        }
    }

    #[test]
    fn test_year_condition_between_deserialization() {
        let json = r#"{"year":{"operator":"between","min":1990,"max":1999}}"#;
        let condition: SeriesCondition = serde_json::from_str(json).unwrap();
        match condition {
            SeriesCondition::Year {
                year: NumberOperator::Between { min, max },
            } => {
                assert_eq!(min, Some(1990));
                assert_eq!(max, Some(1999));
            }
            _ => panic!("Expected Year/Between condition"),
        }
    }

    #[test]
    fn test_author_condition_contains() {
        let json = r#"{"author":{"operator":"contains","value":"Toriyama"}}"#;
        let condition: SeriesCondition = serde_json::from_str(json).unwrap();
        match condition {
            SeriesCondition::Author {
                author: FieldOperator::Contains { value },
            } => assert_eq!(value, "Toriyama"),
            _ => panic!("Expected Author/Contains condition"),
        }
    }

    #[test]
    fn test_series_date_added_condition() {
        let json = r#"{"dateAdded":{"operator":"after","value":"2026-01-01T00:00:00Z"}}"#;
        let condition: SeriesCondition = serde_json::from_str(json).unwrap();
        match condition {
            SeriesCondition::DateAdded {
                date_added: DateOperator::After { .. },
            } => {}
            _ => panic!("Expected SeriesCondition::DateAdded/After"),
        }
    }

    #[test]
    fn test_series_title_condition_round_trip() {
        let condition = SeriesCondition::Title {
            title: FieldOperator::Contains {
                value: "Naruto".to_string(),
            },
        };
        let json = serde_json::to_string(&condition).unwrap();
        assert_eq!(
            json,
            r#"{"title":{"operator":"contains","value":"Naruto"}}"#
        );
        let parsed: SeriesCondition = serde_json::from_str(&json).unwrap();
        match parsed {
            SeriesCondition::Title {
                title: FieldOperator::Contains { value },
            } => assert_eq!(value, "Naruto"),
            _ => panic!("Expected SeriesCondition::Title/Contains"),
        }
    }

    #[test]
    fn test_series_path_condition_round_trip() {
        let condition = SeriesCondition::Path {
            path: FieldOperator::Contains {
                value: "/manga/".to_string(),
            },
        };
        let json = serde_json::to_string(&condition).unwrap();
        let parsed: SeriesCondition = serde_json::from_str(&json).unwrap();
        match parsed {
            SeriesCondition::Path {
                path: FieldOperator::Contains { value },
            } => assert_eq!(value, "/manga/"),
            _ => panic!("Expected SeriesCondition::Path/Contains"),
        }
    }

    #[test]
    fn test_book_title_sort_condition_begins_with() {
        let condition = BookCondition::TitleSort {
            title_sort: FieldOperator::BeginsWith {
                value: "A".to_string(),
            },
        };
        let json = serde_json::to_string(&condition).unwrap();
        assert!(json.contains(r#""titleSort""#));
        assert!(json.contains(r#""operator":"beginsWith""#));
        let parsed: BookCondition = serde_json::from_str(&json).unwrap();
        match parsed {
            BookCondition::TitleSort {
                title_sort: FieldOperator::BeginsWith { value },
            } => assert_eq!(value, "A"),
            _ => panic!("Expected BookCondition::TitleSort/BeginsWith"),
        }
    }

    #[test]
    fn test_book_path_condition_round_trip() {
        let condition = BookCondition::Path {
            path: FieldOperator::Contains {
                value: "/manga/".to_string(),
            },
        };
        let json = serde_json::to_string(&condition).unwrap();
        let parsed: BookCondition = serde_json::from_str(&json).unwrap();
        match parsed {
            BookCondition::Path {
                path: FieldOperator::Contains { value },
            } => assert_eq!(value, "/manga/"),
            _ => panic!("Expected Path/Contains condition"),
        }
    }

    #[test]
    fn test_book_format_condition_is_cbz() {
        let json = r#"{"format":{"operator":"is","value":"cbz"}}"#;
        let condition: BookCondition = serde_json::from_str(json).unwrap();
        match condition {
            BookCondition::Format {
                format: FieldOperator::Is { value },
            } => assert_eq!(value, "cbz"),
            _ => panic!("Expected Format/Is(cbz) condition"),
        }
    }

    #[test]
    fn test_book_page_count_between() {
        let json = r#"{"pageCount":{"operator":"between","min":100,"max":300}}"#;
        let condition: BookCondition = serde_json::from_str(json).unwrap();
        match condition {
            BookCondition::PageCount {
                page_count: NumberOperator::Between { min, max },
            } => {
                assert_eq!(min, Some(100));
                assert_eq!(max, Some(300));
            }
            _ => panic!("Expected PageCount/Between condition"),
        }
    }

    #[test]
    fn test_book_date_added_on_or_before() {
        let json = r#"{"dateAdded":{"operator":"onOrBefore","value":"2026-05-01T12:00:00Z"}}"#;
        let condition: BookCondition = serde_json::from_str(json).unwrap();
        match condition {
            BookCondition::DateAdded {
                date_added: DateOperator::OnOrBefore { .. },
            } => {}
            _ => panic!("Expected BookCondition::DateAdded/OnOrBefore"),
        }
    }

    #[test]
    fn test_date_operator_between_deserialization() {
        let json = r#"{
            "operator": "between",
            "start": "2026-01-01T00:00:00Z",
            "end": "2026-12-31T23:59:59Z"
        }"#;
        let op: DateOperator = serde_json::from_str(json).unwrap();
        match op {
            DateOperator::Between { start, end } => {
                assert!(start.is_some());
                assert!(end.is_some());
            }
            _ => panic!("Expected DateOperator::Between"),
        }
    }

    #[test]
    fn test_date_operator_is_null() {
        let json = r#"{"operator":"isNull"}"#;
        let op: DateOperator = serde_json::from_str(json).unwrap();
        assert!(matches!(op, DateOperator::IsNull));
    }
}
