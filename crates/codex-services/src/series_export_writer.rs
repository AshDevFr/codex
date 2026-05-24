//! Export file writers (JSON, CSV, Markdown)
//!
//! Writes series and/or book export rows to disk. Supports three formats:
//! JSON (full fidelity), CSV (flat), and Markdown (LLM-friendly).
//! All writers stream row-by-row to avoid buffering all data in memory.
//! Files are written atomically via `ExportStorage::write_atomic`.

use anyhow::{Context, Result};
use std::io::Write;
use std::path::PathBuf;

use super::book_export_collector::{BookExportField, BookExportRow};
use super::series_export_collector::{ExportField, SeriesExportRow};

/// Write rows as a JSON array to a file.
///
/// Produces a well-formed JSON array: `[\n  {row1},\n  {row2},\n  ...\n]`.
/// Each row is serialized independently to avoid buffering the full array.
///
/// Returns `(row_count, file_size_bytes)`.
pub async fn write_json<T: serde::Serialize + Send + 'static>(
    path: PathBuf,
    rows: Vec<T>,
) -> Result<(usize, u64)> {
    let row_count = rows.len();

    // Perform blocking I/O on a dedicated thread
    let file_size = tokio::task::spawn_blocking(move || -> Result<u64> {
        let file = std::fs::File::create(&path)
            .with_context(|| format!("Failed to create {}", path.display()))?;
        let mut writer = std::io::BufWriter::new(file);

        writer.write_all(b"[")?;

        for (i, row) in rows.iter().enumerate() {
            if i > 0 {
                writer.write_all(b",")?;
            }
            writer.write_all(b"\n  ")?;
            serde_json::to_writer(&mut writer, row).context("Failed to serialize row to JSON")?;
        }

        if row_count > 0 {
            writer.write_all(b"\n")?;
        }
        writer.write_all(b"]\n")?;
        writer.flush()?;

        // Get file size after flush
        let metadata = std::fs::metadata(&path)
            .with_context(|| format!("Failed to stat {}", path.display()))?;
        Ok(metadata.len())
    })
    .await
    .context("JSON writer task panicked")??;

    Ok((row_count, file_size))
}

/// Write rows as CSV to a file.
///
/// The header row uses the `ExportField::as_str()` keys for each selected field.
/// Values are extracted via `SeriesExportRow::get_field_value()`.
/// Multi-value fields are pre-joined with `;` by the collector; the csv crate
/// handles quoting when cell values contain commas or quotes.
///
/// Returns `(row_count, file_size_bytes)`.
pub async fn write_csv(
    path: PathBuf,
    fields: Vec<ExportField>,
    rows: Vec<SeriesExportRow>,
) -> Result<(usize, u64)> {
    let row_count = rows.len();

    let file_size = tokio::task::spawn_blocking(move || -> Result<u64> {
        let file = std::fs::File::create(&path)
            .with_context(|| format!("Failed to create {}", path.display()))?;
        let mut wtr = csv::Writer::from_writer(std::io::BufWriter::new(file));

        // Write header
        let headers: Vec<&str> = fields.iter().map(|f| f.as_str()).collect();
        wtr.write_record(&headers)
            .context("Failed to write CSV header")?;

        // Write data rows
        for row in &rows {
            let values: Vec<String> = fields.iter().map(|f| row.get_field_value(f)).collect();
            wtr.write_record(&values)
                .context("Failed to write CSV row")?;
        }

        wtr.flush()?;

        let metadata = std::fs::metadata(&path)
            .with_context(|| format!("Failed to stat {}", path.display()))?;
        Ok(metadata.len())
    })
    .await
    .context("CSV writer task panicked")??;

    Ok((row_count, file_size))
}

/// Write rows as Markdown with heading + bullet structure.
///
/// Each row gets an H2 heading (using `SeriesName` as the heading text),
/// followed by bullet points for each selected field (skipping empty values
/// and the name field itself). Designed for LLM consumption and readability.
///
/// Returns `(row_count, file_size_bytes)`.
pub async fn write_markdown(
    path: PathBuf,
    fields: Vec<ExportField>,
    rows: Vec<SeriesExportRow>,
) -> Result<(usize, u64)> {
    let row_count = rows.len();

    let file_size = tokio::task::spawn_blocking(move || -> Result<u64> {
        let file = std::fs::File::create(&path)
            .with_context(|| format!("Failed to create {}", path.display()))?;
        let mut writer = std::io::BufWriter::new(file);

        for (i, row) in rows.iter().enumerate() {
            if i > 0 {
                writer.write_all(b"\n")?;
            }

            // H2 heading with the series/book name
            write!(writer, "## {}\n\n", row.series_name)?;

            // Bullet points for each selected field (skip the name anchor)
            for field in &fields {
                if *field == ExportField::SeriesName {
                    continue;
                }
                let value = row.get_field_value(field);
                if value.is_empty() {
                    continue;
                }
                let label = field.label();
                writeln!(writer, "- **{label}:** {value}")?;
            }
        }

        writer.flush()?;

        let metadata = std::fs::metadata(&path)
            .with_context(|| format!("Failed to stat {}", path.display()))?;
        Ok(metadata.len())
    })
    .await
    .context("Markdown writer task panicked")??;

    Ok((row_count, file_size))
}

// =============================================================================
// Unified export writer
// =============================================================================

/// Write an export file handling all combinations of format and export type.
///
/// For "both" mode (series + books), JSON produces `{"series":[...],"books":[...]}`,
/// and Markdown produces separate H1 sections.
#[allow(clippy::too_many_arguments)]
pub async fn write_export(
    path: PathBuf,
    format: &str,
    export_type: &str,
    series_fields: Vec<ExportField>,
    series_rows: Vec<SeriesExportRow>,
    book_fields: Vec<BookExportField>,
    book_rows: Vec<BookExportRow>,
) -> Result<(usize, u64)> {
    let total_rows = series_rows.len() + book_rows.len();

    match (format, export_type) {
        // Series only
        ("csv", "series") => write_csv(path, series_fields, series_rows).await,
        ("md", "series") => write_markdown(path, series_fields, series_rows).await,
        (_, "series") => write_json(path, series_rows).await,

        // Books only
        ("csv", "books") => write_book_csv(path, book_fields, book_rows).await,
        ("md", "books") => write_book_markdown(path, book_fields, book_rows).await,
        (_, "books") => write_json(path, book_rows).await,

        // Both (CSV not supported, handled at API validation)
        ("md", "both") => {
            write_combined_markdown(path, series_fields, series_rows, book_fields, book_rows).await
        }
        (_, "both") => write_combined_json(path, series_rows, book_rows).await,

        // Fallback: treat as series-only JSON
        _ => write_json(path, series_rows).await,
    }
    .map(|(_, file_size)| (total_rows, file_size))
}

/// Write book rows as CSV.
pub async fn write_book_csv(
    path: PathBuf,
    fields: Vec<BookExportField>,
    rows: Vec<BookExportRow>,
) -> Result<(usize, u64)> {
    let row_count = rows.len();

    let file_size = tokio::task::spawn_blocking(move || -> Result<u64> {
        let file = std::fs::File::create(&path)
            .with_context(|| format!("Failed to create {}", path.display()))?;
        let mut wtr = csv::Writer::from_writer(std::io::BufWriter::new(file));

        let headers: Vec<&str> = fields.iter().map(|f| f.as_str()).collect();
        wtr.write_record(&headers)
            .context("Failed to write CSV header")?;

        for row in &rows {
            let values: Vec<String> = fields.iter().map(|f| row.get_field_value(f)).collect();
            wtr.write_record(&values)
                .context("Failed to write CSV row")?;
        }

        wtr.flush()?;
        let metadata = std::fs::metadata(&path)
            .with_context(|| format!("Failed to stat {}", path.display()))?;
        Ok(metadata.len())
    })
    .await
    .context("CSV writer task panicked")??;

    Ok((row_count, file_size))
}

/// Write book rows as Markdown.
pub async fn write_book_markdown(
    path: PathBuf,
    fields: Vec<BookExportField>,
    rows: Vec<BookExportRow>,
) -> Result<(usize, u64)> {
    let row_count = rows.len();

    let file_size = tokio::task::spawn_blocking(move || -> Result<u64> {
        let file = std::fs::File::create(&path)
            .with_context(|| format!("Failed to create {}", path.display()))?;
        let mut writer = std::io::BufWriter::new(file);

        for (i, row) in rows.iter().enumerate() {
            if i > 0 {
                writer.write_all(b"\n")?;
            }
            write!(writer, "## {}\n\n", row.name())?;

            for field in &fields {
                if *field == BookExportField::BookName {
                    continue;
                }
                let value = row.get_field_value(field);
                if value.is_empty() {
                    continue;
                }
                let label = field.label();
                writeln!(writer, "- **{label}:** {value}")?;
            }
        }

        writer.flush()?;
        let metadata = std::fs::metadata(&path)
            .with_context(|| format!("Failed to stat {}", path.display()))?;
        Ok(metadata.len())
    })
    .await
    .context("Markdown writer task panicked")??;

    Ok((row_count, file_size))
}

/// Write combined JSON with series and books sections.
async fn write_combined_json(
    path: PathBuf,
    series_rows: Vec<SeriesExportRow>,
    book_rows: Vec<BookExportRow>,
) -> Result<(usize, u64)> {
    let total = series_rows.len() + book_rows.len();

    let file_size = tokio::task::spawn_blocking(move || -> Result<u64> {
        let file = std::fs::File::create(&path)
            .with_context(|| format!("Failed to create {}", path.display()))?;
        let mut writer = std::io::BufWriter::new(file);

        #[derive(serde::Serialize)]
        struct Combined {
            series: Vec<SeriesExportRow>,
            books: Vec<BookExportRow>,
        }

        let combined = Combined {
            series: series_rows,
            books: book_rows,
        };

        serde_json::to_writer_pretty(&mut writer, &combined)
            .context("Failed to serialize combined JSON")?;
        writer.write_all(b"\n")?;
        writer.flush()?;

        let metadata = std::fs::metadata(&path)
            .with_context(|| format!("Failed to stat {}", path.display()))?;
        Ok(metadata.len())
    })
    .await
    .context("JSON writer task panicked")??;

    Ok((total, file_size))
}

/// Write combined Markdown with H1 sections.
async fn write_combined_markdown(
    path: PathBuf,
    series_fields: Vec<ExportField>,
    series_rows: Vec<SeriesExportRow>,
    book_fields: Vec<BookExportField>,
    book_rows: Vec<BookExportRow>,
) -> Result<(usize, u64)> {
    let total = series_rows.len() + book_rows.len();

    let file_size = tokio::task::spawn_blocking(move || -> Result<u64> {
        let file = std::fs::File::create(&path)
            .with_context(|| format!("Failed to create {}", path.display()))?;
        let mut writer = std::io::BufWriter::new(file);

        // Series section
        if !series_rows.is_empty() {
            writeln!(writer, "# Series\n")?;
            for (i, row) in series_rows.iter().enumerate() {
                if i > 0 {
                    writer.write_all(b"\n")?;
                }
                write!(writer, "## {}\n\n", row.series_name)?;

                for field in &series_fields {
                    if *field == ExportField::SeriesName {
                        continue;
                    }
                    let value = row.get_field_value(field);
                    if value.is_empty() {
                        continue;
                    }
                    let label = field.label();
                    writeln!(writer, "- **{label}:** {value}")?;
                }
            }
        }

        // Books section
        if !book_rows.is_empty() {
            if !series_rows.is_empty() {
                writer.write_all(b"\n")?;
            }
            writeln!(writer, "# Books\n")?;
            for (i, row) in book_rows.iter().enumerate() {
                if i > 0 {
                    writer.write_all(b"\n")?;
                }
                write!(writer, "## {}\n\n", row.name())?;

                for field in &book_fields {
                    if *field == BookExportField::BookName {
                        continue;
                    }
                    let value = row.get_field_value(field);
                    if value.is_empty() {
                        continue;
                    }
                    let label = field.label();
                    writeln!(writer, "- **{label}:** {value}")?;
                }
            }
        }

        writer.flush()?;
        let metadata = std::fs::metadata(&path)
            .with_context(|| format!("Failed to stat {}", path.display()))?;
        Ok(metadata.len())
    })
    .await
    .context("Markdown writer task panicked")??;

    Ok((total, file_size))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn sample_row(id: &str, name: &str, lib_id: &str) -> SeriesExportRow {
        SeriesExportRow {
            series_name: name.to_string(),
            series_id: Some(id.to_string()),
            library_id: Some(lib_id.to_string()),
            library_name: Some("My Library".to_string()),
            path: Some("/comics/series1".to_string()),
            created_at: Some("2026-01-01T00:00:00+00:00".to_string()),
            updated_at: None,
            title: Some("Series Title".to_string()),
            summary: Some("A great series".to_string()),
            publisher: None,
            status: Some("ongoing".to_string()),
            year: Some(2025),
            language: Some("en".to_string()),
            authors: Some("John Doe (author); Jane Smith (editor)".to_string()),
            genres: Some("action; drama".to_string()),
            tags: Some("tag1; tag2".to_string()),
            alternate_titles: None,
            expected_book_count: Some(20),
            expected_chapter_count: None,
            actual_book_count: Some(15),
            unread_book_count: Some(5),
            progress: Some(66.7),
            user_rating: Some(85),
            user_notes: Some("Great read!".to_string()),
            community_avg_rating: Some(72.5),
            external_ratings: Some("myanimelist=85 (1200 votes); anilist=78".to_string()),
        }
    }

    fn sample_rows() -> Vec<SeriesExportRow> {
        vec![
            sample_row("id-1", "Series A", "lib-1"),
            sample_row("id-2", "Series B", "lib-1"),
            sample_row("id-3", "Series C", "lib-2"),
        ]
    }

    // =========================================================================
    // JSON tests
    // =========================================================================

    #[tokio::test]
    async fn test_write_json_empty() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("empty.json");

        let (count, size) = write_json::<SeriesExportRow>(path.clone(), vec![])
            .await
            .unwrap();

        assert_eq!(count, 0);
        assert!(size > 0);

        let content = std::fs::read_to_string(&path).unwrap();
        // Should be a valid empty JSON array
        let parsed: Vec<serde_json::Value> = serde_json::from_str(&content).unwrap();
        assert!(parsed.is_empty());
    }

    #[tokio::test]
    async fn test_write_json_rows() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("export.json");
        let rows = sample_rows();

        let (count, size) = write_json(path.clone(), rows).await.unwrap();

        assert_eq!(count, 3);
        assert!(size > 100);

        let content = std::fs::read_to_string(&path).unwrap();
        let parsed: Vec<serde_json::Value> = serde_json::from_str(&content).unwrap();
        assert_eq!(parsed.len(), 3);
        assert_eq!(parsed[0]["series_name"], "Series A");
        assert_eq!(parsed[1]["series_name"], "Series B");
        assert_eq!(parsed[2]["library_id"], "lib-2");
        assert_eq!(parsed[0]["progress"], 66.7);

        // Check that populated optional fields are present
        assert_eq!(parsed[0]["user_rating"], 85);
        assert_eq!(parsed[0]["genres"], "action; drama");

        // Check that None optional fields are absent (skip_serializing_if)
        assert!(parsed[0].get("updated_at").is_none());
        assert!(parsed[0].get("publisher").is_none());
    }

    #[tokio::test]
    async fn test_write_json_roundtrip() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("roundtrip.json");
        let rows = sample_rows();

        write_json(path.clone(), rows).await.unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        let parsed: Vec<SeriesExportRow> = serde_json::from_str(&content).unwrap();
        assert_eq!(parsed.len(), 3);
        assert_eq!(parsed[0].series_name, "Series A");
        assert_eq!(parsed[0].user_rating, Some(85));
        assert_eq!(parsed[2].library_id, Some("lib-2".to_string()));
    }

    // =========================================================================
    // CSV tests
    // =========================================================================

    #[tokio::test]
    async fn test_write_csv_empty() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("empty.csv");

        let fields = vec![
            ExportField::SeriesName,
            ExportField::SeriesId,
            ExportField::LibraryId,
        ];

        let (count, size) = write_csv(path.clone(), fields, vec![]).await.unwrap();

        assert_eq!(count, 0);
        assert!(size > 0);

        let content = std::fs::read_to_string(&path).unwrap();
        // Should have header only
        let lines: Vec<&str> = content.trim().lines().collect();
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0], "series_name,series_id,library_id");
    }

    #[tokio::test]
    async fn test_write_csv_rows() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("export.csv");
        let rows = sample_rows();

        let fields = vec![
            ExportField::SeriesName,
            ExportField::SeriesId,
            ExportField::LibraryId,
            ExportField::Title,
            ExportField::Genres,
            ExportField::Year,
            ExportField::UserRating,
        ];

        let (count, size) = write_csv(path.clone(), fields, rows).await.unwrap();

        assert_eq!(count, 3);
        assert!(size > 50);

        // Parse back with csv crate
        let content = std::fs::read_to_string(&path).unwrap();
        let mut rdr = csv::Reader::from_reader(content.as_bytes());

        let headers = rdr.headers().unwrap();
        assert_eq!(
            headers.iter().collect::<Vec<_>>(),
            vec![
                "series_name",
                "series_id",
                "library_id",
                "title",
                "genres",
                "year",
                "user_rating"
            ]
        );

        let records: Vec<csv::StringRecord> = rdr.records().map(|r| r.unwrap()).collect();
        assert_eq!(records.len(), 3);
        assert_eq!(&records[0][0], "Series A");
        assert_eq!(&records[0][1], "id-1");
        assert_eq!(&records[0][4], "action; drama");
        assert_eq!(&records[0][5], "2025");
        assert_eq!(&records[0][6], "85");
    }

    #[tokio::test]
    async fn test_write_csv_quoting_special_chars() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("quoted.csv");

        let mut row = sample_row("id-1", "Series, \"with\" commas", "lib-1");
        row.summary = Some("A story about\nnewlines".to_string());

        let fields = vec![
            ExportField::SeriesName,
            ExportField::SeriesId,
            ExportField::Summary,
        ];

        write_csv(path.clone(), fields, vec![row]).await.unwrap();

        // Parse back - csv crate should handle quoting
        let content = std::fs::read_to_string(&path).unwrap();
        let mut rdr = csv::Reader::from_reader(content.as_bytes());
        let records: Vec<csv::StringRecord> = rdr.records().map(|r| r.unwrap()).collect();

        assert_eq!(records.len(), 1);
        assert_eq!(&records[0][0], "Series, \"with\" commas");
        assert_eq!(&records[0][2], "A story about\nnewlines");
    }

    #[tokio::test]
    async fn test_write_csv_community_avg_precision() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("precision.csv");

        let mut row = sample_row("id-1", "S1", "lib-1");
        row.community_avg_rating = Some(72.33333);

        let fields = vec![ExportField::SeriesId, ExportField::CommunityAvgRating];

        write_csv(path.clone(), fields, vec![row]).await.unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        let mut rdr = csv::Reader::from_reader(content.as_bytes());
        let records: Vec<csv::StringRecord> = rdr.records().map(|r| r.unwrap()).collect();

        // Should be formatted to 2 decimal places
        assert_eq!(&records[0][1], "72.33");
    }

    // =========================================================================
    // Markdown tests
    // =========================================================================

    #[tokio::test]
    async fn test_write_markdown_empty() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("empty.md");

        let fields = vec![ExportField::SeriesName, ExportField::Title];
        let (count, size) = write_markdown(path.clone(), fields, vec![]).await.unwrap();

        assert_eq!(count, 0);
        assert_eq!(size, 0);
    }

    #[tokio::test]
    async fn test_write_markdown_rows() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("export.md");
        let rows = sample_rows();

        let fields = vec![
            ExportField::SeriesName,
            ExportField::Title,
            ExportField::Year,
            ExportField::Genres,
            ExportField::Progress,
        ];

        let (count, size) = write_markdown(path.clone(), fields, rows).await.unwrap();

        assert_eq!(count, 3);
        assert!(size > 50);

        let content = std::fs::read_to_string(&path).unwrap();

        // Check headings
        assert!(content.contains("## Series A"));
        assert!(content.contains("## Series B"));
        assert!(content.contains("## Series C"));

        // Check bullet points
        assert!(content.contains("- **Title:** Series Title"));
        assert!(content.contains("- **Year:** 2025"));
        assert!(content.contains("- **Genres:** action; drama"));
        assert!(content.contains("- **Progress:** 66.7"));

        // Series Name should NOT appear as a bullet (it's the heading)
        assert!(!content.contains("- **Series Name:**"));
    }

    #[tokio::test]
    async fn test_write_markdown_skips_empty_values() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("sparse.md");

        let mut row = sample_row("id-1", "My Series", "lib-1");
        row.publisher = None;
        row.language = None;

        let fields = vec![
            ExportField::SeriesName,
            ExportField::Title,
            ExportField::Publisher,
            ExportField::Language,
        ];

        write_markdown(path.clone(), fields, vec![row])
            .await
            .unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("- **Title:** Series Title"));
        assert!(!content.contains("Publisher"));
        assert!(!content.contains("Language"));
    }
}
