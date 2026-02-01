//! Preprocessing rule engine for applying regex transformations.
//!
//! This module provides functionality to apply preprocessing rules to text,
//! primarily used for cleaning up series titles before metadata searches.
//!
//! ## Features
//!
//! - Applies regex rules in order
//! - Caches compiled regex patterns for performance
//! - Handles errors gracefully (invalid patterns log warnings but don't fail)
//! - Supports capture group replacements ($1, $2, etc.)
//!
//! ## Example
//!
//! ```ignore
//! use codex::services::metadata::preprocessing::{PreprocessingRule, RuleEngine};
//!
//! let rules = vec![
//!     PreprocessingRule::new(r"\s*\(Digital\)$", ""),
//!     PreprocessingRule::new(r"\s+-\s+", " - "),
//! ];
//!
//! let engine = RuleEngine::new(rules)?;
//! let result = engine.apply("One Piece (Digital)")?;
//! assert_eq!(result, "One Piece");
//! ```

use regex::Regex;
use std::collections::HashMap;
use std::sync::RwLock;
use tracing::warn;

use super::types::PreprocessingRule;

// =============================================================================
// Error Types
// =============================================================================

/// Errors that can occur during rule application.
#[derive(Debug, thiserror::Error)]
pub enum RuleError {
    /// Invalid regex pattern in rule.
    #[error("Invalid regex pattern '{pattern}': {message}")]
    InvalidPattern { pattern: String, message: String },

    /// Rule application error.
    #[error("Failed to apply rule: {0}")]
    ApplicationError(String),
}

// =============================================================================
// Rule Engine
// =============================================================================

/// Engine for applying preprocessing rules to text.
///
/// The engine compiles regex patterns once and caches them for efficient
/// repeated use. Patterns are compiled lazily on first use.
pub struct RuleEngine {
    rules: Vec<PreprocessingRule>,
    /// Cache of compiled regex patterns, keyed by pattern string.
    /// Using RwLock for thread-safe access with minimal contention on reads.
    regex_cache: RwLock<HashMap<String, Option<Regex>>>,
}

impl RuleEngine {
    /// Create a new rule engine with the given rules.
    pub fn new(rules: Vec<PreprocessingRule>) -> Self {
        Self {
            rules,
            regex_cache: RwLock::new(HashMap::new()),
        }
    }

    /// Create an empty rule engine with no rules.
    pub fn empty() -> Self {
        Self::new(Vec::new())
    }

    /// Get the number of rules in the engine.
    pub fn rule_count(&self) -> usize {
        self.rules.len()
    }

    /// Check if the engine has any rules.
    pub fn has_rules(&self) -> bool {
        !self.rules.is_empty()
    }

    /// Apply all enabled rules to the input text in order.
    ///
    /// Returns the transformed text. If any rule has an invalid pattern,
    /// it is skipped with a warning.
    pub fn apply(&self, input: &str) -> String {
        let mut result = input.to_string();

        for rule in &self.rules {
            if !rule.enabled {
                continue;
            }

            match self.apply_rule(&result, rule) {
                Ok(transformed) => result = transformed,
                Err(e) => {
                    warn!(
                        pattern = %rule.pattern,
                        error = %e,
                        "Skipping invalid preprocessing rule"
                    );
                }
            }
        }

        result
    }

    /// Apply a single rule to the input text.
    fn apply_rule(&self, input: &str, rule: &PreprocessingRule) -> Result<String, RuleError> {
        let regex = self.get_or_compile_regex(&rule.pattern)?;
        Ok(regex
            .replace_all(input, rule.replacement.as_str())
            .to_string())
    }

    /// Get a compiled regex from cache or compile it.
    fn get_or_compile_regex(&self, pattern: &str) -> Result<Regex, RuleError> {
        // Try to get from read lock first (common case)
        {
            let cache = self.regex_cache.read().unwrap();
            if let Some(cached) = cache.get(pattern) {
                return cached.clone().ok_or_else(|| RuleError::InvalidPattern {
                    pattern: pattern.to_string(),
                    message: "Pattern previously failed to compile".to_string(),
                });
            }
        }

        // Not in cache, need to compile and store
        let mut cache = self.regex_cache.write().unwrap();

        // Double-check after acquiring write lock
        if let Some(cached) = cache.get(pattern) {
            return cached.clone().ok_or_else(|| RuleError::InvalidPattern {
                pattern: pattern.to_string(),
                message: "Pattern previously failed to compile".to_string(),
            });
        }

        // Compile the regex
        match Regex::new(pattern) {
            Ok(regex) => {
                cache.insert(pattern.to_string(), Some(regex.clone()));
                Ok(regex)
            }
            Err(e) => {
                // Cache the failure to avoid repeated compilation attempts
                cache.insert(pattern.to_string(), None);
                Err(RuleError::InvalidPattern {
                    pattern: pattern.to_string(),
                    message: e.to_string(),
                })
            }
        }
    }

    /// Validate a single pattern without applying it.
    pub fn validate_pattern(pattern: &str) -> Result<(), RuleError> {
        Regex::new(pattern)
            .map(|_| ())
            .map_err(|e| RuleError::InvalidPattern {
                pattern: pattern.to_string(),
                message: e.to_string(),
            })
    }

    /// Validate all rules in the engine.
    ///
    /// Returns a list of validation errors for rules with invalid patterns.
    pub fn validate(&self) -> Vec<RuleError> {
        self.rules
            .iter()
            .filter_map(|rule| Self::validate_pattern(&rule.pattern).err())
            .collect()
    }
}

// =============================================================================
// Standalone Functions
// =============================================================================

/// Apply preprocessing rules to text.
///
/// This is a convenience function that creates a temporary engine.
/// For repeated use, prefer creating a `RuleEngine` instance.
pub fn apply_rules(text: &str, rules: &[PreprocessingRule]) -> String {
    let engine = RuleEngine::new(rules.to_vec());
    engine.apply(text)
}

/// Validate preprocessing rules.
///
/// Returns Ok if all rules are valid, or Err with the list of invalid rules.
pub fn validate_rules(rules: &[PreprocessingRule]) -> Result<(), Vec<RuleError>> {
    let errors: Vec<RuleError> = rules
        .iter()
        .filter_map(|rule| RuleEngine::validate_pattern(&rule.pattern).err())
        .collect();

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // RuleEngine Tests
    // =========================================================================

    #[test]
    fn test_rule_engine_new() {
        let rules = vec![PreprocessingRule::new("test", "replacement")];
        let engine = RuleEngine::new(rules);
        assert_eq!(engine.rule_count(), 1);
        assert!(engine.has_rules());
    }

    #[test]
    fn test_rule_engine_empty() {
        let engine = RuleEngine::empty();
        assert_eq!(engine.rule_count(), 0);
        assert!(!engine.has_rules());
    }

    #[test]
    fn test_apply_no_rules() {
        let engine = RuleEngine::empty();
        let result = engine.apply("Test Input");
        assert_eq!(result, "Test Input");
    }

    #[test]
    fn test_apply_simple_replacement() {
        let rules = vec![PreprocessingRule::new("foo", "bar")];
        let engine = RuleEngine::new(rules);
        let result = engine.apply("foo baz foo");
        assert_eq!(result, "bar baz bar");
    }

    #[test]
    fn test_apply_regex_pattern() {
        let rules = vec![PreprocessingRule::new(r"\s*\(Digital\)$", "")];
        let engine = RuleEngine::new(rules);
        let result = engine.apply("One Piece (Digital)");
        assert_eq!(result, "One Piece");
    }

    #[test]
    fn test_apply_capture_groups() {
        let rules = vec![PreprocessingRule::new(r"(\w+)\s+(\w+)", "$2, $1")];
        let engine = RuleEngine::new(rules);
        let result = engine.apply("John Doe");
        assert_eq!(result, "Doe, John");
    }

    #[test]
    fn test_apply_multiple_rules_in_order() {
        // Rules applied in order: first trim, then normalize spaces, then remove suffix
        let rules = vec![
            PreprocessingRule::new(r"^\s+|\s+$", ""),       // Trim first
            PreprocessingRule::new(r"\s+", " "),            // Normalize spaces
            PreprocessingRule::new(r"\s*\(Digital\)$", ""), // Remove suffix last (after trimming)
        ];
        let engine = RuleEngine::new(rules);
        let result = engine.apply("  One  Piece  (Digital)  ");
        assert_eq!(result, "One Piece");
    }

    #[test]
    fn test_apply_disabled_rule_skipped() {
        let rules = vec![PreprocessingRule {
            pattern: "foo".to_string(),
            replacement: "bar".to_string(),
            description: None,
            enabled: false,
        }];
        let engine = RuleEngine::new(rules);
        let result = engine.apply("foo");
        assert_eq!(result, "foo"); // Rule was disabled, no change
    }

    #[test]
    fn test_apply_mixed_enabled_disabled() {
        let rules = vec![
            PreprocessingRule {
                pattern: "a".to_string(),
                replacement: "1".to_string(),
                description: None,
                enabled: true,
            },
            PreprocessingRule {
                pattern: "b".to_string(),
                replacement: "2".to_string(),
                description: None,
                enabled: false,
            },
            PreprocessingRule {
                pattern: "c".to_string(),
                replacement: "3".to_string(),
                description: None,
                enabled: true,
            },
        ];
        let engine = RuleEngine::new(rules);
        let result = engine.apply("abc");
        assert_eq!(result, "1b3"); // 'b' rule was disabled
    }

    #[test]
    fn test_apply_invalid_pattern_skipped() {
        let rules = vec![
            PreprocessingRule::new("[invalid", ""), // Invalid regex
            PreprocessingRule::new("valid", "replaced"),
        ];
        let engine = RuleEngine::new(rules);
        // Invalid pattern is skipped, valid pattern is applied
        let result = engine.apply("valid text");
        assert_eq!(result, "replaced text");
    }

    #[test]
    fn test_regex_caching() {
        let rules = vec![PreprocessingRule::new(r"\d+", "X")];
        let engine = RuleEngine::new(rules);

        // Apply twice to test caching
        let result1 = engine.apply("123 abc 456");
        let result2 = engine.apply("789 def 012");

        assert_eq!(result1, "X abc X");
        assert_eq!(result2, "X def X");

        // Check cache contains the pattern
        let cache = engine.regex_cache.read().unwrap();
        assert!(cache.contains_key(r"\d+"));
    }

    #[test]
    fn test_invalid_pattern_cached() {
        let rules = vec![PreprocessingRule::new("[invalid", "")];
        let engine = RuleEngine::new(rules);

        // Apply twice - both should skip the invalid pattern
        engine.apply("test");
        engine.apply("test");

        // Check cache contains the failed pattern
        let cache = engine.regex_cache.read().unwrap();
        assert!(cache.get("[invalid").unwrap().is_none());
    }

    // =========================================================================
    // Validation Tests
    // =========================================================================

    #[test]
    fn test_validate_pattern_valid() {
        assert!(RuleEngine::validate_pattern(r"\d+").is_ok());
        assert!(RuleEngine::validate_pattern(r"^\s*\(Digital\)$").is_ok());
        assert!(RuleEngine::validate_pattern("simple text").is_ok());
    }

    #[test]
    fn test_validate_pattern_invalid() {
        let result = RuleEngine::validate_pattern("[invalid");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, RuleError::InvalidPattern { .. }));
    }

    #[test]
    fn test_validate_all_valid() {
        let rules = vec![
            PreprocessingRule::new(r"\d+", "X"),
            PreprocessingRule::new(r"\s+", " "),
        ];
        let engine = RuleEngine::new(rules);
        let errors = engine.validate();
        assert!(errors.is_empty());
    }

    #[test]
    fn test_validate_with_invalid() {
        let rules = vec![
            PreprocessingRule::new(r"\d+", "X"),
            PreprocessingRule::new("[invalid", ""),
            PreprocessingRule::new(r"\s+", " "),
            PreprocessingRule::new("(unclosed", ""),
        ];
        let engine = RuleEngine::new(rules);
        let errors = engine.validate();
        assert_eq!(errors.len(), 2);
    }

    // =========================================================================
    // Standalone Function Tests
    // =========================================================================

    #[test]
    fn test_apply_rules_function() {
        let rules = vec![PreprocessingRule::new(r"\s*\(Digital\)$", "")];
        let result = apply_rules("Manga Title (Digital)", &rules);
        assert_eq!(result, "Manga Title");
    }

    #[test]
    fn test_validate_rules_function_valid() {
        let rules = vec![PreprocessingRule::new(r"\d+", "X")];
        assert!(validate_rules(&rules).is_ok());
    }

    #[test]
    fn test_validate_rules_function_invalid() {
        let rules = vec![PreprocessingRule::new("[invalid", "")];
        let result = validate_rules(&rules);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().len(), 1);
    }

    // =========================================================================
    // Real-World Pattern Tests
    // =========================================================================

    #[test]
    fn test_common_patterns_digital_suffix() {
        let rules = vec![PreprocessingRule::new(r"\s*\(Digital\)$", "")];
        let engine = RuleEngine::new(rules);

        assert_eq!(engine.apply("One Piece (Digital)"), "One Piece");
        assert_eq!(engine.apply("Naruto(Digital)"), "Naruto");
        assert_eq!(engine.apply("Bleach"), "Bleach"); // No change
    }

    #[test]
    fn test_common_patterns_volume_suffix() {
        let rules = vec![PreprocessingRule::new(r"\s+v\d+$", "")];
        let engine = RuleEngine::new(rules);

        assert_eq!(engine.apply("Series Name v01"), "Series Name");
        assert_eq!(engine.apply("Series Name v123"), "Series Name");
        assert_eq!(engine.apply("Series Name"), "Series Name"); // No change
    }

    #[test]
    fn test_common_patterns_year_suffix() {
        let rules = vec![PreprocessingRule::new(r"\s*\(\d{4}\)$", "")];
        let engine = RuleEngine::new(rules);

        assert_eq!(engine.apply("Batman (2016)"), "Batman");
        assert_eq!(engine.apply("Spider-Man (2023)"), "Spider-Man");
        assert_eq!(engine.apply("X-Men"), "X-Men"); // No change
    }

    #[test]
    fn test_common_patterns_brackets_content() {
        let rules = vec![PreprocessingRule::new(r"\s*\[[^\]]+\]", "")];
        let engine = RuleEngine::new(rules);

        assert_eq!(engine.apply("Manga [Scan Group]"), "Manga");
        assert_eq!(engine.apply("Title [v1] [HQ]"), "Title");
    }

    #[test]
    fn test_common_patterns_multiple_spaces() {
        let rules = vec![
            PreprocessingRule::new(r"\s+", " "),
            PreprocessingRule::new(r"^\s+|\s+$", ""),
        ];
        let engine = RuleEngine::new(rules);

        assert_eq!(engine.apply("  Too   Many   Spaces  "), "Too Many Spaces");
    }

    #[test]
    fn test_combined_preprocessing() {
        let rules = vec![
            // First, remove bracket content like [Scan Group] (can appear anywhere)
            PreprocessingRule::new(r"\s*\[[^\]]+\]", ""),
            // Trim whitespace
            PreprocessingRule::new(r"^\s+|\s+$", ""),
            // Remove (Digital) suffix (at end after trimming)
            PreprocessingRule::new(r"\s*\(Digital\)$", ""),
            // Remove year suffix like (2016) (at end)
            PreprocessingRule::new(r"\s*\(\d{4}\)$", ""),
            // Normalize multiple spaces
            PreprocessingRule::new(r"\s+", " "),
            // Final trim
            PreprocessingRule::new(r"^\s+|\s+$", ""),
        ];
        let engine = RuleEngine::new(rules);

        assert_eq!(
            engine.apply("  One Piece [Digital] (Digital) "),
            "One Piece"
        );
        assert_eq!(engine.apply("Batman (2016) [Scan Group]"), "Batman");
    }
}
