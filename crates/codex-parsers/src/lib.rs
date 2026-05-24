//! Codex file-format parsers (CBZ, CBR, EPUB, PDF) and shared metadata
//! utilities.
//!
//! Owns its own [`ParserError`] / [`Result`] types. Depends on `codex-utils`
//! only for the file-level hasher. No upward deps to db/services/api.

#[cfg(feature = "rar")]
pub mod cbr;
pub mod cbz;
pub mod comic_info;
pub mod epub;
pub mod error;
pub mod image_utils;
pub mod isbn_utils;
pub mod metadata;
pub mod opf;
pub mod pdf;
pub mod series_json;
pub mod traits;

pub use comic_info::parse_comic_info;
pub use error::{ParserError, Result};
pub use metadata::*;
