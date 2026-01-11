//! Utility types for PATCH request handling.
//!
//! This module provides the `PatchValue<T>` type which allows distinguishing between:
//! - Absent (field not included in JSON) -> Don't change the value
//! - Null (explicitly null) -> Clear/delete the value
//! - Value (has a value) -> Update to this value
//!
//! This is essential for proper PATCH semantics where omitting a field means "keep existing"
//! rather than "set to null".

use sea_orm::ActiveValue;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Represents a field in a PATCH request that can be:
/// - Absent (not included in JSON) -> Don't change
/// - Null (explicitly null) -> Clear/delete the value
/// - Value (has a value) -> Update to this value
///
/// # Usage
///
/// ```ignore
/// use codex::api::dto::patch::PatchValue;
/// use serde::Deserialize;
///
/// #[derive(Deserialize)]
/// pub struct PatchSeriesMetadataRequest {
///     #[serde(default)]
///     pub summary: PatchValue<String>,
///     #[serde(default)]
///     pub publisher: PatchValue<String>,
/// }
/// ```
///
/// In your handler, use `into_active_value()` to convert directly to SeaORM's ActiveValue:
/// ```text
/// // For optional database fields
/// series.summary = request.summary.into_active_value();
/// ```
///
/// Or use `to_active_value()` for manual handling:
/// ```text
/// use sea_orm::Set;
/// if let Some(opt) = request.summary.to_active_value() {
///     series.summary = Set(opt);
/// }
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum PatchValue<T> {
    /// Field was not included in the JSON request - don't change the existing value
    #[default]
    Absent,
    /// Field was explicitly set to null - clear/delete the value
    Null,
    /// Field has a value - update to this value
    Value(T),
}

impl<T> PatchValue<T> {
    /// Returns true if the field was absent (not included in JSON)
    pub fn is_absent(&self) -> bool {
        matches!(self, PatchValue::Absent)
    }

    /// Returns true if the field was explicitly set to null
    pub fn is_null(&self) -> bool {
        matches!(self, PatchValue::Null)
    }

    /// Returns true if the field has a value
    pub fn is_value(&self) -> bool {
        matches!(self, PatchValue::Value(_))
    }

    /// Returns a reference to the value if present
    pub fn as_option(&self) -> Option<&T> {
        match self {
            PatchValue::Value(v) => Some(v),
            _ => None,
        }
    }

    /// Converts to an Option, consuming self
    pub fn into_option(self) -> Option<T> {
        match self {
            PatchValue::Value(v) => Some(v),
            _ => None,
        }
    }

    /// Convert to Option<Option<T>> for SeaORM ActiveValue
    ///
    /// - Absent -> None (don't set the field)
    /// - Null -> Some(None) (set field to null)
    /// - Value(v) -> Some(Some(v)) (set field to value)
    pub fn to_active_value(self) -> Option<Option<T>> {
        match self {
            PatchValue::Absent => None,
            PatchValue::Null => Some(None),
            PatchValue::Value(v) => Some(Some(v)),
        }
    }

    /// Maps the inner value using a function, preserving Absent/Null states
    pub fn map<U, F: FnOnce(T) -> U>(self, f: F) -> PatchValue<U> {
        match self {
            PatchValue::Absent => PatchValue::Absent,
            PatchValue::Null => PatchValue::Null,
            PatchValue::Value(v) => PatchValue::Value(f(v)),
        }
    }
}

impl<T> PatchValue<T>
where
    T: Clone + Into<sea_orm::Value> + sea_orm::sea_query::Nullable,
{
    /// Convert to SeaORM ActiveValue for optional fields
    ///
    /// - Absent -> NotSet (don't update)
    /// - Null -> Set(None) (clear the value)
    /// - Value(v) -> Set(Some(v)) (set to value)
    pub fn into_active_value(self) -> ActiveValue<Option<T>> {
        match self {
            PatchValue::Absent => ActiveValue::NotSet,
            PatchValue::Null => ActiveValue::Set(None),
            PatchValue::Value(v) => ActiveValue::Set(Some(v)),
        }
    }
}

impl<'de, T: Deserialize<'de>> Deserialize<'de> for PatchValue<T> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        // This is only called when the field is present in JSON.
        // If the value is null, we get None; if it has a value, we get Some(v)
        let value: Option<T> = Option::deserialize(deserializer)?;
        Ok(match value {
            Some(v) => PatchValue::Value(v),
            None => PatchValue::Null,
        })
    }
}

impl<T: Serialize> Serialize for PatchValue<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            PatchValue::Absent => serializer.serialize_none(),
            PatchValue::Null => serializer.serialize_none(),
            PatchValue::Value(v) => v.serialize(serializer),
        }
    }
}

// Note: For OpenAPI schema support, the PatchSeriesMetadataRequest DTO
// uses schema(nullable = true) annotations on PatchValue fields since
// PatchValue<T> serializes as Option<T> (nullable type).

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Deserialize, Debug)]
    struct TestStruct {
        #[serde(default)]
        field: PatchValue<String>,
        #[serde(default)]
        number: PatchValue<i32>,
    }

    #[test]
    fn test_patch_value_absent() {
        let json = "{}";
        let t: TestStruct = serde_json::from_str(json).unwrap();
        assert!(t.field.is_absent());
        assert!(t.number.is_absent());
    }

    #[test]
    fn test_patch_value_null() {
        let json = r#"{"field": null, "number": null}"#;
        let t: TestStruct = serde_json::from_str(json).unwrap();
        assert!(t.field.is_null());
        assert!(t.number.is_null());
    }

    #[test]
    fn test_patch_value_value() {
        let json = r#"{"field": "hello", "number": 42}"#;
        let t: TestStruct = serde_json::from_str(json).unwrap();
        assert!(t.field.is_value());
        assert_eq!(t.field.as_option(), Some(&"hello".to_string()));
        assert!(t.number.is_value());
        assert_eq!(t.number.as_option(), Some(&42));
    }

    #[test]
    fn test_patch_value_mixed() {
        let json = r#"{"field": "hello"}"#;
        let t: TestStruct = serde_json::from_str(json).unwrap();
        assert!(t.field.is_value());
        assert!(t.number.is_absent());
    }

    #[test]
    fn test_to_active_value() {
        let absent: PatchValue<String> = PatchValue::Absent;
        let null: PatchValue<String> = PatchValue::Null;
        let value: PatchValue<String> = PatchValue::Value("test".to_string());

        assert_eq!(absent.to_active_value(), None);
        assert_eq!(null.to_active_value(), Some(None));
        assert_eq!(value.to_active_value(), Some(Some("test".to_string())));
    }

    #[test]
    fn test_map() {
        let value: PatchValue<i32> = PatchValue::Value(42);
        let mapped = value.map(|v| v.to_string());
        assert_eq!(mapped.as_option(), Some(&"42".to_string()));

        let null: PatchValue<i32> = PatchValue::Null;
        let mapped_null = null.map(|v| v.to_string());
        assert!(mapped_null.is_null());

        let absent: PatchValue<i32> = PatchValue::Absent;
        let mapped_absent = absent.map(|v| v.to_string());
        assert!(mapped_absent.is_absent());
    }

    #[test]
    fn test_into_option() {
        let value: PatchValue<String> = PatchValue::Value("test".to_string());
        assert_eq!(value.into_option(), Some("test".to_string()));

        let null: PatchValue<String> = PatchValue::Null;
        assert_eq!(null.into_option(), None);

        let absent: PatchValue<String> = PatchValue::Absent;
        assert_eq!(absent.into_option(), None);
    }
}
