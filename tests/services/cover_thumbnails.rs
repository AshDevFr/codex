//! Cover rendering when a page cannot be decoded.
//!
//! AVIF pages are served to clients untouched and every client renders them, but
//! no AVIF decoder is linked into the server, so a cover cannot be built from
//! one. The renderer must walk to a page it can decode, and produce a
//! placeholder rather than an error when no page works.

#[path = "../common/mod.rs"]
mod common;

use codex::config::FilesConfig;
use codex::services::ThumbnailService;
use common::*;
use tempfile::TempDir;
use uuid::Uuid;

/// Decode a JPEG thumbnail and report its dimensions.
fn thumbnail_dimensions(data: &[u8]) -> (u32, u32) {
    let img = image::load_from_memory(data).expect("thumbnail should be a decodable JPEG");
    (img.width(), img.height())
}

fn book_for(path: &std::path::Path, file_name: &str) -> codex::db::entities::books::Model {
    create_test_book(
        Uuid::new_v4(),
        Uuid::new_v4(),
        path.to_str().unwrap(),
        file_name,
        "hash",
        "cbz",
        3,
    )
}

#[tokio::test]
async fn renders_the_cover_from_a_later_page_when_the_first_cannot_be_decoded() {
    let temp_dir = TempDir::new().unwrap();
    let cbz_path = create_avif_first_cbz(&temp_dir);
    let book = book_for(&cbz_path, "avif_first.cbz");

    let service = ThumbnailService::new(FilesConfig::default());
    let thumbnail = service
        .render_cover_thumbnail(&book, 400, 85, None, None)
        .await
        .expect("a decodable later page must produce a cover");

    // Page 2 is 20x28 scaled to fit a 400px bound, which lands on 285x399. The
    // placeholder would be 400x600, which is how this distinguishes a real cover
    // from a fallback.
    assert_eq!(
        thumbnail_dimensions(&thumbnail),
        (285, 399),
        "the cover must come from the WebP page, not the placeholder"
    );
}

#[tokio::test]
async fn falls_back_to_a_placeholder_when_no_page_can_be_decoded() {
    let temp_dir = TempDir::new().unwrap();
    let cbz_path = create_all_avif_cbz(&temp_dir);
    let book = book_for(&cbz_path, "all_avif.cbz");

    let service = ThumbnailService::new(FilesConfig::default());
    let thumbnail = service
        .render_cover_thumbnail(&book, 400, 85, None, None)
        .await
        .expect("an undecodable book must still get a thumbnail");

    // The placeholder is built at the 2:3 ratio used for book covers.
    assert_eq!(
        thumbnail_dimensions(&thumbnail),
        (400, 600),
        "with nothing decodable the placeholder is the honest answer"
    );
}

#[tokio::test]
async fn renders_an_ordinary_cover_from_the_first_page() {
    let temp_dir = TempDir::new().unwrap();
    let cbz_path = create_test_cbz(&temp_dir, 3, false);
    let book = book_for(&cbz_path, "test_comic.cbz");

    let service = ThumbnailService::new(FilesConfig::default());
    let thumbnail = service
        .render_cover_thumbnail(&book, 400, 85, None, None)
        .await
        .expect("a normal archive must render its first page");

    // create_test_cbz writes 10x10 pages, which are already under the bound.
    assert_eq!(thumbnail_dimensions(&thumbnail), (400, 400));
}
