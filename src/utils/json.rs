//! JSON utility functions for custom metadata handling
//!
//! Provides conversion utilities between `String` (database storage) and
//! `serde_json::Value` (API representation) for custom metadata.

use serde_json::Value;

/// Maximum size for custom metadata JSON (64KB)
pub const MAX_CUSTOM_METADATA_SIZE: usize = 64 * 1024;

/// Convert a JSON string from database to `serde_json::Value` for API response.
/// Returns `None` if the string is `None` or invalid JSON.
pub fn parse_custom_metadata(json_str: Option<&str>) -> Option<Value> {
    json_str.and_then(|s| {
        if s.is_empty() {
            None
        } else {
            serde_json::from_str(s).ok()
        }
    })
}

/// Convert `serde_json::Value` to a JSON string for database storage.
/// Returns `None` if the value is `None` or `null`.
pub fn serialize_custom_metadata(value: Option<&Value>) -> Option<String> {
    value.and_then(|v| {
        if v.is_null() {
            None
        } else {
            Some(v.to_string())
        }
    })
}

/// Validate custom metadata JSON size.
/// Returns an error message if the JSON exceeds the maximum size.
pub fn validate_custom_metadata_size(value: Option<&Value>) -> Result<(), String> {
    if let Some(v) = value {
        let json_str = v.to_string();
        if json_str.len() > MAX_CUSTOM_METADATA_SIZE {
            return Err(format!(
                "Custom metadata exceeds maximum size of {} bytes (got {} bytes)",
                MAX_CUSTOM_METADATA_SIZE,
                json_str.len()
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_parse_custom_metadata_valid() {
        let json_str = Some(r#"{"key": "value", "nested": {"num": 42}}"#);
        let result = parse_custom_metadata(json_str);
        assert!(result.is_some());
        let value = result.unwrap();
        assert_eq!(value["key"], "value");
        assert_eq!(value["nested"]["num"], 42);
    }

    #[test]
    fn test_parse_custom_metadata_none() {
        assert!(parse_custom_metadata(None).is_none());
    }

    #[test]
    fn test_parse_custom_metadata_empty() {
        assert!(parse_custom_metadata(Some("")).is_none());
    }

    #[test]
    fn test_parse_custom_metadata_invalid() {
        assert!(parse_custom_metadata(Some("not valid json")).is_none());
    }

    #[test]
    fn test_serialize_custom_metadata_object() {
        let value = json!({"key": "value", "number": 123});
        let result = serialize_custom_metadata(Some(&value));
        assert!(result.is_some());
        let json_str = result.unwrap();
        // Parse back to verify round-trip
        let parsed: Value = serde_json::from_str(&json_str).unwrap();
        assert_eq!(parsed["key"], "value");
        assert_eq!(parsed["number"], 123);
    }

    #[test]
    fn test_serialize_custom_metadata_none() {
        assert!(serialize_custom_metadata(None).is_none());
    }

    #[test]
    fn test_serialize_custom_metadata_null() {
        let value = Value::Null;
        assert!(serialize_custom_metadata(Some(&value)).is_none());
    }

    #[test]
    fn test_validate_custom_metadata_size_valid() {
        let value = json!({"key": "value"});
        assert!(validate_custom_metadata_size(Some(&value)).is_ok());
    }

    #[test]
    fn test_validate_custom_metadata_size_too_large() {
        // Create a large JSON object
        let large_string = "x".repeat(MAX_CUSTOM_METADATA_SIZE + 1);
        let value = json!({"data": large_string});
        assert!(validate_custom_metadata_size(Some(&value)).is_err());
    }

    #[test]
    fn test_round_trip() {
        let original = json!({
            "reading_status": "completed",
            "notes": "Great series!",
            "tags": ["favorite", "reread"],
            "ratings": {
                "art": 9,
                "story": 8.5
            }
        });

        // Serialize to string
        let json_str = serialize_custom_metadata(Some(&original));
        assert!(json_str.is_some());

        // Parse back
        let parsed = parse_custom_metadata(json_str.as_deref());
        assert!(parsed.is_some());
        assert_eq!(parsed.unwrap(), original);
    }
}
