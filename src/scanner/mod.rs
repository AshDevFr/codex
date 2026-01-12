mod analyzer;
mod analyzer_queue;
mod detector;
mod library_scanner;
pub mod strategies;
mod types;

pub use analyzer::analyze_file;
pub use analyzer_queue::{analyze_book, AnalysisResult};
pub use detector::detect_format;
pub use library_scanner::scan_library;
pub use strategies::{
    create_book_strategy, create_strategy, BookMetadata, BookNamingContext, BookNamingStrategy,
    DetectedBook, DetectedSeries, ScanningStrategyImpl,
};
pub use types::{ScanMode, ScanProgress, ScanResult, ScanStatus, ScanningConfig};
