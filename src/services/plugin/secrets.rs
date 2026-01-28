//! Secret handling utilities for plugin credentials.
//!
//! This module provides types that safely handle sensitive data like API keys,
//! tokens, and passwords. The main type is `SecretValue` which wraps `serde_json::Value`
//! and redacts its contents in Debug and Display implementations.
//!
//! ## Usage
//!
//! ```rust
//! use serde_json::json;
//! use codex::services::plugin::secrets::SecretValue;
//!
//! let secret = SecretValue::new(json!({"api_key": "sk-12345"}));
//!
//! // Debug output shows [REDACTED], not the actual value
//! println!("{:?}", secret);  // SecretValue([REDACTED])
//!
//! // Access the actual value when needed
//! let value = secret.inner();
//! ```

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt;

/// A wrapper for sensitive JSON values that redacts content in logs.
///
/// This type wraps a `serde_json::Value` that may contain sensitive data
/// (API keys, tokens, passwords, etc.) and ensures that the actual content
/// is never exposed in debug output or logs.
///
/// The value is still accessible through `inner()` when actually needed.
#[derive(Clone)]
pub struct SecretValue(Value);

impl SecretValue {
    /// Create a new secret value wrapper
    pub fn new(value: Value) -> Self {
        Self(value)
    }

    /// Get a reference to the underlying value
    pub fn inner(&self) -> &Value {
        &self.0
    }

    /// Consume the wrapper and return the underlying value
    pub fn into_inner(self) -> Value {
        self.0
    }
}

impl fmt::Debug for SecretValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SecretValue([REDACTED])")
    }
}

impl fmt::Display for SecretValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[REDACTED]")
    }
}

impl From<Value> for SecretValue {
    fn from(value: Value) -> Self {
        Self(value)
    }
}

impl From<SecretValue> for Value {
    fn from(secret: SecretValue) -> Self {
        secret.0
    }
}

impl Default for SecretValue {
    fn default() -> Self {
        Self(Value::Null)
    }
}

// Serialize passes through to underlying value (for sending to plugins)
impl Serialize for SecretValue {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.0.serialize(serializer)
    }
}

// Deserialize creates a SecretValue wrapper
impl<'de> Deserialize<'de> for SecretValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = Value::deserialize(deserializer)?;
        Ok(Self(value))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_debug_redacts_value() {
        let secret = SecretValue::new(json!({"api_key": "super_secret_key_12345"}));
        let debug_output = format!("{:?}", secret);

        assert_eq!(debug_output, "SecretValue([REDACTED])");
        assert!(!debug_output.contains("super_secret"));
        assert!(!debug_output.contains("api_key"));
    }

    #[test]
    fn test_display_redacts_value() {
        let secret = SecretValue::new(json!({"password": "hunter2"}));
        let display_output = format!("{}", secret);

        assert_eq!(display_output, "[REDACTED]");
        assert!(!display_output.contains("hunter2"));
    }

    #[test]
    fn test_inner_provides_access() {
        let original = json!({"token": "abc123"});
        let secret = SecretValue::new(original.clone());

        assert_eq!(secret.inner(), &original);
    }

    #[test]
    fn test_into_inner_returns_value() {
        let original = json!({"secret": "value"});
        let secret = SecretValue::new(original.clone());

        assert_eq!(secret.into_inner(), original);
    }

    #[test]
    fn test_serialization_passes_through() {
        let original = json!({"api_key": "test123"});
        let secret = SecretValue::new(original.clone());

        // Serialize the secret value
        let serialized = serde_json::to_string(&secret).unwrap();

        // The actual JSON should be serialized, not [REDACTED]
        assert_eq!(serialized, r#"{"api_key":"test123"}"#);
    }

    #[test]
    fn test_from_value() {
        let value = json!({"key": "value"});
        let secret: SecretValue = value.clone().into();

        assert_eq!(secret.inner(), &value);
    }

    #[test]
    fn test_into_value() {
        let original = json!({"data": "secret"});
        let secret = SecretValue::new(original.clone());
        let value: Value = secret.into();

        assert_eq!(value, original);
    }
}
