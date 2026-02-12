use anyhow::{Result, bail};

/// Normalize a cron expression to the 6-part format expected by `tokio-cron-scheduler`.
///
/// The `cron` crate (used by `tokio-cron-scheduler`) expects either:
/// - 6 fields: `sec min hour day_of_month month day_of_week`
/// - 7 fields: `sec min hour day_of_month month day_of_week year`
///
/// Standard Unix cron uses 5 fields: `min hour day_of_month month day_of_week`
///
/// This function detects 5-part expressions and prepends `0` as the seconds field,
/// converting them to valid 6-part expressions. Expressions that already have 6 or 7
/// fields are returned unchanged.
pub fn normalize_cron_expression(expr: &str) -> Result<String> {
    let trimmed = expr.trim();
    if trimmed.is_empty() {
        bail!("Cron expression cannot be empty");
    }

    let parts: Vec<&str> = trimmed.split_whitespace().collect();

    match parts.len() {
        5 => {
            // Standard Unix cron (min hour dom month dow) → prepend "0" for seconds
            Ok(format!("0 {}", trimmed))
        }
        6 | 7 => {
            // Already in tokio-cron-scheduler format
            Ok(trimmed.to_string())
        }
        n => {
            bail!(
                "Invalid cron expression '{}': expected 5, 6, or 7 fields, got {}",
                trimmed,
                n
            );
        }
    }
}

/// Validate that a cron expression can be parsed by the `cron` crate.
///
/// This normalizes 5-part expressions to 6-part first, then attempts to parse.
/// Returns the normalized expression on success.
pub fn validate_cron_expression(expr: &str) -> Result<String> {
    let normalized = normalize_cron_expression(expr)?;

    // Try to parse with the cron crate to catch invalid expressions
    use std::str::FromStr;
    cron::Schedule::from_str(&normalized).map_err(|e| {
        anyhow::anyhow!(
            "Invalid cron expression '{}' (normalized to '{}'): {}",
            expr,
            normalized,
            e
        )
    })?;

    Ok(normalized)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_5_part_to_6_part() {
        // Standard Unix cron → prepend seconds
        assert_eq!(
            normalize_cron_expression("*/6 * * * *").unwrap(),
            "0 */6 * * * *"
        );
        assert_eq!(
            normalize_cron_expression("0 3 * * *").unwrap(),
            "0 0 3 * * *"
        );
        assert_eq!(
            normalize_cron_expression("30 2 * * 1-5").unwrap(),
            "0 30 2 * * 1-5"
        );
    }

    #[test]
    fn test_normalize_6_part_unchanged() {
        assert_eq!(
            normalize_cron_expression("0 */6 * * * *").unwrap(),
            "0 */6 * * * *"
        );
        assert_eq!(
            normalize_cron_expression("0 0 3 * * *").unwrap(),
            "0 0 3 * * *"
        );
    }

    #[test]
    fn test_normalize_7_part_unchanged() {
        assert_eq!(
            normalize_cron_expression("0 0 3 * * * 2026").unwrap(),
            "0 0 3 * * * 2026"
        );
    }

    #[test]
    fn test_normalize_empty_expression() {
        assert!(normalize_cron_expression("").is_err());
        assert!(normalize_cron_expression("   ").is_err());
    }

    #[test]
    fn test_normalize_invalid_field_count() {
        assert!(normalize_cron_expression("* *").is_err());
        assert!(normalize_cron_expression("* * * * * * * *").is_err());
        assert!(normalize_cron_expression("*").is_err());
    }

    #[test]
    fn test_normalize_trims_whitespace() {
        assert_eq!(
            normalize_cron_expression("  */6 * * * *  ").unwrap(),
            "0 */6 * * * *"
        );
    }

    #[test]
    fn test_validate_valid_expressions() {
        // 5-part (gets normalized)
        assert_eq!(
            validate_cron_expression("*/6 * * * *").unwrap(),
            "0 */6 * * * *"
        );
        assert_eq!(
            validate_cron_expression("0 3 * * *").unwrap(),
            "0 0 3 * * *"
        );

        // 6-part (already correct)
        assert_eq!(
            validate_cron_expression("0 */6 * * * *").unwrap(),
            "0 */6 * * * *"
        );
    }

    #[test]
    fn test_validate_invalid_cron_syntax() {
        // Valid field count but invalid cron syntax
        assert!(validate_cron_expression("99 99 99 99 99").is_err());
        assert!(validate_cron_expression("abc def ghi jkl mno").is_err());
    }

    #[test]
    fn test_common_user_cron_expressions() {
        // Every 6 hours
        assert_eq!(
            validate_cron_expression("0 */6 * * *").unwrap(),
            "0 0 */6 * * *"
        );
        // Daily at 3am
        assert_eq!(
            validate_cron_expression("0 3 * * *").unwrap(),
            "0 0 3 * * *"
        );
        // Every hour
        assert_eq!(
            validate_cron_expression("0 * * * *").unwrap(),
            "0 0 * * * *"
        );
        // Every 30 minutes
        assert_eq!(
            validate_cron_expression("*/30 * * * *").unwrap(),
            "0 */30 * * * *"
        );
    }
}
