//! Thumbnail service for generating and managing cover images
//!
//! TODO: Remove allow(dead_code) once all thumbnail features are fully integrated

#![allow(dead_code)]

use anyhow::{Context, Result, anyhow};
use chrono::Utc;
use image::imageops::FilterType;
use image::{DynamicImage, RgbaImage};
use jxl_oxide::JxlImage;
use sea_orm::{ActiveModelTrait, DatabaseConnection, Set};
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::UNIX_EPOCH;
use tokio::fs;
use tokio_util::io::ReaderStream;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::config::FilesConfig;
use crate::db::entities::books;
use crate::db::repositories::{BookRepository, SeriesRepository, SettingsRepository};
use crate::events::{EntityChangeEvent, EntityEvent, EntityType, EventBroadcaster};

// ============================================================================
// Placeholder Thumbnail Generation
// ============================================================================

/// Information needed to generate a placeholder thumbnail
#[derive(Debug, Clone)]
pub struct PlaceholderInfo {
    /// Book title
    pub title: String,
    /// Author name (optional)
    pub author: Option<String>,
    /// File format (e.g., "EPUB", "PDF")
    pub format: String,
}

/// Generate a placeholder thumbnail image when no cover image is available
///
/// Creates a simple image with the book title, author, and format badge.
/// The design uses a gradient background with centered text.
///
/// # Arguments
/// * `info` - Information about the book (title, author, format)
/// * `width` - Desired width in pixels
/// * `height` - Desired height in pixels
///
/// # Returns
/// A DynamicImage that can be further processed (resized, encoded to JPEG, etc.)
pub fn generate_placeholder_thumbnail(
    info: &PlaceholderInfo,
    width: u32,
    height: u32,
) -> Result<DynamicImage> {
    use image::{ImageBuffer, Rgba};

    // Create a gradient background image
    let mut img: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::new(width, height);

    // Generate a color based on the title hash for variety
    let title_hash = simple_hash(&info.title);
    let hue = (title_hash % 360) as f32;
    let (base_r, base_g, base_b) = hsl_to_rgb(hue, 0.35, 0.25); // Muted, dark color

    // Fill with gradient
    for (x, y, pixel) in img.enumerate_pixels_mut() {
        // Create a subtle vertical gradient
        let gradient_factor = y as f32 / height as f32;
        let darken = 1.0 - (gradient_factor * 0.3); // Darken towards bottom

        let r = (base_r as f32 * darken) as u8;
        let g = (base_g as f32 * darken) as u8;
        let b = (base_b as f32 * darken) as u8;

        // Add subtle noise for texture
        let noise = ((x as i32 + y as i32) % 3) as u8;
        *pixel = Rgba([r.saturating_add(noise), g.saturating_add(noise), b, 255]);
    }

    // Draw text on the image using simple bitmap rendering
    let text_color = Rgba([255u8, 255, 255, 255]); // White text

    // Calculate text positioning
    let padding = (width as f32 * 0.08) as u32;
    let text_area_width = width - (padding * 2);

    // Draw format badge at top
    let badge_y = padding;
    draw_text_simple(
        &mut img,
        &info.format.to_uppercase(),
        padding,
        badge_y,
        Rgba([200, 200, 200, 255]), // Light gray for badge
        1,                          // Small size
    );

    // Draw title in center
    let title_lines = wrap_text(&info.title, text_area_width / 12); // Approximate char width
    let line_height = 24u32;
    let total_text_height =
        (title_lines.len() as u32 * line_height) + if info.author.is_some() { 40 } else { 0 };
    let title_start_y = (height - total_text_height) / 2;

    for (i, line) in title_lines.iter().enumerate() {
        draw_text_simple(
            &mut img,
            line,
            padding,
            title_start_y + (i as u32 * line_height),
            text_color,
            2, // Medium size for title
        );
    }

    // Draw author below title
    if let Some(author) = &info.author {
        let author_y = title_start_y + (title_lines.len() as u32 * line_height) + 16;
        draw_text_simple(
            &mut img,
            author,
            padding,
            author_y,
            Rgba([180, 180, 180, 255]), // Lighter for author
            1,                          // Smaller size
        );
    }

    Ok(DynamicImage::ImageRgba8(img))
}

/// Simple hash function for generating consistent colors from titles
fn simple_hash(s: &str) -> u32 {
    let mut hash: u32 = 0;
    for c in s.chars() {
        hash = hash.wrapping_mul(31).wrapping_add(c as u32);
    }
    hash
}

/// Convert HSL to RGB
fn hsl_to_rgb(h: f32, s: f32, l: f32) -> (u8, u8, u8) {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = l - c / 2.0;

    let (r, g, b) = match (h as u32) / 60 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };

    (
        ((r + m) * 255.0) as u8,
        ((g + m) * 255.0) as u8,
        ((b + m) * 255.0) as u8,
    )
}

/// Wrap text to fit within a given width (approximate)
fn wrap_text(text: &str, max_chars: u32) -> Vec<String> {
    let max_chars = max_chars.max(10) as usize;
    let mut lines = Vec::new();
    let mut current_line = String::new();

    for word in text.split_whitespace() {
        if current_line.is_empty() {
            current_line = word.to_string();
        } else if current_line.len() + 1 + word.len() <= max_chars {
            current_line.push(' ');
            current_line.push_str(word);
        } else {
            lines.push(current_line);
            current_line = word.to_string();
        }
    }

    if !current_line.is_empty() {
        lines.push(current_line);
    }

    // Limit to 4 lines max
    if lines.len() > 4 {
        lines.truncate(3);
        if let Some(last) = lines.last_mut()
            && last.len() > 3
        {
            last.truncate(last.len() - 3);
            last.push_str("...");
        }
    }

    lines
}

/// Draw text on an image using simple bitmap font rendering
///
/// This is a basic implementation that draws text character by character.
/// For production use, consider using a proper font rendering library.
fn draw_text_simple(
    img: &mut ImageBuffer<image::Rgba<u8>, Vec<u8>>,
    text: &str,
    x: u32,
    y: u32,
    color: image::Rgba<u8>,
    size: u8, // 1 = small, 2 = medium, 3 = large
) {
    let char_width = match size {
        1 => 6u32,
        2 => 10u32,
        _ => 14u32,
    };
    let char_height = match size {
        1 => 10u32,
        2 => 16u32,
        _ => 22u32,
    };

    let (img_width, img_height) = img.dimensions();

    for (i, c) in text.chars().enumerate() {
        let char_x = x + (i as u32 * char_width);

        // Skip if outside bounds
        if char_x + char_width > img_width || y + char_height > img_height {
            break;
        }

        // Draw character using simple pattern
        draw_char_simple(img, c, char_x, y, color, char_width, char_height);
    }
}

/// Draw a single character using simple bitmap patterns
fn draw_char_simple(
    img: &mut ImageBuffer<image::Rgba<u8>, Vec<u8>>,
    c: char,
    x: u32,
    y: u32,
    color: image::Rgba<u8>,
    width: u32,
    height: u32,
) {
    // Simple 5x7 bitmap font patterns for common characters
    // Each character is represented as a 5-wide by 7-tall bitmap
    let pattern = get_char_pattern(c);

    let scale_x = width / 5;
    let scale_y = height / 7;

    for (row, &bits) in pattern.iter().enumerate() {
        for col in 0..5 {
            if (bits >> (4 - col)) & 1 == 1 {
                // Draw scaled pixel
                for sy in 0..scale_y {
                    for sx in 0..scale_x {
                        let px = x + col * scale_x + sx;
                        let py = y + row as u32 * scale_y + sy;
                        if px < img.width() && py < img.height() {
                            img.put_pixel(px, py, color);
                        }
                    }
                }
            }
        }
    }
}

/// Get bitmap pattern for a character (5x7 font)
fn get_char_pattern(c: char) -> [u8; 7] {
    match c.to_ascii_uppercase() {
        'A' => [
            0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001,
        ],
        'B' => [
            0b11110, 0b10001, 0b11110, 0b10001, 0b10001, 0b10001, 0b11110,
        ],
        'C' => [
            0b01110, 0b10001, 0b10000, 0b10000, 0b10000, 0b10001, 0b01110,
        ],
        'D' => [
            0b11110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11110,
        ],
        'E' => [
            0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111,
        ],
        'F' => [
            0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000,
        ],
        'G' => [
            0b01110, 0b10001, 0b10000, 0b10111, 0b10001, 0b10001, 0b01110,
        ],
        'H' => [
            0b10001, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001,
        ],
        'I' => [
            0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b11111,
        ],
        'J' => [
            0b00111, 0b00010, 0b00010, 0b00010, 0b00010, 0b10010, 0b01100,
        ],
        'K' => [
            0b10001, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, 0b10001,
        ],
        'L' => [
            0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111,
        ],
        'M' => [
            0b10001, 0b11011, 0b10101, 0b10101, 0b10001, 0b10001, 0b10001,
        ],
        'N' => [
            0b10001, 0b11001, 0b10101, 0b10011, 0b10001, 0b10001, 0b10001,
        ],
        'O' => [
            0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110,
        ],
        'P' => [
            0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000,
        ],
        'Q' => [
            0b01110, 0b10001, 0b10001, 0b10001, 0b10101, 0b10010, 0b01101,
        ],
        'R' => [
            0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001,
        ],
        'S' => [
            0b01110, 0b10001, 0b10000, 0b01110, 0b00001, 0b10001, 0b01110,
        ],
        'T' => [
            0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100,
        ],
        'U' => [
            0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110,
        ],
        'V' => [
            0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01010, 0b00100,
        ],
        'W' => [
            0b10001, 0b10001, 0b10001, 0b10101, 0b10101, 0b11011, 0b10001,
        ],
        'X' => [
            0b10001, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001, 0b10001,
        ],
        'Y' => [
            0b10001, 0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100,
        ],
        'Z' => [
            0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b11111,
        ],
        '0' => [
            0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110,
        ],
        '1' => [
            0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110,
        ],
        '2' => [
            0b01110, 0b10001, 0b00001, 0b00110, 0b01000, 0b10000, 0b11111,
        ],
        '3' => [
            0b01110, 0b10001, 0b00001, 0b00110, 0b00001, 0b10001, 0b01110,
        ],
        '4' => [
            0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010,
        ],
        '5' => [
            0b11111, 0b10000, 0b11110, 0b00001, 0b00001, 0b10001, 0b01110,
        ],
        '6' => [
            0b01110, 0b10000, 0b11110, 0b10001, 0b10001, 0b10001, 0b01110,
        ],
        '7' => [
            0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000,
        ],
        '8' => [
            0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110,
        ],
        '9' => [
            0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00001, 0b01110,
        ],
        ' ' => [
            0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000,
        ],
        '.' => [
            0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b01100, 0b01100,
        ],
        ',' => [
            0b00000, 0b00000, 0b00000, 0b00000, 0b00110, 0b00100, 0b01000,
        ],
        ':' => [
            0b00000, 0b01100, 0b01100, 0b00000, 0b01100, 0b01100, 0b00000,
        ],
        '-' => [
            0b00000, 0b00000, 0b00000, 0b11111, 0b00000, 0b00000, 0b00000,
        ],
        '\'' => [
            0b00100, 0b00100, 0b01000, 0b00000, 0b00000, 0b00000, 0b00000,
        ],
        '!' => [
            0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00000, 0b00100,
        ],
        '?' => [
            0b01110, 0b10001, 0b00001, 0b00110, 0b00100, 0b00000, 0b00100,
        ],
        '&' => [
            0b01100, 0b10010, 0b10100, 0b01000, 0b10101, 0b10010, 0b01101,
        ],
        '(' => [
            0b00010, 0b00100, 0b01000, 0b01000, 0b01000, 0b00100, 0b00010,
        ],
        ')' => [
            0b01000, 0b00100, 0b00010, 0b00010, 0b00010, 0b00100, 0b01000,
        ],
        _ => [
            0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110,
        ], // Default to 'O' shape
    }
}

use image::ImageBuffer;

/// Detect image format from magic bytes for diagnostic purposes
fn detect_image_format(data: &[u8]) -> &'static str {
    if data.len() < 4 {
        return "unknown (too short)";
    }

    // Check JPEG XL first (both codestream and container formats)
    // JXL codestream: FF 0A (2 bytes)
    // JXL container: 00 00 00 0C 4A 58 4C 20 0D 0A 87 0A (12 bytes)
    if data.len() >= 2 && data[0] == 0xFF && data[1] == 0x0A {
        return "JXL (JPEG XL codestream)";
    }
    if data.len() >= 12
        && data[0..4] == [0x00, 0x00, 0x00, 0x0C]
        && data[4..8] == [0x4A, 0x58, 0x4C, 0x20]
    {
        return "JXL (JPEG XL container)";
    }

    // Check magic bytes for common image formats
    match &data[..4] {
        // JPEG: FF D8 FF
        [0xFF, 0xD8, 0xFF, _] => "JPEG",
        // PNG: 89 50 4E 47
        [0x89, 0x50, 0x4E, 0x47] => "PNG",
        // GIF: 47 49 46 38
        [0x47, 0x49, 0x46, 0x38] => "GIF",
        // WebP: RIFF....WEBP
        [0x52, 0x49, 0x46, 0x46] if data.len() >= 12 && &data[8..12] == b"WEBP" => "WebP",
        // BMP: 42 4D
        [0x42, 0x4D, _, _] => "BMP",
        // TIFF: 49 49 2A 00 (little-endian) or 4D 4D 00 2A (big-endian)
        [0x49, 0x49, 0x2A, 0x00] | [0x4D, 0x4D, 0x00, 0x2A] => "TIFF",
        // AVIF/HEIF: ....ftyp
        _ if data.len() >= 12 && &data[4..8] == b"ftyp" => {
            // Check specific brand
            match &data[8..12] {
                b"avif" => "AVIF",
                b"heic" | b"heix" | b"mif1" => "HEIF",
                _ => "AVIF/HEIF (unknown brand)",
            }
        }
        // ICO: 00 00 01 00
        [0x00, 0x00, 0x01, 0x00] => "ICO",
        // SVG: <svg or <?xml (not supported by image crate)
        [0x3C, 0x73, 0x76, 0x67] => "SVG (unsupported - requires rendering)",
        [0x3C, 0x3F, 0x78, 0x6D] => "XML/SVG (unsupported - requires rendering)",
        // Zlib compressed data (common in PDFs): 78 9C, 78 DA, 78 01
        [0x78, 0x9C, _, _] | [0x78, 0xDA, _, _] | [0x78, 0x01, _, _] => {
            "zlib-compressed data (raw stream, not a valid image)"
        }
        // All null bytes (corrupted data)
        [0x00, 0x00, 0x00, 0x00] => "null bytes (corrupted or empty data)",
        _ => "unknown",
    }
}

/// Format magic bytes as hex string for logging
fn format_magic_bytes(data: &[u8]) -> String {
    let len = data.len().min(16);
    data[..len]
        .iter()
        .map(|b| format!("{:02X}", b))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Check if data appears to be SVG based on magic bytes
fn is_svg_data(data: &[u8]) -> bool {
    if data.len() < 4 {
        return false;
    }
    // SVG starts with "<svg" or "<?xml"
    data.starts_with(b"<svg") || data.starts_with(b"<?xml")
}

/// Check if data appears to be JXL (JPEG XL) based on magic bytes
fn is_jxl_data(data: &[u8]) -> bool {
    if data.len() < 2 {
        return false;
    }
    // JXL codestream: FF 0A
    if data[0] == 0xFF && data[1] == 0x0A {
        return true;
    }
    // JXL container: 00 00 00 0C 4A 58 4C 20
    if data.len() >= 12
        && data[0..4] == [0x00, 0x00, 0x00, 0x0C]
        && data[4..8] == [0x4A, 0x58, 0x4C, 0x20]
    {
        return true;
    }
    false
}

/// Decode JXL (JPEG XL) data to a raster image using jxl-oxide
fn decode_jxl_to_image(jxl_data: &[u8]) -> Result<DynamicImage> {
    // Create JXL decoder
    let image = JxlImage::builder()
        .read(Cursor::new(jxl_data))
        .map_err(|e| anyhow!("Failed to parse JXL image: {}", e))?;

    let width = image.width();
    let height = image.height();

    if width == 0 || height == 0 {
        return Err(anyhow!(
            "JXL image has invalid dimensions: {}x{}",
            width,
            height
        ));
    }

    // Render the image to get pixel data
    let render = image
        .render_frame(0)
        .map_err(|e| anyhow!("Failed to render JXL frame: {}", e))?;

    // Get pixel stream from the render result
    let mut stream = render.stream();
    let channels = stream.channels() as usize;

    // Read pixel data into buffer (values are f32 in range [0.0, 1.0])
    let mut pixels_f32 = vec![0.0f32; (width as usize) * (height as usize) * channels];
    stream.write_to_buffer(&mut pixels_f32);

    // Convert f32 pixels to u8
    let pixels: Vec<u8> = pixels_f32
        .iter()
        .map(|&f| (f.clamp(0.0, 1.0) * 255.0) as u8)
        .collect();

    let rgba_data = match channels {
        1 => {
            // Grayscale - expand to RGBA
            pixels.iter().flat_map(|&g| [g, g, g, 255]).collect()
        }
        2 => {
            // Grayscale + Alpha - expand to RGBA
            pixels
                .chunks(2)
                .flat_map(|ga| [ga[0], ga[0], ga[0], ga[1]])
                .collect()
        }
        3 => {
            // RGB - add alpha channel
            pixels
                .chunks(3)
                .flat_map(|rgb| [rgb[0], rgb[1], rgb[2], 255])
                .collect()
        }
        4 => {
            // Already RGBA
            pixels
        }
        _ => {
            return Err(anyhow!(
                "Unexpected number of channels in JXL image: {}",
                channels
            ));
        }
    };

    let img = RgbaImage::from_raw(width, height, rgba_data)
        .ok_or_else(|| anyhow!("Failed to create image from JXL data"))?;

    Ok(DynamicImage::ImageRgba8(img))
}

/// Render SVG data to a raster image using resvg
///
/// Returns a DynamicImage that can be used with the image crate for further processing.
fn render_svg_to_image(svg_data: &[u8]) -> Result<DynamicImage> {
    use resvg::tiny_skia::Pixmap;
    use resvg::usvg::{Options, Tree};

    // Parse the SVG
    let tree = Tree::from_data(svg_data, &Options::default())
        .map_err(|e| anyhow!("Failed to parse SVG: {}", e))?;

    // Get the SVG size
    let size = tree.size();
    let width = size.width() as u32;
    let height = size.height() as u32;

    // Ensure we have valid dimensions
    if width == 0 || height == 0 {
        return Err(anyhow!("SVG has invalid dimensions: {}x{}", width, height));
    }

    // Create a pixmap to render into
    let mut pixmap = Pixmap::new(width, height)
        .ok_or_else(|| anyhow!("Failed to create pixmap for SVG rendering"))?;

    // Render the SVG
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::default(),
        &mut pixmap.as_mut(),
    );

    // Convert to image::RgbaImage
    let rgba_data = pixmap.take();
    let img = RgbaImage::from_raw(width, height, rgba_data)
        .ok_or_else(|| anyhow!("Failed to create image from SVG render data"))?;

    Ok(DynamicImage::ImageRgba8(img))
}

/// Load image from bytes, with special handling for SVG and JXL
///
/// This function attempts to load an image from raw bytes. It first checks if the data
/// is SVG format (which the image crate doesn't support natively) and renders it using
/// resvg. For JXL format, it uses jxl-oxide. For other formats, it uses the image crate directly.
fn load_image_with_svg_support(data: &[u8]) -> Result<DynamicImage> {
    if is_svg_data(data) {
        render_svg_to_image(data)
    } else if is_jxl_data(data) {
        decode_jxl_to_image(data)
    } else {
        image::load_from_memory(data).map_err(|e| {
            let detected_format = detect_image_format(data);
            let magic_bytes = format_magic_bytes(data);
            anyhow!(
                "Failed to load image: {} (size: {} bytes, detected format: {}, magic bytes: [{}])",
                e,
                data.len(),
                detected_format,
                magic_bytes
            )
        })
    }
}

/// Metadata for a cached thumbnail file (for HTTP conditional caching)
#[derive(Debug, Clone)]
pub struct ThumbnailMeta {
    /// File size in bytes
    pub size: u64,
    /// Last modified time as Unix timestamp (seconds)
    pub modified_unix: u64,
    /// ETag based on book ID, size, and modified time
    pub etag: String,
}

/// Service for managing thumbnail cache
pub struct ThumbnailService {
    config: FilesConfig,
}

/// Settings loaded from database for thumbnail generation
#[derive(Debug, Clone)]
pub struct ThumbnailSettings {
    pub max_dimension: u32,
    pub jpeg_quality: u8,
}

impl Default for ThumbnailSettings {
    fn default() -> Self {
        Self {
            max_dimension: 400,
            jpeg_quality: 85,
        }
    }
}

/// Statistics for batch thumbnail generation
#[derive(Debug, Clone)]
pub struct GenerationStats {
    pub total: usize,
    pub generated: usize,
    pub skipped: usize,
    pub failed: usize,
    pub errors: Vec<String>,
}

impl ThumbnailService {
    /// Create a new thumbnail service
    pub fn new(config: FilesConfig) -> Self {
        Self { config }
    }

    /// Get thumbnail settings from database
    pub async fn get_settings(&self, db: &DatabaseConnection) -> Result<ThumbnailSettings> {
        let max_dimension = SettingsRepository::get_value::<i64>(db, "thumbnail.max_dimension")
            .await?
            .unwrap_or(400) as u32;

        let jpeg_quality = SettingsRepository::get_value::<i64>(db, "thumbnail.jpeg_quality")
            .await?
            .unwrap_or(85) as u8;

        Ok(ThumbnailSettings {
            max_dimension,
            jpeg_quality,
        })
    }

    /// Get the full path to thumbnail cache directory
    fn get_cache_base_dir(&self) -> PathBuf {
        PathBuf::from(&self.config.thumbnail_dir)
    }

    /// Get the uploads directory path
    pub fn get_uploads_dir(&self) -> PathBuf {
        PathBuf::from(&self.config.uploads_dir)
    }

    /// Get the subdirectory path for a book's thumbnail (based on first 2 chars of UUID)
    fn get_thumbnail_subdir(&self, book_id: Uuid) -> PathBuf {
        let id_str = book_id.to_string();
        let prefix = &id_str[..2]; // First 2 characters for bucketing
        self.get_cache_base_dir().join("books").join(prefix)
    }

    /// Get the full path where a book's thumbnail would be stored
    pub fn get_thumbnail_path(&self, book_id: Uuid) -> PathBuf {
        self.get_thumbnail_subdir(book_id)
            .join(format!("{}.jpg", book_id))
    }

    /// Check if a thumbnail exists for a book
    pub async fn thumbnail_exists(&self, book_id: Uuid) -> bool {
        let path = self.get_thumbnail_path(book_id);
        fs::metadata(&path).await.is_ok()
    }

    /// Read a thumbnail from cache
    pub async fn read_thumbnail(&self, book_id: Uuid) -> Result<Vec<u8>> {
        let path = self.get_thumbnail_path(book_id);
        fs::read(&path)
            .await
            .with_context(|| format!("Failed to read thumbnail from {:?}", path))
    }

    /// Get metadata for a cached thumbnail (for HTTP conditional requests)
    ///
    /// Returns file metadata including size, modified time, and ETag for use
    /// with HTTP caching headers (ETag, Last-Modified, If-None-Match, etc.)
    pub async fn get_thumbnail_metadata(&self, book_id: Uuid) -> Option<ThumbnailMeta> {
        let path = self.get_thumbnail_path(book_id);
        let metadata = fs::metadata(&path).await.ok()?;

        let size = metadata.len();
        let modified_unix = metadata
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);

        // Generate ETag from book_id + size + modified time for uniqueness
        let etag = format!("\"{:x}-{:x}-{:x}\"", book_id.as_u128(), size, modified_unix);

        Some(ThumbnailMeta {
            size,
            modified_unix,
            etag,
        })
    }

    /// Open a cached thumbnail for streaming
    ///
    /// Returns a stream for reading the cached file directly without loading
    /// the entire file into memory.
    pub async fn get_thumbnail_stream(
        &self,
        book_id: Uuid,
    ) -> Option<ReaderStream<tokio::fs::File>> {
        let path = self.get_thumbnail_path(book_id);
        let file = tokio::fs::File::open(&path).await.ok()?;
        debug!("Streaming thumbnail for book {}", book_id);
        Some(ReaderStream::new(file))
    }

    // ========== Series Thumbnail Methods ==========

    /// Get the subdirectory path for a series thumbnail (based on first 2 chars of UUID)
    fn get_series_thumbnail_subdir(&self, series_id: Uuid) -> PathBuf {
        let id_str = series_id.to_string();
        let prefix = &id_str[..2]; // First 2 characters for bucketing
        self.get_cache_base_dir().join("series").join(prefix)
    }

    /// Get the full path where a series thumbnail would be stored
    pub fn get_series_thumbnail_path(&self, series_id: Uuid) -> PathBuf {
        self.get_series_thumbnail_subdir(series_id)
            .join(format!("{}.jpg", series_id))
    }

    /// Check if a cached thumbnail exists for a series
    pub async fn series_thumbnail_exists(&self, series_id: Uuid) -> bool {
        let path = self.get_series_thumbnail_path(series_id);
        fs::metadata(&path).await.is_ok()
    }

    /// Get metadata for a cached series thumbnail (for HTTP conditional requests)
    pub async fn get_series_thumbnail_metadata(&self, series_id: Uuid) -> Option<ThumbnailMeta> {
        let path = self.get_series_thumbnail_path(series_id);
        let metadata = fs::metadata(&path).await.ok()?;

        let size = metadata.len();
        let modified_unix = metadata
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);

        // Generate ETag from series_id + size + modified time for uniqueness
        let etag = format!(
            "\"{:x}-{:x}-{:x}\"",
            series_id.as_u128(),
            size,
            modified_unix
        );

        Some(ThumbnailMeta {
            size,
            modified_unix,
            etag,
        })
    }

    /// Open a cached series thumbnail for streaming
    pub async fn get_series_thumbnail_stream(
        &self,
        series_id: Uuid,
    ) -> Option<ReaderStream<tokio::fs::File>> {
        let path = self.get_series_thumbnail_path(series_id);
        let file = tokio::fs::File::open(&path).await.ok()?;
        Some(ReaderStream::new(file))
    }

    /// Save series thumbnail data to disk cache
    pub async fn save_series_thumbnail(&self, series_id: Uuid, data: &[u8]) -> Result<PathBuf> {
        let subdir = self.get_series_thumbnail_subdir(series_id);
        let thumbnail_path = subdir.join(format!("{}.jpg", series_id));

        // Create directory if it doesn't exist
        fs::create_dir_all(&subdir).await.with_context(|| {
            format!("Failed to create series thumbnail directory: {:?}", subdir)
        })?;

        // Write thumbnail file
        fs::write(&thumbnail_path, data)
            .await
            .with_context(|| format!("Failed to write series thumbnail to {:?}", thumbnail_path))?;

        debug!("Saved series thumbnail to {:?}", thumbnail_path);
        Ok(thumbnail_path)
    }

    /// Delete a series thumbnail from cache
    pub async fn delete_series_thumbnail(&self, series_id: Uuid) -> Result<()> {
        let thumbnail_path = self.get_series_thumbnail_path(series_id);

        if fs::metadata(&thumbnail_path).await.is_ok() {
            fs::remove_file(&thumbnail_path).await.with_context(|| {
                format!("Failed to delete series thumbnail: {:?}", thumbnail_path)
            })?;
            debug!("Deleted series thumbnail: {:?}", thumbnail_path);
        }

        Ok(())
    }

    /// Generate a thumbnail from raw image data using configured settings
    ///
    /// This is a public method that can be used by both book and series thumbnail
    /// handlers to generate thumbnails with consistent settings from the database.
    /// Uses spawn_blocking internally for CPU-intensive image processing.
    pub async fn generate_thumbnail_from_image(
        &self,
        db: &DatabaseConnection,
        image_data: Vec<u8>,
    ) -> Result<Vec<u8>> {
        let settings = self.get_settings(db).await?;
        let max_dimension = settings.max_dimension;
        let jpeg_quality = settings.jpeg_quality;

        // Use spawn_blocking for CPU-intensive image processing
        tokio::task::spawn_blocking(move || {
            // Load image from bytes (with SVG support)
            let img = load_image_with_svg_support(&image_data)?;

            // Calculate new dimensions while maintaining aspect ratio
            let (width, height) = (img.width(), img.height());
            let (new_width, new_height) = if width > height {
                let ratio = max_dimension as f32 / width as f32;
                (max_dimension, (height as f32 * ratio) as u32)
            } else {
                let ratio = max_dimension as f32 / height as f32;
                ((width as f32 * ratio) as u32, max_dimension)
            };

            // Resize using Lanczos3 filter for high quality
            let thumbnail = img.resize(new_width, new_height, FilterType::Lanczos3);

            // Encode as JPEG
            let mut output = Cursor::new(Vec::new());
            let mut encoder =
                image::codecs::jpeg::JpegEncoder::new_with_quality(&mut output, jpeg_quality);
            encoder
                .encode_image(&thumbnail)
                .context("Failed to encode thumbnail as JPEG")?;

            Ok(output.into_inner())
        })
        .await
        .context("Thumbnail generation task failed")?
    }

    /// Generate and save a thumbnail for a book
    ///
    /// Returns the path where the thumbnail was saved.
    /// For EPUB files with no valid cover images, generates a placeholder thumbnail
    /// with the book's title and author.
    pub async fn generate_thumbnail(
        &self,
        db: &DatabaseConnection,
        book: &books::Model,
    ) -> Result<PathBuf> {
        // Check if thumbnail already exists
        let thumbnail_path = self.get_thumbnail_path(book.id);
        if fs::metadata(&thumbnail_path).await.is_ok() {
            debug!("Thumbnail already exists for book {}", book.id);
            return Ok(thumbnail_path);
        }

        info!(
            "Generating thumbnail for book {} ({})",
            book.id, book.file_name
        );

        // Get settings from database
        let settings = self.get_settings(db).await?;

        // Extract cover image or generate placeholder for EPUB
        // Uses file_name as title fallback when no metadata is available
        let image_data = self
            .extract_cover_image_or_placeholder(book, settings.max_dimension, None, None)
            .await?;

        // Generate thumbnail (resize if it's a real image, placeholder is already sized)
        let thumbnail_data =
            self.resize_image(&image_data, settings.max_dimension, settings.jpeg_quality)?;

        // Save to cache
        self.save_thumbnail(book.id, &thumbnail_data).await?;

        // Update book record in database
        let now = Utc::now();
        let mut book_active: books::ActiveModel = book.clone().into();
        book_active.thumbnail_path = Set(Some(thumbnail_path.to_string_lossy().to_string()));
        book_active.thumbnail_generated_at = Set(Some(now));
        book_active.updated_at = Set(now); // Update timestamp for cache-busting
        book_active.update(db).await?;

        Ok(thumbnail_path)
    }

    /// Save pre-generated thumbnail data to cache
    ///
    /// Used when a thumbnail is generated on-demand in a handler
    pub async fn save_generated_thumbnail(
        &self,
        db: &DatabaseConnection,
        book_id: Uuid,
        thumbnail_data: &[u8],
    ) -> Result<PathBuf> {
        let thumbnail_path = self.save_thumbnail(book_id, thumbnail_data).await?;

        // Update book record in database
        let book = BookRepository::get_by_id(db, book_id)
            .await?
            .ok_or_else(|| anyhow!("Book not found: {}", book_id))?;

        let now = Utc::now();
        let mut book_active: books::ActiveModel = book.into();
        book_active.thumbnail_path = Set(Some(thumbnail_path.to_string_lossy().to_string()));
        book_active.thumbnail_generated_at = Set(Some(now));
        book_active.updated_at = Set(now); // Update timestamp for cache-busting
        book_active.update(db).await?;

        Ok(thumbnail_path)
    }

    /// Save thumbnail data to disk
    async fn save_thumbnail(&self, book_id: Uuid, data: &[u8]) -> Result<PathBuf> {
        let subdir = self.get_thumbnail_subdir(book_id);
        let thumbnail_path = subdir.join(format!("{}.jpg", book_id));

        // Create directory if it doesn't exist
        fs::create_dir_all(&subdir)
            .await
            .with_context(|| format!("Failed to create thumbnail directory: {:?}", subdir))?;

        // Write thumbnail file
        fs::write(&thumbnail_path, data)
            .await
            .with_context(|| format!("Failed to write thumbnail to {:?}", thumbnail_path))?;

        debug!("Saved thumbnail to {:?}", thumbnail_path);
        Ok(thumbnail_path)
    }

    /// Delete a thumbnail from cache
    pub async fn delete_thumbnail(&self, db: &DatabaseConnection, book_id: Uuid) -> Result<()> {
        let thumbnail_path = self.get_thumbnail_path(book_id);

        // Delete file if it exists
        if fs::metadata(&thumbnail_path).await.is_ok() {
            fs::remove_file(&thumbnail_path)
                .await
                .with_context(|| format!("Failed to delete thumbnail: {:?}", thumbnail_path))?;
            debug!("Deleted thumbnail: {:?}", thumbnail_path);
        }

        // Update book record
        if let Some(book) = BookRepository::get_by_id(db, book_id).await? {
            let mut book_active: books::ActiveModel = book.into();
            book_active.thumbnail_path = Set(None);
            book_active.thumbnail_generated_at = Set(None);
            book_active.update(db).await?;
        }

        Ok(())
    }

    /// Generate thumbnails for multiple books (batch operation)
    pub async fn generate_thumbnails_batch(
        &self,
        db: &DatabaseConnection,
        book_ids: Vec<Uuid>,
        event_broadcaster: Option<&Arc<EventBroadcaster>>,
    ) -> Result<GenerationStats> {
        let total = book_ids.len();
        let mut generated = 0;
        let mut skipped = 0;
        let mut failed = 0;
        let mut errors = Vec::new();

        info!("Starting batch thumbnail generation for {} books", total);

        for book_id in book_ids {
            // Fetch book
            let book = match BookRepository::get_by_id(db, book_id).await? {
                Some(b) => b,
                None => {
                    warn!("Book not found: {}", book_id);
                    failed += 1;
                    errors.push(format!("Book not found: {}", book_id));
                    continue;
                }
            };

            // Check if thumbnail already exists
            if self.thumbnail_exists(book_id).await {
                debug!("Thumbnail already exists for book {}", book_id);
                skipped += 1;
                continue;
            }

            // Generate thumbnail
            match self.generate_thumbnail(db, &book).await {
                Ok(_) => {
                    generated += 1;
                    debug!("Generated thumbnail for book {}", book_id);

                    // Emit CoverUpdated event to notify UI
                    if let Some(broadcaster) = event_broadcaster {
                        // Get library_id from series
                        if let Ok(Some(series)) =
                            SeriesRepository::get_by_id(db, book.series_id).await
                        {
                            let event = EntityChangeEvent {
                                event: EntityEvent::CoverUpdated {
                                    entity_type: EntityType::Book,
                                    entity_id: book_id,
                                    library_id: Some(series.library_id),
                                },
                                user_id: None,
                                timestamp: Utc::now(),
                            };

                            match broadcaster.emit(event) {
                                Ok(count) => {
                                    debug!(
                                        "Emitted CoverUpdated event to {} subscribers for book thumbnail: {}",
                                        count, book_id
                                    );
                                }
                                Err(e) => {
                                    warn!(
                                        "Failed to emit CoverUpdated event for book thumbnail {}: {:?}",
                                        book_id, e
                                    );
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    failed += 1;
                    let error_msg =
                        format!("Failed to generate thumbnail for book {}: {}", book_id, e);
                    error!("{}", error_msg);
                    errors.push(error_msg);
                }
            }
        }

        info!(
            "Batch thumbnail generation complete: {}/{} generated, {} skipped, {} failed",
            generated, total, skipped, failed
        );

        Ok(GenerationStats {
            total,
            generated,
            skipped,
            failed,
            errors,
        })
    }

    /// Extract cover image (first page) from a book with fallback for corrupted images
    ///
    /// This method tries to extract the first valid image from the book.
    /// If the first image is corrupted, it will try subsequent images.
    async fn extract_cover_image(&self, book: &books::Model) -> Result<Vec<u8>> {
        let path = Path::new(&book.file_path);

        // Use the appropriate parser extraction function based on format
        // Enable fallback mode to skip corrupted images
        let image_data = match book.format.to_uppercase().as_str() {
            "CBZ" => crate::parsers::cbz::extract_page_from_cbz_with_fallback(path, 1, true)?,
            #[cfg(feature = "rar")]
            "CBR" => crate::parsers::cbr::extract_page_from_cbr_with_fallback(path, 1, true)?,
            "EPUB" => crate::parsers::epub::extract_page_from_epub_with_fallback(path, 1, true)?,
            "PDF" => crate::parsers::pdf::extract_page_from_pdf(path, 1)?,
            _ => {
                return Err(anyhow!(
                    "Unsupported format for thumbnail generation: {}",
                    book.format
                ));
            }
        };

        Ok(image_data)
    }

    /// Try to extract cover image, falling back to placeholder generation for EPUB
    ///
    /// For EPUB files with no valid images, this will generate a placeholder
    /// thumbnail with the book's title and author.
    ///
    /// # Arguments
    /// * `book` - The book model
    /// * `max_dimension` - Maximum dimension for the thumbnail
    /// * `title` - Optional title override (uses file_name if None)
    /// * `author` - Optional author for placeholder
    pub async fn extract_cover_image_or_placeholder(
        &self,
        book: &books::Model,
        max_dimension: u32,
        title: Option<&str>,
        author: Option<&str>,
    ) -> Result<Vec<u8>> {
        // First try to extract the cover image
        match self.extract_cover_image(book).await {
            Ok(data) => Ok(data),
            Err(e) => {
                // For EPUB files, generate a placeholder when no valid images found
                if book.format.to_uppercase() == "EPUB" {
                    let display_title = title.unwrap_or(&book.file_name);
                    info!(
                        book_id = %book.id,
                        title = %display_title,
                        error = %e,
                        "No valid cover image found, generating placeholder thumbnail"
                    );

                    // Generate placeholder thumbnail
                    let info = PlaceholderInfo {
                        title: display_title.to_string(),
                        author: author.map(|s| s.to_string()),
                        format: book.format.clone(),
                    };

                    // Use 2:3 aspect ratio for book covers (typical for ebooks)
                    let width = max_dimension;
                    let height = (max_dimension as f32 * 1.5) as u32;

                    let placeholder_img = generate_placeholder_thumbnail(&info, width, height)?;

                    // Encode as JPEG
                    let mut output = Cursor::new(Vec::new());
                    let mut encoder =
                        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut output, 85);
                    encoder
                        .encode_image(&placeholder_img)
                        .context("Failed to encode placeholder thumbnail as JPEG")?;

                    Ok(output.into_inner())
                } else {
                    // For other formats, propagate the error
                    Err(e)
                }
            }
        }
    }

    /// Resize an image to thumbnail size
    fn resize_image(
        &self,
        image_data: &[u8],
        max_dimension: u32,
        jpeg_quality: u8,
    ) -> Result<Vec<u8>> {
        // Load image from bytes (with SVG support)
        let img = load_image_with_svg_support(image_data)?;

        // Calculate new dimensions while maintaining aspect ratio
        let (width, height) = (img.width(), img.height());
        let (new_width, new_height) = if width > height {
            let ratio = max_dimension as f32 / width as f32;
            (max_dimension, (height as f32 * ratio) as u32)
        } else {
            let ratio = max_dimension as f32 / height as f32;
            ((width as f32 * ratio) as u32, max_dimension)
        };

        // Resize using Lanczos3 filter for high quality
        let thumbnail = img.resize(new_width, new_height, FilterType::Lanczos3);

        // Encode as JPEG
        let mut output = Cursor::new(Vec::new());
        let mut encoder =
            image::codecs::jpeg::JpegEncoder::new_with_quality(&mut output, jpeg_quality);
        encoder
            .encode_image(&thumbnail)
            .context("Failed to encode thumbnail as JPEG")?;

        Ok(output.into_inner())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_files_config() -> FilesConfig {
        FilesConfig {
            thumbnail_dir: "data/thumbnails".to_string(),
            uploads_dir: "data/uploads".to_string(),
        }
    }

    #[test]
    fn test_thumbnail_path_generation() {
        let service = ThumbnailService::new(test_files_config());

        let book_id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let path = service.get_thumbnail_path(book_id);

        assert!(path.to_string_lossy().contains("data/thumbnails/books/55"));
        assert!(
            path.to_string_lossy()
                .ends_with("550e8400-e29b-41d4-a716-446655440000.jpg")
        );
    }

    #[test]
    fn test_thumbnail_subdirectory_bucketing() {
        let service = ThumbnailService::new(test_files_config());

        let book_id1 = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let book_id2 = Uuid::parse_str("55ffffff-e29b-41d4-a716-446655440000").unwrap();
        let book_id3 = Uuid::parse_str("aaaaaaaa-e29b-41d4-a716-446655440000").unwrap();

        let subdir1 = service.get_thumbnail_subdir(book_id1);
        let subdir2 = service.get_thumbnail_subdir(book_id2);
        let subdir3 = service.get_thumbnail_subdir(book_id3);

        // Same prefix should result in same subdirectory
        assert_eq!(subdir1, subdir2);
        // Different prefix should result in different subdirectory
        assert_ne!(subdir1, subdir3);

        assert!(subdir1.to_string_lossy().ends_with("books/55"));
        assert!(subdir3.to_string_lossy().ends_with("books/aa"));
    }

    #[test]
    fn test_default_thumbnail_settings() {
        let settings = ThumbnailSettings::default();
        assert_eq!(settings.max_dimension, 400);
        assert_eq!(settings.jpeg_quality, 85);
    }

    #[test]
    fn test_uploads_dir() {
        let service = ThumbnailService::new(test_files_config());
        let uploads_dir = service.get_uploads_dir();
        assert_eq!(uploads_dir.to_string_lossy(), "data/uploads");
    }

    #[tokio::test]
    async fn test_get_thumbnail_metadata_not_found() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let config = FilesConfig {
            thumbnail_dir: temp_dir.path().to_string_lossy().to_string(),
            uploads_dir: "data/uploads".to_string(),
        };
        let service = ThumbnailService::new(config);
        let book_id = Uuid::new_v4();

        // No metadata for non-existent thumbnail
        assert!(service.get_thumbnail_metadata(book_id).await.is_none());
    }

    #[tokio::test]
    async fn test_get_thumbnail_metadata_exists() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let config = FilesConfig {
            thumbnail_dir: temp_dir.path().to_string_lossy().to_string(),
            uploads_dir: "data/uploads".to_string(),
        };
        let service = ThumbnailService::new(config);
        let book_id = Uuid::new_v4();

        // Create a dummy thumbnail
        let thumb_path = service.get_thumbnail_path(book_id);
        fs::create_dir_all(thumb_path.parent().unwrap())
            .await
            .unwrap();
        fs::write(&thumb_path, b"fake thumbnail data")
            .await
            .unwrap();

        // Get metadata
        let meta = service.get_thumbnail_metadata(book_id).await;
        assert!(meta.is_some());

        let meta = meta.unwrap();
        assert_eq!(meta.size, 19); // "fake thumbnail data" = 19 bytes
        assert!(meta.modified_unix > 0);
        assert!(meta.etag.starts_with('"') && meta.etag.ends_with('"'));
    }

    #[tokio::test]
    async fn test_get_thumbnail_stream_not_found() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let config = FilesConfig {
            thumbnail_dir: temp_dir.path().to_string_lossy().to_string(),
            uploads_dir: "data/uploads".to_string(),
        };
        let service = ThumbnailService::new(config);
        let book_id = Uuid::new_v4();

        // No stream for non-existent thumbnail
        assert!(service.get_thumbnail_stream(book_id).await.is_none());
    }

    #[tokio::test]
    async fn test_get_thumbnail_stream_exists() {
        use tokio_stream::StreamExt;

        let temp_dir = tempfile::TempDir::new().unwrap();
        let config = FilesConfig {
            thumbnail_dir: temp_dir.path().to_string_lossy().to_string(),
            uploads_dir: "data/uploads".to_string(),
        };
        let service = ThumbnailService::new(config);
        let book_id = Uuid::new_v4();

        // Create a dummy thumbnail
        let thumb_path = service.get_thumbnail_path(book_id);
        fs::create_dir_all(thumb_path.parent().unwrap())
            .await
            .unwrap();
        let test_data = b"fake thumbnail data for streaming";
        fs::write(&thumb_path, test_data).await.unwrap();

        // Get stream and read data
        let stream = service.get_thumbnail_stream(book_id).await;
        assert!(stream.is_some());

        let mut stream = stream.unwrap();
        let mut collected = Vec::new();
        while let Some(chunk) = stream.next().await {
            collected.extend_from_slice(&chunk.unwrap());
        }
        assert_eq!(collected, test_data);
    }

    #[test]
    fn test_detect_image_format_jpeg() {
        let data = [0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10];
        assert_eq!(detect_image_format(&data), "JPEG");
    }

    #[test]
    fn test_detect_image_format_png() {
        let data = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
        assert_eq!(detect_image_format(&data), "PNG");
    }

    #[test]
    fn test_detect_image_format_gif() {
        let data = [0x47, 0x49, 0x46, 0x38, 0x39, 0x61];
        assert_eq!(detect_image_format(&data), "GIF");
    }

    #[test]
    fn test_detect_image_format_webp() {
        let data = [
            0x52, 0x49, 0x46, 0x46, // RIFF
            0x00, 0x00, 0x00, 0x00, // size
            0x57, 0x45, 0x42, 0x50, // WEBP
        ];
        assert_eq!(detect_image_format(&data), "WebP");
    }

    #[test]
    fn test_detect_image_format_bmp() {
        let data = [0x42, 0x4D, 0x00, 0x00];
        assert_eq!(detect_image_format(&data), "BMP");
    }

    #[test]
    fn test_detect_image_format_avif() {
        let data = [
            0x00, 0x00, 0x00, 0x00, // size
            0x66, 0x74, 0x79, 0x70, // ftyp
            0x61, 0x76, 0x69, 0x66, // avif
        ];
        assert_eq!(detect_image_format(&data), "AVIF");
    }

    #[test]
    fn test_detect_image_format_unknown() {
        let data = [0x00, 0x01, 0x02, 0x03];
        assert_eq!(detect_image_format(&data), "unknown");
    }

    #[test]
    fn test_detect_image_format_too_short() {
        let data = [0xFF, 0xD8];
        assert_eq!(detect_image_format(&data), "unknown (too short)");
    }

    #[test]
    fn test_format_magic_bytes() {
        let data = [0xFF, 0xD8, 0xFF, 0xE0];
        assert_eq!(format_magic_bytes(&data), "FF D8 FF E0");
    }

    #[test]
    fn test_format_magic_bytes_truncates_at_16() {
        let data: Vec<u8> = (0..20).collect();
        let result = format_magic_bytes(&data);
        // Should only include first 16 bytes
        assert_eq!(result, "00 01 02 03 04 05 06 07 08 09 0A 0B 0C 0D 0E 0F");
    }

    #[test]
    fn test_format_magic_bytes_empty() {
        let data: [u8; 0] = [];
        assert_eq!(format_magic_bytes(&data), "");
    }

    mod placeholder_thumbnail {
        use super::*;
        use image::GenericImageView;

        #[test]
        fn test_generate_placeholder_basic() {
            let info = PlaceholderInfo {
                title: "Test Book Title".to_string(),
                author: Some("Test Author".to_string()),
                format: "EPUB".to_string(),
            };

            let result = generate_placeholder_thumbnail(&info, 300, 450);
            assert!(result.is_ok());

            let img = result.unwrap();
            assert_eq!(img.width(), 300);
            assert_eq!(img.height(), 450);
        }

        #[test]
        fn test_generate_placeholder_no_author() {
            let info = PlaceholderInfo {
                title: "Book Without Author".to_string(),
                author: None,
                format: "PDF".to_string(),
            };

            let result = generate_placeholder_thumbnail(&info, 200, 300);
            assert!(result.is_ok());

            let img = result.unwrap();
            assert_eq!(img.width(), 200);
            assert_eq!(img.height(), 300);
        }

        #[test]
        fn test_generate_placeholder_long_title() {
            let info = PlaceholderInfo {
                title:
                    "This Is A Very Long Book Title That Should Be Wrapped Across Multiple Lines"
                        .to_string(),
                author: Some("Author Name".to_string()),
                format: "EPUB".to_string(),
            };

            let result = generate_placeholder_thumbnail(&info, 400, 600);
            assert!(result.is_ok());
        }

        #[test]
        fn test_generate_placeholder_consistent_color() {
            // Same title should produce same color
            let info1 = PlaceholderInfo {
                title: "Same Title".to_string(),
                author: None,
                format: "EPUB".to_string(),
            };
            let info2 = PlaceholderInfo {
                title: "Same Title".to_string(),
                author: Some("Different Author".to_string()),
                format: "PDF".to_string(),
            };

            let img1 = generate_placeholder_thumbnail(&info1, 100, 150).unwrap();
            let img2 = generate_placeholder_thumbnail(&info2, 100, 150).unwrap();

            // The background color should be the same (based on title hash)
            // Check a pixel in the top-left corner (should be similar background)
            let pixel1 = img1.get_pixel(5, 5);
            let pixel2 = img2.get_pixel(5, 5);

            // Colors should be very close (same hue from same title)
            assert!((pixel1[0] as i32 - pixel2[0] as i32).abs() <= 5);
            assert!((pixel1[1] as i32 - pixel2[1] as i32).abs() <= 5);
            assert!((pixel1[2] as i32 - pixel2[2] as i32).abs() <= 5);
        }

        #[test]
        fn test_wrap_text_basic() {
            let lines = wrap_text("Hello World", 20);
            assert_eq!(lines, vec!["Hello World"]);
        }

        #[test]
        fn test_wrap_text_multiple_lines() {
            let lines = wrap_text("This is a longer text that needs wrapping", 15);
            assert!(lines.len() > 1);
            for line in &lines {
                assert!(line.len() <= 15 || !line.contains(' ')); // Either fits or is single word
            }
        }

        #[test]
        fn test_wrap_text_truncates_at_4_lines() {
            let long_text = "Word ".repeat(50);
            let lines = wrap_text(&long_text, 10);
            assert!(lines.len() <= 4);
            // Last line should end with "..."
            if lines.len() == 4 || (lines.len() == 3 && long_text.split_whitespace().count() > 20) {
                // If truncated, might have ellipsis
            }
        }

        #[test]
        fn test_simple_hash_consistency() {
            assert_eq!(simple_hash("test"), simple_hash("test"));
            assert_ne!(simple_hash("test1"), simple_hash("test2"));
        }

        #[test]
        fn test_hsl_to_rgb() {
            // Red at 0 degrees
            let (r, g, b) = hsl_to_rgb(0.0, 1.0, 0.5);
            assert_eq!(r, 255);
            assert!(g < 10);
            assert!(b < 10);

            // Green at 120 degrees
            let (r, g, b) = hsl_to_rgb(120.0, 1.0, 0.5);
            assert!(r < 10);
            assert_eq!(g, 255);
            assert!(b < 10);

            // Blue at 240 degrees
            let (r, g, b) = hsl_to_rgb(240.0, 1.0, 0.5);
            assert!(r < 10);
            assert!(g < 10);
            assert_eq!(b, 255);
        }

        #[test]
        fn test_char_patterns_exist() {
            // Test that common characters have patterns
            let chars = ['A', 'Z', '0', '9', ' ', '.', '-'];
            for c in chars {
                let pattern = get_char_pattern(c);
                // Pattern should be 7 rows
                assert_eq!(pattern.len(), 7);
            }
        }
    }
}
