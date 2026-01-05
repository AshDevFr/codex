use crate::parsers::BookMetadata;
use crate::scanner::{analyze_file, detect_format};
use std::path::PathBuf;
use tabled::{Table, Tabled};

/// Display row for table output
#[derive(Tabled)]
struct BookRow {
    #[tabled(rename = "Format")]
    format: String,
    #[tabled(rename = "Title")]
    title: String,
    #[tabled(rename = "Series")]
    series: String,
    #[tabled(rename = "#")]
    number: String,
    #[tabled(rename = "Year")]
    year: String,
    #[tabled(rename = "Publisher")]
    publisher: String,
    #[tabled(rename = "Writer")]
    writer: String,
    #[tabled(rename = "Pages")]
    pages: usize,
    #[tabled(rename = "Size")]
    size: String,
    #[tabled(rename = "ISBN")]
    isbn: String,
    #[tabled(rename = "Path")]
    path: String,
}

impl From<&BookMetadata> for BookRow {
    fn from(metadata: &BookMetadata) -> Self {
        let title = metadata
            .comic_info
            .as_ref()
            .and_then(|ci| ci.title.clone())
            .unwrap_or_else(|| {
                // Extract filename without extension as fallback
                std::path::Path::new(&metadata.file_path)
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("Unknown")
                    .to_string()
            });

        let series = metadata
            .comic_info
            .as_ref()
            .and_then(|ci| ci.series.clone())
            .unwrap_or_else(|| "-".to_string());

        let number = metadata
            .comic_info
            .as_ref()
            .and_then(|ci| ci.number.clone())
            .unwrap_or_else(|| "-".to_string());

        let year = metadata
            .comic_info
            .as_ref()
            .and_then(|ci| ci.year.map(|y| y.to_string()))
            .unwrap_or_else(|| "-".to_string());

        let publisher = metadata
            .comic_info
            .as_ref()
            .and_then(|ci| ci.publisher.clone())
            .unwrap_or_else(|| "-".to_string());

        let writer = metadata
            .comic_info
            .as_ref()
            .and_then(|ci| ci.writer.clone())
            .unwrap_or_else(|| "-".to_string());

        let isbn = if metadata.isbns.is_empty() {
            "-".to_string()
        } else {
            metadata.isbns[0].clone()
        };

        let size = format_bytes(metadata.file_size);

        BookRow {
            format: format!("{:?}", metadata.format),
            title,
            series,
            number,
            year,
            publisher,
            writer,
            pages: metadata.page_count,
            size,
            isbn,
            path: metadata.file_path.clone(),
        }
    }
}

/// Format bytes into human-readable size
fn format_bytes(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB"];
    let mut size = bytes as f64;
    let mut unit_idx = 0;

    while size >= 1024.0 && unit_idx < UNITS.len() - 1 {
        size /= 1024.0;
        unit_idx += 1;
    }

    if unit_idx == 0 {
        format!("{} {}", size as u64, UNITS[unit_idx])
    } else {
        format!("{:.2} {}", size, UNITS[unit_idx])
    }
}

/// Main scan command handler
pub fn scan_command(path: PathBuf, json: bool, pages: bool, verbose: bool) -> anyhow::Result<()> {
    if !path.exists() {
        anyhow::bail!("Path does not exist: {}", path.display());
    }

    let mut results = Vec::new();

    if path.is_file() {
        // Scan single file
        if verbose {
            eprintln!("Scanning file: {}", path.display());
        }
        match analyze_file(&path) {
            Ok(metadata) => results.push(metadata),
            Err(e) => {
                if verbose {
                    eprintln!("Error analyzing {}: {}", path.display(), e);
                }
            }
        }
    } else if path.is_dir() {
        // Scan directory
        if verbose {
            eprintln!("Scanning directory: {}", path.display());
        }
        scan_directory(&path, &mut results, verbose)?;
    }

    // Output results
    if json {
        // JSON output
        let json_output = if results.len() == 1 {
            serde_json::to_string_pretty(&results[0])?
        } else {
            serde_json::to_string_pretty(&results)?
        };
        println!("{}", json_output);
    } else {
        // Table output (default)
        if results.is_empty() {
            println!("No supported files found.");
        } else {
            let rows: Vec<BookRow> = results.iter().map(BookRow::from).collect();
            let table = Table::new(rows).to_string();
            println!("{}", table);

            // If pages flag is set, show detailed page info
            if pages {
                println!();
                for metadata in &results {
                    println!("📖 {} ({} pages)", metadata.file_path, metadata.page_count);
                    if metadata.pages.is_empty() {
                        println!("  No page information available");
                    } else {
                        for page in &metadata.pages {
                            println!(
                                "  Page {:>3}: {} ({:?}, {}x{}, {})",
                                page.page_number,
                                page.file_name,
                                page.format,
                                page.width,
                                page.height,
                                format_bytes(page.file_size)
                            );
                        }
                    }
                    println!();
                }
            }
        }
    }

    if verbose {
        eprintln!("\nScanned {} file(s)", results.len());
    }

    Ok(())
}

/// Recursively scan a directory
fn scan_directory(
    dir: &PathBuf,
    results: &mut Vec<BookMetadata>,
    verbose: bool,
) -> anyhow::Result<()> {
    use walkdir::WalkDir;

    for entry in WalkDir::new(dir)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();

        // Skip directories
        if !path.is_file() {
            continue;
        }

        // Check if it's a supported format
        if detect_format(path).is_none() {
            continue;
        }

        if verbose {
            eprintln!("Analyzing: {}", path.display());
        }

        match analyze_file(path) {
            Ok(metadata) => {
                results.push(metadata);
            }
            Err(e) => {
                if verbose {
                    eprintln!("Error analyzing {}: {}", path.display(), e);
                }
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_bytes_b() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(1023), "1023 B");
    }

    #[test]
    fn test_format_bytes_kb() {
        assert_eq!(format_bytes(1024), "1.00 KB");
        assert_eq!(format_bytes(1536), "1.50 KB");
        assert_eq!(format_bytes(10240), "10.00 KB");
    }

    #[test]
    fn test_format_bytes_mb() {
        assert_eq!(format_bytes(1048576), "1.00 MB");
        assert_eq!(format_bytes(5242880), "5.00 MB");
        assert_eq!(format_bytes(10485760), "10.00 MB");
    }

    #[test]
    fn test_format_bytes_gb() {
        assert_eq!(format_bytes(1073741824), "1.00 GB");
        assert_eq!(format_bytes(5368709120), "5.00 GB");
    }
}
