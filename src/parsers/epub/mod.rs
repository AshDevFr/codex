pub mod parser;

#[allow(unused_imports)] // Public API - may be used by external callers
pub use parser::{
    EpubParser, extract_cover_from_epub, extract_cover_from_epub_with_fallback,
    extract_page_from_epub, extract_page_from_epub_with_fallback,
};
