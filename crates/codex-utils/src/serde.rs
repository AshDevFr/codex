//! Shared serde helpers for serialization and deserialization.
//!
//! This module provides common serde helper functions used across DTOs and models.

use serde::{Deserialize, Deserializer};

/// Custom deserializer that distinguishes between absent field and explicit null.
///
/// Use this with `#[serde(default, deserialize_with = "deserialize_optional_nullable")]`
/// on `Option<serde_json::Value>` fields where you need to differentiate between:
/// - Absent field -> `None`
/// - Explicit null -> `Some(Value::Null)`
/// - Any value -> `Some(value)`
///
/// This is useful for PATCH endpoints where you want to:
/// - Skip updating a field when it's not provided
/// - Clear a field when it's explicitly set to null
/// - Set a field when a value is provided
pub fn deserialize_optional_nullable<'de, D>(
    deserializer: D,
) -> Result<Option<serde_json::Value>, D::Error>
where
    D: Deserializer<'de>,
{
    // Deserialize to Value where null becomes Value::Null (not None)
    // This is different from the default Option<T> behavior which treats null as None
    let value = serde_json::Value::deserialize(deserializer)?;
    Ok(Some(value))
}

/// Helper for `#[serde(skip_serializing_if = "is_false")]` to skip false boolean values.
///
/// Use this when you want to omit a boolean field from serialization when it's false.
pub fn is_false(b: &bool) -> bool {
    !b
}

/// Helper for `#[serde(default = "default_true")]` to default a boolean to true.
///
/// Use this when a boolean field should default to true when not provided.
pub fn default_true() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Serialize;

    #[derive(Debug, Deserialize, Serialize)]
    #[serde(rename_all = "camelCase")]
    struct TestStruct {
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "deserialize_optional_nullable"
        )]
        nullable_field: Option<serde_json::Value>,

        #[serde(default = "default_true")]
        bool_field: bool,

        #[serde(default, skip_serializing_if = "is_false")]
        skippable_bool: bool,
    }

    #[test]
    fn test_deserialize_optional_nullable_with_null() {
        let json = r#"{"nullableField": null}"#;
        let parsed: TestStruct = serde_json::from_str(json).unwrap();
        assert!(parsed.nullable_field.is_some());
        assert!(parsed.nullable_field.unwrap().is_null());
    }

    #[test]
    fn test_deserialize_optional_nullable_absent() {
        let json = r#"{}"#;
        let parsed: TestStruct = serde_json::from_str(json).unwrap();
        assert!(parsed.nullable_field.is_none());
    }

    #[test]
    fn test_deserialize_optional_nullable_with_value() {
        let json = r#"{"nullableField": "test"}"#;
        let parsed: TestStruct = serde_json::from_str(json).unwrap();
        assert!(parsed.nullable_field.is_some());
        assert_eq!(parsed.nullable_field.unwrap().as_str(), Some("test"));
    }

    #[test]
    fn test_default_true() {
        let json = r#"{}"#;
        let parsed: TestStruct = serde_json::from_str(json).unwrap();
        assert!(parsed.bool_field);
    }

    #[test]
    fn test_is_false_skips_serialization() {
        let test = TestStruct {
            nullable_field: None,
            bool_field: true,
            skippable_bool: false,
        };
        let json = serde_json::to_string(&test).unwrap();
        assert!(!json.contains("skippableBool"));
    }

    #[test]
    fn test_is_false_keeps_true() {
        let test = TestStruct {
            nullable_field: None,
            bool_field: true,
            skippable_bool: true,
        };
        let json = serde_json::to_string(&test).unwrap();
        assert!(json.contains("skippableBool"));
    }
}
