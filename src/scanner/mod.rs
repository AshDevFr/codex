mod analyzer;
mod analyzer_queue;
mod detector;
mod library_scanner;
mod types;

pub use analyzer::analyze_file;
pub use analyzer_queue::{
    analyze_book, analyze_library_books, analyze_series_books, AnalysisResult, AnalyzerConfig,
};
pub use detector::detect_format;
pub use library_scanner::scan_library;
pub use types::{ScanMode, ScanProgress, ScanResult, ScanStatus, ScanningConfig};
