//! Condition evaluator for auto-match conditions.
//!
//! This module provides functionality to evaluate conditions against a series
//! context to determine whether auto-matching should proceed.
//!
//! ## Features
//!
//! - Support for all comparison operators (is_null, equals, gte, contains, etc.)
//! - Mode-based evaluation (all = AND, any = OR)
//! - Short-circuit evaluation for performance
//! - Detailed evaluation results for debugging
//!
//! ## Example
//!
//! ```ignore
//! use codex::services::metadata::preprocessing::{
//!     AutoMatchConditions, ConditionMode, ConditionRule, ConditionOperator,
//!     SeriesContext, evaluate_conditions,
//! };
//!
//! let conditions = AutoMatchConditions::new(ConditionMode::All)
//!     .with_rule(ConditionRule::new("external_ids.plugin:mangabaka", ConditionOperator::IsNull))
//!     .with_rule(ConditionRule::with_value("book_count", ConditionOperator::Gte, json!(1)));
//!
//! let context = SeriesContext::new()
//!     .book_count(5);
//!
//! let result = evaluate_conditions(&conditions, &context);
//! assert!(result.passed); // No external ID and book_count >= 1
//! ```

use regex::Regex;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::RwLock;
use tracing::debug;

use super::context::{FieldValue, SeriesContext};
use super::types::{AutoMatchConditions, ConditionMode, ConditionOperator, ConditionRule};

// =============================================================================
// Evaluation Result
// =============================================================================

/// Result of evaluating conditions against a context.
#[derive(Debug, Clone)]
pub struct EvaluationResult {
    /// Whether all conditions passed according to the mode.
    pub passed: bool,
    /// Results for each individual rule.
    pub rule_results: Vec<RuleResult>,
    /// Reason for failure (if any).
    pub failure_reason: Option<String>,
}

/// Result of evaluating a single rule.
#[derive(Debug, Clone)]
pub struct RuleResult {
    /// The field path that was evaluated.
    pub field: String,
    /// The operator used.
    pub operator: ConditionOperator,
    /// Whether this rule passed.
    pub passed: bool,
    /// The actual value found (if any).
    pub actual_value: Option<String>,
    /// The expected value (if any).
    pub expected_value: Option<String>,
}

impl EvaluationResult {
    /// Create a passing result with no rules.
    pub fn pass() -> Self {
        Self {
            passed: true,
            rule_results: Vec::new(),
            failure_reason: None,
        }
    }

    /// Create a failing result with a reason.
    pub fn fail(reason: impl Into<String>) -> Self {
        Self {
            passed: false,
            rule_results: Vec::new(),
            failure_reason: Some(reason.into()),
        }
    }
}

// =============================================================================
// Condition Evaluator
// =============================================================================

/// Evaluator for auto-match conditions with regex caching.
pub struct ConditionEvaluator {
    /// Cache of compiled regex patterns.
    regex_cache: RwLock<HashMap<String, Option<Regex>>>,
}

impl Default for ConditionEvaluator {
    fn default() -> Self {
        Self::new()
    }
}

impl ConditionEvaluator {
    /// Create a new condition evaluator.
    pub fn new() -> Self {
        Self {
            regex_cache: RwLock::new(HashMap::new()),
        }
    }

    /// Evaluate conditions against a context.
    pub fn evaluate(
        &self,
        conditions: &AutoMatchConditions,
        context: &SeriesContext,
    ) -> EvaluationResult {
        // Empty conditions always pass
        if conditions.is_empty() {
            return EvaluationResult::pass();
        }

        let mut rule_results = Vec::with_capacity(conditions.rules.len());
        let mut any_passed = false;
        let mut all_passed = true;

        for rule in &conditions.rules {
            let result = self.evaluate_rule(rule, context);
            let passed = result.passed;
            rule_results.push(result);

            if passed {
                any_passed = true;
                // Short-circuit for "any" mode
                if conditions.mode == ConditionMode::Any {
                    debug!(
                        field = %rule.field,
                        "Condition passed (any mode, short-circuiting)"
                    );
                    return EvaluationResult {
                        passed: true,
                        rule_results,
                        failure_reason: None,
                    };
                }
            } else {
                all_passed = false;
                // Short-circuit for "all" mode
                if conditions.mode == ConditionMode::All {
                    debug!(
                        field = %rule.field,
                        "Condition failed (all mode, short-circuiting)"
                    );
                    return EvaluationResult {
                        passed: false,
                        rule_results,
                        failure_reason: Some(format!(
                            "Rule failed: {} {:?}",
                            rule.field, rule.operator
                        )),
                    };
                }
            }
        }

        // Final result based on mode
        let passed = match conditions.mode {
            ConditionMode::All => all_passed,
            ConditionMode::Any => any_passed,
        };

        let failure_reason = if !passed {
            Some(match conditions.mode {
                ConditionMode::All => "One or more conditions failed".to_string(),
                ConditionMode::Any => "No conditions passed".to_string(),
            })
        } else {
            None
        };

        EvaluationResult {
            passed,
            rule_results,
            failure_reason,
        }
    }

    /// Evaluate a single rule against a context.
    fn evaluate_rule(&self, rule: &ConditionRule, context: &SeriesContext) -> RuleResult {
        let field_value = context.get_field(&rule.field);

        let passed = match rule.operator {
            ConditionOperator::IsNull => self.is_null(&field_value),
            ConditionOperator::IsNotNull => !self.is_null(&field_value),
            ConditionOperator::Equals => self.equals(&field_value, rule.value.as_ref()),
            ConditionOperator::NotEquals => !self.equals(&field_value, rule.value.as_ref()),
            ConditionOperator::Gt => {
                self.compare_numeric(&field_value, rule.value.as_ref(), |a, b| a > b)
            }
            ConditionOperator::Gte => {
                self.compare_numeric(&field_value, rule.value.as_ref(), |a, b| a >= b)
            }
            ConditionOperator::Lt => {
                self.compare_numeric(&field_value, rule.value.as_ref(), |a, b| a < b)
            }
            ConditionOperator::Lte => {
                self.compare_numeric(&field_value, rule.value.as_ref(), |a, b| a <= b)
            }
            ConditionOperator::Contains => self.contains(&field_value, rule.value.as_ref()),
            ConditionOperator::NotContains => !self.contains(&field_value, rule.value.as_ref()),
            ConditionOperator::StartsWith => self.starts_with(&field_value, rule.value.as_ref()),
            ConditionOperator::EndsWith => self.ends_with(&field_value, rule.value.as_ref()),
            ConditionOperator::Matches => self.matches(&field_value, rule.value.as_ref()),
            ConditionOperator::In => self.in_array(&field_value, rule.value.as_ref()),
            ConditionOperator::NotIn => !self.in_array(&field_value, rule.value.as_ref()),
        };

        RuleResult {
            field: rule.field.clone(),
            operator: rule.operator,
            passed,
            actual_value: field_value.and_then(|v| v.as_string()),
            expected_value: rule.value.as_ref().map(|v| format!("{}", v)),
        }
    }

    // =========================================================================
    // Comparison Methods
    // =========================================================================

    fn is_null(&self, value: &Option<FieldValue>) -> bool {
        match value {
            None => true,
            Some(v) => v.is_null_or_empty(),
        }
    }

    fn equals(&self, value: &Option<FieldValue>, expected: Option<&Value>) -> bool {
        match (value, expected) {
            (None, None) => true,
            (None, Some(Value::Null)) => true,
            (Some(FieldValue::Null), None) => true,
            (Some(FieldValue::Null), Some(Value::Null)) => true,
            (Some(v), Some(e)) => self.values_equal(v, e),
            _ => false,
        }
    }

    fn values_equal(&self, value: &FieldValue, expected: &Value) -> bool {
        match (value, expected) {
            (FieldValue::String(s), Value::String(e)) => s == e,
            (FieldValue::Number(n), Value::Number(e)) => e
                .as_f64()
                .map(|e| (*n - e).abs() < f64::EPSILON)
                .unwrap_or(false),
            (FieldValue::Bool(b), Value::Bool(e)) => b == e,
            (FieldValue::Null, Value::Null) => true,
            // Try string comparison for mixed types
            _ => {
                let s1 = value.as_string();
                let s2 = match expected {
                    Value::String(s) => Some(s.clone()),
                    Value::Number(n) => Some(n.to_string()),
                    Value::Bool(b) => Some(b.to_string()),
                    _ => None,
                };
                s1.zip(s2).map(|(a, b)| a == b).unwrap_or(false)
            }
        }
    }

    fn compare_numeric(
        &self,
        value: &Option<FieldValue>,
        expected: Option<&Value>,
        cmp: fn(f64, f64) -> bool,
    ) -> bool {
        let actual = value.as_ref().and_then(|v| v.as_number());
        let expected = expected.and_then(|v| v.as_f64());

        match (actual, expected) {
            (Some(a), Some(e)) => cmp(a, e),
            _ => false,
        }
    }

    fn contains(&self, value: &Option<FieldValue>, search: Option<&Value>) -> bool {
        let haystack = value.as_ref().and_then(|v| v.as_string());
        let needle = search.and_then(|v| v.as_str());

        match (haystack, needle) {
            (Some(h), Some(n)) => h.contains(n),
            _ => false,
        }
    }

    fn starts_with(&self, value: &Option<FieldValue>, prefix: Option<&Value>) -> bool {
        let string = value.as_ref().and_then(|v| v.as_string());
        let prefix = prefix.and_then(|v| v.as_str());

        match (string, prefix) {
            (Some(s), Some(p)) => s.starts_with(p),
            _ => false,
        }
    }

    fn ends_with(&self, value: &Option<FieldValue>, suffix: Option<&Value>) -> bool {
        let string = value.as_ref().and_then(|v| v.as_string());
        let suffix = suffix.and_then(|v| v.as_str());

        match (string, suffix) {
            (Some(s), Some(p)) => s.ends_with(p),
            _ => false,
        }
    }

    fn matches(&self, value: &Option<FieldValue>, pattern: Option<&Value>) -> bool {
        let string = value.as_ref().and_then(|v| v.as_string());
        let pattern = pattern.and_then(|v| v.as_str());

        match (string, pattern) {
            (Some(s), Some(p)) => {
                if let Some(regex) = self.get_or_compile_regex(p) {
                    regex.is_match(&s)
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    fn in_array(&self, value: &Option<FieldValue>, array: Option<&Value>) -> bool {
        let actual = value.as_ref();
        let array = array.and_then(|v| v.as_array());

        match (actual, array) {
            (Some(v), Some(arr)) => arr.iter().any(|e| self.values_equal(v, e)),
            _ => false,
        }
    }

    // =========================================================================
    // Regex Caching
    // =========================================================================

    fn get_or_compile_regex(&self, pattern: &str) -> Option<Regex> {
        // Try to get from read lock first
        {
            let cache = self.regex_cache.read().unwrap();
            if let Some(cached) = cache.get(pattern) {
                return cached.clone();
            }
        }

        // Compile and cache
        let mut cache = self.regex_cache.write().unwrap();

        // Double-check after acquiring write lock
        if let Some(cached) = cache.get(pattern) {
            return cached.clone();
        }

        let regex = Regex::new(pattern).ok();
        cache.insert(pattern.to_string(), regex.clone());
        regex
    }
}

// =============================================================================
// Standalone Functions
// =============================================================================

/// Evaluate conditions against a context.
///
/// This is a convenience function that creates a temporary evaluator.
/// For repeated evaluations, prefer creating a `ConditionEvaluator` instance.
pub fn evaluate_conditions(
    conditions: &AutoMatchConditions,
    context: &SeriesContext,
) -> EvaluationResult {
    let evaluator = ConditionEvaluator::new();
    evaluator.evaluate(conditions, context)
}

/// Evaluate conditions and return just the pass/fail result.
pub fn should_match(conditions: &AutoMatchConditions, context: &SeriesContext) -> bool {
    evaluate_conditions(conditions, context).passed
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn evaluator() -> ConditionEvaluator {
        ConditionEvaluator::new()
    }

    // =========================================================================
    // Empty Conditions Tests
    // =========================================================================

    #[test]
    fn test_empty_conditions_pass() {
        let conditions = AutoMatchConditions::default();
        let context = SeriesContext::new();
        let result = evaluator().evaluate(&conditions, &context);
        assert!(result.passed);
    }

    // =========================================================================
    // Is Null / Is Not Null Tests
    // =========================================================================

    #[test]
    fn test_is_null_missing_field() {
        let conditions = AutoMatchConditions::new(ConditionMode::All).with_rule(
            ConditionRule::new("external_ids.plugin:mangabaka", ConditionOperator::IsNull),
        );
        let context = SeriesContext::new();
        let result = evaluator().evaluate(&conditions, &context);
        assert!(result.passed);
    }

    #[test]
    fn test_is_null_present_field() {
        let conditions = AutoMatchConditions::new(ConditionMode::All).with_rule(
            ConditionRule::new("external_ids.plugin:mangabaka", ConditionOperator::IsNull),
        );
        let context = SeriesContext::new().external_id("plugin:mangabaka", "12345");
        let result = evaluator().evaluate(&conditions, &context);
        assert!(!result.passed);
    }

    #[test]
    fn test_is_not_null_missing_field() {
        let conditions =
            AutoMatchConditions::new(ConditionMode::All).with_rule(ConditionRule::new(
                "external_ids.plugin:mangabaka",
                ConditionOperator::IsNotNull,
            ));
        let context = SeriesContext::new();
        let result = evaluator().evaluate(&conditions, &context);
        assert!(!result.passed);
    }

    #[test]
    fn test_is_not_null_present_field() {
        let conditions =
            AutoMatchConditions::new(ConditionMode::All).with_rule(ConditionRule::new(
                "external_ids.plugin:mangabaka",
                ConditionOperator::IsNotNull,
            ));
        let context = SeriesContext::new().external_id("plugin:mangabaka", "12345");
        let result = evaluator().evaluate(&conditions, &context);
        assert!(result.passed);
    }

    // =========================================================================
    // Numeric Comparison Tests
    // =========================================================================

    #[test]
    fn test_gte_pass() {
        let conditions = AutoMatchConditions::new(ConditionMode::All).with_rule(
            ConditionRule::with_value("book_count", ConditionOperator::Gte, json!(5)),
        );
        let context = SeriesContext::new().book_count(10);
        let result = evaluator().evaluate(&conditions, &context);
        assert!(result.passed);
    }

    #[test]
    fn test_gte_fail() {
        let conditions = AutoMatchConditions::new(ConditionMode::All).with_rule(
            ConditionRule::with_value("book_count", ConditionOperator::Gte, json!(10)),
        );
        let context = SeriesContext::new().book_count(5);
        let result = evaluator().evaluate(&conditions, &context);
        assert!(!result.passed);
    }

    #[test]
    fn test_gt() {
        let conditions = AutoMatchConditions::new(ConditionMode::All).with_rule(
            ConditionRule::with_value("book_count", ConditionOperator::Gt, json!(5)),
        );

        let context1 = SeriesContext::new().book_count(6);
        assert!(evaluator().evaluate(&conditions, &context1).passed);

        let context2 = SeriesContext::new().book_count(5);
        assert!(!evaluator().evaluate(&conditions, &context2).passed);
    }

    #[test]
    fn test_lt() {
        let conditions = AutoMatchConditions::new(ConditionMode::All).with_rule(
            ConditionRule::with_value("book_count", ConditionOperator::Lt, json!(10)),
        );

        let context1 = SeriesContext::new().book_count(5);
        assert!(evaluator().evaluate(&conditions, &context1).passed);

        let context2 = SeriesContext::new().book_count(10);
        assert!(!evaluator().evaluate(&conditions, &context2).passed);
    }

    #[test]
    fn test_lte() {
        let conditions = AutoMatchConditions::new(ConditionMode::All).with_rule(
            ConditionRule::with_value("book_count", ConditionOperator::Lte, json!(10)),
        );

        let context1 = SeriesContext::new().book_count(10);
        assert!(evaluator().evaluate(&conditions, &context1).passed);

        let context2 = SeriesContext::new().book_count(11);
        assert!(!evaluator().evaluate(&conditions, &context2).passed);
    }

    // =========================================================================
    // Equals / Not Equals Tests
    // =========================================================================

    #[test]
    fn test_equals_string() {
        let conditions =
            AutoMatchConditions::new(ConditionMode::All).with_rule(ConditionRule::with_value(
                "metadata.status",
                ConditionOperator::Equals,
                json!("ongoing"),
            ));

        let metadata = super::super::context::MetadataContext {
            status: Some("ongoing".to_string()),
            ..Default::default()
        };
        let context = SeriesContext::new().metadata(metadata);
        assert!(evaluator().evaluate(&conditions, &context).passed);

        let metadata2 = super::super::context::MetadataContext {
            status: Some("ended".to_string()),
            ..Default::default()
        };
        let context2 = SeriesContext::new().metadata(metadata2);
        assert!(!evaluator().evaluate(&conditions, &context2).passed);
    }

    #[test]
    fn test_not_equals_string() {
        let conditions =
            AutoMatchConditions::new(ConditionMode::All).with_rule(ConditionRule::with_value(
                "metadata.status",
                ConditionOperator::NotEquals,
                json!("ended"),
            ));

        let metadata = super::super::context::MetadataContext {
            status: Some("ongoing".to_string()),
            ..Default::default()
        };
        let context = SeriesContext::new().metadata(metadata);
        assert!(evaluator().evaluate(&conditions, &context).passed);
    }

    // =========================================================================
    // String Comparison Tests
    // =========================================================================

    #[test]
    fn test_contains() {
        let conditions =
            AutoMatchConditions::new(ConditionMode::All).with_rule(ConditionRule::with_value(
                "metadata.title",
                ConditionOperator::Contains,
                json!("Piece"),
            ));

        let metadata = super::super::context::MetadataContext {
            title: Some("One Piece".to_string()),
            ..Default::default()
        };
        let context = SeriesContext::new().metadata(metadata);
        assert!(evaluator().evaluate(&conditions, &context).passed);
    }

    #[test]
    fn test_not_contains() {
        let conditions =
            AutoMatchConditions::new(ConditionMode::All).with_rule(ConditionRule::with_value(
                "metadata.title",
                ConditionOperator::NotContains,
                json!("Naruto"),
            ));

        let metadata = super::super::context::MetadataContext {
            title: Some("One Piece".to_string()),
            ..Default::default()
        };
        let context = SeriesContext::new().metadata(metadata);
        assert!(evaluator().evaluate(&conditions, &context).passed);
    }

    #[test]
    fn test_starts_with() {
        let conditions =
            AutoMatchConditions::new(ConditionMode::All).with_rule(ConditionRule::with_value(
                "metadata.title",
                ConditionOperator::StartsWith,
                json!("One"),
            ));

        let metadata = super::super::context::MetadataContext {
            title: Some("One Piece".to_string()),
            ..Default::default()
        };
        let context = SeriesContext::new().metadata(metadata);
        assert!(evaluator().evaluate(&conditions, &context).passed);
    }

    #[test]
    fn test_ends_with() {
        let conditions =
            AutoMatchConditions::new(ConditionMode::All).with_rule(ConditionRule::with_value(
                "metadata.title",
                ConditionOperator::EndsWith,
                json!("Piece"),
            ));

        let metadata = super::super::context::MetadataContext {
            title: Some("One Piece".to_string()),
            ..Default::default()
        };
        let context = SeriesContext::new().metadata(metadata);
        assert!(evaluator().evaluate(&conditions, &context).passed);
    }

    #[test]
    fn test_matches_regex() {
        let conditions =
            AutoMatchConditions::new(ConditionMode::All).with_rule(ConditionRule::with_value(
                "metadata.title",
                ConditionOperator::Matches,
                json!(r"^One\s+\w+"),
            ));

        let metadata = super::super::context::MetadataContext {
            title: Some("One Piece".to_string()),
            ..Default::default()
        };
        let context = SeriesContext::new().metadata(metadata);
        assert!(evaluator().evaluate(&conditions, &context).passed);
    }

    #[test]
    fn test_matches_invalid_regex() {
        let conditions =
            AutoMatchConditions::new(ConditionMode::All).with_rule(ConditionRule::with_value(
                "metadata.title",
                ConditionOperator::Matches,
                json!("[invalid"),
            ));

        let metadata = super::super::context::MetadataContext {
            title: Some("One Piece".to_string()),
            ..Default::default()
        };
        let context = SeriesContext::new().metadata(metadata);
        // Invalid regex should fail the match
        assert!(!evaluator().evaluate(&conditions, &context).passed);
    }

    // =========================================================================
    // In / Not In Tests
    // =========================================================================

    #[test]
    fn test_in_array() {
        let conditions =
            AutoMatchConditions::new(ConditionMode::All).with_rule(ConditionRule::with_value(
                "metadata.status",
                ConditionOperator::In,
                json!(["ongoing", "hiatus"]),
            ));

        let metadata = super::super::context::MetadataContext {
            status: Some("ongoing".to_string()),
            ..Default::default()
        };
        let context = SeriesContext::new().metadata(metadata);
        assert!(evaluator().evaluate(&conditions, &context).passed);

        let metadata2 = super::super::context::MetadataContext {
            status: Some("ended".to_string()),
            ..Default::default()
        };
        let context2 = SeriesContext::new().metadata(metadata2);
        assert!(!evaluator().evaluate(&conditions, &context2).passed);
    }

    #[test]
    fn test_not_in_array() {
        let conditions =
            AutoMatchConditions::new(ConditionMode::All).with_rule(ConditionRule::with_value(
                "metadata.status",
                ConditionOperator::NotIn,
                json!(["ended", "abandoned"]),
            ));

        let metadata = super::super::context::MetadataContext {
            status: Some("ongoing".to_string()),
            ..Default::default()
        };
        let context = SeriesContext::new().metadata(metadata);
        assert!(evaluator().evaluate(&conditions, &context).passed);
    }

    // =========================================================================
    // Mode Tests (All vs Any)
    // =========================================================================

    #[test]
    fn test_all_mode_all_pass() {
        let conditions = AutoMatchConditions::new(ConditionMode::All)
            .with_rule(ConditionRule::with_value(
                "book_count",
                ConditionOperator::Gte,
                json!(1),
            ))
            .with_rule(ConditionRule::new(
                "external_ids.plugin:mangabaka",
                ConditionOperator::IsNull,
            ));

        let context = SeriesContext::new().book_count(5);
        let result = evaluator().evaluate(&conditions, &context);
        assert!(result.passed);
    }

    #[test]
    fn test_all_mode_one_fails() {
        let conditions = AutoMatchConditions::new(ConditionMode::All)
            .with_rule(ConditionRule::with_value(
                "book_count",
                ConditionOperator::Gte,
                json!(10),
            ))
            .with_rule(ConditionRule::new(
                "external_ids.plugin:mangabaka",
                ConditionOperator::IsNull,
            ));

        let context = SeriesContext::new().book_count(5);
        let result = evaluator().evaluate(&conditions, &context);
        assert!(!result.passed);
    }

    #[test]
    fn test_any_mode_one_passes() {
        let conditions = AutoMatchConditions::new(ConditionMode::Any)
            .with_rule(ConditionRule::with_value(
                "book_count",
                ConditionOperator::Gte,
                json!(10),
            ))
            .with_rule(ConditionRule::new(
                "external_ids.plugin:mangabaka",
                ConditionOperator::IsNull,
            ));

        let context = SeriesContext::new().book_count(5);
        let result = evaluator().evaluate(&conditions, &context);
        assert!(result.passed); // Second rule passes
    }

    #[test]
    fn test_any_mode_none_pass() {
        let conditions = AutoMatchConditions::new(ConditionMode::Any)
            .with_rule(ConditionRule::with_value(
                "book_count",
                ConditionOperator::Gte,
                json!(10),
            ))
            .with_rule(ConditionRule::new(
                "external_ids.plugin:mangabaka",
                ConditionOperator::IsNotNull,
            ));

        let context = SeriesContext::new().book_count(5);
        let result = evaluator().evaluate(&conditions, &context);
        assert!(!result.passed);
    }

    // =========================================================================
    // Lock Field Tests
    // =========================================================================

    #[test]
    fn test_lock_field_check() {
        let conditions =
            AutoMatchConditions::new(ConditionMode::All).with_rule(ConditionRule::with_value(
                "metadata.title_lock",
                ConditionOperator::Equals,
                json!(false),
            ));

        let metadata = super::super::context::MetadataContext {
            title_lock: false,
            ..Default::default()
        };
        let context = SeriesContext::new().metadata(metadata);
        assert!(evaluator().evaluate(&conditions, &context).passed);

        let metadata2 = super::super::context::MetadataContext {
            title_lock: true,
            ..Default::default()
        };
        let context2 = SeriesContext::new().metadata(metadata2);
        assert!(!evaluator().evaluate(&conditions, &context2).passed);
    }

    // =========================================================================
    // Custom Field Tests
    // =========================================================================

    #[test]
    fn test_custom_field() {
        let conditions =
            AutoMatchConditions::new(ConditionMode::All).with_rule(ConditionRule::with_value(
                "custom_metadata.myField",
                ConditionOperator::Equals,
                json!("myValue"),
            ));

        let context = SeriesContext::new().custom_metadata(json!({"myField": "myValue"}));
        assert!(evaluator().evaluate(&conditions, &context).passed);
    }

    // =========================================================================
    // Standalone Function Tests
    // =========================================================================

    #[test]
    fn test_evaluate_conditions_function() {
        let conditions = AutoMatchConditions::new(ConditionMode::All).with_rule(
            ConditionRule::with_value("book_count", ConditionOperator::Gte, json!(1)),
        );

        let context = SeriesContext::new().book_count(5);
        let result = evaluate_conditions(&conditions, &context);
        assert!(result.passed);
    }

    #[test]
    fn test_should_match_function() {
        let conditions = AutoMatchConditions::new(ConditionMode::All).with_rule(
            ConditionRule::with_value("book_count", ConditionOperator::Gte, json!(1)),
        );

        let context = SeriesContext::new().book_count(5);
        assert!(should_match(&conditions, &context));
    }

    // =========================================================================
    // Real-World Scenario Tests
    // =========================================================================

    #[test]
    fn test_typical_auto_match_conditions() {
        // Typical conditions: series has no external ID and has at least one book
        let conditions = AutoMatchConditions::new(ConditionMode::All)
            .with_rule(ConditionRule::new(
                "external_ids.plugin:mangabaka",
                ConditionOperator::IsNull,
            ))
            .with_rule(ConditionRule::with_value(
                "book_count",
                ConditionOperator::Gte,
                json!(1),
            ));

        // New series with books - should match
        let context1 = SeriesContext::new().book_count(5);
        assert!(evaluator().evaluate(&conditions, &context1).passed);

        // Series already has external ID - should not match
        let context2 = SeriesContext::new()
            .book_count(5)
            .external_id("plugin:mangabaka", "12345");
        assert!(!evaluator().evaluate(&conditions, &context2).passed);

        // Series with no books - should not match
        let context3 = SeriesContext::new().book_count(0);
        assert!(!evaluator().evaluate(&conditions, &context3).passed);
    }

    #[test]
    fn test_skip_locked_series() {
        // Skip series where title is locked (user manually edited)
        let conditions =
            AutoMatchConditions::new(ConditionMode::All).with_rule(ConditionRule::with_value(
                "metadata.title_lock",
                ConditionOperator::Equals,
                json!(false),
            ));

        let metadata1 = super::super::context::MetadataContext {
            title_lock: false,
            ..Default::default()
        };
        let context1 = SeriesContext::new().metadata(metadata1);
        assert!(evaluator().evaluate(&conditions, &context1).passed);

        let metadata2 = super::super::context::MetadataContext {
            title_lock: true,
            ..Default::default()
        };
        let context2 = SeriesContext::new().metadata(metadata2);
        assert!(!evaluator().evaluate(&conditions, &context2).passed);
    }
}
