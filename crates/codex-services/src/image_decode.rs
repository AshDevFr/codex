//! Bounded image decoding.
//!
//! `image::load_from_memory` decodes with no allocation cap, so a file that
//! declares enormous dimensions (a decompression bomb, often tiny on disk)
//! makes the decoder allocate `width * height * channels` bytes and can OOM the
//! worker process — and, with no container memory limit, the host. These
//! helpers cap the decoder's allocation budget so such an image returns an
//! error instead of taking the process down.

use image::DynamicImage;
use std::io::Cursor;

/// Maximum bytes the decoder may allocate for a single image (512 MiB).
///
/// Legitimate comic/book pages decode well under this (a 4K-ish RGBA page is
/// ~30 MiB); decompression bombs exceed it and fail cleanly.
pub const MAX_IMAGE_DECODE_BYTES: u64 = 512 * 1024 * 1024;

/// Decode an image from memory with the default allocation cap.
pub fn decode_image_limited(data: &[u8]) -> image::ImageResult<DynamicImage> {
    decode_image_with_limit(data, MAX_IMAGE_DECODE_BYTES)
}

/// Decode an image from memory, failing with an [`image::error::LimitError`] if
/// decoding would allocate more than `max_alloc` bytes.
pub fn decode_image_with_limit(data: &[u8], max_alloc: u64) -> image::ImageResult<DynamicImage> {
    let mut reader = image::ImageReader::new(Cursor::new(data)).with_guessed_format()?;
    let mut limits = image::Limits::default();
    limits.max_alloc = Some(max_alloc);
    reader.limits(limits);
    reader.decode()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encode_png(w: u32, h: u32) -> Vec<u8> {
        let img = image::RgbaImage::from_pixel(w, h, image::Rgba([1, 2, 3, 255]));
        let mut buf = Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(img)
            .write_to(&mut buf, image::ImageFormat::Png)
            .unwrap();
        buf.into_inner()
    }

    #[test]
    fn decodes_within_limit() {
        // 64x64 RGBA decodes to ~16 KiB, comfortably under a 10 MiB budget.
        let data = encode_png(64, 64);
        let img = decode_image_with_limit(&data, 10 * 1024 * 1024).expect("should decode");
        assert_eq!((img.width(), img.height()), (64, 64));
    }

    #[test]
    fn rejects_when_decode_exceeds_limit() {
        // 1024x1024 RGBA decodes to 4 MiB; a 256 KiB budget must reject it
        // rather than allocating the full buffer.
        let data = encode_png(1024, 1024);
        assert!(
            decode_image_with_limit(&data, 256 * 1024).is_err(),
            "decode must fail when it would exceed the allocation limit"
        );
    }

    #[test]
    fn rejects_non_image_bytes() {
        assert!(decode_image_with_limit(b"not an image", MAX_IMAGE_DECODE_BYTES).is_err());
    }
}
