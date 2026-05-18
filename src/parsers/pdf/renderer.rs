//! PDF page rendering using PDFium
//!
//! This module provides PDF page rendering capabilities using the pdfium-render crate.
//! It supports rendering PDF pages to JPEG images, getting page counts, and page dimensions.
//!
//! PDFium must be available either:
//! - As a system library
//! - At a configured path
//! - In the same directory as the executable
//!
//! The library is bound once at startup into a process-global `Pdfium` instance. The
//! `PdfHandleCache` borrows that `&'static Pdfium` to keep `PdfDocument<'static>` values
//! resident across page renders; without a shared instance every open would need its own
//! `Pdfium` and the borrow lifetime would prevent caching.

use anyhow::{Context, Result};
use pdfium_render::prelude::*;
use std::io::Cursor;
use std::path::Path;
use std::sync::OnceLock;

/// Global PDFium instance, initialised once at startup.
///
/// Stored as the owned value (not just a config) so callers can hand out
/// `&'static Pdfium` references for the duration of the process. The handle
/// cache requires `'static` to make cached `PdfDocument`s outlive any single
/// request.
static PDFIUM_INSTANCE: OnceLock<Pdfium> = OnceLock::new();

/// Check if PDFium has been initialized (instance bound)
pub fn is_initialized() -> bool {
    PDFIUM_INSTANCE.get().is_some()
}

/// Initialize PDFium library configuration
///
/// This should be called once at application startup. If `library_path` is provided,
/// it will attempt to load PDFium from that path. Otherwise, it will try:
/// 1. Current executable directory
/// 2. System library paths
///
/// # Arguments
/// * `library_path` - Optional path to PDFium library
///
/// # Returns
/// * `Ok(())` if initialization succeeded
/// * `Err` if PDFium could not be loaded or was already initialized
pub fn init_pdfium(library_path: Option<&Path>) -> Result<()> {
    let pdfium = create_pdfium_instance(library_path)?;

    PDFIUM_INSTANCE
        .set(pdfium)
        .map_err(|_| anyhow::anyhow!("PDFium already initialized"))?;

    tracing::info!("PDFium library initialized successfully");
    Ok(())
}

/// Borrow the process-global `Pdfium` instance.
///
/// Returns `None` if `init_pdfium` has not been called. The lifetime is
/// `'static`, which is what `PdfHandleCache` needs to keep cached
/// `PdfDocument<'static>` values alive past the calling stack frame.
pub fn static_pdfium() -> Option<&'static Pdfium> {
    PDFIUM_INSTANCE.get()
}

/// Create a new Pdfium instance
///
/// This creates a fresh Pdfium instance bound to the library at the given
/// path (or discovered via the platform's library search). Only used at
/// startup; callers thereafter borrow `static_pdfium()`.
fn create_pdfium_instance(library_path: Option<&Path>) -> Result<Pdfium> {
    let bindings = match library_path {
        Some(path) => {
            // Load from specified path
            let lib_path = if path.is_dir() {
                Pdfium::pdfium_platform_library_name_at_path(path)
            } else {
                path.to_path_buf()
            };
            Pdfium::bind_to_library(&lib_path)
                .with_context(|| format!("Failed to bind to PDFium library at {:?}", lib_path))?
        }
        None => {
            // Try multiple locations in order of preference:
            // 1. Current directory (for portable deployments)
            // 2. Common Linux paths (for Docker/system installs)
            // 3. System library paths (uses dlopen search)
            let search_paths = [
                Pdfium::pdfium_platform_library_name_at_path("./"),
                std::path::PathBuf::from("/usr/local/lib/libpdfium.so"),
                std::path::PathBuf::from("/usr/lib/libpdfium.so"),
            ];

            let mut last_error = None;
            let mut bindings_result = None;

            for path in &search_paths {
                match Pdfium::bind_to_library(path) {
                    Ok(b) => {
                        bindings_result = Some(b);
                        break;
                    }
                    Err(e) => {
                        tracing::trace!("PDFium not found at {:?}: {}", path, e);
                        last_error = Some(e);
                    }
                }
            }

            match bindings_result {
                Some(b) => b,
                None => Pdfium::bind_to_system_library().map_err(|e| {
                    anyhow::anyhow!(
                        "Failed to find PDFium library. Tried paths: {:?}. Last error: {}",
                        search_paths,
                        last_error
                            .map(|e| e.to_string())
                            .unwrap_or_else(|| e.to_string())
                    )
                })?,
            }
        }
    };

    Ok(Pdfium::new(bindings))
}

/// Open a PDF document against the process-global Pdfium binding.
///
/// Returns `PdfDocument<'static>` so the document can be cached past the
/// caller's stack frame. Fails if `init_pdfium()` has not been called.
pub fn open_pdf_document(path: &Path) -> Result<PdfDocument<'static>> {
    let pdfium = static_pdfium()
        .ok_or_else(|| anyhow::anyhow!("PDFium not initialized. Call init_pdfium() first."))?;

    pdfium
        .load_pdf_from_file(path, None)
        .with_context(|| format!("Failed to load PDF: {:?}", path))
}

/// Render a single page from an already-open `PdfDocument`.
///
/// This is the inner loop the handle cache exercises: callers open the PDF
/// once (via [`open_pdf_document`]) and then render many pages without
/// paying the cold-open cost again. The document is borrowed immutably;
/// pdfium-rs does not require `&mut` for page access here.
///
/// # Arguments
/// * `document` - An open PDF document
/// * `page_number` - Page number (1-indexed)
/// * `dpi` - Render resolution in DPI (72-300 recommended)
pub fn render_page_from_doc(
    document: &PdfDocument<'_>,
    page_number: i32,
    dpi: u16,
) -> Result<Vec<u8>> {
    let page_index = (page_number - 1) as u16; // Convert 1-indexed to 0-indexed
    let pages = document.pages();
    let page_count = pages.len() as usize;

    if page_index as usize >= page_count {
        anyhow::bail!(
            "Page {} not found in PDF (total pages: {})",
            page_number,
            page_count
        );
    }

    let page = pages
        .get(page_index)
        .with_context(|| format!("Failed to get page {} from PDF", page_number))?;

    // Calculate render size based on DPI
    // PDF page dimensions are in points (1 point = 1/72 inch)
    let width_points = page.width().value;
    let height_points = page.height().value;
    let width_pixels = (width_points / 72.0 * dpi as f32) as i32;
    let height_pixels = (height_points / 72.0 * dpi as f32) as i32;

    let render_config = PdfRenderConfig::new()
        .set_target_width(width_pixels)
        .set_target_height(height_pixels)
        .render_annotations(true);

    let bitmap = page
        .render_with_config(&render_config)
        .context("Failed to render page")?;

    // Convert to JPEG using the image crate
    let image = bitmap.as_image();
    let rgb_image = image.into_rgb8();

    let mut bytes = Vec::new();
    rgb_image
        .write_to(&mut Cursor::new(&mut bytes), image::ImageFormat::Jpeg)
        .context("Failed to encode image as JPEG")?;

    Ok(bytes)
}

/// Render a PDF page to JPEG image bytes
///
/// Convenience wrapper that opens the document and renders a single page.
/// Hot paths that render multiple pages should call [`open_pdf_document`] +
/// [`render_page_from_doc`] (via the handle cache) instead, to avoid the
/// per-page reopen cost.
///
/// # Arguments
/// * `path` - Path to the PDF file
/// * `page_number` - Page number (1-indexed)
/// * `dpi` - Render resolution in DPI (72-300 recommended)
pub fn render_page(path: &Path, page_number: i32, dpi: u16) -> Result<Vec<u8>> {
    let document = open_pdf_document(path)?;
    render_page_from_doc(&document, page_number, dpi)
}

/// Render a PDF page to JPEG image bytes with configurable quality
///
/// # Arguments
/// * `path` - Path to the PDF file
/// * `page_number` - Page number (1-indexed)
/// * `dpi` - Render resolution in DPI (72-300 recommended)
/// * `quality` - JPEG quality (1-100)
///
/// # Returns
/// * `Ok(Vec<u8>)` - JPEG image data
/// * `Err` if the page could not be rendered
#[allow(dead_code)]
pub fn render_page_with_quality(
    path: &Path,
    page_number: i32,
    dpi: u16,
    quality: u8,
) -> Result<Vec<u8>> {
    let document = open_pdf_document(path)?;

    let page_index = (page_number - 1) as u16;
    let pages = document.pages();
    let page_count = pages.len() as usize;

    if page_index as usize >= page_count {
        anyhow::bail!(
            "Page {} not found in PDF (total pages: {})",
            page_number,
            page_count
        );
    }

    let page = pages
        .get(page_index)
        .with_context(|| format!("Failed to get page {} from PDF", page_number))?;

    let width_points = page.width().value;
    let height_points = page.height().value;
    let width_pixels = (width_points / 72.0 * dpi as f32) as i32;
    let height_pixels = (height_points / 72.0 * dpi as f32) as i32;

    let render_config = PdfRenderConfig::new()
        .set_target_width(width_pixels)
        .set_target_height(height_pixels)
        .render_annotations(true);

    let bitmap = page
        .render_with_config(&render_config)
        .context("Failed to render page")?;

    let image = bitmap.as_image();
    let rgb_image = image.into_rgb8();

    let mut bytes = Vec::new();
    let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut bytes, quality);
    rgb_image
        .write_with_encoder(encoder)
        .context("Failed to encode image as JPEG")?;

    Ok(bytes)
}

/// Get PDF page count
///
/// # Arguments
/// * `path` - Path to the PDF file
///
/// # Returns
/// * `Ok(usize)` - Number of pages in the PDF
/// * `Err` if the PDF could not be loaded
#[allow(dead_code)]
pub fn get_page_count(path: &Path) -> Result<usize> {
    let document = open_pdf_document(path)?;
    Ok(document.pages().len() as usize)
}

/// Get page dimensions (width, height) in points
///
/// # Arguments
/// * `path` - Path to the PDF file
/// * `page_number` - Page number (1-indexed)
///
/// # Returns
/// * `Ok((f32, f32))` - Width and height in points (1 point = 1/72 inch)
/// * `Err` if the page could not be accessed
pub fn get_page_dimensions(path: &Path, page_number: i32) -> Result<(f32, f32)> {
    let document = open_pdf_document(path)?;

    let page_index = (page_number - 1) as u16;
    let pages = document.pages();
    let page_count = pages.len() as usize;

    if page_index as usize >= page_count {
        anyhow::bail!(
            "Page {} not found in PDF (total pages: {})",
            page_number,
            page_count
        );
    }

    let page = pages
        .get(page_index)
        .with_context(|| format!("Failed to get page {} from PDF", page_number))?;

    Ok((page.width().value, page.height().value))
}

/// Get page dimensions in pixels at a given DPI
///
/// # Arguments
/// * `path` - Path to the PDF file
/// * `page_number` - Page number (1-indexed)
/// * `dpi` - Target DPI
///
/// # Returns
/// * `Ok((u32, u32))` - Width and height in pixels
/// * `Err` if the page could not be accessed
pub fn get_page_dimensions_pixels(path: &Path, page_number: i32, dpi: u16) -> Result<(u32, u32)> {
    let (width_points, height_points) = get_page_dimensions(path, page_number)?;
    let width_pixels = (width_points / 72.0 * dpi as f32) as u32;
    let height_pixels = (height_points / 72.0 * dpi as f32) as u32;
    Ok((width_pixels, height_pixels))
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: These tests require PDFium to be available on the system.
    // They will be skipped if PDFium is not installed.

    fn ensure_pdfium_init() -> bool {
        if is_initialized() {
            return true;
        }
        // Try to initialize - if it fails, PDFium is not available
        init_pdfium(None).is_ok()
    }

    #[test]
    fn test_is_initialized_before_init() {
        // This test must run before any PDFium initialization
        // In practice, other tests might initialize it first
        // so we just verify the function doesn't panic
        let _ = is_initialized();
    }

    #[test]
    fn test_open_pdf_document_without_init() {
        // Without a global init, open_pdf_document must fail with a clear message.
        if !is_initialized() {
            let result = open_pdf_document(Path::new("/nonexistent/file.pdf"));
            assert!(result.is_err());
            assert!(
                result
                    .unwrap_err()
                    .to_string()
                    .contains("PDFium not initialized")
            );
        }
    }

    #[test]
    fn test_render_page_invalid_path() {
        if !ensure_pdfium_init() {
            eprintln!("Skipping test: PDFium not available");
            return;
        }

        let result = render_page(Path::new("/nonexistent/file.pdf"), 1, 150);
        assert!(result.is_err());
    }

    #[test]
    fn test_get_page_count_invalid_path() {
        if !ensure_pdfium_init() {
            eprintln!("Skipping test: PDFium not available");
            return;
        }

        let result = get_page_count(Path::new("/nonexistent/file.pdf"));
        assert!(result.is_err());
    }

    #[test]
    fn test_get_page_dimensions_invalid_path() {
        if !ensure_pdfium_init() {
            eprintln!("Skipping test: PDFium not available");
            return;
        }

        let result = get_page_dimensions(Path::new("/nonexistent/file.pdf"), 1);
        assert!(result.is_err());
    }

    #[test]
    fn test_get_page_dimensions_pixels_calculation() {
        // Test the pixel calculation logic
        // At 72 DPI, 1 point = 1 pixel
        // At 150 DPI, points * (150/72) = pixels
        let width_points = 612.0; // US Letter width
        let height_points = 792.0; // US Letter height
        let dpi: u16 = 150;

        let expected_width = (width_points / 72.0 * dpi as f32) as u32;
        let expected_height = (height_points / 72.0 * dpi as f32) as u32;

        // 612 * (150/72) = 1275
        assert_eq!(expected_width, 1275);
        // 792 * (150/72) = 1650
        assert_eq!(expected_height, 1650);
    }
}
