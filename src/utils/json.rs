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

/// Merge a JSON patch into a base value using RFC 7386 (JSON Merge Patch) semantics.
///
/// Rules:
/// - If `patch` is an object and `base` is an object, merge keys recursively
/// - If a patch key's value is `null`, remove that key from the result
/// - If a patch key's value is non-null, set/overwrite that key recursively
/// - If `patch` is not an object, it replaces `base` entirely
///
/// See: <https://datatracker.ietf.org/doc/html/rfc7386>
pub fn json_merge_patch(base: &Value, patch: &Value) -> Value {
    if let Value::Object(patch_map) = patch {
        let mut result = if let Value::Object(_) = base {
            base.clone()
        } else {
            Value::Object(serde_json::Map::new())
        };

        if let Value::Object(ref mut result_map) = result {
            for (key, patch_value) in patch_map {
                if patch_value.is_null() {
                    result_map.remove(key);
                } else if let Some(existing) = result_map.get(key).cloned() {
                    result_map.insert(key.clone(), json_merge_patch(&existing, patch_value));
                } else {
                    result_map.insert(key.clone(), json_merge_patch(&Value::Null, patch_value));
                }
            }
        }

        result
    } else {
        patch.clone()
    }
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

    // ========================================================================
    // json_merge_patch tests (RFC 7386)
    // ========================================================================

    #[test]
    fn test_json_merge_patch_add_field() {
        let base = json!({"a": 1});
        let patch = json!({"b": 2});
        let result = json_merge_patch(&base, &patch);
        assert_eq!(result, json!({"a": 1, "b": 2}));
    }

    #[test]
    fn test_json_merge_patch_update_field() {
        let base = json!({"a": 1, "b": "old"});
        let patch = json!({"b": "new"});
        let result = json_merge_patch(&base, &patch);
        assert_eq!(result, json!({"a": 1, "b": "new"}));
    }

    #[test]
    fn test_json_merge_patch_delete_field() {
        let base = json!({"a": 1, "b": 2, "c": 3});
        let patch = json!({"b": null});
        let result = json_merge_patch(&base, &patch);
        assert_eq!(result, json!({"a": 1, "c": 3}));
    }

    #[test]
    fn test_json_merge_patch_nested_merge() {
        let base = json!({"a": {"x": 1, "y": 2}, "b": 3});
        let patch = json!({"a": {"y": 10, "z": 20}});
        let result = json_merge_patch(&base, &patch);
        assert_eq!(result, json!({"a": {"x": 1, "y": 10, "z": 20}, "b": 3}));
    }

    #[test]
    fn test_json_merge_patch_nested_delete() {
        let base = json!({"a": {"x": 1, "y": 2}});
        let patch = json!({"a": {"x": null}});
        let result = json_merge_patch(&base, &patch);
        assert_eq!(result, json!({"a": {"y": 2}}));
    }

    #[test]
    fn test_json_merge_patch_replace_non_object_base() {
        let base = json!("string");
        let patch = json!({"a": 1});
        let result = json_merge_patch(&base, &patch);
        assert_eq!(result, json!({"a": 1}));
    }

    #[test]
    fn test_json_merge_patch_non_object_patch_replaces() {
        let base = json!({"a": 1});
        let patch = json!("replaced");
        let result = json_merge_patch(&base, &patch);
        assert_eq!(result, json!("replaced"));
    }

    #[test]
    fn test_json_merge_patch_empty_patch_no_change() {
        let base = json!({"a": 1, "b": 2});
        let patch = json!({});
        let result = json_merge_patch(&base, &patch);
        assert_eq!(result, json!({"a": 1, "b": 2}));
    }

    #[test]
    fn test_json_merge_patch_null_base() {
        let base = Value::Null;
        let patch = json!({"a": 1});
        let result = json_merge_patch(&base, &patch);
        assert_eq!(result, json!({"a": 1}));
    }

    #[test]
    fn test_json_merge_patch_complex_scenario() {
        // Simulates a real bulk edit scenario
        let base = json!({
            "rating": 5,
            "notes": "Great series",
            "tags": ["favorite"],
            "nested": {"a": 1, "b": 2}
        });
        let patch = json!({
            "rating": 8,
            "notes": null,
            "status": "completed",
            "nested": {"b": null, "c": 3}
        });
        let result = json_merge_patch(&base, &patch);
        assert_eq!(
            result,
            json!({
                "rating": 8,
                "tags": ["favorite"],
                "status": "completed",
                "nested": {"a": 1, "c": 3}
            })
        );
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
