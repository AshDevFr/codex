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
    /// Filter by series name/title
    Name { name: FieldOperator },
    /// Filter by read status (unread, in_progress, read)
    ReadStatus {
        #[serde(rename = "readStatus")]
        read_status: FieldOperator,
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
    /// Filter by book title
    Title { title: FieldOperator },
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
}

/// Request body for POST /series/list
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SeriesListRequest {
    /// Filter condition (optional - no condition returns all)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Option<Object>)]
    pub condition: Option<SeriesCondition>,

    /// Full-text search query (case-insensitive search on series name)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub full_text_search: Option<String>,

    /// Page number (0-indexed)
    #[serde(default)]
    pub page: u64,

    /// Items per page (default 20, max 100)
    #[serde(default = "default_page_size")]
    pub page_size: u64,

    /// Sort field and direction (e.g., "name,asc" or "createdAt,desc")
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sort: Option<String>,
}

/// Request body for POST /books/list
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BookListRequest {
    /// Filter condition (optional - no condition returns all)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Option<Object>)]
    pub condition: Option<BookCondition>,

    /// Full-text search query (case-insensitive search on book title)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub full_text_search: Option<String>,

    /// Page number (0-indexed)
    #[serde(default)]
    pub page: u64,

    /// Items per page (default 20, max 100)
    #[serde(default = "default_page_size")]
    pub page_size: u64,

    /// Sort field and direction (e.g., "title,asc" or "createdAt,desc")
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sort: Option<String>,
}

fn default_page_size() -> u64 {
    20
}

impl Default for SeriesListRequest {
    fn default() -> Self {
        Self {
            condition: None,
            full_text_search: None,
            page: 0,
            page_size: default_page_size(),
            sort: None,
        }
    }
}

impl Default for BookListRequest {
    fn default() -> Self {
        Self {
            condition: None,
            full_text_search: None,
            page: 0,
            page_size: default_page_size(),
            sort: None,
        }
    }
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
            full_text_search: None,
            page: 0,
            page_size: 20,
            sort: Some("name,asc".to_string()),
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains(r#""condition""#));
        assert!(json.contains(r#""page":0"#));
        assert!(json.contains(r#""pageSize":20"#));
        assert!(json.contains(r#""sort":"name,asc""#));
    }

    #[test]
    fn test_series_list_request_empty() {
        let request = SeriesListRequest::default();

        let json = serde_json::to_string(&request).unwrap();
        // Empty optional fields should be omitted
        assert!(!json.contains(r#""condition""#));
        assert!(!json.contains(r#""fullTextSearch""#));
        assert!(!json.contains(r#""sort""#));
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
}
