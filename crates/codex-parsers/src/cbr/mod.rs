mod parser;

pub use parser::{
    CbrParser, extract_page_from_cbr, extract_page_from_cbr_by_name,
    extract_page_from_cbr_with_fallback,
};
