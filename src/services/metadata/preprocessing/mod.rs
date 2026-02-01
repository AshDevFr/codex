//! Preprocessing services for metadata matching.
//!
//! This module provides preprocessing utilities for series metadata matching:
//!
//! - **Title Preprocessing**: Apply regex rules to clean up series titles before search
//! - **Search Query Templates**: Handlebars templates for customizing search queries
//! - **Auto-Match Conditions**: Conditional logic to control when matching occurs
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                        SCAN TIME                                 │
//! ├─────────────────────────────────────────────────────────────────┤
//! │  Directory Name                                                  │
//! │       ↓                                                          │
//! │  series.name = original (preserved for file recognition)        │
//! │       ↓                                                          │
//! │  [Library title_preprocessing_rules]                             │
//! │       ↓                                                          │
//! │  series_metadata.title = cleaned (display & default search)     │
//! └─────────────────────────────────────────────────────────────────┘
//!                               ↓
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                      AUTO-MATCH TIME                             │
//! ├─────────────────────────────────────────────────────────────────┤
//! │  1. Check library auto_match_conditions → skip if fails          │
//! │  2. Check plugin auto_match_conditions → skip if fails           │
//! │  3. If use_existing_external_id && external ID exists:           │
//! │     → Call plugin.get(external_id) directly, skip to step 7     │
//! │  4. Apply plugin search_query_template (Handlebars)              │
//! │  5. Apply plugin search_preprocessing_rules                      │
//! │  6. Call plugin.search(query) → get best match                   │
//! │  7. Call plugin.get(external_id) for full metadata               │
//! │  8. Apply metadata (respecting locks/permissions)                │
//! │  9. Upsert series_external_ids record                            │
//! └─────────────────────────────────────────────────────────────────┘
//! ```
//!
//! ## Usage
//!
//! ### Title Preprocessing
//!
//! ```ignore
//! use codex::services::metadata::preprocessing::{PreprocessingRule, RuleEngine};
//!
//! let rules = vec![
//!     PreprocessingRule::new(r"\s*\(Digital\)$", ""),
//!     PreprocessingRule::new(r"\s*\[\w+\]", ""),
//! ];
//!
//! let engine = RuleEngine::new(rules);
//! let cleaned = engine.apply("One Piece (Digital) [Scan Group]");
//! assert_eq!(cleaned, "One Piece");
//! ```
//!
//! ### Search Query Templates
//!
//! ```ignore
//! use codex::services::metadata::preprocessing::templates::TemplateEngine;
//! use serde_json::json;
//!
//! let engine = TemplateEngine::new();
//! let template = "{{lowercase title}}{{#exists year}} {{year}}{{/exists}}";
//! let context = json!({"title": "One Piece", "year": 1999});
//! let query = engine.render(template, &context)?;
//! assert_eq!(query, "one piece 1999");
//! ```
//!
//! ### Auto-Match Conditions
//!
//! ```ignore
//! use codex::services::metadata::preprocessing::{
//!     AutoMatchConditions, ConditionMode, ConditionRule, ConditionOperator,
//!     SeriesContext, should_match,
//! };
//! use serde_json::json;
//!
//! let conditions = AutoMatchConditions::new(ConditionMode::All)
//!     .with_rule(ConditionRule::new("external_ids.plugin:mangabaka", ConditionOperator::IsNull))
//!     .with_rule(ConditionRule::with_value("book_count", ConditionOperator::Gte, json!(1)));
//!
//! let context = SeriesContext::new().book_count(5);
//! assert!(should_match(&conditions, &context));
//! ```

// Allow dead code in preprocessing submodules - these are public APIs for future use
#[allow(dead_code)]
pub mod conditions;
#[allow(dead_code)]
pub mod context;
#[allow(dead_code)]
pub mod rules;
#[allow(dead_code)]
pub mod templates;
#[allow(dead_code)]
pub mod types;

// Re-export commonly used types
// Note: Some types are only used in tests or by external consumers, hence allow(unused_imports)
#[allow(unused_imports)]
pub use conditions::should_match;
#[allow(unused_imports)]
pub use context::{MetadataContext, SeriesContext, SeriesContextBuilder};
#[allow(unused_imports)]
pub use rules::apply_rules;
#[allow(unused_imports)]
pub use templates::render_template;
#[allow(unused_imports)]
pub use types::{
    parse_auto_match_conditions, parse_preprocessing_rules, AutoMatchConditions, ConditionMode,
    ConditionOperator, ConditionRule, PreprocessingRule,
};
