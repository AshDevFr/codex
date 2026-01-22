//! Komga-compatible pagination DTOs
//!
//! These DTOs match the Spring Data Page format used by Komga.

use serde::{Deserialize, Serialize};

/// Komga pagination sort information
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct KomgaSort {
    /// Whether the results are sorted in ascending or descending order
    pub sorted: bool,
    /// Whether the results are unsorted
    pub unsorted: bool,
    /// Whether the sort is empty
    pub empty: bool,
}

impl KomgaSort {
    /// Create a new sorted instance
    pub fn sorted() -> Self {
        Self {
            sorted: true,
            unsorted: false,
            empty: false,
        }
    }

    /// Create an unsorted instance
    pub fn unsorted() -> Self {
        Self {
            sorted: false,
            unsorted: true,
            empty: true,
        }
    }
}

/// Komga pageable information (Spring Data style)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KomgaPageable {
    /// Current page number (0-indexed)
    pub page_number: i32,
    /// Page size (number of items per page)
    pub page_size: i32,
    /// Sort information
    pub sort: KomgaSort,
    /// Offset from start (page_number * page_size)
    pub offset: i64,
    /// Whether the pageable is paged (always true for paginated results)
    pub paged: bool,
    /// Whether the pageable is unpaged (always false for paginated results)
    pub unpaged: bool,
}

impl KomgaPageable {
    /// Create a new pageable instance
    pub fn new(page: i32, size: i32) -> Self {
        Self {
            page_number: page,
            page_size: size,
            sort: KomgaSort::sorted(),
            offset: (page as i64) * (size as i64),
            paged: true,
            unpaged: false,
        }
    }
}

impl Default for KomgaPageable {
    fn default() -> Self {
        Self::new(0, 20)
    }
}

/// Komga paginated response wrapper (Spring Data Page format)
///
/// This matches the exact structure Komic expects from Komga.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KomgaPage<T> {
    /// The content items for this page
    pub content: Vec<T>,
    /// Pageable information
    pub pageable: KomgaPageable,
    /// Total number of elements across all pages
    pub total_elements: i64,
    /// Total number of pages
    pub total_pages: i32,
    /// Whether this is the last page
    pub last: bool,
    /// Current page number (0-indexed)
    pub number: i32,
    /// Page size
    pub size: i32,
    /// Number of elements on this page
    pub number_of_elements: i32,
    /// Whether this is the first page
    pub first: bool,
    /// Whether the page is empty
    pub empty: bool,
    /// Sort information
    pub sort: KomgaSort,
}

impl<T> KomgaPage<T> {
    /// Create a new paginated response
    pub fn new(content: Vec<T>, page: i32, size: i32, total: i64) -> Self {
        let number_of_elements = content.len() as i32;
        let total_pages = if size > 0 {
            ((total as f64) / (size as f64)).ceil() as i32
        } else {
            0
        };

        Self {
            content,
            pageable: KomgaPageable::new(page, size),
            total_elements: total,
            total_pages,
            last: page >= total_pages - 1 || total_pages == 0,
            number: page,
            size,
            number_of_elements,
            first: page == 0,
            empty: number_of_elements == 0,
            sort: KomgaSort::sorted(),
        }
    }

    /// Create an empty page
    pub fn empty(page: i32, size: i32) -> Self {
        Self::new(Vec::new(), page, size, 0)
    }
}

impl<T> Default for KomgaPage<T> {
    fn default() -> Self {
        Self::empty(0, 20)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json;

    #[test]
    fn test_komga_page_serialization() {
        let page: KomgaPage<String> =
            KomgaPage::new(vec!["item1".to_string(), "item2".to_string()], 0, 20, 100);

        let json = serde_json::to_string(&page).unwrap();
        assert!(json.contains("\"content\":[\"item1\",\"item2\"]"));
        assert!(json.contains("\"totalElements\":100"));
        assert!(json.contains("\"totalPages\":5"));
        assert!(json.contains("\"first\":true"));
        assert!(json.contains("\"last\":false"));
        assert!(json.contains("\"numberOfElements\":2"));
    }

    #[test]
    fn test_komga_page_camel_case_fields() {
        let page: KomgaPage<i32> = KomgaPage::new(vec![1, 2, 3], 0, 10, 30);
        let json = serde_json::to_string(&page).unwrap();

        // Verify camelCase field names
        assert!(json.contains("\"totalElements\""));
        assert!(json.contains("\"totalPages\""));
        assert!(json.contains("\"numberOfElements\""));
        assert!(json.contains("\"pageNumber\""));
        assert!(json.contains("\"pageSize\""));
    }

    #[test]
    fn test_komga_page_first_page() {
        let page: KomgaPage<i32> = KomgaPage::new(vec![1, 2], 0, 10, 25);

        assert!(page.first);
        assert!(!page.last);
        assert_eq!(page.number, 0);
        assert_eq!(page.total_pages, 3);
    }

    #[test]
    fn test_komga_page_last_page() {
        let page: KomgaPage<i32> = KomgaPage::new(vec![1, 2, 3, 4, 5], 2, 10, 25);

        assert!(!page.first);
        assert!(page.last);
        assert_eq!(page.number, 2);
    }

    #[test]
    fn test_komga_page_empty() {
        let page: KomgaPage<i32> = KomgaPage::empty(0, 20);

        assert!(page.empty);
        assert!(page.first);
        assert!(page.last);
        assert_eq!(page.total_elements, 0);
        assert_eq!(page.total_pages, 0);
    }

    #[test]
    fn test_komga_pageable_offset() {
        let pageable = KomgaPageable::new(3, 20);

        assert_eq!(pageable.page_number, 3);
        assert_eq!(pageable.page_size, 20);
        assert_eq!(pageable.offset, 60);
        assert!(pageable.paged);
        assert!(!pageable.unpaged);
    }

    #[test]
    fn test_komga_sort_sorted() {
        let sort = KomgaSort::sorted();

        assert!(sort.sorted);
        assert!(!sort.unsorted);
        assert!(!sort.empty);
    }

    #[test]
    fn test_komga_sort_unsorted() {
        let sort = KomgaSort::unsorted();

        assert!(!sort.sorted);
        assert!(sort.unsorted);
        assert!(sort.empty);
    }
}
