mod parser;

pub use parser::{
    CbzParser, extract_page_from_cbz, extract_page_from_cbz_by_name,
    extract_page_from_cbz_with_fallback,
};
