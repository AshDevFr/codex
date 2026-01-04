pub mod cbz;
#[cfg(feature = "rar")]
pub mod cbr;
pub mod epub;
pub mod pdf;
pub mod comic_info;
pub mod metadata;
pub mod traits;
pub mod image_utils;

pub use comic_info::parse_comic_info;
pub use metadata::*;
pub use traits::FormatParser;
pub use image_utils::{is_image_file, get_image_format};
