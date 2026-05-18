//! Integration tests for the `PdfHandleCache` end-to-end with a real PDFium runtime.
//!
//! The unit tests in `src/services/pdf_handle_cache.rs` use a fake value type
//! to verify the LRU semantics. These tests exercise the real production type:
//! they open an actual PDF through `open_pdf_document`, render multiple pages
//! through the cached handle, and assert the underlying PDFium binding was
//! opened exactly once.
//!
//! Tests skip cleanly when PDFium is not installed (matches the convention used
//! by `tests/parsers/pdf_rendering.rs`).

#[path = "../common/mod.rs"]
mod common;

use codex::parsers::pdf::{open_pdf_document, render_page_from_doc, renderer};
use codex::services::PdfHandleCache;
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;
use uuid::Uuid;

fn ensure_pdfium_init() -> bool {
    if renderer::is_initialized() {
        return true;
    }
    renderer::init_pdfium(None).is_ok()
}

/// Three sequential page requests for the same book should result in exactly
/// one PDFium open. The cache hits dominate after the first miss.
#[test]
fn sequential_renders_open_pdf_once() {
    if !ensure_pdfium_init() {
        eprintln!("Skipping test: PDFium not available");
        return;
    }

    let temp_dir = TempDir::new().unwrap();
    let pdf_path = common::create_text_only_pdf(&temp_dir, 3);
    let cache: PdfHandleCache = PdfHandleCache::new(8, Duration::from_secs(60), true);
    let book_id = Uuid::new_v4();

    for page in 1..=3 {
        let path = pdf_path.clone();
        let doc_arc = cache
            .get_or_open(book_id, path.clone(), move || open_pdf_document(&path))
            .expect("cache must open pdf");
        let doc = doc_arc.blocking_lock();
        let bytes = render_page_from_doc(&doc, page, 100).expect("render must succeed");
        assert!(
            !bytes.is_empty(),
            "page {} should render non-empty bytes",
            page
        );
    }

    let stats = cache.stats();
    assert_eq!(stats.opens(), 1, "PDFium open should happen exactly once");
    assert_eq!(stats.misses(), 1, "first request is a miss, rest are hits");
    assert_eq!(stats.hits(), 2, "the next two are cache hits");

    let snap = cache.snapshot();
    assert_eq!(snap.current_size, 1);
    assert_eq!(snap.entries[0].book_id, book_id);
}

/// Evicting a book forces the next request to re-open. This guards the
/// invalidation path used by the scanner and the entity-event subscriber.
#[test]
fn evict_forces_subsequent_reopen() {
    if !ensure_pdfium_init() {
        eprintln!("Skipping test: PDFium not available");
        return;
    }

    let temp_dir = TempDir::new().unwrap();
    let pdf_path = common::create_text_only_pdf(&temp_dir, 1);
    let cache: PdfHandleCache = PdfHandleCache::new(8, Duration::from_secs(60), true);
    let book_id = Uuid::new_v4();

    let render_once = || {
        let path = pdf_path.clone();
        let doc_arc = cache
            .get_or_open(book_id, path.clone(), move || open_pdf_document(&path))
            .expect("cache must open pdf");
        let doc = doc_arc.blocking_lock();
        render_page_from_doc(&doc, 1, 100).expect("render must succeed");
    };

    render_once();
    render_once();
    assert_eq!(cache.stats().opens(), 1);

    assert!(cache.evict(book_id));
    render_once();
    assert_eq!(
        cache.stats().opens(),
        2,
        "after eviction the next request must open again"
    );
}

/// Renders against two distinct books should populate two cache entries,
/// each opened exactly once.
#[test]
fn two_books_open_independently() {
    if !ensure_pdfium_init() {
        eprintln!("Skipping test: PDFium not available");
        return;
    }

    let temp_dir = TempDir::new().unwrap();
    let pdf_a = common::create_text_only_pdf(&temp_dir, 2);
    let pdf_b = common::create_text_only_pdf(&temp_dir, 2);
    // create_text_only_pdf reuses the same filename, so put one in a subdir.
    let nested = temp_dir.path().join("nested");
    std::fs::create_dir_all(&nested).unwrap();
    let pdf_b_dest = nested.join("book_b.pdf");
    std::fs::copy(&pdf_b, &pdf_b_dest).unwrap();

    let cache: Arc<PdfHandleCache> =
        Arc::new(PdfHandleCache::new(8, Duration::from_secs(60), true));
    let book_a = Uuid::new_v4();
    let book_b = Uuid::new_v4();

    for (book, path) in &[(book_a, &pdf_a), (book_b, &pdf_b_dest)] {
        let path = (*path).clone();
        let doc_arc = cache
            .get_or_open(*book, path.clone(), move || open_pdf_document(&path))
            .expect("open");
        let doc = doc_arc.blocking_lock();
        let _ = render_page_from_doc(&doc, 1, 100).expect("render");
    }

    assert_eq!(cache.stats().opens(), 2);
    assert_eq!(cache.snapshot().current_size, 2);
}
