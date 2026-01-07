use anyhow::{Context, Result};
use chrono::Utc;
use futures::stream::{self, StreamExt};
use sea_orm::{prelude::Decimal, DatabaseConnection};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::db::entities::books;
use crate::db::repositories::BookRepository;
use crate::scanner::analyze_file;

use super::types::ScanProgress;

/// Configuration for the analysis queue
#[derive(Debug, Clone)]
pub struct AnalyzerConfig {
    /// Maximum number of files to analyze concurrently
    pub max_concurrent: usize,
}

impl Default for AnalyzerConfig {
    fn default() -> Self {
        Self { max_concurrent: 4 }
    }
}

/// Result of analyzing a batch of books
#[derive(Debug, Default)]
pub struct AnalysisResult {
    pub books_analyzed: usize,
    pub errors: Vec<String>,
}

/// Analyze unanalyzed books in a library with parallel processing
pub async fn analyze_library_books(
    db: &DatabaseConnection,
    library_id: Uuid,
    config: AnalyzerConfig,
    progress_tx: Option<mpsc::Sender<ScanProgress>>,
) -> Result<AnalysisResult> {
    let analysis_start = Instant::now();
    info!(
        "Starting parallel analysis for library {} with concurrency={}",
        library_id, config.max_concurrent
    );

    // Get all unanalyzed books for this library
    let unanalyzed_books = BookRepository::get_unanalyzed_in_library(db, library_id).await?;
    analyze_books(
        db,
        unanalyzed_books,
        config,
        progress_tx,
        analysis_start,
        format!("library {}", library_id),
    )
    .await
}

/// Analyze unanalyzed books in a series with parallel processing
pub async fn analyze_series_books(
    db: &DatabaseConnection,
    series_id: Uuid,
    config: AnalyzerConfig,
    progress_tx: Option<mpsc::Sender<ScanProgress>>,
) -> Result<AnalysisResult> {
    let analysis_start = Instant::now();
    info!(
        "Starting parallel analysis for series {} with concurrency={}",
        series_id, config.max_concurrent
    );

    // Get all unanalyzed books for this series
    let unanalyzed_books = BookRepository::list_by_series(db, series_id, false)
        .await?
        .into_iter()
        .filter(|book| !book.analyzed)
        .collect::<Vec<_>>();

    analyze_books(
        db,
        unanalyzed_books,
        config,
        progress_tx,
        analysis_start,
        format!("series {}", series_id),
    )
    .await
}

/// Analyze a single book (force reanalysis even if already analyzed)
pub async fn analyze_book(db: &DatabaseConnection, book_id: Uuid) -> Result<AnalysisResult> {
    let analysis_start = Instant::now();
    info!("Starting analysis for book {}", book_id);

    // Get the book
    let book = BookRepository::get_by_id(db, book_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Book not found"))?;

    let mut result = AnalysisResult::default();

    match analyze_single_book(db, book, None).await {
        Ok(_) => {
            result.books_analyzed = 1;
            info!(
                "Analysis completed for book {} in {:?}",
                book_id,
                analysis_start.elapsed()
            );
        }
        Err(e) => {
            let error_msg = format!("Failed to analyze book {}: {}", book_id, e);
            error!("{}", error_msg);
            result.errors.push(error_msg);
        }
    }

    Ok(result)
}

/// Common function to analyze a list of books with parallel processing
async fn analyze_books(
    db: &DatabaseConnection,
    books: Vec<books::Model>,
    config: AnalyzerConfig,
    progress_tx: Option<mpsc::Sender<ScanProgress>>,
    analysis_start: Instant,
    context: String,
) -> Result<AnalysisResult> {
    let total_books = books.len();

    if total_books == 0 {
        info!("No unanalyzed books found for {}", context);
        return Ok(AnalysisResult::default());
    }

    info!(
        "Found {} unanalyzed books to process for {}",
        total_books, context
    );

    let mut result = AnalysisResult::default();
    let db = Arc::new(db.clone());

    // Process books in parallel with bounded concurrency
    let results: Vec<Result<(), String>> = stream::iter(books)
        .map(|book| {
            let db = Arc::clone(&db);
            let progress_tx = progress_tx.clone();
            let book_path = book.file_path.clone();

            async move {
                match analyze_single_book(&db, book, progress_tx).await {
                    Ok(_) => Ok(()),
                    Err(e) => {
                        let error_msg = format!("Failed to analyze book {}: {}", book_path, e);
                        error!("{}", error_msg);
                        Err(error_msg)
                    }
                }
            }
        })
        .buffer_unordered(config.max_concurrent)
        .collect()
        .await;

    // Collect errors
    for res in results {
        match res {
            Ok(_) => result.books_analyzed += 1,
            Err(e) => result.errors.push(e),
        }
    }

    let duration = analysis_start.elapsed();
    let books_per_sec = if duration.as_secs_f64() > 0.0 {
        result.books_analyzed as f64 / duration.as_secs_f64()
    } else {
        0.0
    };

    info!(
        "Analysis completed for {} in {:?} - analyzed {} books ({:.2} books/sec), {} errors",
        context,
        duration,
        result.books_analyzed,
        books_per_sec,
        result.errors.len()
    );

    Ok(result)
}

/// Analyze a single book and update the database
async fn analyze_single_book(
    db: &DatabaseConnection,
    mut book: books::Model,
    progress_tx: Option<mpsc::Sender<ScanProgress>>,
) -> Result<()> {
    let analyze_start = Instant::now();
    let file_path = PathBuf::from(&book.file_path);

    debug!("Analyzing book: {}", book.file_path);

    // Analyze the file (blocking I/O operation)
    let file_path_clone = file_path.clone();
    let metadata = tokio::task::spawn_blocking(move || analyze_file(&file_path_clone))
        .await
        .map_err(|e| anyhow::anyhow!("Failed to spawn file analysis task: {}", e))?
        .with_context(|| format!("Failed to analyze file: {}", book.file_path))?;

    let analyze_duration = analyze_start.elapsed();

    if analyze_duration.as_millis() > 500 {
        debug!(
            "File analysis took {:?}: {}",
            analyze_duration,
            file_path.file_name().unwrap_or_default().to_string_lossy()
        );
    }

    // Update book with analyzed metadata
    let now = Utc::now();
    book.title = metadata.comic_info.as_ref().and_then(|ci| ci.title.clone());
    book.number = metadata.comic_info.as_ref().and_then(|ci| {
        ci.number
            .as_ref()
            .and_then(|n| n.parse::<f64>().ok())
            .map(|v| Decimal::from_f64_retain(v).unwrap_or_default())
    });
    book.file_size = metadata.file_size as i64;
    book.file_hash = metadata.file_hash.clone();
    book.format = format!("{:?}", metadata.format).to_lowercase();
    book.page_count = metadata.page_count as i32;
    book.modified_at = metadata.modified_at;
    book.analyzed = true; // Mark as analyzed
    book.updated_at = now;

    BookRepository::update(db, &book).await?;

    debug!(
        "Analyzed and updated book: {} (took {:?})",
        book.file_path, analyze_duration
    );

    // Send progress update if channel is provided
    if let Some(tx) = progress_tx {
        // Note: You may want to update ScanProgress to track analysis progress
        // For now, we just send an update to indicate activity
        let _ = tx.send(ScanProgress::new(Uuid::nil())).await;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_analyzer_config_default() {
        let config = AnalyzerConfig::default();
        assert_eq!(config.max_concurrent, 4);
    }
}
