pub mod cbz;
#[cfg(feature = "rar")]
pub mod cbr;
pub mod epub;
pub mod pdf;
pub mod comic_info;
pub mod metadata;
pub mod traits;

pub use comic_info::parse_comic_info;
pub use metadata::*;
pub use traits::FormatParser;
