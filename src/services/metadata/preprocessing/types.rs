//! Types for preprocessing rules and auto-match conditions.
//!
//! This module defines the data structures used for:
//! - Title preprocessing rules (regex-based transformations)
//! - Auto-match conditions (conditional logic for plugin matching)
//! - Condition operators (comparison operations)
//!
//! ## Example: Preprocessing Rules
//!
//! ```json
//! [
//!   {
//!     "pattern": "\\s*\\(Digital\\)$",
//!     "replacement": "",
//!     "description": "Remove (Digital) suffix",
//!     "enabled": true
//!   }
//! ]
//! ```
//!
//! ## Example: Auto-Match Conditions
//!
//! ```json
//! {
//!   "mode": "all",
//!   "rules": [
//!     {
//!       "field": "external_ids.plugin:mangabaka",
//!       "operator": "is_null"
//!     },
//!     {
//!       "field": "book_count",
//!       "operator": "gte",
//!       "value": 1
//!     }
//!   ]
//! }
//! ```

use serde::{Deserialize, Serialize};
use serde_json::Value;

// =============================================================================
// Preprocessing Rules
// =============================================================================

/// A single preprocessing rule that transforms text using regex.
///
/// Rules are applied in order during scan time to clean up series titles
/// before they are used for metadata searches.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PreprocessingRule {
    /// Regex pattern to match (uses Rust regex syntax)
    pub pattern: String,

    /// Replacement string (supports $1, $2, etc. for capture groups)
    pub replacement: String,

    /// Human-readable description of what this rule does
    #[serde(default)]
    pub description: Option<String>,

    /// Whether this rule is active (default: true)
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

fn default_enabled() -> bool {
    true
}

impl PreprocessingRule {
    /// Create a new preprocessing rule.
    pub fn new(pattern: impl Into<String>, replacement: impl Into<String>) -> Self {
        Self {
            pattern: pattern.into(),
            replacement: replacement.into(),
            description: None,
            enabled: true,
        }
    }

    /// Create a new preprocessing rule with a description.
    pub fn with_description(
        pattern: impl Into<String>,
        replacement: impl Into<String>,
        description: impl Into<String>,
    ) -> Self {
        Self {
            pattern: pattern.into(),
            replacement: replacement.into(),
            description: Some(description.into()),
            enabled: true,
        }
    }
}

// =============================================================================
// Auto-Match Conditions
// =============================================================================

/// Auto-match conditions that control when plugin matching should occur.
///
/// Conditions can be configured at both library and plugin levels:
/// - Library conditions are checked first (if any fail, skip auto-match for this library)
/// - Plugin conditions are checked second (if any fail, skip this plugin)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AutoMatchConditions {
    /// How to combine the rules: "all" (AND) or "any" (OR)
    #[serde(default)]
    pub mode: ConditionMode,

    /// List of condition rules to evaluate
    #[serde(default)]
    pub rules: Vec<ConditionRule>,
}

impl Default for AutoMatchConditions {
    fn default() -> Self {
        Self {
            mode: ConditionMode::All,
            rules: Vec::new(),
        }
    }
}

impl AutoMatchConditions {
    /// Create new conditions with the given mode.
    pub fn new(mode: ConditionMode) -> Self {
        Self {
            mode,
            rules: Vec::new(),
        }
    }

    /// Add a rule to the conditions.
    pub fn with_rule(mut self, rule: ConditionRule) -> Self {
        self.rules.push(rule);
        self
    }

    /// Check if there are any rules to evaluate.
    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }
}

/// How to combine multiple condition rules.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ConditionMode {
    /// All rules must pass (logical AND)
    #[default]
    All,
    /// Any rule must pass (logical OR)
    Any,
}

/// A single condition rule that evaluates a field against an operator.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConditionRule {
    /// Field path to evaluate (e.g., "book_count", "metadata.title", "external_ids.plugin:mangabaka")
    pub field: String,

    /// Comparison operator
    pub operator: ConditionOperator,

    /// Value to compare against (not required for is_null/is_not_null)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<Value>,
}

impl ConditionRule {
    /// Create a new condition rule.
    pub fn new(field: impl Into<String>, operator: ConditionOperator) -> Self {
        Self {
            field: field.into(),
            operator,
            value: None,
        }
    }

    /// Create a new condition rule with a value.
    pub fn with_value(field: impl Into<String>, operator: ConditionOperator, value: Value) -> Self {
        Self {
            field: field.into(),
            operator,
            value: Some(value),
        }
    }
}

/// Comparison operators for condition evaluation.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ConditionOperator {
    /// Field is null, empty, or missing
    IsNull,
    /// Field has a non-null, non-empty value
    IsNotNull,
    /// Exact match (string or number)
    Equals,
    /// Not equal
    NotEquals,
    /// Greater than (numeric)
    Gt,
    /// Greater than or equal (numeric)
    Gte,
    /// Less than (numeric)
    Lt,
    /// Less than or equal (numeric)
    Lte,
    /// String contains substring
    Contains,
    /// String does not contain substring
    NotContains,
    /// String starts with prefix
    StartsWith,
    /// String ends with suffix
    EndsWith,
    /// String matches regex pattern
    Matches,
    /// Value is in the provided array
    In,
    /// Value is not in the provided array
    NotIn,
}

impl ConditionOperator {
    /// Check if this operator requires a value.
    pub fn requires_value(&self) -> bool {
        !matches!(
            self,
            ConditionOperator::IsNull | ConditionOperator::IsNotNull
        )
    }

    /// Check if this operator is for numeric comparison.
    pub fn is_numeric(&self) -> bool {
        matches!(
            self,
            ConditionOperator::Gt
                | ConditionOperator::Gte
                | ConditionOperator::Lt
                | ConditionOperator::Lte
        )
    }

    /// Check if this operator is for string comparison.
    pub fn is_string(&self) -> bool {
        matches!(
            self,
            ConditionOperator::Contains
                | ConditionOperator::NotContains
                | ConditionOperator::StartsWith
                | ConditionOperator::EndsWith
                | ConditionOperator::Matches
        )
    }
}

// =============================================================================
// Parsing Helpers
// =============================================================================

/// Parse preprocessing rules from JSON string.
pub fn parse_preprocessing_rules(json: Option<&str>) -> Result<Vec<PreprocessingRule>, String> {
    match json {
        None => Ok(Vec::new()),
        Some(s) if s.trim().is_empty() => Ok(Vec::new()),
        Some(s) => serde_json::from_str(s)
            .map_err(|e| format!("Failed to parse preprocessing rules: {}", e)),
    }
}

/// Parse auto-match conditions from JSON string.
pub fn parse_auto_match_conditions(
    json: Option<&str>,
) -> Result<Option<AutoMatchConditions>, String> {
    match json {
        None => Ok(None),
        Some(s) if s.trim().is_empty() => Ok(None),
        Some(s) => serde_json::from_str(s)
            .map(Some)
            .map_err(|e| format!("Failed to parse auto-match conditions: {}", e)),
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // PreprocessingRule Tests
    // =========================================================================

    #[test]
    fn test_preprocessing_rule_new() {
        let rule = PreprocessingRule::new(r"\s*\(Digital\)$", "");
        assert_eq!(rule.pattern, r"\s*\(Digital\)$");
        assert_eq!(rule.replacement, "");
        assert!(rule.description.is_none());
        assert!(rule.enabled);
    }

    #[test]
    fn test_preprocessing_rule_with_description() {
        let rule =
            PreprocessingRule::with_description(r"\s*\(Digital\)$", "", "Remove (Digital) suffix");
        assert_eq!(
            rule.description,
            Some("Remove (Digital) suffix".to_string())
        );
    }

    #[test]
    fn test_preprocessing_rule_serialization() {
        let rule = PreprocessingRule {
            pattern: r"\s*\(Digital\)$".to_string(),
            replacement: "".to_string(),
            description: Some("Remove (Digital) suffix".to_string()),
            enabled: true,
        };

        let json = serde_json::to_string(&rule).unwrap();
        let parsed: PreprocessingRule = serde_json::from_str(&json).unwrap();
        assert_eq!(rule, parsed);
    }

    #[test]
    fn test_preprocessing_rule_default_enabled() {
        // When enabled is not specified, it should default to true
        let json = r#"{"pattern": "test", "replacement": ""}"#;
        let rule: PreprocessingRule = serde_json::from_str(json).unwrap();
        assert!(rule.enabled);
    }

    #[test]
    fn test_preprocessing_rules_array() {
        let json = r#"[
            {"pattern": "\\s*\\(Digital\\)$", "replacement": ""},
            {"pattern": "\\s+v\\d+$", "replacement": "", "description": "Remove version suffix", "enabled": false}
        ]"#;
        let rules: Vec<PreprocessingRule> = serde_json::from_str(json).unwrap();
        assert_eq!(rules.len(), 2);
        assert!(rules[0].enabled);
        assert!(!rules[1].enabled);
    }

    // =========================================================================
    // AutoMatchConditions Tests
    // =========================================================================

    #[test]
    fn test_auto_match_conditions_default() {
        let conditions = AutoMatchConditions::default();
        assert_eq!(conditions.mode, ConditionMode::All);
        assert!(conditions.rules.is_empty());
        assert!(conditions.is_empty());
    }

    #[test]
    fn test_auto_match_conditions_builder() {
        let conditions = AutoMatchConditions::new(ConditionMode::Any)
            .with_rule(ConditionRule::new("book_count", ConditionOperator::Gte))
            .with_rule(ConditionRule::new(
                "external_ids.count",
                ConditionOperator::IsNull,
            ));

        assert_eq!(conditions.mode, ConditionMode::Any);
        assert_eq!(conditions.rules.len(), 2);
        assert!(!conditions.is_empty());
    }

    #[test]
    fn test_auto_match_conditions_serialization() {
        let conditions = AutoMatchConditions {
            mode: ConditionMode::All,
            rules: vec![
                ConditionRule {
                    field: "external_ids.plugin:mangabaka".to_string(),
                    operator: ConditionOperator::IsNull,
                    value: None,
                },
                ConditionRule {
                    field: "book_count".to_string(),
                    operator: ConditionOperator::Gte,
                    value: Some(serde_json::json!(1)),
                },
            ],
        };

        let json = serde_json::to_string_pretty(&conditions).unwrap();
        let parsed: AutoMatchConditions = serde_json::from_str(&json).unwrap();
        assert_eq!(conditions, parsed);
    }

    #[test]
    fn test_auto_match_conditions_from_json() {
        let json = r#"{
            "mode": "all",
            "rules": [
                {"field": "external_ids.plugin:mangabaka", "operator": "is_null"},
                {"field": "book_count", "operator": "gte", "value": 1}
            ]
        }"#;
        let conditions: AutoMatchConditions = serde_json::from_str(json).unwrap();
        assert_eq!(conditions.mode, ConditionMode::All);
        assert_eq!(conditions.rules.len(), 2);
        assert_eq!(conditions.rules[0].operator, ConditionOperator::IsNull);
        assert_eq!(conditions.rules[1].operator, ConditionOperator::Gte);
    }

    // =========================================================================
    // ConditionRule Tests
    // =========================================================================

    #[test]
    fn test_condition_rule_new() {
        let rule = ConditionRule::new("book_count", ConditionOperator::IsNull);
        assert_eq!(rule.field, "book_count");
        assert_eq!(rule.operator, ConditionOperator::IsNull);
        assert!(rule.value.is_none());
    }

    #[test]
    fn test_condition_rule_with_value() {
        let rule =
            ConditionRule::with_value("book_count", ConditionOperator::Gte, serde_json::json!(5));
        assert_eq!(rule.field, "book_count");
        assert_eq!(rule.operator, ConditionOperator::Gte);
        assert_eq!(rule.value, Some(serde_json::json!(5)));
    }

    // =========================================================================
    // ConditionOperator Tests
    // =========================================================================

    #[test]
    fn test_condition_operator_requires_value() {
        assert!(!ConditionOperator::IsNull.requires_value());
        assert!(!ConditionOperator::IsNotNull.requires_value());
        assert!(ConditionOperator::Equals.requires_value());
        assert!(ConditionOperator::Gte.requires_value());
        assert!(ConditionOperator::Contains.requires_value());
    }

    #[test]
    fn test_condition_operator_is_numeric() {
        assert!(ConditionOperator::Gt.is_numeric());
        assert!(ConditionOperator::Gte.is_numeric());
        assert!(ConditionOperator::Lt.is_numeric());
        assert!(ConditionOperator::Lte.is_numeric());
        assert!(!ConditionOperator::Equals.is_numeric());
        assert!(!ConditionOperator::Contains.is_numeric());
    }

    #[test]
    fn test_condition_operator_is_string() {
        assert!(ConditionOperator::Contains.is_string());
        assert!(ConditionOperator::NotContains.is_string());
        assert!(ConditionOperator::StartsWith.is_string());
        assert!(ConditionOperator::EndsWith.is_string());
        assert!(ConditionOperator::Matches.is_string());
        assert!(!ConditionOperator::Equals.is_string());
        assert!(!ConditionOperator::Gt.is_string());
    }

    #[test]
    fn test_condition_operator_serialization() {
        let json = serde_json::to_string(&ConditionOperator::IsNull).unwrap();
        assert_eq!(json, "\"is_null\"");

        let json = serde_json::to_string(&ConditionOperator::Gte).unwrap();
        assert_eq!(json, "\"gte\"");

        let parsed: ConditionOperator = serde_json::from_str("\"not_contains\"").unwrap();
        assert_eq!(parsed, ConditionOperator::NotContains);
    }

    // =========================================================================
    // Parsing Helper Tests
    // =========================================================================

    #[test]
    fn test_parse_preprocessing_rules_none() {
        let result = parse_preprocessing_rules(None);
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn test_parse_preprocessing_rules_empty() {
        let result = parse_preprocessing_rules(Some(""));
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());

        let result = parse_preprocessing_rules(Some("   "));
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn test_parse_preprocessing_rules_valid() {
        let json = r#"[{"pattern": "test", "replacement": ""}]"#;
        let result = parse_preprocessing_rules(Some(json));
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 1);
    }

    #[test]
    fn test_parse_preprocessing_rules_invalid() {
        let result = parse_preprocessing_rules(Some("not json"));
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .contains("Failed to parse preprocessing rules")
        );
    }

    #[test]
    fn test_parse_auto_match_conditions_none() {
        let result = parse_auto_match_conditions(None);
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn test_parse_auto_match_conditions_empty() {
        let result = parse_auto_match_conditions(Some(""));
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn test_parse_auto_match_conditions_valid() {
        let json = r#"{"mode": "all", "rules": []}"#;
        let result = parse_auto_match_conditions(Some(json));
        assert!(result.is_ok());
        let conditions = result.unwrap().unwrap();
        assert_eq!(conditions.mode, ConditionMode::All);
    }

    #[test]
    fn test_parse_auto_match_conditions_invalid() {
        let result = parse_auto_match_conditions(Some("not json"));
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .contains("Failed to parse auto-match conditions")
        );
    }
}
