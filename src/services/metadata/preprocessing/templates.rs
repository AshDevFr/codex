//! Handlebars template service for search query customization.
//!
//! This module provides a Handlebars-based template engine for customizing
//! search queries before they are sent to metadata plugins.
//!
//! ## Features
//!
//! - Matches frontend template helpers for consistency
//! - Safe execution with output limits
//! - Caching of compiled templates
//!
//! ## Available Helpers
//!
//! - `lowercase`, `uppercase`, `capitalize`: Case transformations
//! - `trim`: Remove leading/trailing whitespace
//! - `truncate`: Limit string length with optional suffix
//! - `replace`: Replace all occurrences of a substring
//! - `split`: Split and get item by index
//! - `join`: Join array with separator
//! - `default`: Provide fallback for null/empty values
//! - `urlencode`: URL-encode a string
//! - `length`: Get length of string or array
//! - `padStart`: Pad with leading characters
//! - `exists`, `ifEquals`, `ifNotEquals`: Conditional helpers
//! - `gt`, `lt`, `and`, `or`: Comparison helpers
//! - `math`: Basic arithmetic operations
//! - `lookup`: Dynamic property access
//!
//! ## Example
//!
//! ```ignore
//! use codex::services::metadata::preprocessing::templates::TemplateEngine;
//! use serde_json::json;
//!
//! let engine = TemplateEngine::new();
//! let template = "{{lowercase title}}";
//! let context = json!({"title": "One Piece"});
//! let result = engine.render(template, &context)?;
//! assert_eq!(result, "one piece");
//! ```

use handlebars::{
    Context, Handlebars, Helper, HelperDef, HelperResult, Output, RenderContext, Renderable,
};
use serde_json::Value;

// Maximum output length to prevent memory issues
const MAX_OUTPUT_LENGTH: usize = 10_000;

// =============================================================================
// Error Types
// =============================================================================

/// Errors that can occur during template operations.
#[derive(Debug, thiserror::Error)]
pub enum TemplateError {
    /// Template compilation error.
    #[error("Template compilation error: {0}")]
    CompilationError(String),

    /// Template rendering error.
    #[error("Template rendering error: {0}")]
    RenderError(String),

    /// Output exceeded maximum length.
    #[error("Template output exceeded maximum length ({MAX_OUTPUT_LENGTH} characters)")]
    OutputTooLarge,
}

// =============================================================================
// Template Engine
// =============================================================================

/// Handlebars template engine with safe helpers.
///
/// The engine is designed to match the frontend template helpers for consistency.
pub struct TemplateEngine {
    handlebars: Handlebars<'static>,
}

impl Default for TemplateEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl TemplateEngine {
    /// Create a new template engine with all helpers registered.
    pub fn new() -> Self {
        let mut handlebars = Handlebars::new();

        // Don't escape HTML - we're generating search queries, not HTML
        handlebars.register_escape_fn(handlebars::no_escape);

        // Disable strict mode to allow missing properties to return empty
        handlebars.set_strict_mode(false);

        // Register all helpers
        register_helpers(&mut handlebars);

        Self { handlebars }
    }

    /// Render a template with the given context.
    pub fn render(&self, template: &str, context: &Value) -> Result<String, TemplateError> {
        let output = self
            .handlebars
            .render_template(template, context)
            .map_err(|e| TemplateError::RenderError(e.to_string()))?;

        if output.len() > MAX_OUTPUT_LENGTH {
            return Err(TemplateError::OutputTooLarge);
        }

        Ok(output)
    }

    /// Validate a template by compiling and doing a test render.
    pub fn validate(&self, template: &str) -> Result<(), TemplateError> {
        // First try to compile
        self.handlebars
            .render_template(template, &serde_json::json!({}))
            .map_err(|e| TemplateError::CompilationError(e.to_string()))?;

        Ok(())
    }

    /// Get a list of all available helper names.
    pub fn available_helpers() -> Vec<&'static str> {
        vec![
            "lowercase",
            "uppercase",
            "capitalize",
            "trim",
            "truncate",
            "replace",
            "split",
            "join",
            "default",
            "urlencode",
            "length",
            "padStart",
            "exists",
            "ifEquals",
            "ifNotEquals",
            "gt",
            "lt",
            "and",
            "or",
            "math",
            "lookup",
            "includes",
        ]
    }
}

// =============================================================================
// Helper Registration
// =============================================================================

fn register_helpers(handlebars: &mut Handlebars<'static>) {
    // String transformation helpers
    handlebars.register_helper("lowercase", Box::new(LowercaseHelper));
    handlebars.register_helper("uppercase", Box::new(UppercaseHelper));
    handlebars.register_helper("capitalize", Box::new(CapitalizeHelper));
    handlebars.register_helper("trim", Box::new(TrimHelper));
    handlebars.register_helper("truncate", Box::new(TruncateHelper));
    handlebars.register_helper("replace", Box::new(ReplaceHelper));
    handlebars.register_helper("split", Box::new(SplitHelper));
    handlebars.register_helper("join", Box::new(JoinHelper));
    handlebars.register_helper("urlencode", Box::new(UrlencodeHelper));
    handlebars.register_helper("padStart", Box::new(PadStartHelper));

    // Value helpers
    handlebars.register_helper("default", Box::new(DefaultHelper));
    handlebars.register_helper("length", Box::new(LengthHelper));
    handlebars.register_helper("lookup", Box::new(LookupHelper));

    // Comparison helpers
    handlebars.register_helper("gt", Box::new(GtHelper));
    handlebars.register_helper("lt", Box::new(LtHelper));
    handlebars.register_helper("and", Box::new(AndHelper));
    handlebars.register_helper("or", Box::new(OrHelper));
    handlebars.register_helper("math", Box::new(MathHelper));

    // Block helpers
    handlebars.register_helper("exists", Box::new(ExistsHelper));
    handlebars.register_helper("ifEquals", Box::new(IfEqualsHelper));
    handlebars.register_helper("ifNotEquals", Box::new(IfNotEqualsHelper));
    handlebars.register_helper("includes", Box::new(IncludesHelper));
}

// =============================================================================
// Helper Implementations
// =============================================================================

// --- String Transformation Helpers ---

struct LowercaseHelper;

impl HelperDef for LowercaseHelper {
    fn call<'reg: 'rc, 'rc>(
        &self,
        h: &Helper<'rc>,
        _: &'reg Handlebars<'reg>,
        _: &'rc Context,
        _: &mut RenderContext<'reg, 'rc>,
        out: &mut dyn Output,
    ) -> HelperResult {
        let value = h.param(0).and_then(|v| v.value().as_str()).unwrap_or("");
        out.write(&value.to_lowercase())?;
        Ok(())
    }
}

struct UppercaseHelper;

impl HelperDef for UppercaseHelper {
    fn call<'reg: 'rc, 'rc>(
        &self,
        h: &Helper<'rc>,
        _: &'reg Handlebars<'reg>,
        _: &'rc Context,
        _: &mut RenderContext<'reg, 'rc>,
        out: &mut dyn Output,
    ) -> HelperResult {
        let value = h.param(0).and_then(|v| v.value().as_str()).unwrap_or("");
        out.write(&value.to_uppercase())?;
        Ok(())
    }
}

struct CapitalizeHelper;

impl HelperDef for CapitalizeHelper {
    fn call<'reg: 'rc, 'rc>(
        &self,
        h: &Helper<'rc>,
        _: &'reg Handlebars<'reg>,
        _: &'rc Context,
        _: &mut RenderContext<'reg, 'rc>,
        out: &mut dyn Output,
    ) -> HelperResult {
        let value = h.param(0).and_then(|v| v.value().as_str()).unwrap_or("");
        if value.is_empty() {
            return Ok(());
        }
        let mut chars = value.chars();
        if let Some(first) = chars.next() {
            let result = format!("{}{}", first.to_uppercase(), chars.as_str().to_lowercase());
            out.write(&result)?;
        }
        Ok(())
    }
}

struct TrimHelper;

impl HelperDef for TrimHelper {
    fn call<'reg: 'rc, 'rc>(
        &self,
        h: &Helper<'rc>,
        _: &'reg Handlebars<'reg>,
        _: &'rc Context,
        _: &mut RenderContext<'reg, 'rc>,
        out: &mut dyn Output,
    ) -> HelperResult {
        let value = h.param(0).and_then(|v| v.value().as_str()).unwrap_or("");
        out.write(value.trim())?;
        Ok(())
    }
}

struct TruncateHelper;

impl HelperDef for TruncateHelper {
    fn call<'reg: 'rc, 'rc>(
        &self,
        h: &Helper<'rc>,
        _: &'reg Handlebars<'reg>,
        _: &'rc Context,
        _: &mut RenderContext<'reg, 'rc>,
        out: &mut dyn Output,
    ) -> HelperResult {
        let value = h.param(0).and_then(|v| v.value().as_str()).unwrap_or("");
        let length = h.param(1).and_then(|v| v.value().as_u64()).unwrap_or(100) as usize;
        let suffix = h.param(2).and_then(|v| v.value().as_str()).unwrap_or("...");

        if value.len() <= length {
            out.write(value)?;
        } else {
            let truncated = &value[..value
                .char_indices()
                .take(length)
                .last()
                .map(|(i, c)| i + c.len_utf8())
                .unwrap_or(0)];
            out.write(truncated)?;
            out.write(suffix)?;
        }
        Ok(())
    }
}

struct ReplaceHelper;

impl HelperDef for ReplaceHelper {
    fn call<'reg: 'rc, 'rc>(
        &self,
        h: &Helper<'rc>,
        _: &'reg Handlebars<'reg>,
        _: &'rc Context,
        _: &mut RenderContext<'reg, 'rc>,
        out: &mut dyn Output,
    ) -> HelperResult {
        let value = h.param(0).and_then(|v| v.value().as_str()).unwrap_or("");
        let search = h.param(1).and_then(|v| v.value().as_str()).unwrap_or("");
        let replacement = h.param(2).and_then(|v| v.value().as_str()).unwrap_or("");

        out.write(&value.replace(search, replacement))?;
        Ok(())
    }
}

struct SplitHelper;

impl HelperDef for SplitHelper {
    fn call<'reg: 'rc, 'rc>(
        &self,
        h: &Helper<'rc>,
        _: &'reg Handlebars<'reg>,
        _: &'rc Context,
        _: &mut RenderContext<'reg, 'rc>,
        out: &mut dyn Output,
    ) -> HelperResult {
        let value = h.param(0).and_then(|v| v.value().as_str()).unwrap_or("");
        let separator = h.param(1).and_then(|v| v.value().as_str()).unwrap_or(" ");
        let index = h.param(2).and_then(|v| v.value().as_u64()).unwrap_or(0) as usize;

        let parts: Vec<&str> = value.split(separator).collect();
        if let Some(part) = parts.get(index) {
            out.write(part)?;
        }
        Ok(())
    }
}

struct JoinHelper;

impl HelperDef for JoinHelper {
    fn call<'reg: 'rc, 'rc>(
        &self,
        h: &Helper<'rc>,
        _: &'reg Handlebars<'reg>,
        _: &'rc Context,
        _: &mut RenderContext<'reg, 'rc>,
        out: &mut dyn Output,
    ) -> HelperResult {
        let array = h.param(0).map(|v| v.value());
        let separator = h.param(1).and_then(|v| v.value().as_str()).unwrap_or(", ");

        if let Some(Value::Array(arr)) = array {
            let strings: Vec<String> = arr
                .iter()
                .filter_map(|v| match v {
                    Value::String(s) => Some(s.clone()),
                    Value::Number(n) => Some(n.to_string()),
                    Value::Bool(b) => Some(b.to_string()),
                    _ => None,
                })
                .collect();
            out.write(&strings.join(separator))?;
        }
        Ok(())
    }
}

struct UrlencodeHelper;

impl HelperDef for UrlencodeHelper {
    fn call<'reg: 'rc, 'rc>(
        &self,
        h: &Helper<'rc>,
        _: &'reg Handlebars<'reg>,
        _: &'rc Context,
        _: &mut RenderContext<'reg, 'rc>,
        out: &mut dyn Output,
    ) -> HelperResult {
        let value = h.param(0).and_then(|v| v.value().as_str()).unwrap_or("");
        out.write(&urlencoding::encode(value))?;
        Ok(())
    }
}

struct PadStartHelper;

impl HelperDef for PadStartHelper {
    fn call<'reg: 'rc, 'rc>(
        &self,
        h: &Helper<'rc>,
        _: &'reg Handlebars<'reg>,
        _: &'rc Context,
        _: &mut RenderContext<'reg, 'rc>,
        out: &mut dyn Output,
    ) -> HelperResult {
        let value = h
            .param(0)
            .map(|v| match v.value() {
                Value::String(s) => s.clone(),
                Value::Number(n) => n.to_string(),
                _ => String::new(),
            })
            .unwrap_or_default();
        let length = h.param(1).and_then(|v| v.value().as_u64()).unwrap_or(2) as usize;
        let pad_char = h.param(2).and_then(|v| v.value().as_str()).unwrap_or("0");

        if value.len() >= length {
            out.write(&value)?;
        } else {
            let padding: String = pad_char
                .chars()
                .cycle()
                .take(length - value.len())
                .collect();
            out.write(&padding)?;
            out.write(&value)?;
        }
        Ok(())
    }
}

// --- Value Helpers ---

struct DefaultHelper;

impl HelperDef for DefaultHelper {
    fn call<'reg: 'rc, 'rc>(
        &self,
        h: &Helper<'rc>,
        _: &'reg Handlebars<'reg>,
        _: &'rc Context,
        _: &mut RenderContext<'reg, 'rc>,
        out: &mut dyn Output,
    ) -> HelperResult {
        let value = h.param(0).map(|v| v.value());
        let default = h.param(1).and_then(|v| v.value().as_str()).unwrap_or("");

        let output = match value {
            None => default.to_string(),
            Some(Value::Null) => default.to_string(),
            Some(Value::String(s)) if s.is_empty() => default.to_string(),
            Some(Value::String(s)) => s.clone(),
            Some(Value::Number(n)) => n.to_string(),
            Some(Value::Bool(b)) => b.to_string(),
            Some(_) => default.to_string(),
        };
        out.write(&output)?;
        Ok(())
    }
}

struct LengthHelper;

impl HelperDef for LengthHelper {
    fn call<'reg: 'rc, 'rc>(
        &self,
        h: &Helper<'rc>,
        _: &'reg Handlebars<'reg>,
        _: &'rc Context,
        _: &mut RenderContext<'reg, 'rc>,
        out: &mut dyn Output,
    ) -> HelperResult {
        let value = h.param(0).map(|v| v.value());

        let len = match value {
            Some(Value::Array(arr)) => arr.len(),
            Some(Value::String(s)) => s.len(),
            _ => 0,
        };
        out.write(&len.to_string())?;
        Ok(())
    }
}

struct LookupHelper;

impl HelperDef for LookupHelper {
    fn call<'reg: 'rc, 'rc>(
        &self,
        h: &Helper<'rc>,
        _: &'reg Handlebars<'reg>,
        _: &'rc Context,
        _: &mut RenderContext<'reg, 'rc>,
        out: &mut dyn Output,
    ) -> HelperResult {
        let obj = h.param(0).map(|v| v.value());
        let key = h.param(1).and_then(|v| v.value().as_str());

        if let (Some(Value::Object(map)), Some(k)) = (obj, key) {
            if let Some(value) = map.get(k) {
                let output = match value {
                    Value::String(s) => s.clone(),
                    Value::Number(n) => n.to_string(),
                    Value::Bool(b) => b.to_string(),
                    _ => String::new(),
                };
                out.write(&output)?;
            }
        }
        Ok(())
    }
}

// --- Comparison Helpers ---

struct GtHelper;

impl HelperDef for GtHelper {
    fn call<'reg: 'rc, 'rc>(
        &self,
        h: &Helper<'rc>,
        r: &'reg Handlebars<'reg>,
        ctx: &'rc Context,
        rc: &mut RenderContext<'reg, 'rc>,
        out: &mut dyn Output,
    ) -> HelperResult {
        let v1 = h.param(0).and_then(|v| v.value().as_f64()).unwrap_or(0.0);
        let v2 = h.param(1).and_then(|v| v.value().as_f64()).unwrap_or(0.0);

        let template = if v1 > v2 { h.template() } else { h.inverse() };

        if let Some(t) = template {
            t.render(r, ctx, rc, out)?;
        }
        Ok(())
    }
}

struct LtHelper;

impl HelperDef for LtHelper {
    fn call<'reg: 'rc, 'rc>(
        &self,
        h: &Helper<'rc>,
        r: &'reg Handlebars<'reg>,
        ctx: &'rc Context,
        rc: &mut RenderContext<'reg, 'rc>,
        out: &mut dyn Output,
    ) -> HelperResult {
        let v1 = h.param(0).and_then(|v| v.value().as_f64()).unwrap_or(0.0);
        let v2 = h.param(1).and_then(|v| v.value().as_f64()).unwrap_or(0.0);

        let template = if v1 < v2 { h.template() } else { h.inverse() };

        if let Some(t) = template {
            t.render(r, ctx, rc, out)?;
        }
        Ok(())
    }
}

struct AndHelper;

impl HelperDef for AndHelper {
    fn call<'reg: 'rc, 'rc>(
        &self,
        h: &Helper<'rc>,
        r: &'reg Handlebars<'reg>,
        ctx: &'rc Context,
        rc: &mut RenderContext<'reg, 'rc>,
        out: &mut dyn Output,
    ) -> HelperResult {
        let v1 = is_truthy(h.param(0).map(|v| v.value()));
        let v2 = is_truthy(h.param(1).map(|v| v.value()));

        let template = if v1 && v2 { h.template() } else { h.inverse() };

        if let Some(t) = template {
            t.render(r, ctx, rc, out)?;
        }
        Ok(())
    }
}

struct OrHelper;

impl HelperDef for OrHelper {
    fn call<'reg: 'rc, 'rc>(
        &self,
        h: &Helper<'rc>,
        r: &'reg Handlebars<'reg>,
        ctx: &'rc Context,
        rc: &mut RenderContext<'reg, 'rc>,
        out: &mut dyn Output,
    ) -> HelperResult {
        let v1 = is_truthy(h.param(0).map(|v| v.value()));
        let v2 = is_truthy(h.param(1).map(|v| v.value()));

        let template = if v1 || v2 { h.template() } else { h.inverse() };

        if let Some(t) = template {
            t.render(r, ctx, rc, out)?;
        }
        Ok(())
    }
}

struct MathHelper;

impl HelperDef for MathHelper {
    fn call<'reg: 'rc, 'rc>(
        &self,
        h: &Helper<'rc>,
        _: &'reg Handlebars<'reg>,
        _: &'rc Context,
        _: &mut RenderContext<'reg, 'rc>,
        out: &mut dyn Output,
    ) -> HelperResult {
        let v1 = h.param(0).and_then(|v| v.value().as_f64());
        let op = h.param(1).and_then(|v| v.value().as_str()).unwrap_or("");
        let v2 = h.param(2).and_then(|v| v.value().as_f64());

        if let (Some(a), Some(b)) = (v1, v2) {
            let result = match op {
                "+" => Some(a + b),
                "-" => Some(a - b),
                "*" => Some(a * b),
                "/" if b != 0.0 => Some(a / b),
                "%" if b != 0.0 => Some(a % b),
                _ => None,
            };

            if let Some(r) = result {
                // Output as integer if it's a whole number
                if r.fract() == 0.0 {
                    out.write(&(r as i64).to_string())?;
                } else {
                    out.write(&r.to_string())?;
                }
            }
        }
        Ok(())
    }
}

// --- Block Helpers ---

struct ExistsHelper;

impl HelperDef for ExistsHelper {
    fn call<'reg: 'rc, 'rc>(
        &self,
        h: &Helper<'rc>,
        r: &'reg Handlebars<'reg>,
        ctx: &'rc Context,
        rc: &mut RenderContext<'reg, 'rc>,
        out: &mut dyn Output,
    ) -> HelperResult {
        let value = h.param(0).map(|v| v.value());
        let exists = match value {
            None => false,
            Some(Value::Null) => false,
            Some(Value::String(s)) => !s.is_empty(),
            Some(_) => true,
        };

        let template = if exists { h.template() } else { h.inverse() };

        if let Some(t) = template {
            t.render(r, ctx, rc, out)?;
        }
        Ok(())
    }
}

struct IfEqualsHelper;

impl HelperDef for IfEqualsHelper {
    fn call<'reg: 'rc, 'rc>(
        &self,
        h: &Helper<'rc>,
        r: &'reg Handlebars<'reg>,
        ctx: &'rc Context,
        rc: &mut RenderContext<'reg, 'rc>,
        out: &mut dyn Output,
    ) -> HelperResult {
        let v1 = h.param(0).map(|v| v.value());
        let v2 = h.param(1).map(|v| v.value());

        let equals = match (v1, v2) {
            (Some(a), Some(b)) => values_equal(a, b),
            (None, None) => true,
            _ => false,
        };

        let template = if equals { h.template() } else { h.inverse() };

        if let Some(t) = template {
            t.render(r, ctx, rc, out)?;
        }
        Ok(())
    }
}

struct IfNotEqualsHelper;

impl HelperDef for IfNotEqualsHelper {
    fn call<'reg: 'rc, 'rc>(
        &self,
        h: &Helper<'rc>,
        r: &'reg Handlebars<'reg>,
        ctx: &'rc Context,
        rc: &mut RenderContext<'reg, 'rc>,
        out: &mut dyn Output,
    ) -> HelperResult {
        let v1 = h.param(0).map(|v| v.value());
        let v2 = h.param(1).map(|v| v.value());

        let not_equals = match (v1, v2) {
            (Some(a), Some(b)) => !values_equal(a, b),
            (None, None) => false,
            _ => true,
        };

        let template = if not_equals {
            h.template()
        } else {
            h.inverse()
        };

        if let Some(t) = template {
            t.render(r, ctx, rc, out)?;
        }
        Ok(())
    }
}

struct IncludesHelper;

impl HelperDef for IncludesHelper {
    fn call<'reg: 'rc, 'rc>(
        &self,
        h: &Helper<'rc>,
        r: &'reg Handlebars<'reg>,
        ctx: &'rc Context,
        rc: &mut RenderContext<'reg, 'rc>,
        out: &mut dyn Output,
    ) -> HelperResult {
        let value = h.param(0).and_then(|v| v.value().as_str()).unwrap_or("");
        let search = h.param(1).and_then(|v| v.value().as_str()).unwrap_or("");

        let template = if value.contains(search) {
            h.template()
        } else {
            h.inverse()
        };

        if let Some(t) = template {
            t.render(r, ctx, rc, out)?;
        }
        Ok(())
    }
}

// =============================================================================
// Helper Utilities
// =============================================================================

/// Check if a value is "truthy" (non-null, non-empty, non-zero, non-false).
fn is_truthy(value: Option<&Value>) -> bool {
    match value {
        None => false,
        Some(Value::Null) => false,
        Some(Value::Bool(b)) => *b,
        Some(Value::Number(n)) => n.as_f64().map(|f| f != 0.0).unwrap_or(false),
        Some(Value::String(s)) => !s.is_empty(),
        Some(Value::Array(a)) => !a.is_empty(),
        Some(Value::Object(o)) => !o.is_empty(),
    }
}

/// Check if two JSON values are equal.
fn values_equal(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::String(s1), Value::String(s2)) => s1 == s2,
        (Value::Number(n1), Value::Number(n2)) => n1
            .as_f64()
            .zip(n2.as_f64())
            .map(|(a, b)| (a - b).abs() < f64::EPSILON)
            .unwrap_or(false),
        (Value::Bool(b1), Value::Bool(b2)) => b1 == b2,
        (Value::Null, Value::Null) => true,
        // Try string comparison for mixed types
        _ => {
            let s1 = value_to_string(a);
            let s2 = value_to_string(b);
            s1 == s2
        }
    }
}

/// Convert a JSON value to a string for comparison.
fn value_to_string(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Null => String::new(),
        _ => String::new(),
    }
}

// =============================================================================
// Standalone Functions
// =============================================================================

/// Render a template with the given context.
///
/// This is a convenience function that creates a temporary engine.
/// For repeated use, prefer creating a `TemplateEngine` instance.
pub fn render_template(template: &str, context: &Value) -> Result<String, TemplateError> {
    let engine = TemplateEngine::new();
    engine.render(template, context)
}

/// Validate a template string.
pub fn validate_template(template: &str) -> Result<(), TemplateError> {
    let engine = TemplateEngine::new();
    engine.validate(template)
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn engine() -> TemplateEngine {
        TemplateEngine::new()
    }

    // =========================================================================
    // String Transformation Helper Tests
    // =========================================================================

    #[test]
    fn test_lowercase() {
        let result = engine().render("{{lowercase title}}", &json!({"title": "One Piece"}));
        assert_eq!(result.unwrap(), "one piece");
    }

    #[test]
    fn test_uppercase() {
        let result = engine().render("{{uppercase title}}", &json!({"title": "One Piece"}));
        assert_eq!(result.unwrap(), "ONE PIECE");
    }

    #[test]
    fn test_capitalize() {
        let result = engine().render("{{capitalize title}}", &json!({"title": "one piece"}));
        assert_eq!(result.unwrap(), "One piece");

        let result = engine().render("{{capitalize title}}", &json!({"title": "ONE PIECE"}));
        assert_eq!(result.unwrap(), "One piece");
    }

    #[test]
    fn test_trim() {
        let result = engine().render("{{trim title}}", &json!({"title": "  One Piece  "}));
        assert_eq!(result.unwrap(), "One Piece");
    }

    #[test]
    fn test_truncate() {
        let result = engine().render("{{truncate title 5}}", &json!({"title": "One Piece"}));
        assert_eq!(result.unwrap(), "One P...");

        let result = engine().render("{{truncate title 5 \"!\"}}", &json!({"title": "One Piece"}));
        assert_eq!(result.unwrap(), "One P!");

        let result = engine().render("{{truncate title 100}}", &json!({"title": "Short"}));
        assert_eq!(result.unwrap(), "Short");
    }

    #[test]
    fn test_replace() {
        let result = engine().render(
            "{{replace title \"Piece\" \"Peace\"}}",
            &json!({"title": "One Piece"}),
        );
        assert_eq!(result.unwrap(), "One Peace");
    }

    #[test]
    fn test_split() {
        let result = engine().render("{{split title \" \" 0}}", &json!({"title": "One Piece"}));
        assert_eq!(result.unwrap(), "One");

        let result = engine().render("{{split title \" \" 1}}", &json!({"title": "One Piece"}));
        assert_eq!(result.unwrap(), "Piece");

        let result = engine().render("{{split title \"-\" 0}}", &json!({"title": "foo-bar-baz"}));
        assert_eq!(result.unwrap(), "foo");
    }

    #[test]
    fn test_join() {
        let result = engine().render(
            "{{join genres \", \"}}",
            &json!({"genres": ["Action", "Adventure"]}),
        );
        assert_eq!(result.unwrap(), "Action, Adventure");

        let result = engine().render("{{join tags \" | \"}}", &json!({"tags": ["tag1", "tag2"]}));
        assert_eq!(result.unwrap(), "tag1 | tag2");
    }

    #[test]
    fn test_urlencode() {
        let result = engine().render("{{urlencode title}}", &json!({"title": "One Piece"}));
        assert_eq!(result.unwrap(), "One%20Piece");

        let result = engine().render("{{urlencode title}}", &json!({"title": "foo&bar=baz"}));
        assert_eq!(result.unwrap(), "foo%26bar%3Dbaz");
    }

    #[test]
    fn test_pad_start() {
        let result = engine().render("{{padStart num 3 \"0\"}}", &json!({"num": 5}));
        assert_eq!(result.unwrap(), "005");

        let result = engine().render("{{padStart num 5 \"0\"}}", &json!({"num": "42"}));
        assert_eq!(result.unwrap(), "00042");
    }

    // =========================================================================
    // Value Helper Tests
    // =========================================================================

    #[test]
    fn test_default() {
        let result = engine().render(
            "{{default title \"Unknown\"}}",
            &json!({"title": "One Piece"}),
        );
        assert_eq!(result.unwrap(), "One Piece");

        let result = engine().render("{{default title \"Unknown\"}}", &json!({}));
        assert_eq!(result.unwrap(), "Unknown");

        let result = engine().render("{{default title \"Unknown\"}}", &json!({"title": null}));
        assert_eq!(result.unwrap(), "Unknown");

        let result = engine().render("{{default title \"Unknown\"}}", &json!({"title": ""}));
        assert_eq!(result.unwrap(), "Unknown");
    }

    #[test]
    fn test_length() {
        let result = engine().render(
            "{{length genres}}",
            &json!({"genres": ["Action", "Adventure"]}),
        );
        assert_eq!(result.unwrap(), "2");

        let result = engine().render("{{length title}}", &json!({"title": "One Piece"}));
        assert_eq!(result.unwrap(), "9");
    }

    #[test]
    fn test_lookup() {
        let result = engine().render(
            "{{lookup data \"key\"}}",
            &json!({"data": {"key": "value"}}),
        );
        assert_eq!(result.unwrap(), "value");
    }

    // =========================================================================
    // Comparison Helper Tests
    // =========================================================================

    #[test]
    fn test_gt() {
        let result = engine().render("{{#gt count 5}}yes{{else}}no{{/gt}}", &json!({"count": 10}));
        assert_eq!(result.unwrap(), "yes");

        let result = engine().render("{{#gt count 5}}yes{{else}}no{{/gt}}", &json!({"count": 3}));
        assert_eq!(result.unwrap(), "no");
    }

    #[test]
    fn test_lt() {
        let result = engine().render("{{#lt count 5}}yes{{else}}no{{/lt}}", &json!({"count": 3}));
        assert_eq!(result.unwrap(), "yes");

        let result = engine().render("{{#lt count 5}}yes{{else}}no{{/lt}}", &json!({"count": 10}));
        assert_eq!(result.unwrap(), "no");
    }

    #[test]
    fn test_and() {
        let result = engine().render(
            "{{#and a b}}yes{{else}}no{{/and}}",
            &json!({"a": true, "b": true}),
        );
        assert_eq!(result.unwrap(), "yes");

        let result = engine().render(
            "{{#and a b}}yes{{else}}no{{/and}}",
            &json!({"a": true, "b": false}),
        );
        assert_eq!(result.unwrap(), "no");
    }

    #[test]
    fn test_or() {
        let result = engine().render(
            "{{#or a b}}yes{{else}}no{{/or}}",
            &json!({"a": false, "b": true}),
        );
        assert_eq!(result.unwrap(), "yes");

        let result = engine().render(
            "{{#or a b}}yes{{else}}no{{/or}}",
            &json!({"a": false, "b": false}),
        );
        assert_eq!(result.unwrap(), "no");
    }

    #[test]
    fn test_math() {
        let result = engine().render("{{math a \"+\" b}}", &json!({"a": 5, "b": 3}));
        assert_eq!(result.unwrap(), "8");

        let result = engine().render("{{math a \"-\" b}}", &json!({"a": 10, "b": 4}));
        assert_eq!(result.unwrap(), "6");

        let result = engine().render("{{math a \"*\" b}}", &json!({"a": 3, "b": 4}));
        assert_eq!(result.unwrap(), "12");

        let result = engine().render("{{math a \"/\" b}}", &json!({"a": 10, "b": 2}));
        assert_eq!(result.unwrap(), "5");
    }

    // =========================================================================
    // Block Helper Tests
    // =========================================================================

    #[test]
    fn test_exists() {
        let result = engine().render(
            "{{#exists title}}has title{{else}}no title{{/exists}}",
            &json!({"title": "One Piece"}),
        );
        assert_eq!(result.unwrap(), "has title");

        let result = engine().render(
            "{{#exists title}}has title{{else}}no title{{/exists}}",
            &json!({}),
        );
        assert_eq!(result.unwrap(), "no title");

        let result = engine().render(
            "{{#exists title}}has title{{else}}no title{{/exists}}",
            &json!({"title": ""}),
        );
        assert_eq!(result.unwrap(), "no title");
    }

    #[test]
    fn test_if_equals() {
        let result = engine().render(
            "{{#ifEquals status \"ongoing\"}}yes{{else}}no{{/ifEquals}}",
            &json!({"status": "ongoing"}),
        );
        assert_eq!(result.unwrap(), "yes");

        let result = engine().render(
            "{{#ifEquals status \"ongoing\"}}yes{{else}}no{{/ifEquals}}",
            &json!({"status": "completed"}),
        );
        assert_eq!(result.unwrap(), "no");
    }

    #[test]
    fn test_if_not_equals() {
        let result = engine().render(
            "{{#ifNotEquals status \"completed\"}}yes{{else}}no{{/ifNotEquals}}",
            &json!({"status": "ongoing"}),
        );
        assert_eq!(result.unwrap(), "yes");

        let result = engine().render(
            "{{#ifNotEquals status \"completed\"}}yes{{else}}no{{/ifNotEquals}}",
            &json!({"status": "completed"}),
        );
        assert_eq!(result.unwrap(), "no");
    }

    #[test]
    fn test_includes() {
        let result = engine().render(
            "{{#includes title \"Piece\"}}yes{{else}}no{{/includes}}",
            &json!({"title": "One Piece"}),
        );
        assert_eq!(result.unwrap(), "yes");

        let result = engine().render(
            "{{#includes title \"Naruto\"}}yes{{else}}no{{/includes}}",
            &json!({"title": "One Piece"}),
        );
        assert_eq!(result.unwrap(), "no");
    }

    // =========================================================================
    // Complex Template Tests
    // =========================================================================

    #[test]
    fn test_complex_template() {
        let template = "{{lowercase (default title \"untitled\")}}";
        let result = engine().render(template, &json!({"title": "One Piece"}));
        assert_eq!(result.unwrap(), "one piece");
    }

    #[test]
    fn test_template_with_missing_values() {
        let template = "{{title}} - {{year}}";
        let result = engine().render(template, &json!({"title": "One Piece"}));
        assert_eq!(result.unwrap(), "One Piece - ");
    }

    #[test]
    fn test_search_query_template() {
        // Simulate a search query template
        let template = "{{lowercase (trim title)}}{{#exists year}} {{year}}{{/exists}}";
        let result = engine().render(template, &json!({"title": "  One Piece  ", "year": 1999}));
        assert_eq!(result.unwrap(), "one piece 1999");
    }

    // =========================================================================
    // Validation Tests
    // =========================================================================

    #[test]
    fn test_validate_valid_template() {
        assert!(engine().validate("{{title}}").is_ok());
        assert!(engine().validate("{{#if title}}yes{{/if}}").is_ok());
        assert!(engine().validate("{{lowercase title}}").is_ok());
    }

    #[test]
    fn test_validate_invalid_template() {
        assert!(engine().validate("{{#if}}").is_err());
        assert!(engine().validate("{{/if}}").is_err());
    }

    // =========================================================================
    // Standalone Function Tests
    // =========================================================================

    #[test]
    fn test_render_template_function() {
        let result = render_template("{{lowercase title}}", &json!({"title": "TEST"}));
        assert_eq!(result.unwrap(), "test");
    }

    #[test]
    fn test_validate_template_function() {
        assert!(validate_template("{{title}}").is_ok());
        assert!(validate_template("{{#if}}").is_err());
    }

    // =========================================================================
    // Edge Case Tests
    // =========================================================================

    #[test]
    fn test_empty_template() {
        let result = engine().render("", &json!({}));
        assert_eq!(result.unwrap(), "");
    }

    #[test]
    fn test_plain_text_template() {
        let result = engine().render("Hello, World!", &json!({}));
        assert_eq!(result.unwrap(), "Hello, World!");
    }

    #[test]
    fn test_null_values() {
        let result = engine().render("{{title}}", &json!({"title": null}));
        assert_eq!(result.unwrap(), "");
    }

    #[test]
    fn test_helper_with_missing_param() {
        let result = engine().render("{{lowercase}}", &json!({}));
        assert_eq!(result.unwrap(), "");
    }
}
