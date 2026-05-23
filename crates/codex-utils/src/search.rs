use unicode_normalization::UnicodeNormalization;

/// Check if a character is a Latin combining diacritical mark (accents, umlauts, etc.).
///
/// Only strips marks in the "Combining Diacritical Marks" block (U+0300-U+036F),
/// preserving combining marks used in other scripts (e.g., Japanese dakuten/handakuten).
fn is_latin_combining_mark(c: char) -> bool {
    ('\u{0300}'..='\u{036F}').contains(&c)
}

/// Normalize a string for accent-insensitive, case-insensitive search.
///
/// Performs NFD Unicode decomposition, strips Latin combining diacritical marks
/// (accents, umlauts, cedillas, etc.), and lowercases the result.
/// This allows matching "MÄR" with "mar", "Café" with "cafe", etc.
///
/// Non-Latin scripts (Japanese, Korean, Chinese, etc.) are preserved as-is,
/// including their combining marks (dakuten/handakuten for kana).
pub fn normalize_for_search(input: &str) -> String {
    input
        .nfd()
        .filter(|c| !is_latin_combining_mark(*c))
        .nfc()
        .collect::<String>()
        .to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_ascii() {
        assert_eq!(normalize_for_search("Hello World"), "hello world");
        assert_eq!(normalize_for_search("UPPERCASE"), "uppercase");
        assert_eq!(normalize_for_search("lowercase"), "lowercase");
    }

    #[test]
    fn test_normalize_german_umlauts() {
        assert_eq!(normalize_for_search("MÄR"), "mar");
        assert_eq!(normalize_for_search("Märchen"), "marchen");
        assert_eq!(normalize_for_search("Über"), "uber");
        assert_eq!(normalize_for_search("Köln"), "koln");
        assert_eq!(normalize_for_search("Düsseldorf"), "dusseldorf");
    }

    #[test]
    fn test_normalize_french_accents() {
        assert_eq!(normalize_for_search("Café"), "cafe");
        assert_eq!(normalize_for_search("naïve"), "naive");
        assert_eq!(normalize_for_search("résumé"), "resume");
        assert_eq!(normalize_for_search("crème brûlée"), "creme brulee");
    }

    #[test]
    fn test_normalize_spanish_accents() {
        assert_eq!(normalize_for_search("señor"), "senor");
        assert_eq!(normalize_for_search("España"), "espana");
        assert_eq!(normalize_for_search("jalapeño"), "jalapeno");
    }

    #[test]
    fn test_normalize_japanese_stays_unchanged() {
        // CJK characters have no combining marks, so they pass through unchanged
        assert_eq!(normalize_for_search("進撃の巨人"), "進撃の巨人");
        assert_eq!(normalize_for_search("ワンピース"), "ワンピース");
    }

    #[test]
    fn test_normalize_mixed_content() {
        assert_eq!(normalize_for_search("MÄR Omega"), "mar omega");
        assert_eq!(
            normalize_for_search("Shingeki no Kyojin - 進撃の巨人"),
            "shingeki no kyojin - 進撃の巨人"
        );
    }

    #[test]
    fn test_normalize_empty_string() {
        assert_eq!(normalize_for_search(""), "");
    }

    #[test]
    fn test_normalize_numbers_and_symbols() {
        assert_eq!(normalize_for_search("Vol. 123"), "vol. 123");
        assert_eq!(normalize_for_search("Issue #5"), "issue #5");
    }

    #[test]
    fn test_normalize_precomposed_vs_decomposed() {
        // Both precomposed (NFC) and decomposed (NFD) forms should normalize the same way
        let precomposed = "Ä"; // U+00C4 (single codepoint)
        let decomposed = "A\u{0308}"; // A + combining diaeresis
        assert_eq!(
            normalize_for_search(precomposed),
            normalize_for_search(decomposed)
        );
        assert_eq!(normalize_for_search(precomposed), "a");
    }
}
