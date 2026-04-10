//! Series export file writers (JSON and CSV)
//!
//! Writes `SeriesExportRow`s to disk in either JSON array or CSV format.
//! Both writers stream row-by-row to avoid buffering all data in memory.
//! Files are written atomically via `ExportStorage::write_atomic`.

use anyhow::{Context, Result};
use std::io::Write;
use std::path::PathBuf;

use super::series_export_collector::{ExportField, SeriesExportRow};

/// Write rows as a JSON array to a file.
///
/// Produces a well-formed JSON array: `[\n  {row1},\n  {row2},\n  ...\n]`.
/// Each row is serialized independently to avoid buffering the full array.
///
/// Returns `(row_count, file_size_bytes)`.
pub async fn write_json(path: PathBuf, rows: Vec<SeriesExportRow>) -> Result<(usize, u64)> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn sample_row(id: &str, name: &str, lib_id: &str) -> SeriesExportRow {
        SeriesExportRow {
            series_id: id.to_string(),
            series_name: name.to_string(),
            library_id: lib_id.to_string(),
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
            actual_book_count: Some(15),
            unread_book_count: Some(5),
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

        let (count, size) = write_json(path.clone(), vec![]).await.unwrap();

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
        assert_eq!(parsed[2].library_id, "lib-2");
    }

    // =========================================================================
    // CSV tests
    // =========================================================================

    #[tokio::test]
    async fn test_write_csv_empty() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("empty.csv");

        let fields = vec![
            ExportField::SeriesId,
            ExportField::SeriesName,
            ExportField::LibraryId,
        ];

        let (count, size) = write_csv(path.clone(), fields, vec![]).await.unwrap();

        assert_eq!(count, 0);
        assert!(size > 0);

        let content = std::fs::read_to_string(&path).unwrap();
        // Should have header only
        let lines: Vec<&str> = content.trim().lines().collect();
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0], "series_id,series_name,library_id");
    }

    #[tokio::test]
    async fn test_write_csv_rows() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("export.csv");
        let rows = sample_rows();

        let fields = vec![
            ExportField::SeriesId,
            ExportField::SeriesName,
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
                "series_id",
                "series_name",
                "library_id",
                "title",
                "genres",
                "year",
                "user_rating"
            ]
        );

        let records: Vec<csv::StringRecord> = rdr.records().map(|r| r.unwrap()).collect();
        assert_eq!(records.len(), 3);
        assert_eq!(&records[0][0], "id-1");
        assert_eq!(&records[0][1], "Series A");
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
            ExportField::SeriesId,
            ExportField::SeriesName,
            ExportField::Summary,
        ];

        write_csv(path.clone(), fields, vec![row]).await.unwrap();

        // Parse back - csv crate should handle quoting
        let content = std::fs::read_to_string(&path).unwrap();
        let mut rdr = csv::Reader::from_reader(content.as_bytes());
        let records: Vec<csv::StringRecord> = rdr.records().map(|r| r.unwrap()).collect();

        assert_eq!(records.len(), 1);
        assert_eq!(&records[0][1], "Series, \"with\" commas");
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
}
