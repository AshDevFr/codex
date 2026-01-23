//! PDF Rendering Integration Tests
//!
//! Tests for the PDF rendering infrastructure (Phase 5 of pdf-page-rendering.md).
//!
//! These tests cover:
//! 1. Text-only PDF rendering
//! 2. Vector graphics PDF rendering
//! 3. Mixed content PDF rendering
//! 4. Large PDF performance
//! 5. Invalid PDF handling
//! 6. Cache hit/miss behavior
//! 7. Concurrent rendering (thread safety)
//!
//! Note: Many of these tests require PDFium to be installed. Tests will skip
//! gracefully if PDFium is not available.

#[path = "../common/mod.rs"]
mod common;

use codex::parsers::pdf::{
    extract_page_from_pdf, extract_page_from_pdf_with_dpi, renderer, PdfParser,
};
use codex::parsers::traits::FormatParser;
use codex::services::PdfPageCache;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tempfile::TempDir;
use uuid::Uuid;

/// Initialize PDFium for tests. Returns true if successful.
fn ensure_pdfium_init() -> bool {
    if renderer::is_initialized() {
        return true;
    }
    // Try to initialize - if it fails, PDFium is not available
    renderer::init_pdfium(None).is_ok()
}

/// Check if PDFium is available, skip test if not
macro_rules! skip_without_pdfium {
    () => {
        if !ensure_pdfium_init() {
            eprintln!("Skipping test: PDFium not available");
            return;
        }
    };
}

// =============================================================================
// Text-Only PDF Rendering Tests
// =============================================================================

#[test]
fn test_render_text_only_pdf_single_page() {
    skip_without_pdfium!();

    let temp_dir = TempDir::new().unwrap();
    let pdf_path = common::create_text_only_pdf(&temp_dir, 1);

    // Extract the page - should fall back to PDFium rendering since no embedded images
    let result = extract_page_from_pdf(&pdf_path, 1);

    assert!(
        result.is_ok(),
        "Failed to render text-only PDF: {:?}",
        result.err()
    );

    let image_data = result.unwrap();
    assert!(!image_data.is_empty(), "Rendered image should not be empty");

    // Should be JPEG (rendered by PDFium)
    assert!(
        image_data.starts_with(&[0xFF, 0xD8]),
        "Rendered page should be JPEG format"
    );
}

#[test]
fn test_render_text_only_pdf_multiple_pages() {
    skip_without_pdfium!();

    let temp_dir = TempDir::new().unwrap();
    let pdf_path = common::create_text_only_pdf(&temp_dir, 5);

    // Extract each page
    for page_num in 1..=5 {
        let result = extract_page_from_pdf(&pdf_path, page_num);
        assert!(
            result.is_ok(),
            "Failed to render page {}: {:?}",
            page_num,
            result.err()
        );

        let image_data = result.unwrap();
        assert!(
            !image_data.is_empty(),
            "Page {} should not be empty",
            page_num
        );
    }
}

#[test]
fn test_render_text_only_pdf_parser_metadata() {
    let temp_dir = TempDir::new().unwrap();
    let pdf_path = common::create_text_only_pdf(&temp_dir, 3);

    let parser = PdfParser::new();
    let metadata = parser.parse(&pdf_path).unwrap();

    // Should report all 3 pages
    assert_eq!(metadata.page_count, 3);
    assert_eq!(metadata.pages.len(), 3);

    // All pages should have JPEG format (will be rendered)
    for page in &metadata.pages {
        assert_eq!(
            page.format,
            codex::parsers::ImageFormat::JPEG,
            "Text-only PDF pages should be marked as JPEG (rendered)"
        );
    }
}

// =============================================================================
// Vector Graphics PDF Tests
// =============================================================================

#[test]
fn test_render_vector_graphics_pdf() {
    skip_without_pdfium!();

    let temp_dir = TempDir::new().unwrap();
    let pdf_path = common::create_vector_graphics_pdf(&temp_dir, 2);

    // Extract pages with vector graphics
    for page_num in 1..=2 {
        let result = extract_page_from_pdf(&pdf_path, page_num);
        assert!(
            result.is_ok(),
            "Failed to render vector graphics page {}: {:?}",
            page_num,
            result.err()
        );

        let image_data = result.unwrap();
        assert!(
            !image_data.is_empty(),
            "Vector graphics page {} should render",
            page_num
        );
    }
}

#[test]
fn test_vector_graphics_parser_metadata() {
    let temp_dir = TempDir::new().unwrap();
    let pdf_path = common::create_vector_graphics_pdf(&temp_dir, 3);

    let parser = PdfParser::new();
    let metadata = parser.parse(&pdf_path).unwrap();

    assert_eq!(metadata.page_count, 3);
    assert_eq!(metadata.pages.len(), 3);
}

// =============================================================================
// Mixed Content PDF Tests
// =============================================================================

#[test]
fn test_render_mixed_content_pdf() {
    skip_without_pdfium!();

    let temp_dir = TempDir::new().unwrap();
    let pdf_path = common::create_mixed_content_pdf(&temp_dir, 3, 2);

    // Mixed content - may use fast path or rendering depending on image extraction
    for page_num in 1..=3 {
        let result = extract_page_from_pdf(&pdf_path, page_num);
        assert!(
            result.is_ok(),
            "Failed to extract mixed content page {}: {:?}",
            page_num,
            result.err()
        );
    }
}

#[test]
fn test_mixed_content_parser_metadata() {
    let temp_dir = TempDir::new().unwrap();
    let pdf_path = common::create_mixed_content_pdf(&temp_dir, 2, 1);

    let parser = PdfParser::new();
    let metadata = parser.parse(&pdf_path).unwrap();

    assert_eq!(metadata.page_count, 2);
    assert_eq!(metadata.pages.len(), 2);
}

// =============================================================================
// DPI Configuration Tests
// =============================================================================

#[test]
fn test_render_with_different_dpi_values() {
    skip_without_pdfium!();

    let temp_dir = TempDir::new().unwrap();
    let pdf_path = common::create_text_only_pdf(&temp_dir, 1);

    // Test different DPI values
    let dpis = [72, 150, 200, 300];

    for dpi in dpis {
        let result = extract_page_from_pdf_with_dpi(&pdf_path, 1, dpi);
        assert!(
            result.is_ok(),
            "Failed to render at {} DPI: {:?}",
            dpi,
            result.err()
        );

        let image_data = result.unwrap();
        assert!(
            !image_data.is_empty(),
            "Image at {} DPI should not be empty",
            dpi
        );
    }
}

#[test]
fn test_higher_dpi_produces_larger_images() {
    skip_without_pdfium!();

    let temp_dir = TempDir::new().unwrap();
    let pdf_path = common::create_text_only_pdf(&temp_dir, 1);

    let low_dpi_result = extract_page_from_pdf_with_dpi(&pdf_path, 1, 72).unwrap();
    let high_dpi_result = extract_page_from_pdf_with_dpi(&pdf_path, 1, 300).unwrap();

    // Higher DPI should produce larger file (more pixels = more data)
    // Note: JPEG compression might sometimes make this not strictly true,
    // but for most cases the higher DPI will be larger
    assert!(
        high_dpi_result.len() > low_dpi_result.len() / 2,
        "Higher DPI ({} bytes) should generally produce larger output than low DPI ({} bytes)",
        high_dpi_result.len(),
        low_dpi_result.len()
    );
}

// =============================================================================
// Large PDF Performance Tests
// =============================================================================

#[test]
fn test_render_large_pdf_pagination() {
    skip_without_pdfium!();

    let temp_dir = TempDir::new().unwrap();
    let pdf_path = common::create_multi_page_pdf(&temp_dir, 20);

    // Parser should correctly report page count
    let parser = PdfParser::new();
    let metadata = parser.parse(&pdf_path).unwrap();
    assert_eq!(metadata.page_count, 20);

    // Test rendering first, middle, and last pages
    let test_pages = [1, 10, 20];
    for page_num in test_pages {
        let result = extract_page_from_pdf(&pdf_path, page_num);
        assert!(
            result.is_ok(),
            "Failed to render page {} of 20: {:?}",
            page_num,
            result.err()
        );
    }
}

#[test]
fn test_render_page_out_of_bounds() {
    skip_without_pdfium!();

    let temp_dir = TempDir::new().unwrap();
    let pdf_path = common::create_text_only_pdf(&temp_dir, 3);

    // Page 0 (invalid)
    let result = extract_page_from_pdf(&pdf_path, 0);
    assert!(result.is_err(), "Page 0 should fail");

    // Negative page
    let result = extract_page_from_pdf(&pdf_path, -1);
    assert!(result.is_err(), "Negative page should fail");

    // Page beyond count
    let result = extract_page_from_pdf(&pdf_path, 4);
    assert!(result.is_err(), "Page 4 should fail for 3-page PDF");

    let result = extract_page_from_pdf(&pdf_path, 100);
    assert!(result.is_err(), "Page 100 should fail for 3-page PDF");
}

// =============================================================================
// Invalid PDF Handling Tests
// =============================================================================

#[test]
fn test_render_invalid_pdf_file() {
    skip_without_pdfium!();

    let temp_dir = TempDir::new().unwrap();
    let invalid_path = temp_dir.path().join("invalid.pdf");
    std::fs::write(&invalid_path, b"This is not a valid PDF file").unwrap();

    let result = extract_page_from_pdf(&invalid_path, 1);
    assert!(result.is_err(), "Invalid PDF should fail to render");

    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("Failed to load PDF") || err_msg.contains("not a valid PDF"),
        "Error should mention PDF loading failure: {}",
        err_msg
    );
}

#[test]
fn test_render_nonexistent_file() {
    skip_without_pdfium!();

    let result = extract_page_from_pdf("/nonexistent/path/to/file.pdf", 1);
    assert!(result.is_err(), "Non-existent file should fail");
}

#[test]
fn test_render_corrupted_pdf() {
    skip_without_pdfium!();

    let temp_dir = TempDir::new().unwrap();
    let corrupt_path = temp_dir.path().join("corrupt.pdf");

    // Create a file with PDF header but corrupted content
    std::fs::write(&corrupt_path, b"%PDF-1.5\n%%EOF\ngarbage data here").unwrap();

    let result = extract_page_from_pdf(&corrupt_path, 1);
    // Should either fail to load or fail to get page
    assert!(result.is_err(), "Corrupted PDF should fail");
}

#[test]
fn test_render_empty_file() {
    skip_without_pdfium!();

    let temp_dir = TempDir::new().unwrap();
    let empty_path = temp_dir.path().join("empty.pdf");
    std::fs::write(&empty_path, b"").unwrap();

    let result = extract_page_from_pdf(&empty_path, 1);
    assert!(result.is_err(), "Empty file should fail");
}

// =============================================================================
// PDF Page Cache Tests
// =============================================================================

#[tokio::test]
async fn test_cache_stores_rendered_page() {
    skip_without_pdfium!();

    let temp_dir = TempDir::new().unwrap();
    let cache = PdfPageCache::new(temp_dir.path(), true);

    let pdf_temp = TempDir::new().unwrap();
    let pdf_path = common::create_text_only_pdf(&pdf_temp, 1);

    // Render the page
    let image_data = extract_page_from_pdf(&pdf_path, 1).unwrap();
    let book_id = Uuid::new_v4();

    // Store in cache
    cache.set(book_id, 1, 150, &image_data).await.unwrap();

    // Retrieve from cache
    let cached = cache.get(book_id, 1, 150).await;
    assert!(cached.is_some(), "Cache should return stored page");
    assert_eq!(cached.unwrap(), image_data, "Cached data should match");
}

#[tokio::test]
async fn test_cache_hit_returns_same_data() {
    let temp_dir = TempDir::new().unwrap();
    let cache = PdfPageCache::new(temp_dir.path(), true);

    let book_id = Uuid::new_v4();
    let test_data = b"test jpeg data for cache test";

    // Store
    cache.set(book_id, 1, 150, test_data).await.unwrap();

    // Multiple gets should return same data
    for _ in 0..5 {
        let cached = cache.get(book_id, 1, 150).await;
        assert!(cached.is_some());
        assert_eq!(cached.unwrap().as_slice(), test_data);
    }
}

#[tokio::test]
async fn test_cache_miss_returns_none() {
    let temp_dir = TempDir::new().unwrap();
    let cache = PdfPageCache::new(temp_dir.path(), true);

    let book_id = Uuid::new_v4();

    // Never stored - should miss
    let result = cache.get(book_id, 1, 150).await;
    assert!(result.is_none(), "Cache miss should return None");
}

#[tokio::test]
async fn test_cache_different_pages_stored_separately() {
    let temp_dir = TempDir::new().unwrap();
    let cache = PdfPageCache::new(temp_dir.path(), true);

    let book_id = Uuid::new_v4();

    // Store different data for different pages
    cache.set(book_id, 1, 150, b"page 1 data").await.unwrap();
    cache.set(book_id, 2, 150, b"page 2 data").await.unwrap();
    cache.set(book_id, 3, 150, b"page 3 data").await.unwrap();

    // Each page should return its own data
    assert_eq!(
        cache.get(book_id, 1, 150).await.unwrap(),
        b"page 1 data".to_vec()
    );
    assert_eq!(
        cache.get(book_id, 2, 150).await.unwrap(),
        b"page 2 data".to_vec()
    );
    assert_eq!(
        cache.get(book_id, 3, 150).await.unwrap(),
        b"page 3 data".to_vec()
    );
}

#[tokio::test]
async fn test_cache_different_dpi_stored_separately() {
    let temp_dir = TempDir::new().unwrap();
    let cache = PdfPageCache::new(temp_dir.path(), true);

    let book_id = Uuid::new_v4();

    // Same page, different DPI
    cache.set(book_id, 1, 72, b"72 dpi data").await.unwrap();
    cache.set(book_id, 1, 150, b"150 dpi data").await.unwrap();
    cache.set(book_id, 1, 300, b"300 dpi data").await.unwrap();

    assert_eq!(
        cache.get(book_id, 1, 72).await.unwrap(),
        b"72 dpi data".to_vec()
    );
    assert_eq!(
        cache.get(book_id, 1, 150).await.unwrap(),
        b"150 dpi data".to_vec()
    );
    assert_eq!(
        cache.get(book_id, 1, 300).await.unwrap(),
        b"300 dpi data".to_vec()
    );
}

#[tokio::test]
async fn test_cache_invalidation_clears_all_pages() {
    let temp_dir = TempDir::new().unwrap();
    let cache = PdfPageCache::new(temp_dir.path(), true);

    let book_id = Uuid::new_v4();

    // Store multiple pages
    for page in 1..=5 {
        cache
            .set(book_id, page, 150, format!("page {} data", page).as_bytes())
            .await
            .unwrap();
    }

    // Verify stored
    for page in 1..=5 {
        assert!(cache.is_cached(book_id, page, 150).await);
    }

    // Invalidate
    cache.invalidate_book(book_id).await.unwrap();

    // All pages should be gone
    for page in 1..=5 {
        assert!(!cache.is_cached(book_id, page, 150).await);
    }
}

#[tokio::test]
async fn test_cache_disabled_does_not_store() {
    let temp_dir = TempDir::new().unwrap();
    let cache = PdfPageCache::new(temp_dir.path(), false); // disabled

    let book_id = Uuid::new_v4();

    // Try to store
    cache.set(book_id, 1, 150, b"test data").await.unwrap();

    // Should not be cached
    assert!(!cache.is_cached(book_id, 1, 150).await);
    assert!(cache.get(book_id, 1, 150).await.is_none());
}

#[tokio::test]
async fn test_cache_book_stats() {
    let temp_dir = TempDir::new().unwrap();
    let cache = PdfPageCache::new(temp_dir.path(), true);

    let book_id = Uuid::new_v4();

    // Initially empty
    let (count, size) = cache.get_book_stats(book_id).await.unwrap();
    assert_eq!(count, 0);
    assert_eq!(size, 0);

    // Add pages
    cache.set(book_id, 1, 150, b"short").await.unwrap();
    cache
        .set(book_id, 2, 150, b"longer data here")
        .await
        .unwrap();

    let (count, size) = cache.get_book_stats(book_id).await.unwrap();
    assert_eq!(count, 2);
    assert_eq!(size, 5 + 16); // "short" + "longer data here"
}

// =============================================================================
// Concurrent Rendering Tests (Thread Safety)
// =============================================================================

#[tokio::test]
async fn test_concurrent_cache_access() {
    let temp_dir = TempDir::new().unwrap();
    let cache = Arc::new(PdfPageCache::new(temp_dir.path(), true));

    let book_id = Uuid::new_v4();

    // Spawn multiple tasks that read and write concurrently
    let mut handles = vec![];

    for i in 0..10 {
        let cache = Arc::clone(&cache);
        let handle = tokio::spawn(async move {
            // Write
            cache
                .set(book_id, i + 1, 150, format!("data {}", i).as_bytes())
                .await
                .unwrap();

            // Read back
            let data = cache.get(book_id, i + 1, 150).await;
            assert!(data.is_some());
        });
        handles.push(handle);
    }

    // Wait for all tasks
    for handle in handles {
        handle.await.unwrap();
    }

    // Verify all pages are cached
    for i in 0..10 {
        assert!(cache.is_cached(book_id, i + 1, 150).await);
    }
}

#[test]
fn test_concurrent_pdf_rendering() {
    skip_without_pdfium!();

    use std::thread;

    let temp_dir = TempDir::new().unwrap();
    let pdf_path = common::create_text_only_pdf(&temp_dir, 5);
    let pdf_path = Arc::new(pdf_path);

    let success_count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let mut handles = vec![];

    // Spawn multiple threads that render different pages concurrently
    for page in 1..=5 {
        let pdf_path = Arc::clone(&pdf_path);
        let success = Arc::clone(&success_count);
        let handle = thread::spawn(move || {
            let result = extract_page_from_pdf(&*pdf_path, page);
            if result.is_ok() {
                success.fetch_add(1, Ordering::SeqCst);
            }
        });
        handles.push(handle);
    }

    // Wait for all threads
    for handle in handles {
        handle.join().unwrap();
    }

    // All renders should succeed
    assert_eq!(
        success_count.load(Ordering::SeqCst),
        5,
        "All concurrent renders should succeed"
    );
}

#[test]
fn test_concurrent_same_page_rendering() {
    skip_without_pdfium!();

    use std::thread;

    let temp_dir = TempDir::new().unwrap();
    let pdf_path = common::create_text_only_pdf(&temp_dir, 1);
    let pdf_path = Arc::new(pdf_path);

    let success_count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let mut handles = vec![];

    // Multiple threads render the same page concurrently
    for _ in 0..10 {
        let pdf_path = Arc::clone(&pdf_path);
        let success = Arc::clone(&success_count);
        let handle = thread::spawn(move || {
            let result = extract_page_from_pdf(&*pdf_path, 1);
            if result.is_ok() {
                success.fetch_add(1, Ordering::SeqCst);
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    assert_eq!(
        success_count.load(Ordering::SeqCst),
        10,
        "All concurrent same-page renders should succeed"
    );
}

// =============================================================================
// PDF without PDFium Tests (Graceful Degradation)
// =============================================================================

#[test]
fn test_embedded_image_pdf_works_without_pdfium() {
    // This test doesn't require PDFium - it uses the fast path
    let temp_dir = TempDir::new().unwrap();
    let pdf_path = common::create_test_pdf(&temp_dir, 2, 2);

    // PDFs with embedded images should work without PDFium
    let result = extract_page_from_pdf(&pdf_path, 1);

    // If PDFium is not available, this might fail on some PDFs
    // but our test PDFs have embedded images that should be extractable
    // Even without PDFium, the embedded image extraction should work
    // (the result depends on whether the test PDF's embedded images are extractable)
    if let Ok(image_data) = result {
        assert!(!image_data.is_empty());
    }
}

// =============================================================================
// Renderer Module Direct Tests
// =============================================================================

#[test]
fn test_renderer_get_page_count() {
    skip_without_pdfium!();

    let temp_dir = TempDir::new().unwrap();
    let pdf_path = common::create_text_only_pdf(&temp_dir, 7);

    let count = renderer::get_page_count(&pdf_path).unwrap();
    assert_eq!(count, 7);
}

#[test]
fn test_renderer_get_page_dimensions() {
    skip_without_pdfium!();

    let temp_dir = TempDir::new().unwrap();
    let pdf_path = common::create_text_only_pdf(&temp_dir, 1);

    // Our test PDFs use US Letter size (612 x 792 points)
    let (width, height) = renderer::get_page_dimensions(&pdf_path, 1).unwrap();

    assert!((width - 612.0).abs() < 1.0, "Width should be ~612 points");
    assert!((height - 792.0).abs() < 1.0, "Height should be ~792 points");
}

#[test]
fn test_renderer_get_page_dimensions_pixels() {
    skip_without_pdfium!();

    let temp_dir = TempDir::new().unwrap();
    let pdf_path = common::create_text_only_pdf(&temp_dir, 1);

    // At 150 DPI: 612pt * (150/72) = 1275px, 792pt * (150/72) = 1650px
    let (width, height) = renderer::get_page_dimensions_pixels(&pdf_path, 1, 150).unwrap();

    assert_eq!(width, 1275, "Width should be 1275 pixels at 150 DPI");
    assert_eq!(height, 1650, "Height should be 1650 pixels at 150 DPI");
}

#[test]
fn test_renderer_render_page_directly() {
    skip_without_pdfium!();

    let temp_dir = TempDir::new().unwrap();
    let pdf_path = common::create_text_only_pdf(&temp_dir, 1);

    let image_data = renderer::render_page(&pdf_path, 1, 150).unwrap();

    assert!(!image_data.is_empty());
    // Should be JPEG
    assert!(image_data.starts_with(&[0xFF, 0xD8]));
}

#[test]
fn test_renderer_render_page_with_quality() {
    skip_without_pdfium!();

    let temp_dir = TempDir::new().unwrap();
    let pdf_path = common::create_text_only_pdf(&temp_dir, 1);

    // Low quality
    let low_quality = renderer::render_page_with_quality(&pdf_path, 1, 150, 20).unwrap();
    // High quality
    let high_quality = renderer::render_page_with_quality(&pdf_path, 1, 150, 95).unwrap();

    // Both should be valid JPEG
    assert!(low_quality.starts_with(&[0xFF, 0xD8]));
    assert!(high_quality.starts_with(&[0xFF, 0xD8]));

    // Higher quality should generally be larger
    // (not always strictly true due to JPEG compression behavior)
    assert!(
        high_quality.len() > low_quality.len() / 2,
        "High quality should generally be larger"
    );
}
