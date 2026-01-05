use anyhow::{Context, Result};
use chrono::Utc;
use sea_orm::{prelude::Decimal, ActiveModelTrait, DatabaseConnection, Set};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};
use uuid::Uuid;
use walkdir::WalkDir;

use crate::db::entities::{books, series};
use crate::db::repositories::{BookRepository, LibraryRepository, SeriesRepository};
use crate::scanner::{analyze_file, detect_format};
use crate::parsers::FileFormat;

use super::types::{ScanMode, ScanProgress, ScanResult, ScanStatus};

const HASH_READ_SIZE: usize = 1024 * 1024; // 1MB for partial hash
const SUPPORTED_EXTENSIONS: &[&str] = &["cbz", "cbr", "epub", "pdf"];

/// Main library scanner that orchestrates the scanning process
pub async fn scan_library(
    db: &DatabaseConnection,
    library_id: Uuid,
    mode: ScanMode,
    progress_tx: Option<mpsc::Sender<ScanProgress>>,
) -> Result<ScanResult> {
    info!("Starting {} scan for library {}", mode, library_id);

    // Load library from database
    let library = LibraryRepository::get_by_id(db, library_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Library not found: {}", library_id))?;

    // Verify library path exists
    if !Path::new(&library.path).exists() {
        return Err(anyhow::anyhow!(
            "Library path does not exist: {}",
            library.path
        ));
    }

    // Initialize progress tracking
    let mut progress = ScanProgress::new(library_id);
    progress.start();
    send_progress(&progress_tx, &progress).await;

    // Execute scan based on mode
    let result = match mode {
        ScanMode::Normal => scan_normal(db, &library, &mut progress, &progress_tx).await,
        ScanMode::Deep => scan_deep(db, &library, &mut progress, &progress_tx).await,
    };

    // Update progress based on result
    match &result {
        Ok(scan_result) => {
            if scan_result.has_errors() {
                progress.status = ScanStatus::Completed;
                for error in &scan_result.errors {
                    progress.add_error(error.clone());
                }
            } else {
                progress.complete();
            }
        }
        Err(e) => {
            progress.fail(e.to_string());
        }
    }

    send_progress(&progress_tx, &progress).await;

    // Update last_scanned_at timestamp
    if result.is_ok() {
        if let Err(e) = LibraryRepository::update_last_scanned(db, library_id).await {
            warn!("Failed to update last_scanned_at: {}", e);
        }
    }

    info!(
        "Scan completed for library {} - processed {} files, created {} series, {} books",
        library_id,
        result.as_ref().map(|r| r.files_processed).unwrap_or(0),
        result.as_ref().map(|r| r.series_created).unwrap_or(0),
        result.as_ref().map(|r| r.books_created).unwrap_or(0),
    );

    result
}

/// Normal scan - only process new/changed files
async fn scan_normal(
    db: &DatabaseConnection,
    library: &crate::db::entities::libraries::Model,
    progress: &mut ScanProgress,
    progress_tx: &Option<mpsc::Sender<ScanProgress>>,
) -> Result<ScanResult> {
    let mut result = ScanResult::new();

    // Load existing books from database
    let existing_books = load_existing_books(db, library.id).await?;
    debug!("Found {} existing books in database", existing_books.len());

    // Discover all files in library
    let discovered_files = discover_files(&library.path)?;
    progress.files_total = discovered_files.len();
    send_progress(progress_tx, progress).await;

    info!(
        "Discovered {} files in library {}",
        discovered_files.len(),
        library.name
    );

    // Organize files by series (folder structure)
    let series_map = organize_by_series(&discovered_files, &library.path);

    // Process each series
    for (series_name, file_paths) in series_map {
        match process_series(
            db,
            library,
            &series_name,
            &file_paths,
            &existing_books,
            ScanMode::Normal,
            progress,
            progress_tx,
            &mut result,
        )
        .await
        {
            Ok(_) => {}
            Err(e) => {
                let error_msg = format!("Error processing series '{}': {}", series_name, e);
                error!("{}", error_msg);
                result.errors.push(error_msg);
            }
        }
    }

    // TODO: Detect deleted files (in DB but not on filesystem)
    // For now, we don't delete them, just leave them in the database

    Ok(result)
}

/// Deep scan - re-process all files
async fn scan_deep(
    db: &DatabaseConnection,
    library: &crate::db::entities::libraries::Model,
    progress: &mut ScanProgress,
    progress_tx: &Option<mpsc::Sender<ScanProgress>>,
) -> Result<ScanResult> {
    let mut result = ScanResult::new();

    // Discover all files in library
    let discovered_files = discover_files(&library.path)?;
    progress.files_total = discovered_files.len();
    send_progress(progress_tx, progress).await;

    info!(
        "Discovered {} files for deep scan in library {}",
        discovered_files.len(),
        library.name
    );

    // Organize files by series (folder structure)
    let series_map = organize_by_series(&discovered_files, &library.path);

    // For deep scan, we don't use existing books cache (process everything)
    let existing_books = HashMap::new();

    // Process each series
    for (series_name, file_paths) in series_map {
        match process_series(
            db,
            library,
            &series_name,
            &file_paths,
            &existing_books,
            ScanMode::Deep, // Always deep mode
            progress,
            progress_tx,
            &mut result,
        )
        .await
        {
            Ok(_) => {}
            Err(e) => {
                let error_msg = format!("Error processing series '{}': {}", series_name, e);
                error!("{}", error_msg);
                result.errors.push(error_msg);
            }
        }
    }

    Ok(result)
}

/// Process a single series with its files
async fn process_series(
    db: &DatabaseConnection,
    library: &crate::db::entities::libraries::Model,
    series_name: &str,
    file_paths: &[PathBuf],
    existing_books: &HashMap<String, (String, books::Model)>,
    mode: ScanMode,
    progress: &mut ScanProgress,
    progress_tx: &Option<mpsc::Sender<ScanProgress>>,
    result: &mut ScanResult,
) -> Result<()> {
    // Find or create series
    let series_model = find_or_create_series(db, library.id, series_name).await?;

    let is_new_series = existing_books
        .values()
        .all(|(_, book)| book.series_id != series_model.id);

    if is_new_series {
        result.series_created += 1;
        progress.increment_series();
    }

    // Process each file in the series
    for file_path in file_paths {
        match process_file(
            db,
            &series_model,
            file_path,
            existing_books,
            mode,
            result,
        )
        .await
        {
            Ok(_) => {
                result.files_processed += 1;
                progress.files_processed += 1;

                // Send progress update every 10 files
                if progress.files_processed % 10 == 0 {
                    send_progress(progress_tx, progress).await;
                }
            }
            Err(e) => {
                let error_msg = format!("Error processing file '{}': {}", file_path.display(), e);
                error!("{}", error_msg);
                result.errors.push(error_msg);
            }
        }
    }

    Ok(())
}

/// Process a single file
async fn process_file(
    db: &DatabaseConnection,
    series_model: &series::Model,
    file_path: &Path,
    existing_books: &HashMap<String, (String, books::Model)>,
    mode: ScanMode,
    result: &mut ScanResult,
) -> Result<()> {
    let path_str = file_path.to_string_lossy().to_string();

    // Check if we should skip this file (normal mode only)
    if mode == ScanMode::Normal {
        if let Some((existing_hash, _)) = existing_books.get(&path_str) {
            // Calculate current hash
            let current_hash = calculate_file_hash(file_path)?;

            if &current_hash == existing_hash {
                debug!("Skipping unchanged file: {}", path_str);
                return Ok(());
            }
        }
    }

    // Analyze the file
    let metadata = analyze_file(file_path)
        .with_context(|| format!("Failed to analyze file: {}", path_str))?;

    // Check if book already exists by path
    let existing_book = BookRepository::get_by_path(db, &path_str).await?;

    let now = Utc::now();

    if let Some(mut existing) = existing_book {
        // Update existing book
        existing.title = metadata.comic_info.as_ref().and_then(|ci| ci.title.clone());
        existing.number = metadata.comic_info.as_ref().and_then(|ci| {
            ci.number.as_ref().and_then(|n| n.parse::<f64>().ok()).map(|v| Decimal::from_f64_retain(v).unwrap_or_default())
        });
        existing.file_size = metadata.file_size as i64;
        existing.file_hash = metadata.file_hash.clone();
        existing.format = format!("{:?}", metadata.format).to_lowercase();
        existing.page_count = metadata.page_count as i32;
        existing.modified_at = metadata.modified_at;
        existing.updated_at = now;

        BookRepository::update(db, &existing).await?;
        result.books_updated += 1;

        debug!("Updated book: {}", path_str);
    } else {
        // Create new book
        let book_model = books::Model {
            id: Uuid::new_v4(),
            series_id: series_model.id,
            title: metadata.comic_info.as_ref().and_then(|ci| ci.title.clone()),
            number: metadata.comic_info.as_ref().and_then(|ci| {
                ci.number.as_ref().and_then(|n| n.parse::<f64>().ok()).map(|v| Decimal::from_f64_retain(v).unwrap_or_default())
            }),
            file_path: path_str.clone(),
            file_name: file_path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string(),
            file_size: metadata.file_size as i64,
            file_hash: metadata.file_hash.clone(),
            format: format!("{:?}", metadata.format).to_lowercase(),
            page_count: metadata.page_count as i32,
            modified_at: metadata.modified_at,
            created_at: now,
            updated_at: now,
        };

        BookRepository::create(db, &book_model).await?;
        result.books_created += 1;

        debug!("Created book: {}", path_str);
    }

    Ok(())
}

/// Find or create a series by name
async fn find_or_create_series(
    db: &DatabaseConnection,
    library_id: Uuid,
    series_name: &str,
) -> Result<series::Model> {
    // Try to find existing series by normalized name
    let series_list = SeriesRepository::list_by_library(db, library_id).await?;

    let normalized_search = series_name.to_lowercase();
    if let Some(existing) = series_list.iter().find(|s| {
        s.normalized_name.to_lowercase() == normalized_search
    }) {
        return Ok(existing.clone());
    }

    // Create new series
    SeriesRepository::create(db, library_id, series_name).await
}

/// Load existing books from database into a map
async fn load_existing_books(
    db: &DatabaseConnection,
    library_id: Uuid,
) -> Result<HashMap<String, (String, books::Model)>> {
    let series_list = SeriesRepository::list_by_library(db, library_id).await?;
    let mut books_map = HashMap::new();

    for series in series_list {
        let books = BookRepository::list_by_series(db, series.id).await?;
        for book in books {
            books_map.insert(
                book.file_path.clone(),
                (book.file_hash.clone(), book),
            );
        }
    }

    Ok(books_map)
}

/// Discover all supported files in library path
fn discover_files(library_path: &str) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();

    for entry in WalkDir::new(library_path)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();

        // Skip if not a file
        if !path.is_file() {
            continue;
        }

        // Check extension
        if let Some(ext) = path.extension() {
            let ext_str = ext.to_string_lossy().to_lowercase();
            if SUPPORTED_EXTENSIONS.contains(&ext_str.as_str()) {
                files.push(path.to_path_buf());
            }
        }
    }

    Ok(files)
}

/// Organize files by series based on folder structure
/// Strategy: Direct child folders of library = series
fn organize_by_series(files: &[PathBuf], library_path: &str) -> HashMap<String, Vec<PathBuf>> {
    let mut series_map: HashMap<String, Vec<PathBuf>> = HashMap::new();
    let library_path = Path::new(library_path);

    for file_path in files {
        // Get the series name from folder structure
        let series_name = extract_series_name(file_path, library_path);

        series_map
            .entry(series_name)
            .or_insert_with(Vec::new)
            .push(file_path.clone());
    }

    series_map
}

/// Extract series name from file path based on library root
/// Direct child folder of library = series name
fn extract_series_name(file_path: &Path, library_path: &Path) -> String {
    // Get relative path from library root
    let relative = file_path
        .strip_prefix(library_path)
        .unwrap_or(file_path);

    // Get first component (direct child folder)
    let components: Vec<_> = relative.components().collect();

    if components.len() > 1 {
        // Use first folder as series name
        components[0]
            .as_os_str()
            .to_string_lossy()
            .to_string()
    } else {
        // File is directly in library root
        "Unsorted".to_string()
    }
}

/// Calculate SHA-256 hash of file (partial - first 1MB for performance)
fn calculate_file_hash(path: &Path) -> Result<String> {
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; HASH_READ_SIZE];

    let bytes_read = file.read(&mut buffer)?;
    hasher.update(&buffer[..bytes_read]);

    Ok(format!("{:x}", hasher.finalize()))
}

/// Send progress update through channel
async fn send_progress(
    progress_tx: &Option<mpsc::Sender<ScanProgress>>,
    progress: &ScanProgress,
) {
    if let Some(tx) = progress_tx {
        if let Err(e) = tx.send(progress.clone()).await {
            warn!("Failed to send progress update: {}", e);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_extract_series_name() {
        let library = Path::new("/library");

        // File in series folder
        let path = PathBuf::from("/library/Batman/issue1.cbz");
        assert_eq!(extract_series_name(&path, library), "Batman");

        // File in nested folder
        let path = PathBuf::from("/library/Batman/Year One/issue1.cbz");
        assert_eq!(extract_series_name(&path, library), "Batman");

        // File directly in library
        let path = PathBuf::from("/library/standalone.cbz");
        assert_eq!(extract_series_name(&path, library), "Unsorted");
    }

    #[test]
    fn test_organize_by_series() {
        let files = vec![
            PathBuf::from("/library/Batman/issue1.cbz"),
            PathBuf::from("/library/Batman/issue2.cbz"),
            PathBuf::from("/library/Superman/issue1.cbz"),
            PathBuf::from("/library/standalone.cbz"),
        ];

        let series_map = organize_by_series(&files, "/library");

        assert_eq!(series_map.len(), 3);
        assert_eq!(series_map.get("Batman").unwrap().len(), 2);
        assert_eq!(series_map.get("Superman").unwrap().len(), 1);
        assert_eq!(series_map.get("Unsorted").unwrap().len(), 1);
    }
}
