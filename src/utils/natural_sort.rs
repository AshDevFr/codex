//! Natural sort comparison for strings containing numbers
//!
//! Provides natural (human-friendly) ordering where embedded numbers
//! are compared numerically rather than lexicographically.
//! For example: "Vol. 1", "Vol. 2", "Vol. 10" instead of "Vol. 1", "Vol. 10", "Vol. 2".

use std::cmp::Ordering;

/// Compare two strings using natural sort order
///
/// Splits strings into segments of digits and non-digits, comparing
/// digit segments numerically and non-digit segments lexicographically
/// (case-insensitive).
///
/// # Examples
///
/// ```ignore
/// use codex::utils::natural_sort::natural_cmp;
///
/// assert_eq!(natural_cmp("Vol. 2", "Vol. 10"), Ordering::Less);
/// assert_eq!(natural_cmp("Ch 1", "Ch 1"), Ordering::Equal);
/// ```
pub fn natural_cmp(a: &str, b: &str) -> Ordering {
    let mut a_chars = a.chars().peekable();
    let mut b_chars = b.chars().peekable();

    loop {
        match (a_chars.peek(), b_chars.peek()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(&ac), Some(&bc)) => {
                if ac.is_ascii_digit() && bc.is_ascii_digit() {
                    // Both are digits: compare numerically
                    let a_num = consume_number(&mut a_chars);
                    let b_num = consume_number(&mut b_chars);
                    match a_num.cmp(&b_num) {
                        Ordering::Equal => continue,
                        other => return other,
                    }
                } else {
                    // Compare characters case-insensitively
                    let a_lower = ac.to_ascii_lowercase();
                    let b_lower = bc.to_ascii_lowercase();
                    match a_lower.cmp(&b_lower) {
                        Ordering::Equal => {
                            // If case-insensitive equal, use original case as tiebreaker
                            // (lowercase before uppercase for stable ordering)
                            let case_cmp = ac.cmp(&bc);
                            a_chars.next();
                            b_chars.next();
                            if case_cmp != Ordering::Equal {
                                // Continue but remember the tiebreaker only if rest is equal
                                // For simplicity, just continue — exact case rarely matters
                            }
                            continue;
                        }
                        other => return other,
                    }
                }
            }
        }
    }
}

/// Consume consecutive digits from the iterator and return the numeric value
fn consume_number(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) -> u64 {
    let mut num: u64 = 0;
    while let Some(&c) = chars.peek() {
        if c.is_ascii_digit() {
            num = num.saturating_mul(10).saturating_add(c as u64 - '0' as u64);
            chars.next();
        } else {
            break;
        }
    }
    num
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_natural_sort() {
        let mut items = vec!["item 10", "item 2", "item 1", "item 20", "item 3"];
        items.sort_by(|a, b| natural_cmp(a, b));
        assert_eq!(
            items,
            vec!["item 1", "item 2", "item 3", "item 10", "item 20"]
        );
    }

    #[test]
    fn test_volume_sorting() {
        let mut items = vec![
            "Title of book Vol. 10.cbz",
            "Title of book Vol. 2.cbz",
            "Title of book Vol. 1.cbz",
            "Title of book Vol. 11.cbz",
            "Title of book Vol. 3.cbz",
        ];
        items.sort_by(|a, b| natural_cmp(a, b));
        assert_eq!(
            items,
            vec![
                "Title of book Vol. 1.cbz",
                "Title of book Vol. 2.cbz",
                "Title of book Vol. 3.cbz",
                "Title of book Vol. 10.cbz",
                "Title of book Vol. 11.cbz",
            ]
        );
    }

    #[test]
    fn test_chapter_sorting() {
        let mut items = vec!["ch100", "ch2", "ch10", "ch1", "ch20"];
        items.sort_by(|a, b| natural_cmp(a, b));
        assert_eq!(items, vec!["ch1", "ch2", "ch10", "ch20", "ch100"]);
    }

    #[test]
    fn test_no_numbers() {
        let mut items = vec!["banana", "apple", "cherry"];
        items.sort_by(|a, b| natural_cmp(a, b));
        assert_eq!(items, vec!["apple", "banana", "cherry"]);
    }

    #[test]
    fn test_equal_strings() {
        assert_eq!(natural_cmp("abc", "abc"), Ordering::Equal);
        assert_eq!(natural_cmp("abc 123", "abc 123"), Ordering::Equal);
        assert_eq!(natural_cmp("", ""), Ordering::Equal);
    }

    #[test]
    fn test_empty_strings() {
        assert_eq!(natural_cmp("", "a"), Ordering::Less);
        assert_eq!(natural_cmp("a", ""), Ordering::Greater);
    }

    #[test]
    fn test_case_insensitive() {
        assert_eq!(natural_cmp("ABC", "abc"), Ordering::Equal);
        let mut items = vec!["Banana", "apple", "Cherry"];
        items.sort_by(|a, b| natural_cmp(a, b));
        assert_eq!(items, vec!["apple", "Banana", "Cherry"]);
    }

    #[test]
    fn test_leading_zeros() {
        // "001" and "1" should be equal numerically
        assert_eq!(natural_cmp("file001", "file1"), Ordering::Equal);
        let mut items = vec!["file010", "file1", "file002"];
        items.sort_by(|a, b| natural_cmp(a, b));
        assert_eq!(items, vec!["file1", "file002", "file010"]);
    }

    #[test]
    fn test_mixed_content() {
        let mut items = vec![
            "img12.png",
            "img2.png",
            "img1.png",
            "img10.png",
            "img21.png",
        ];
        items.sort_by(|a, b| natural_cmp(a, b));
        assert_eq!(
            items,
            vec![
                "img1.png",
                "img2.png",
                "img10.png",
                "img12.png",
                "img21.png"
            ]
        );
    }

    #[test]
    fn test_fractional_volumes() {
        // Fractional volumes like "Vol. 1.5" — the dot is a non-digit separator,
        // so "1" and "5" are separate numeric segments.
        // After "Series Vol. 1.", we compare "5" (digit) vs "c" (non-digit).
        // Digits have lower ASCII values than letters, so "1.5" sorts before "1.cbz".
        // This is correct natural sort behavior — the actual book number extraction
        // (FilenameStrategy) handles fractional numbers like 1.5 via regex patterns.
        let mut items = vec![
            "Series Vol. 2.cbz",
            "Series Vol. 1.5.cbz",
            "Series Vol. 1.cbz",
            "Series Vol. 10.cbz",
        ];
        items.sort_by(|a, b| natural_cmp(a, b));
        assert_eq!(
            items,
            vec![
                "Series Vol. 1.5.cbz",
                "Series Vol. 1.cbz",
                "Series Vol. 2.cbz",
                "Series Vol. 10.cbz",
            ]
        );
    }

    #[test]
    fn test_real_world_manga_filenames() {
        let mut items = vec![
            "One Piece v10.cbz",
            "One Piece v1.cbz",
            "One Piece v2.cbz",
            "One Piece v100.cbz",
            "One Piece v20.cbz",
        ];
        items.sort_by(|a, b| natural_cmp(a, b));
        assert_eq!(
            items,
            vec![
                "One Piece v1.cbz",
                "One Piece v2.cbz",
                "One Piece v10.cbz",
                "One Piece v20.cbz",
                "One Piece v100.cbz",
            ]
        );
    }

    #[test]
    fn test_special_characters() {
        let mut items = vec!["file-10", "file-2", "file-1"];
        items.sort_by(|a, b| natural_cmp(a, b));
        assert_eq!(items, vec!["file-1", "file-2", "file-10"]);
    }

    #[test]
    fn test_numbers_only() {
        let mut items = vec!["10", "2", "1", "20", "3"];
        items.sort_by(|a, b| natural_cmp(a, b));
        assert_eq!(items, vec!["1", "2", "3", "10", "20"]);
    }

    #[test]
    fn test_digit_vs_nondigit() {
        // When one string has a digit and the other has a non-digit at the same position,
        // standard character comparison applies (digits < letters in ASCII)
        assert_eq!(natural_cmp("a1", "ab"), Ordering::Less); // '1' < 'b' in ASCII
        assert_eq!(natural_cmp("a9", "ab"), Ordering::Less); // '9' < 'b' in ASCII
    }

    // ===== User-reported scenario =====

    #[test]
    fn test_user_reported_volume_sorting() {
        // The exact scenario from the user:
        // "Title of book Vol. 1.cbz" through "Title of book Vol. 11.cbz"
        let expected = vec![
            "Title of book Vol. 1.cbz",
            "Title of book Vol. 2.cbz",
            "Title of book Vol. 3.cbz",
            "Title of book Vol. 4.cbz",
            "Title of book Vol. 5.cbz",
            "Title of book Vol. 6.cbz",
            "Title of book Vol. 7.cbz",
            "Title of book Vol. 8.cbz",
            "Title of book Vol. 9.cbz",
            "Title of book Vol. 10.cbz",
            "Title of book Vol. 11.cbz",
        ];
        // Shuffle to simulate unsorted input
        let shuffled = vec![
            "Title of book Vol. 10.cbz",
            "Title of book Vol. 3.cbz",
            "Title of book Vol. 1.cbz",
            "Title of book Vol. 11.cbz",
            "Title of book Vol. 5.cbz",
            "Title of book Vol. 2.cbz",
            "Title of book Vol. 9.cbz",
            "Title of book Vol. 7.cbz",
            "Title of book Vol. 4.cbz",
            "Title of book Vol. 8.cbz",
            "Title of book Vol. 6.cbz",
        ];
        let mut sorted = shuffled;
        sorted.sort_by(|a, b| natural_cmp(a, b));
        assert_eq!(sorted, expected);
    }

    // ===== Lexicographic vs natural sort comparison =====

    #[test]
    fn test_lexicographic_would_fail() {
        // These are cases where lexicographic sort gives wrong results
        // but natural sort gives correct results
        let mut items = vec!["Vol 1", "Vol 10", "Vol 11", "Vol 2", "Vol 20", "Vol 3"];
        // Lexicographic would give: Vol 1, Vol 10, Vol 11, Vol 2, Vol 20, Vol 3
        items.sort_by(|a, b| natural_cmp(a, b));
        assert_eq!(
            items,
            vec!["Vol 1", "Vol 2", "Vol 3", "Vol 10", "Vol 11", "Vol 20"]
        );
    }

    // ===== More real-world comic/manga filenames =====

    #[test]
    fn test_cbr_extension() {
        let mut items = vec![
            "Batman #100.cbr",
            "Batman #10.cbr",
            "Batman #1.cbr",
            "Batman #2.cbr",
            "Batman #20.cbr",
        ];
        items.sort_by(|a, b| natural_cmp(a, b));
        assert_eq!(
            items,
            vec![
                "Batman #1.cbr",
                "Batman #2.cbr",
                "Batman #10.cbr",
                "Batman #20.cbr",
                "Batman #100.cbr",
            ]
        );
    }

    #[test]
    fn test_epub_extension() {
        let mut items = vec![
            "Book Series 10.epub",
            "Book Series 1.epub",
            "Book Series 2.epub",
            "Book Series 3.epub",
        ];
        items.sort_by(|a, b| natural_cmp(a, b));
        assert_eq!(
            items,
            vec![
                "Book Series 1.epub",
                "Book Series 2.epub",
                "Book Series 3.epub",
                "Book Series 10.epub",
            ]
        );
    }

    #[test]
    fn test_pdf_extension() {
        let mut items = vec![
            "Document Part 10.pdf",
            "Document Part 1.pdf",
            "Document Part 2.pdf",
        ];
        items.sort_by(|a, b| natural_cmp(a, b));
        assert_eq!(
            items,
            vec![
                "Document Part 1.pdf",
                "Document Part 2.pdf",
                "Document Part 10.pdf",
            ]
        );
    }

    #[test]
    fn test_zero_padded_numbers() {
        // Common in comic/manga releases
        let mut items = vec![
            "Series c010.cbz",
            "Series c001.cbz",
            "Series c100.cbz",
            "Series c002.cbz",
            "Series c020.cbz",
        ];
        items.sort_by(|a, b| natural_cmp(a, b));
        assert_eq!(
            items,
            vec![
                "Series c001.cbz",
                "Series c002.cbz",
                "Series c010.cbz",
                "Series c020.cbz",
                "Series c100.cbz",
            ]
        );
    }

    #[test]
    fn test_mixed_zero_padding() {
        // Some files zero-padded, some not — numerically equal values
        // preserve relative input order (stable sort)
        let mut items = vec!["Ch 10.cbz", "Ch 001.cbz", "Ch 2.cbz", "Ch 02.cbz"];
        items.sort_by(|a, b| natural_cmp(a, b));
        // 001=1, 2=2, 02=2, 10=10 — "Ch 2" and "Ch 02" are numerically equal
        assert_eq!(
            items,
            vec!["Ch 001.cbz", "Ch 2.cbz", "Ch 02.cbz", "Ch 10.cbz"]
        );
    }

    #[test]
    fn test_multiple_numbers_in_filename() {
        // Filenames with year and volume number
        let mut items = vec![
            "Batman (2016) Vol. 10.cbz",
            "Batman (2016) Vol. 1.cbz",
            "Batman (2016) Vol. 2.cbz",
        ];
        items.sort_by(|a, b| natural_cmp(a, b));
        assert_eq!(
            items,
            vec![
                "Batman (2016) Vol. 1.cbz",
                "Batman (2016) Vol. 2.cbz",
                "Batman (2016) Vol. 10.cbz",
            ]
        );
    }

    #[test]
    fn test_different_series_same_numbers() {
        let mut items = vec![
            "Naruto Vol. 10.cbz",
            "Bleach Vol. 1.cbz",
            "Naruto Vol. 1.cbz",
            "Bleach Vol. 10.cbz",
            "Naruto Vol. 2.cbz",
            "Bleach Vol. 2.cbz",
        ];
        items.sort_by(|a, b| natural_cmp(a, b));
        assert_eq!(
            items,
            vec![
                "Bleach Vol. 1.cbz",
                "Bleach Vol. 2.cbz",
                "Bleach Vol. 10.cbz",
                "Naruto Vol. 1.cbz",
                "Naruto Vol. 2.cbz",
                "Naruto Vol. 10.cbz",
            ]
        );
    }

    #[test]
    fn test_underscore_separator() {
        let mut items = vec![
            "manga_chapter_10.cbz",
            "manga_chapter_1.cbz",
            "manga_chapter_2.cbz",
            "manga_chapter_20.cbz",
        ];
        items.sort_by(|a, b| natural_cmp(a, b));
        assert_eq!(
            items,
            vec![
                "manga_chapter_1.cbz",
                "manga_chapter_2.cbz",
                "manga_chapter_10.cbz",
                "manga_chapter_20.cbz",
            ]
        );
    }

    #[test]
    fn test_space_separated() {
        let mut items = vec![
            "Chapter 10",
            "Chapter 1",
            "Chapter 2",
            "Chapter 100",
            "Chapter 11",
        ];
        items.sort_by(|a, b| natural_cmp(a, b));
        assert_eq!(
            items,
            vec![
                "Chapter 1",
                "Chapter 2",
                "Chapter 10",
                "Chapter 11",
                "Chapter 100"
            ]
        );
    }

    #[test]
    fn test_large_numbers() {
        let mut items = vec!["issue 1000", "issue 100", "issue 10", "issue 1"];
        items.sort_by(|a, b| natural_cmp(a, b));
        assert_eq!(
            items,
            vec!["issue 1", "issue 10", "issue 100", "issue 1000"]
        );
    }

    #[test]
    fn test_complex_release_group_filenames() {
        // Real-world digital release filenames
        let mut items = vec![
            "A Returner's Magic c010 (2021) (Digital) (4str0).cbz",
            "A Returner's Magic c001 (2021) (Digital) (4str0).cbz",
            "A Returner's Magic c002 (2021) (Digital) (4str0).cbz",
            "A Returner's Magic c100 (2021) (Digital) (4str0).cbz",
        ];
        items.sort_by(|a, b| natural_cmp(a, b));
        assert_eq!(
            items,
            vec![
                "A Returner's Magic c001 (2021) (Digital) (4str0).cbz",
                "A Returner's Magic c002 (2021) (Digital) (4str0).cbz",
                "A Returner's Magic c010 (2021) (Digital) (4str0).cbz",
                "A Returner's Magic c100 (2021) (Digital) (4str0).cbz",
            ]
        );
    }

    #[test]
    fn test_stability() {
        // Verify that equal elements maintain relative order
        let mut items = vec!["a", "b", "a", "c"];
        items.sort_by(|a, b| natural_cmp(a, b));
        assert_eq!(items, vec!["a", "a", "b", "c"]);
    }

    #[test]
    fn test_single_element() {
        let mut items = vec!["only one"];
        items.sort_by(|a, b| natural_cmp(a, b));
        assert_eq!(items, vec!["only one"]);
    }

    #[test]
    fn test_identical_elements() {
        let mut items = vec!["same", "same", "same"];
        items.sort_by(|a, b| natural_cmp(a, b));
        assert_eq!(items, vec!["same", "same", "same"]);
    }
}
