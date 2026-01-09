/// Utilities for ISBN (International Standard Book Number) validation and extraction.
///
/// This module provides functions for cleaning, validating, and extracting ISBNs from text.
/// It supports both ISBN-10 and ISBN-13 formats.
///
/// ISBN-10 format: 10 digits (last may be X for check digit 10)
/// ISBN-13 format: 13 digits (usually starting with 978 or 979)
use regex::Regex;
use std::sync::OnceLock;

/// Clean an ISBN string by removing common separators and whitespace.
///
/// This function:
/// - Removes hyphens, spaces, and dots
/// - Converts to uppercase (for 'X' check digit)
/// - Preserves only alphanumeric characters
///
/// # Examples
///
/// ```
/// use codex::parsers::isbn_utils::clean_isbn;
///
/// assert_eq!(clean_isbn("978-0-123-45678-9"), "9780123456789");
/// assert_eq!(clean_isbn("0-123-45678-X"), "012345678X");
/// assert_eq!(clean_isbn("978 0 123 45678 9"), "9780123456789");
/// ```
pub fn clean_isbn(isbn: &str) -> String {
    isbn.chars()
        .filter(|c| c.is_alphanumeric())
        .collect::<String>()
        .to_uppercase()
}

/// Check if a string is a valid ISBN-10 or ISBN-13.
///
/// This function validates both formats:
/// - ISBN-10: 10 characters, last may be 'X'
/// - ISBN-13: 13 digits
///
/// Note: This performs format validation only, not checksum verification.
/// For production use, consider adding checksum validation.
///
/// # Examples
///
/// ```
/// use codex::parsers::isbn_utils::is_valid_isbn;
///
/// assert!(is_valid_isbn("9780123456789"));
/// assert!(is_valid_isbn("012345678X"));
/// assert!(!is_valid_isbn("123"));
/// assert!(!is_valid_isbn("abc123"));
/// ```
pub fn is_valid_isbn(isbn: &str) -> bool {
    let cleaned = clean_isbn(isbn);

    match cleaned.len() {
        10 => {
            // ISBN-10: 9 digits + check digit (0-9 or X)
            let (digits, check) = cleaned.split_at(9);
            digits.chars().all(|c| c.is_ascii_digit())
                && (check == "X" || check.chars().all(|c| c.is_ascii_digit()))
        }
        13 => {
            // ISBN-13: 13 digits
            cleaned.chars().all(|c| c.is_ascii_digit())
        }
        _ => false,
    }
}

/// Validate ISBN-10 checksum.
///
/// The ISBN-10 check digit is calculated using modulus 11 with weights 10-1.
/// The check digit may be 'X' representing 10.
///
/// # Examples
///
/// ```
/// use codex::parsers::isbn_utils::validate_isbn10_checksum;
///
/// assert!(validate_isbn10_checksum("0306406152"));
/// assert!(validate_isbn10_checksum("043942089X"));
/// assert!(!validate_isbn10_checksum("0000000000"));
/// ```
pub fn validate_isbn10_checksum(isbn: &str) -> bool {
    let cleaned = clean_isbn(isbn);
    if cleaned.len() != 10 {
        return false;
    }

    // Reject all-zero ISBNs (not a valid ISBN even if checksum passes)
    if cleaned == "0000000000" {
        return false;
    }

    let mut sum = 0;
    for (i, ch) in cleaned.chars().enumerate() {
        let weight = 10 - i;
        let digit = if ch == 'X' && i == 9 {
            10
        } else if let Some(d) = ch.to_digit(10) {
            d as usize
        } else {
            return false;
        };
        sum += weight * digit;
    }

    sum % 11 == 0
}

/// Validate ISBN-13 checksum.
///
/// The ISBN-13 check digit is calculated using modulus 10 with alternating weights of 1 and 3.
///
/// # Examples
///
/// ```
/// use codex::parsers::isbn_utils::validate_isbn13_checksum;
///
/// assert!(validate_isbn13_checksum("9780306406157"));
/// assert!(validate_isbn13_checksum("9780134685991"));
/// assert!(!validate_isbn13_checksum("9780000000000"));
/// ```
pub fn validate_isbn13_checksum(isbn: &str) -> bool {
    let cleaned = clean_isbn(isbn);
    if cleaned.len() != 13 || !cleaned.chars().all(|c| c.is_ascii_digit()) {
        return false;
    }

    let mut sum = 0;
    for (i, ch) in cleaned.chars().enumerate() {
        let digit = ch.to_digit(10).unwrap() as usize;
        let weight = if i % 2 == 0 { 1 } else { 3 };
        sum += weight * digit;
    }

    sum % 10 == 0
}

/// Get compiled regex for ISBN extraction.
///
/// This regex matches both ISBN-10 and ISBN-13 patterns with various separators.
/// Patterns matched:
/// - ISBN-13: 978-X-XXX-XXXXX-X or 979-X-XXX-XXXXX-X
/// - ISBN-10: X-XXX-XXXXX-X
/// - With separators: hyphens, spaces, dots
fn isbn_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        // Match ISBN-13 (978/979 prefix) or ISBN-10 patterns with optional separators
        // This regex looks for:
        // 1. Optional "ISBN" prefix (with optional -10/-13 suffix and colon/space)
        // 2. Either an ISBN-13 (13 digits starting with 978/979) or ISBN-10 (10 digits)
        // 3. Separators (-, space, .) are optional between digits
        Regex::new(
            r"(?i)(?:ISBN(?:-?1[03])?:?\s*)?((?:97[89][\s\-\.]?\d[\s\-\.]?\d[\s\-\.]?\d[\s\-\.]?\d[\s\-\.]?\d[\s\-\.]?\d[\s\-\.]?\d[\s\-\.]?\d[\s\-\.]?\d[\s\-\.]?\d)|(?:\d[\s\-\.]?\d[\s\-\.]?\d[\s\-\.]?\d[\s\-\.]?\d[\s\-\.]?\d[\s\-\.]?\d[\s\-\.]?\d[\s\-\.]?\d[\s\-\.]?[\dXx]))\b"
        )
        .unwrap()
    })
}

/// Extract all potential ISBNs from a text string.
///
/// This function:
/// 1. Searches for ISBN patterns in the text
/// 2. Cleans each match
/// 3. Validates format (length and characters)
/// 4. Optionally validates checksums
/// 5. Returns unique, valid ISBNs
///
/// # Arguments
///
/// * `text` - Text to search for ISBNs
/// * `validate_checksum` - If true, only return ISBNs with valid checksums
///
/// # Examples
///
/// ```
/// use codex::parsers::isbn_utils::extract_isbns;
///
/// let text = "ISBN: 978-0-306-40615-7 and ISBN-10: 0-306-40615-2";
/// let isbns = extract_isbns(text, false);
/// assert_eq!(isbns.len(), 2);
/// ```
pub fn extract_isbns(text: &str, validate_checksum: bool) -> Vec<String> {
    let regex = isbn_regex();
    let mut isbns = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for cap in regex.captures_iter(text) {
        if let Some(matched) = cap.get(1) {
            let cleaned = clean_isbn(matched.as_str());

            // Validate format
            if !is_valid_isbn(&cleaned) {
                continue;
            }

            // Optionally validate checksum
            if validate_checksum {
                let valid_checksum = match cleaned.len() {
                    10 => validate_isbn10_checksum(&cleaned),
                    13 => validate_isbn13_checksum(&cleaned),
                    _ => false,
                };

                if !valid_checksum {
                    continue;
                }
            }

            // Add unique ISBNs only
            if seen.insert(cleaned.clone()) {
                isbns.push(cleaned);
            }
        }
    }

    isbns
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clean_isbn_removes_separators() {
        assert_eq!(clean_isbn("978-0-123-45678-9"), "9780123456789");
        assert_eq!(clean_isbn("978 0 123 45678 9"), "9780123456789");
        assert_eq!(clean_isbn("978.0.123.45678.9"), "9780123456789");
    }

    #[test]
    fn test_clean_isbn_converts_to_uppercase() {
        assert_eq!(clean_isbn("0-123-45678-x"), "012345678X");
        assert_eq!(clean_isbn("043942089x"), "043942089X");
    }

    #[test]
    fn test_clean_isbn_removes_isbn_prefix() {
        assert_eq!(clean_isbn("ISBN 978-0-123-45678-9"), "ISBN9780123456789");
        assert_eq!(
            clean_isbn("ISBN-13: 978-0-123-45678-9"),
            "ISBN139780123456789"
        );
    }

    #[test]
    fn test_is_valid_isbn_accepts_isbn13() {
        assert!(is_valid_isbn("9780123456789"));
        assert!(is_valid_isbn("978-0-123-45678-9"));
        assert!(is_valid_isbn("979 0 123 45678 9"));
    }

    #[test]
    fn test_is_valid_isbn_accepts_isbn10() {
        assert!(is_valid_isbn("0123456789"));
        assert!(is_valid_isbn("012345678X"));
        assert!(is_valid_isbn("0-123-45678-X"));
    }

    #[test]
    fn test_is_valid_isbn_rejects_invalid_length() {
        assert!(!is_valid_isbn("123"));
        assert!(!is_valid_isbn("12345678901234"));
        assert!(!is_valid_isbn(""));
    }

    #[test]
    fn test_is_valid_isbn_rejects_non_numeric() {
        assert!(!is_valid_isbn("abc123def4567"));
        assert!(!is_valid_isbn("978-abc-def-ghi-j"));
    }

    #[test]
    fn test_is_valid_isbn_rejects_x_not_at_end_isbn10() {
        assert!(!is_valid_isbn("X123456789"));
        assert!(!is_valid_isbn("012X456789"));
    }

    #[test]
    fn test_validate_isbn10_checksum_valid() {
        // Known valid ISBN-10s
        assert!(validate_isbn10_checksum("0306406152"));
        assert!(validate_isbn10_checksum("043942089X"));
        assert!(validate_isbn10_checksum("0-306-40615-2"));
    }

    #[test]
    fn test_validate_isbn10_checksum_invalid() {
        assert!(!validate_isbn10_checksum("1234567890"));
        assert!(!validate_isbn10_checksum("0306406153")); // Wrong check digit
        assert!(!validate_isbn10_checksum("1234512345")); // Wrong check digit
    }

    #[test]
    fn test_validate_isbn13_checksum_valid() {
        // Known valid ISBN-13s
        assert!(validate_isbn13_checksum("9780306406157"));
        assert!(validate_isbn13_checksum("9780134685991"));
        assert!(validate_isbn13_checksum("978-0-306-40615-7"));
    }

    #[test]
    fn test_validate_isbn13_checksum_invalid() {
        assert!(!validate_isbn13_checksum("9780000000000"));
        assert!(!validate_isbn13_checksum("9781234567890"));
        assert!(!validate_isbn13_checksum("9780306406158")); // Wrong check digit
    }

    #[test]
    fn test_extract_isbns_from_text() {
        let text = "This book ISBN: 978-0-306-40615-7 is great!";
        let isbns = extract_isbns(text, false);
        assert_eq!(isbns.len(), 1);
        assert_eq!(isbns[0], "9780306406157");
    }

    #[test]
    fn test_extract_isbns_multiple() {
        let text = "ISBN: 978-0-306-40615-7 and ISBN-10: 0-306-40615-2";
        let isbns = extract_isbns(text, false);
        assert_eq!(isbns.len(), 2);
        assert!(isbns.contains(&"9780306406157".to_string()));
        assert!(isbns.contains(&"0306406152".to_string()));
    }

    #[test]
    fn test_extract_isbns_with_checksum_validation() {
        let text = "Valid: 978-0-306-40615-7 Invalid: 978-0-000-00000-0";
        let isbns = extract_isbns(text, true);
        assert_eq!(isbns.len(), 1);
        assert_eq!(isbns[0], "9780306406157");
    }

    #[test]
    fn test_extract_isbns_deduplicates() {
        let text = "ISBN: 978-0-306-40615-7 and again 978-0-306-40615-7";
        let isbns = extract_isbns(text, false);
        assert_eq!(isbns.len(), 1);
    }

    #[test]
    fn test_extract_isbns_empty_text() {
        let isbns = extract_isbns("No ISBNs here!", false);
        assert_eq!(isbns.len(), 0);
    }

    #[test]
    fn test_extract_isbns_with_prefix_variations() {
        let text = "ISBN 9780306406157, ISBN-13: 978-0-134-68599-1, ISBN-10: 0-306-40615-2";
        let isbns = extract_isbns(text, false);
        assert_eq!(isbns.len(), 3);
    }
}
