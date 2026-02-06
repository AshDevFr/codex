//! Calibre scanning strategy
//!
//! For Calibre library imports:
//! - Author folder → Book title folder (with optional ID suffix) → book files
//! - Example: /library/Brandon Sanderson/Mistborn (45)/Mistborn.epub

use anyhow::Result;
use regex::Regex;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tracing::debug;

use crate::models::{CalibreSeriesMode, CalibreStrategyConfig, SeriesStrategy};
use crate::parsers::opf;

use super::super::common::{DetectedBook, DetectedSeries, SeriesMetadata};
use super::ScanningStrategyImpl;

/// Calibre strategy implementation
///
/// Author/book folder structure with optional series grouping
pub struct CalibreStrategy {
    config: CalibreStrategyConfig,
    /// Pattern to strip Calibre ID suffix (e.g., " (123)")
    id_suffix_pattern: Regex,
}

impl CalibreStrategy {
    pub fn new(config: CalibreStrategyConfig) -> Self {
        Self {
            config,
            id_suffix_pattern: Regex::new(r"\s*\(\d+\)\s*$").unwrap(),
        }
    }

    /// Strip Calibre ID suffix from folder/book name
    fn strip_id_suffix(&self, name: &str) -> String {
        if self.config.strip_id_suffix {
            self.id_suffix_pattern.replace(name, "").to_string()
        } else {
            name.to_string()
        }
    }

    /// Extract author from folder structure (grandparent folder - parent of book folder)
    fn extract_author(&self, file_path: &Path, library_path: &Path) -> Option<String> {
        if !self.config.author_from_folder {
            return None;
        }

        // Author is the grandparent folder (parent of the book folder)
        // Structure: library/Author/Book Title/file.epub
        if let Some(parent) = file_path.parent()
            && let Some(grandparent) = parent.parent()
        {
            // Only if grandparent is not the library root
            if grandparent != library_path
                && let Some(author_name) = grandparent.file_name()
            {
                return Some(author_name.to_string_lossy().to_string());
            }
        }
        None
    }

    /// Extract book title from folder structure (immediate parent folder)
    fn extract_book_title(&self, file_path: &Path, _library_path: &Path) -> Option<String> {
        // Use immediate parent folder as book title
        if let Some(parent) = file_path.parent()
            && let Some(folder_name) = parent.file_name()
        {
            let name = folder_name.to_string_lossy().to_string();
            return Some(self.strip_id_suffix(&name));
        }
        None
    }

    /// Determine series name based on series_mode
    fn determine_series_name(
        &self,
        author: Option<&str>,
        book_title: Option<&str>,
        file_path: &Path,
    ) -> String {
        match self.config.series_mode {
            CalibreSeriesMode::Standalone => {
                // Each book is its own series
                book_title.unwrap_or("Unknown").to_string()
            }
            CalibreSeriesMode::ByAuthor => {
                // Group by author
                author.unwrap_or("Unknown Author").to_string()
            }
            CalibreSeriesMode::FromMetadata => {
                // Read calibre:series from metadata.opf in the book's parent directory
                if self.config.read_opf_metadata
                    && let Some(series_name) = self.read_series_from_opf(file_path)
                {
                    return series_name;
                }
                // Fall back to book title (standalone behavior) if OPF unavailable
                book_title.unwrap_or("Unknown").to_string()
            }
        }
    }

    /// Read the `calibre:series` field from a sidecar `metadata.opf` file
    /// in the same directory as the given book file.
    fn read_series_from_opf(&self, file_path: &Path) -> Option<String> {
        let parent = file_path.parent()?;
        let opf_path = parent.join("metadata.opf");

        match opf::parse_opf_file(&opf_path) {
            Ok(meta) => {
                if let Some(ref series) = meta.calibre_series {
                    debug!(
                        "FromMetadata: found calibre:series '{}' in {}",
                        series,
                        opf_path.display()
                    );
                }
                meta.calibre_series
            }
            Err(_) => {
                debug!(
                    "FromMetadata: no metadata.opf found at {}",
                    opf_path.display()
                );
                None
            }
        }
    }
}

impl ScanningStrategyImpl for CalibreStrategy {
    fn strategy_type(&self) -> SeriesStrategy {
        SeriesStrategy::Calibre
    }

    fn organize_files(
        &self,
        files: &[PathBuf],
        library_path: &Path,
    ) -> Result<HashMap<String, DetectedSeries>> {
        let mut series_map: HashMap<String, DetectedSeries> = HashMap::new();

        for file_path in files {
            // Skip metadata files
            if let Some(filename) = file_path.file_name() {
                let name = filename.to_string_lossy();
                if name == "metadata.opf" || name == "metadata.db" || name == "cover.jpg" {
                    continue;
                }
            }

            let author = self.extract_author(file_path, library_path);
            let book_title = self.extract_book_title(file_path, library_path);
            let series_name =
                self.determine_series_name(author.as_deref(), book_title.as_deref(), file_path);

            let series = series_map.entry(series_name.clone()).or_insert_with(|| {
                let mut s = DetectedSeries::new(&series_name);

                // Set author metadata
                if let Some(author_name) = &author {
                    s.metadata = SeriesMetadata {
                        author: Some(author_name.clone()),
                        ..Default::default()
                    };
                }

                s
            });

            // Set series path if not already set
            // For ByAuthor mode, use the author folder (grandparent)
            // For other modes, use the book folder (parent)
            if series.path.is_none() {
                let target_folder = match self.config.series_mode {
                    CalibreSeriesMode::ByAuthor => file_path.parent().and_then(|p| p.parent()),
                    _ => file_path.parent(),
                };

                if let Some(folder) = target_folder
                    && let Ok(rel_path) = folder.strip_prefix(library_path)
                {
                    series.path = Some(rel_path.to_string_lossy().to_string());
                }
            }

            series.add_book(DetectedBook::new(file_path.clone()));
        }

        Ok(series_map)
    }

    fn extract_series_name(&self, file_path: &Path, library_path: &Path) -> String {
        let author = self.extract_author(file_path, library_path);
        let book_title = self.extract_book_title(file_path, library_path);
        self.determine_series_name(author.as_deref(), book_title.as_deref(), file_path)
    }

    fn validate(&self, library_path: &Path) -> Result<()> {
        if !library_path.exists() {
            anyhow::bail!("Library path does not exist: {}", library_path.display());
        }
        if !library_path.is_dir() {
            anyhow::bail!(
                "Library path is not a directory: {}",
                library_path.display()
            );
        }

        // Warn if metadata.db doesn't exist (not a Calibre library)
        let metadata_db = library_path.join("metadata.db");
        if !metadata_db.exists() {
            tracing::warn!(
                "metadata.db not found in {}. This may not be a Calibre library.",
                library_path.display()
            );
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strategy() -> CalibreStrategy {
        CalibreStrategy::new(CalibreStrategyConfig::default())
    }

    fn strategy_with_mode(mode: CalibreSeriesMode) -> CalibreStrategy {
        CalibreStrategy::new(CalibreStrategyConfig {
            series_mode: mode,
            ..Default::default()
        })
    }

    #[test]
    fn test_strip_id_suffix() {
        let strategy = strategy();

        assert_eq!(strategy.strip_id_suffix("Mistborn (45)"), "Mistborn");
        assert_eq!(
            strategy.strip_id_suffix("The Well of Ascension (46)"),
            "The Well of Ascension"
        );
        assert_eq!(
            strategy.strip_id_suffix("Book Without ID"),
            "Book Without ID"
        );
    }

    #[test]
    fn test_extract_author() {
        let library = Path::new("/library");
        let strategy = strategy();

        let path = PathBuf::from("/library/Brandon Sanderson/Mistborn (45)/Mistborn.epub");
        assert_eq!(
            strategy.extract_author(&path, library),
            Some("Brandon Sanderson".to_string())
        );
    }

    #[test]
    fn test_extract_book_title() {
        let library = Path::new("/library");
        let strategy = strategy();

        let path = PathBuf::from("/library/Brandon Sanderson/Mistborn (45)/Mistborn.epub");
        assert_eq!(
            strategy.extract_book_title(&path, library),
            Some("Mistborn".to_string())
        );
    }

    #[test]
    fn test_standalone_mode() {
        let library = Path::new("/library");
        let strategy = strategy_with_mode(CalibreSeriesMode::Standalone);

        let files = vec![
            PathBuf::from("/library/Brandon Sanderson/Mistborn (45)/Mistborn.epub"),
            PathBuf::from(
                "/library/Brandon Sanderson/The Well of Ascension (46)/The Well of Ascension.epub",
            ),
        ];

        let result = strategy.organize_files(&files, library).unwrap();

        // Each book is its own series
        assert_eq!(result.len(), 2);
        assert!(result.contains_key("Mistborn"));
        assert!(result.contains_key("The Well of Ascension"));

        // Series path should be the book folder (parent)
        assert_eq!(
            result["Mistborn"].path,
            Some("Brandon Sanderson/Mistborn (45)".to_string())
        );
    }

    #[test]
    fn test_by_author_mode() {
        let library = Path::new("/library");
        let strategy = strategy_with_mode(CalibreSeriesMode::ByAuthor);

        let files = vec![
            PathBuf::from("/library/Brandon Sanderson/Mistborn (45)/Mistborn.epub"),
            PathBuf::from(
                "/library/Brandon Sanderson/The Well of Ascension (46)/The Well of Ascension.epub",
            ),
            PathBuf::from(
                "/library/George R. R. Martin/A Game of Thrones (1)/A Game of Thrones.epub",
            ),
        ];

        let result = strategy.organize_files(&files, library).unwrap();

        // Books grouped by author
        assert_eq!(result.len(), 2);
        assert!(result.contains_key("Brandon Sanderson"));
        assert!(result.contains_key("George R. R. Martin"));

        assert_eq!(result["Brandon Sanderson"].books.len(), 2);
        assert_eq!(result["George R. R. Martin"].books.len(), 1);

        // Series path should be the author folder (grandparent), not the book folder
        assert_eq!(
            result["Brandon Sanderson"].path,
            Some("Brandon Sanderson".to_string())
        );
        assert_eq!(
            result["George R. R. Martin"].path,
            Some("George R. R. Martin".to_string())
        );
    }

    #[test]
    fn test_author_metadata() {
        let library = Path::new("/library");
        let strategy = strategy();

        let files = vec![PathBuf::from(
            "/library/Brandon Sanderson/Mistborn (45)/Mistborn.epub",
        )];

        let result = strategy.organize_files(&files, library).unwrap();

        assert_eq!(
            result["Mistborn"].metadata.author,
            Some("Brandon Sanderson".to_string())
        );
    }

    #[test]
    fn test_skip_metadata_files() {
        let library = Path::new("/library");
        let strategy = strategy();

        let files = vec![
            PathBuf::from("/library/Author/Book (1)/Book.epub"),
            PathBuf::from("/library/Author/Book (1)/metadata.opf"),
            PathBuf::from("/library/Author/Book (1)/cover.jpg"),
        ];

        let result = strategy.organize_files(&files, library).unwrap();

        // Only the epub should be included
        assert_eq!(result["Book"].books.len(), 1);
    }

    #[test]
    fn test_no_strip_id_suffix() {
        let strategy = CalibreStrategy::new(CalibreStrategyConfig {
            strip_id_suffix: false,
            ..Default::default()
        });

        assert_eq!(strategy.strip_id_suffix("Mistborn (45)"), "Mistborn (45)");
    }

    #[test]
    fn test_strategy_type() {
        assert_eq!(strategy().strategy_type(), SeriesStrategy::Calibre);
    }

    #[test]
    fn test_from_metadata_mode_with_opf() {
        // Create a temp dir simulating Calibre structure with metadata.opf
        let temp_dir = tempfile::tempdir().unwrap();
        let library_path = temp_dir.path();

        // Create: library/Author/Book (1)/Book.epub + metadata.opf
        let book_dir = library_path.join("Brandon Sanderson").join("Mistborn (45)");
        std::fs::create_dir_all(&book_dir).unwrap();

        let book_file = book_dir.join("Mistborn.epub");
        std::fs::write(&book_file, "fake epub content").unwrap();

        let opf_content = r#"<?xml version="1.0" encoding="UTF-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="2.0">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:title>Mistborn: The Final Empire</dc:title>
    <dc:creator>Brandon Sanderson</dc:creator>
    <meta name="calibre:series" content="Mistborn Era 1"/>
    <meta name="calibre:series_index" content="1.0"/>
  </metadata>
</package>"#;
        std::fs::write(book_dir.join("metadata.opf"), opf_content).unwrap();

        // Second book in same series
        let book_dir2 = library_path
            .join("Brandon Sanderson")
            .join("The Well of Ascension (46)");
        std::fs::create_dir_all(&book_dir2).unwrap();

        let book_file2 = book_dir2.join("The Well of Ascension.epub");
        std::fs::write(&book_file2, "fake epub content").unwrap();

        let opf_content2 = r#"<?xml version="1.0" encoding="UTF-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="2.0">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:title>The Well of Ascension</dc:title>
    <dc:creator>Brandon Sanderson</dc:creator>
    <meta name="calibre:series" content="Mistborn Era 1"/>
    <meta name="calibre:series_index" content="2.0"/>
  </metadata>
</package>"#;
        std::fs::write(book_dir2.join("metadata.opf"), opf_content2).unwrap();

        let strategy = strategy_with_mode(CalibreSeriesMode::FromMetadata);
        let files = vec![book_file, book_file2];
        let result = strategy.organize_files(&files, library_path).unwrap();

        // Both books should be grouped under "Mistborn Era 1" from OPF
        assert_eq!(result.len(), 1);
        assert!(result.contains_key("Mistborn Era 1"));
        assert_eq!(result["Mistborn Era 1"].books.len(), 2);
    }

    #[test]
    fn test_from_metadata_mode_fallback_no_opf() {
        // When no metadata.opf exists, fall back to book title (standalone behavior)
        let strategy = strategy_with_mode(CalibreSeriesMode::FromMetadata);

        // Use non-existent paths — no metadata.opf will be found
        let file_path = Path::new("/nonexistent/Author/Book Title (1)/Book.epub");
        let series_name =
            strategy.determine_series_name(Some("Author"), Some("Book Title"), file_path);

        assert_eq!(series_name, "Book Title");
    }

    #[test]
    fn test_from_metadata_mode_no_series_in_opf() {
        // OPF exists but has no calibre:series field — fall back to book title
        let temp_dir = tempfile::tempdir().unwrap();
        let book_dir = temp_dir.path().join("Author").join("Book (1)");
        std::fs::create_dir_all(&book_dir).unwrap();

        let book_file = book_dir.join("Book.epub");
        std::fs::write(&book_file, "fake epub content").unwrap();

        let opf_content = r#"<?xml version="1.0" encoding="UTF-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="2.0">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:title>A Standalone Book</dc:title>
    <dc:creator>Some Author</dc:creator>
  </metadata>
</package>"#;
        std::fs::write(book_dir.join("metadata.opf"), opf_content).unwrap();

        let strategy = strategy_with_mode(CalibreSeriesMode::FromMetadata);
        let series_name = strategy.determine_series_name(Some("Author"), Some("Book"), &book_file);

        // No calibre:series in OPF, falls back to book title
        assert_eq!(series_name, "Book");
    }

    #[test]
    fn test_from_metadata_mode_read_opf_disabled() {
        // When read_opf_metadata is false, FromMetadata falls back to standalone
        let temp_dir = tempfile::tempdir().unwrap();
        let book_dir = temp_dir.path().join("Author").join("Book (1)");
        std::fs::create_dir_all(&book_dir).unwrap();

        let book_file = book_dir.join("Book.epub");
        std::fs::write(&book_file, "fake epub content").unwrap();

        let opf_content = r#"<?xml version="1.0" encoding="UTF-8"?>
<package><metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <meta name="calibre:series" content="Should Not Be Used"/>
</metadata></package>"#;
        std::fs::write(book_dir.join("metadata.opf"), opf_content).unwrap();

        let strategy = CalibreStrategy::new(CalibreStrategyConfig {
            series_mode: CalibreSeriesMode::FromMetadata,
            read_opf_metadata: false,
            ..Default::default()
        });

        let series_name = strategy.determine_series_name(Some("Author"), Some("Book"), &book_file);

        // read_opf_metadata is false, so it should NOT read the OPF
        assert_eq!(series_name, "Book");
    }
}
