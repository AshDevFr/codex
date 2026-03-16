/// Integration tests for book analysis with metadata extraction and storage
use anyhow::Result;
use chrono::Utc;
use codex::db::ScanningStrategy;
use codex::db::entities::{books, series};
use codex::db::repositories::{
    BookExternalLinkRepository, BookMetadataRepository, BookRepository, LibraryRepository,
    PageRepository, SeriesMetadataRepository, SeriesRepository, library::CreateLibraryParams,
};
use codex::models::BookStrategy;
use codex::scanner::analyze_book;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use tempfile::TempDir;
use uuid::Uuid;
use zip::ZipWriter;
use zip::write::SimpleFileOptions;

#[path = "../common/mod.rs"]
mod common;
use common::{files::create_test_cbz_with_metadata, files::create_test_png, setup_test_db_wrapper};

/// Helper to create a test book in database
async fn create_test_book(
    db: &codex::db::Database,
    file_path: &str,
) -> Result<(books::Model, series::Model)> {
    let library = LibraryRepository::create(
        db.sea_orm_connection(),
        "Test Library",
        "/test/path",
        ScanningStrategy::Default,
    )
    .await?;

    let series =
        SeriesRepository::create(db.sea_orm_connection(), library.id, "Test Series", None).await?;

    let book = books::Model {
        id: Uuid::new_v4(),
        series_id: series.id,
        library_id: library.id,
        file_path: file_path.to_string(),
        file_name: PathBuf::from(file_path)
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_string(),
        file_size: 0,
        file_hash: "test_hash".to_string(),
        partial_hash: String::new(),
        format: "cbz".to_string(),
        page_count: 0,
        deleted: false,
        analyzed: false,
        analysis_error: None,
        analysis_errors: None,
        modified_at: Utc::now(),
        created_at: Utc::now(),
        updated_at: Utc::now(),
        thumbnail_path: None,
        thumbnail_generated_at: None,
        koreader_hash: None,
        epub_positions: None,
        epub_spine_items: None,
    };

    let created_book = BookRepository::create(db.sea_orm_connection(), &book, None).await?;
    Ok((created_book, series))
}

/// Helper to create a test book with a specific book naming strategy
async fn create_test_book_with_strategy(
    db: &codex::db::Database,
    file_path: &str,
    book_strategy: BookStrategy,
) -> Result<(books::Model, series::Model)> {
    let params =
        CreateLibraryParams::new("Test Library", "/test/path").with_book_strategy(book_strategy);

    let library = LibraryRepository::create_with_params(db.sea_orm_connection(), params).await?;

    let series =
        SeriesRepository::create(db.sea_orm_connection(), library.id, "Test Series", None).await?;

    let book = books::Model {
        id: Uuid::new_v4(),
        series_id: series.id,
        library_id: library.id,
        file_path: file_path.to_string(),
        file_name: PathBuf::from(file_path)
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_string(),
        file_size: 0,
        file_hash: "test_hash".to_string(),
        partial_hash: String::new(),
        format: "cbz".to_string(),
        page_count: 0,
        deleted: false,
        analyzed: false,
        analysis_error: None,
        analysis_errors: None,
        modified_at: Utc::now(),
        created_at: Utc::now(),
        updated_at: Utc::now(),
        thumbnail_path: None,
        thumbnail_generated_at: None,
        koreader_hash: None,
        epub_positions: None,
        epub_spine_items: None,
    };

    let created_book = BookRepository::create(db.sea_orm_connection(), &book, None).await?;
    Ok((created_book, series))
}

#[tokio::test]
async fn test_analyze_book_saves_metadata() -> Result<()> {
    let (db, _temp_dir) = setup_test_db_wrapper().await;
    let temp_dir = TempDir::new()?;

    // Create a test CBZ file with metadata
    let cbz_path = create_test_cbz_with_metadata(&temp_dir, "test_comic.cbz");

    // Create book record in database
    let (book, _series) = create_test_book(&db, cbz_path.to_str().unwrap()).await?;

    // Verify book is not analyzed yet
    assert!(!book.analyzed);

    // Analyze the book
    let result = analyze_book(db.sea_orm_connection(), book.id, false, None).await?;

    // Verify analysis succeeded
    if !result.errors.is_empty() {
        eprintln!("Analysis errors: {:?}", result.errors);
    }
    assert_eq!(
        result.errors.len(),
        0,
        "Expected no errors, got: {:?}",
        result.errors
    );
    assert_eq!(
        result.books_analyzed, 1,
        "Expected 1 book analyzed, got {}",
        result.books_analyzed
    );

    // Verify book is marked as analyzed
    let updated_book = BookRepository::get_by_id(db.sea_orm_connection(), book.id)
        .await?
        .expect("Book should exist");
    assert!(updated_book.analyzed);
    assert_eq!(updated_book.page_count, 3);

    // Verify metadata was saved
    let metadata = BookMetadataRepository::get_by_book_id(db.sea_orm_connection(), book.id)
        .await?
        .expect("Metadata should exist");

    assert_eq!(
        metadata.summary,
        Some("This is a test comic book summary with detailed description.".to_string())
    );
    // Verify authors are stored in authors_json
    let authors: Vec<serde_json::Value> =
        serde_json::from_str(metadata.authors_json.as_deref().unwrap_or("[]")).unwrap();
    assert!(
        authors
            .iter()
            .any(|a| a["name"] == "Test Writer" && a["role"] == "writer"),
        "Expected author 'Test Writer' with role 'writer' in authors_json"
    );
    assert!(
        authors
            .iter()
            .any(|a| a["name"] == "Test Penciller" && a["role"] == "penciller"),
        "Expected author 'Test Penciller' with role 'penciller' in authors_json"
    );
    assert!(
        authors
            .iter()
            .any(|a| a["name"] == "Test Inker" && a["role"] == "inker"),
        "Expected author 'Test Inker' with role 'inker' in authors_json"
    );
    assert!(
        authors
            .iter()
            .any(|a| a["name"] == "Test Colorist" && a["role"] == "colorist"),
        "Expected author 'Test Colorist' with role 'colorist' in authors_json"
    );
    assert!(
        authors
            .iter()
            .any(|a| a["name"] == "Test Letterer" && a["role"] == "letterer"),
        "Expected author 'Test Letterer' with role 'letterer' in authors_json"
    );
    assert!(
        authors
            .iter()
            .any(|a| a["name"] == "Test Cover Artist" && a["role"] == "cover_artist"),
        "Expected author 'Test Cover Artist' with role 'cover_artist' in authors_json"
    );
    assert!(
        authors
            .iter()
            .any(|a| a["name"] == "Test Editor" && a["role"] == "editor"),
        "Expected author 'Test Editor' with role 'editor' in authors_json"
    );
    assert_eq!(metadata.publisher, Some("Test Publisher".to_string()));
    assert_eq!(metadata.imprint, Some("Test Imprint".to_string()));
    assert_eq!(metadata.genre, Some("Action, Adventure".to_string()));
    // web is now stored as an external link instead of a metadata field
    let external_links =
        BookExternalLinkRepository::get_for_book(db.sea_orm_connection(), book.id).await?;
    let comicinfo_link = external_links.iter().find(|l| l.source_name == "comicinfo");
    assert!(
        comicinfo_link.is_some(),
        "Expected a comicinfo external link"
    );
    assert_eq!(comicinfo_link.unwrap().url, "https://example.com/comic");
    assert_eq!(metadata.language_iso, Some("en".to_string()));
    assert_eq!(metadata.format_detail, Some("Comic".to_string()));
    assert_eq!(metadata.black_and_white, Some(false));
    assert_eq!(metadata.manga, Some(false));
    assert_eq!(metadata.year, Some(2024));
    assert_eq!(metadata.month, Some(1));
    assert_eq!(metadata.day, Some(15));
    assert_eq!(metadata.volume, Some(1));
    assert_eq!(metadata.count, Some(12));

    Ok(())
}

#[tokio::test]
async fn test_analyze_book_saves_pages() -> Result<()> {
    let (db, _temp_dir) = setup_test_db_wrapper().await;
    let temp_dir = TempDir::new()?;

    // Create a test CBZ file
    let cbz_path = create_test_cbz_with_metadata(&temp_dir, "test_pages.cbz");

    // Create book record in database
    let (book, _series) = create_test_book(&db, cbz_path.to_str().unwrap()).await?;

    // Analyze the book
    analyze_book(db.sea_orm_connection(), book.id, false, None).await?;

    // Verify pages were saved
    let pages = PageRepository::list_by_book(db.sea_orm_connection(), book.id).await?;

    assert_eq!(pages.len(), 3, "Should have 3 pages");

    // Verify page details
    for (i, page) in pages.iter().enumerate() {
        assert_eq!(page.page_number, (i + 1) as i32);
        assert_eq!(page.file_name, format!("page{:03}.png", i + 1));
        assert_eq!(page.format, "png");
        assert!(page.width > 0, "Width should be set");
        assert!(page.height > 0, "Height should be set");
        assert!(page.file_size > 0, "File size should be set");
    }

    Ok(())
}

#[tokio::test]
async fn test_analyze_book_without_comic_info() -> Result<()> {
    let (db, _temp_dir) = setup_test_db_wrapper().await;
    let temp_dir = TempDir::new()?;

    // Create a CBZ without ComicInfo.xml
    let file_path = temp_dir.path().join("no_metadata.cbz");
    let file = fs::File::create(&file_path)?;
    let mut zip = ZipWriter::new(file);

    // Add only image pages, no ComicInfo.xml
    for i in 1..=2 {
        let page_name = format!("page{:03}.png", i);
        zip.start_file(&page_name, SimpleFileOptions::default())?;
        let png_data = create_test_png(10, 10);
        zip.write_all(&png_data)?;
    }

    zip.finish()?;

    // Create book record
    let (book, _series) = create_test_book(&db, file_path.to_str().unwrap()).await?;

    // Analyze the book
    let result = analyze_book(db.sea_orm_connection(), book.id, false, None).await?;

    // Verify analysis succeeded even without metadata
    assert_eq!(result.books_analyzed, 1);
    assert!(result.errors.is_empty());

    // Verify book is marked as analyzed
    let updated_book = BookRepository::get_by_id(db.sea_orm_connection(), book.id)
        .await?
        .expect("Book should exist");
    assert!(updated_book.analyzed);
    assert_eq!(updated_book.page_count, 2);

    // Verify title is set from filename when no ComicInfo.xml is available
    // Title is now stored in book_metadata table
    let metadata = BookMetadataRepository::get_by_book_id(db.sea_orm_connection(), book.id).await?;
    assert!(
        metadata.is_some(),
        "Metadata record should be created with title from filename"
    );
    let metadata = metadata.unwrap();
    assert_eq!(
        metadata.title,
        Some("no_metadata".to_string()),
        "Title should be extracted from filename when ComicInfo.xml is missing"
    );

    // Verify pages were still saved
    let pages = PageRepository::list_by_book(db.sea_orm_connection(), book.id).await?;
    assert_eq!(pages.len(), 2);

    Ok(())
}

#[tokio::test]
async fn test_analyze_book_title_fallback_to_filename() -> Result<()> {
    let (db, _temp_dir) = setup_test_db_wrapper().await;
    let temp_dir = TempDir::new()?;

    // Create a CBZ with ComicInfo.xml but without Title field
    let file_path = temp_dir.path().join("my_awesome_book.cbz");
    let file = fs::File::create(&file_path)?;
    let mut zip = ZipWriter::new(file);

    // ComicInfo.xml without Title field
    let comic_info_xml = r#"<?xml version="1.0"?>
<ComicInfo>
  <Writer>Test Writer</Writer>
  <Publisher>Test Publisher</Publisher>
</ComicInfo>"#;

    zip.start_file("ComicInfo.xml", SimpleFileOptions::default())?;
    zip.write_all(comic_info_xml.as_bytes())?;

    // Add image pages
    for i in 1..=2 {
        let page_name = format!("page{:03}.png", i);
        zip.start_file(&page_name, SimpleFileOptions::default())?;
        let png_data = create_test_png(10, 10);
        zip.write_all(&png_data)?;
    }

    zip.finish()?;

    // Create book record
    let (book, _series) = create_test_book(&db, file_path.to_str().unwrap()).await?;

    // Analyze the book
    let result = analyze_book(db.sea_orm_connection(), book.id, false, None).await?;

    // Verify analysis succeeded
    assert_eq!(result.books_analyzed, 1);
    assert!(result.errors.is_empty());

    // Verify book is marked as analyzed
    let updated_book = BookRepository::get_by_id(db.sea_orm_connection(), book.id)
        .await?
        .expect("Book should exist");
    assert!(updated_book.analyzed);

    // Verify title is set from filename when Title field is missing from ComicInfo
    // Title is now stored in book_metadata table
    let metadata = BookMetadataRepository::get_by_book_id(db.sea_orm_connection(), book.id)
        .await?
        .expect("Metadata should exist even without Title");
    assert_eq!(
        metadata.title,
        Some("my_awesome_book".to_string()),
        "Title should be extracted from filename when Title field is missing from ComicInfo.xml"
    );

    // Verify metadata record was created with other fields
    let authors: Vec<serde_json::Value> =
        serde_json::from_str(metadata.authors_json.as_deref().unwrap_or("[]")).unwrap();
    assert!(
        authors
            .iter()
            .any(|a| a["name"] == "Test Writer" && a["role"] == "writer"),
        "Expected author 'Test Writer' with role 'writer' in authors_json"
    );
    assert_eq!(metadata.publisher, Some("Test Publisher".to_string()));

    Ok(())
}

#[tokio::test]
async fn test_analyze_book_title_from_metadata_takes_precedence() -> Result<()> {
    let (db, _temp_dir) = setup_test_db_wrapper().await;
    let temp_dir = TempDir::new()?;

    // Create a CBZ with ComicInfo.xml that has a Title field
    let file_path = temp_dir.path().join("filename_fallback.cbz");
    let file = fs::File::create(&file_path)?;
    let mut zip = ZipWriter::new(file);

    // ComicInfo.xml with Title field - should take precedence over filename
    // when using MetadataFirst book naming strategy
    let comic_info_xml = r#"<?xml version="1.0"?>
<ComicInfo>
  <Title>Actual Book Title from Metadata</Title>
  <Writer>Test Writer</Writer>
</ComicInfo>"#;

    zip.start_file("ComicInfo.xml", SimpleFileOptions::default())?;
    zip.write_all(comic_info_xml.as_bytes())?;

    // Add image pages
    for i in 1..=2 {
        let page_name = format!("page{:03}.png", i);
        zip.start_file(&page_name, SimpleFileOptions::default())?;
        let png_data = create_test_png(10, 10);
        zip.write_all(&png_data)?;
    }

    zip.finish()?;

    // Create book record with MetadataFirst strategy (required for metadata to override filename)
    let (book, _series) = create_test_book_with_strategy(
        &db,
        file_path.to_str().unwrap(),
        BookStrategy::MetadataFirst,
    )
    .await?;

    // Analyze the book
    let result = analyze_book(db.sea_orm_connection(), book.id, false, None).await?;

    // Verify analysis succeeded
    assert_eq!(result.books_analyzed, 1);
    assert!(result.errors.is_empty());

    // Verify book is marked as analyzed
    let updated_book = BookRepository::get_by_id(db.sea_orm_connection(), book.id)
        .await?
        .expect("Book should exist");
    assert!(updated_book.analyzed);

    // Verify title from metadata is used, not filename (MetadataFirst strategy)
    // Title is now stored in book_metadata table
    let metadata = BookMetadataRepository::get_by_book_id(db.sea_orm_connection(), book.id)
        .await?
        .expect("Metadata should exist");
    assert_eq!(
        metadata.title,
        Some("Actual Book Title from Metadata".to_string()),
        "Title from ComicInfo.xml should take precedence over filename with MetadataFirst strategy"
    );

    Ok(())
}

#[tokio::test]
async fn test_analyze_book_filename_no_extension() -> Result<()> {
    let (db, _temp_dir) = setup_test_db_wrapper().await;
    let temp_dir = TempDir::new()?;

    // Create a CBZ without ComicInfo.xml
    // Use .cbz extension for format detection, but test filename extraction logic
    // by creating a book with a file_name that has no extension (simulating edge case)
    let file_path = temp_dir.path().join("noextension.cbz");
    let file = fs::File::create(&file_path)?;
    let mut zip = ZipWriter::new(file);

    // Add only image pages, no ComicInfo.xml
    for i in 1..=2 {
        let page_name = format!("page{:03}.png", i);
        zip.start_file(&page_name, SimpleFileOptions::default())?;
        let png_data = create_test_png(10, 10);
        zip.write_all(&png_data)?;
    }

    zip.finish()?;

    // Create book record - file_path has extension but we'll test with file_name without extension
    let (mut book, _series) = create_test_book(&db, file_path.to_str().unwrap()).await?;

    // Manually set file_name to have no extension to test the fallback logic
    book.file_name = "noextension".to_string();
    BookRepository::update(db.sea_orm_connection(), &book, None).await?;

    // Analyze the book
    let result = analyze_book(db.sea_orm_connection(), book.id, false, None).await?;

    // Verify analysis succeeded
    assert_eq!(result.books_analyzed, 1);
    assert!(result.errors.is_empty());

    // Verify book is marked as analyzed
    let updated_book = BookRepository::get_by_id(db.sea_orm_connection(), book.id)
        .await?
        .expect("Book should exist");
    assert!(updated_book.analyzed);

    // Verify title is set from full filename when no extension exists in file_name
    // Title is now stored in book_metadata table
    let metadata = BookMetadataRepository::get_by_book_id(db.sea_orm_connection(), book.id)
        .await?
        .expect("Metadata should exist");
    assert_eq!(
        metadata.title,
        Some("noextension".to_string()),
        "Title should be the full filename when file_name has no extension"
    );

    Ok(())
}

#[tokio::test]
async fn test_analyze_book_filename_multiple_dots() -> Result<()> {
    let (db, _temp_dir) = setup_test_db_wrapper().await;
    let temp_dir = TempDir::new()?;

    // Create a CBZ without ComicInfo.xml and with filename that has multiple dots
    let file_path = temp_dir.path().join("book.vol.1.cbz");
    let file = fs::File::create(&file_path)?;
    let mut zip = ZipWriter::new(file);

    // Add only image pages, no ComicInfo.xml
    for i in 1..=2 {
        let page_name = format!("page{:03}.png", i);
        zip.start_file(&page_name, SimpleFileOptions::default())?;
        let png_data = create_test_png(10, 10);
        zip.write_all(&png_data)?;
    }

    zip.finish()?;

    // Create book record
    let (book, _series) = create_test_book(&db, file_path.to_str().unwrap()).await?;

    // Analyze the book
    let result = analyze_book(db.sea_orm_connection(), book.id, false, None).await?;

    // Verify analysis succeeded
    assert_eq!(result.books_analyzed, 1);
    assert!(result.errors.is_empty());

    // Verify book is marked as analyzed
    let updated_book = BookRepository::get_by_id(db.sea_orm_connection(), book.id)
        .await?
        .expect("Book should exist");
    assert!(updated_book.analyzed);

    // Verify title uses last dot as extension separator (book.vol.1)
    // Title is now stored in book_metadata table
    let metadata = BookMetadataRepository::get_by_book_id(db.sea_orm_connection(), book.id)
        .await?
        .expect("Metadata should exist");
    assert_eq!(
        metadata.title,
        Some("book.vol.1".to_string()),
        "Title should extract filename up to the last dot when multiple dots exist"
    );

    Ok(())
}

#[tokio::test]
async fn test_analyze_book_empty_title_in_comic_info() -> Result<()> {
    let (db, _temp_dir) = setup_test_db_wrapper().await;
    let temp_dir = TempDir::new()?;

    // Create a CBZ with ComicInfo.xml that has an empty Title field
    let file_path = temp_dir.path().join("empty_title_test.cbz");
    let file = fs::File::create(&file_path)?;
    let mut zip = ZipWriter::new(file);

    // ComicInfo.xml with empty Title field
    let comic_info_xml = r#"<?xml version="1.0"?>
<ComicInfo>
  <Title></Title>
  <Writer>Test Writer</Writer>
</ComicInfo>"#;

    zip.start_file("ComicInfo.xml", SimpleFileOptions::default())?;
    zip.write_all(comic_info_xml.as_bytes())?;

    // Add image pages
    for i in 1..=2 {
        let page_name = format!("page{:03}.png", i);
        zip.start_file(&page_name, SimpleFileOptions::default())?;
        let png_data = create_test_png(10, 10);
        zip.write_all(&png_data)?;
    }

    zip.finish()?;

    // Create book record
    let (book, _series) = create_test_book(&db, file_path.to_str().unwrap()).await?;

    // Analyze the book
    let result = analyze_book(db.sea_orm_connection(), book.id, false, None).await?;

    // Verify analysis succeeded
    assert_eq!(result.books_analyzed, 1);
    assert!(result.errors.is_empty());

    // Verify book is marked as analyzed
    let updated_book = BookRepository::get_by_id(db.sea_orm_connection(), book.id)
        .await?
        .expect("Book should exist");
    assert!(updated_book.analyzed);

    // Verify title falls back to filename when Title field is empty string
    // Empty <Title></Title> in XML becomes Some("") in Rust, which we filter out
    // and fallback to filename
    // Title is now stored in book_metadata table
    let metadata = BookMetadataRepository::get_by_book_id(db.sea_orm_connection(), book.id)
        .await?
        .expect("Metadata should exist");
    assert_eq!(
        metadata.title,
        Some("empty_title_test".to_string()),
        "Title should fallback to filename when ComicInfo Title is empty string"
    );

    Ok(())
}

#[tokio::test]
async fn test_reanalyze_book_updates_metadata() -> Result<()> {
    let (db, _temp_dir) = setup_test_db_wrapper().await;
    let temp_dir = TempDir::new()?;

    // Create initial CBZ
    let cbz_path = create_test_cbz_with_metadata(&temp_dir, "reanalysis_test.cbz");

    // Create and analyze book
    let (book, _series) = create_test_book(&db, cbz_path.to_str().unwrap()).await?;
    analyze_book(db.sea_orm_connection(), book.id, false, None).await?;

    // Get initial metadata
    let initial_metadata = BookMetadataRepository::get_by_book_id(db.sea_orm_connection(), book.id)
        .await?
        .expect("Initial metadata should exist");

    let initial_authors: Vec<serde_json::Value> =
        serde_json::from_str(initial_metadata.authors_json.as_deref().unwrap_or("[]")).unwrap();
    assert!(
        initial_authors
            .iter()
            .any(|a| a["name"] == "Test Writer" && a["role"] == "writer"),
        "Expected initial author 'Test Writer' with role 'writer'"
    );

    // Simulate file change by creating a new CBZ with different metadata
    let file_path = temp_dir.path().join("reanalysis_test.cbz");
    let file = fs::File::create(&file_path)?;
    let mut zip = ZipWriter::new(file);

    // New ComicInfo with updated writer
    let comic_info_xml = r#"<?xml version="1.0"?>
<ComicInfo>
  <Title>Updated Comic Title</Title>
  <Writer>Updated Writer</Writer>
  <Publisher>Updated Publisher</Publisher>
</ComicInfo>"#;

    zip.start_file("ComicInfo.xml", SimpleFileOptions::default())?;
    zip.write_all(comic_info_xml.as_bytes())?;

    zip.start_file("page001.png", SimpleFileOptions::default())?;
    zip.write_all(&create_test_png(10, 10))?;

    zip.finish()?;

    // Re-analyze the book
    let reanalysis_result = analyze_book(db.sea_orm_connection(), book.id, false, None).await?;
    if !reanalysis_result.errors.is_empty() {
        eprintln!("Re-analysis errors: {:?}", reanalysis_result.errors);
    }
    assert_eq!(
        reanalysis_result.errors.len(),
        0,
        "Expected no re-analysis errors"
    );
    assert_eq!(
        reanalysis_result.books_analyzed, 1,
        "Expected 1 book re-analyzed"
    );

    // Verify metadata was updated
    let updated_metadata = BookMetadataRepository::get_by_book_id(db.sea_orm_connection(), book.id)
        .await?
        .expect("Updated metadata should exist");

    eprintln!("Initial authors_json: {:?}", initial_metadata.authors_json);
    eprintln!("Updated authors_json: {:?}", updated_metadata.authors_json);

    let updated_authors: Vec<serde_json::Value> =
        serde_json::from_str(updated_metadata.authors_json.as_deref().unwrap_or("[]")).unwrap();
    assert!(
        updated_authors
            .iter()
            .any(|a| a["name"] == "Updated Writer" && a["role"] == "writer"),
        "Expected updated author 'Updated Writer' with role 'writer'"
    );
    assert_eq!(
        updated_metadata.publisher,
        Some("Updated Publisher".to_string())
    );

    // Verify pages were updated (should only have 1 now)
    let pages = PageRepository::list_by_book(db.sea_orm_connection(), book.id).await?;
    assert_eq!(pages.len(), 1);

    Ok(())
}

#[tokio::test]
async fn test_analyze_book_with_isbns() -> Result<()> {
    let (db, _temp_dir) = setup_test_db_wrapper().await;
    let temp_dir = TempDir::new()?;

    let file_path = temp_dir.path().join("isbn_test.cbz");
    let file = fs::File::create(&file_path)?;
    let mut zip = ZipWriter::new(file);

    // Add ComicInfo.xml
    let comic_info_xml = r#"<?xml version="1.0"?>
<ComicInfo>
  <Title>ISBN Test</Title>
</ComicInfo>"#;

    zip.start_file("ComicInfo.xml", SimpleFileOptions::default())?;
    zip.write_all(comic_info_xml.as_bytes())?;

    zip.start_file("page001.png", SimpleFileOptions::default())?;
    zip.write_all(&create_test_png(10, 10))?;

    zip.finish()?;

    // Create and analyze book
    let (book, _series) = create_test_book(&db, file_path.to_str().unwrap()).await?;
    analyze_book(db.sea_orm_connection(), book.id, false, None).await?;

    // Note: ISBN extraction happens in the parser if barcodes are detected
    // For now, we just verify the metadata exists
    let metadata = BookMetadataRepository::get_by_book_id(db.sea_orm_connection(), book.id)
        .await?
        .expect("Metadata should exist");

    // ISBNs field should be None or an empty array JSON
    assert!(
        metadata.isbns.is_none() || metadata.isbns == Some("[]".to_string()),
        "ISBNs should be None or empty array"
    );

    Ok(())
}

#[tokio::test]
async fn test_analyze_book_with_manga_flag() -> Result<()> {
    let (db, _temp_dir) = setup_test_db_wrapper().await;
    let temp_dir = TempDir::new()?;

    let file_path = temp_dir.path().join("manga_test.cbz");
    let file = fs::File::create(&file_path)?;
    let mut zip = ZipWriter::new(file);

    // Test various manga flag formats
    let comic_info_xml = r#"<?xml version="1.0"?>
<ComicInfo>
  <Title>Manga Test</Title>
  <Manga>YesAndRightToLeft</Manga>
</ComicInfo>"#;

    zip.start_file("ComicInfo.xml", SimpleFileOptions::default())?;
    zip.write_all(comic_info_xml.as_bytes())?;

    zip.start_file("page001.png", SimpleFileOptions::default())?;
    zip.write_all(&create_test_png(10, 10))?;

    zip.finish()?;

    let (book, _series) = create_test_book(&db, file_path.to_str().unwrap()).await?;
    analyze_book(db.sea_orm_connection(), book.id, false, None).await?;

    let metadata = BookMetadataRepository::get_by_book_id(db.sea_orm_connection(), book.id)
        .await?
        .expect("Metadata should exist");

    assert_eq!(metadata.manga, Some(true));

    Ok(())
}

#[tokio::test]
async fn test_series_metadata_populated_from_first_book() -> Result<()> {
    // Create test database and library
    let (db, temp_dir) = setup_test_db_wrapper().await;
    let library_path = temp_dir.path().join("library");
    fs::create_dir_all(&library_path)?;

    let library = LibraryRepository::create(
        db.sea_orm_connection(),
        "Test Library",
        library_path.to_str().unwrap(),
        ScanningStrategy::Default,
    )
    .await?;

    // Create a series manually
    let series =
        SeriesRepository::create(db.sea_orm_connection(), library.id, "Test Series", None).await?;

    // Verify series metadata has no values initially (just the title)
    let metadata = SeriesMetadataRepository::get_by_series_id(db.sea_orm_connection(), series.id)
        .await?
        .expect("Metadata should exist");
    assert_eq!(metadata.summary, None);
    assert_eq!(metadata.publisher, None);
    assert_eq!(metadata.year, None);

    // Create first book with ComicInfo metadata
    let file_path = temp_dir.path().join("book1.cbz");
    let file = fs::File::create(&file_path)?;
    let mut zip = ZipWriter::new(file);

    let comic_info_xml = r#"<?xml version="1.0"?>
<ComicInfo>
    <Title>Test Book 1</Title>
    <Series>Test Series</Series>
    <Number>1</Number>
    <Summary>Test Series Summary</Summary>
    <Publisher>Marvel Comics</Publisher>
    <Year>2024</Year>
    <Writer>Test Writer</Writer>
</ComicInfo>"#;

    zip.start_file("ComicInfo.xml", SimpleFileOptions::default())?;
    zip.write_all(comic_info_xml.as_bytes())?;

    zip.start_file("page001.png", SimpleFileOptions::default())?;
    zip.write_all(&create_test_png(10, 10))?;

    zip.finish()?;

    // Create book directly attached to our series
    let now = Utc::now();
    let book1 = books::Model {
        id: Uuid::new_v4(),
        series_id: series.id,
        library_id: library.id,
        file_path: file_path.to_string_lossy().to_string(),
        file_name: "book1.cbz".to_string(),
        file_size: 0,
        file_hash: "test_hash".to_string(),
        partial_hash: String::new(),
        format: "cbz".to_string(),
        page_count: 0,
        deleted: false,
        analyzed: false,
        analysis_error: None,
        analysis_errors: None,
        modified_at: now,
        created_at: now,
        updated_at: now,
        thumbnail_path: None,
        thumbnail_generated_at: None,
        koreader_hash: None,
        epub_positions: None,
        epub_spine_items: None,
    };

    BookRepository::create(db.sea_orm_connection(), &book1, None).await?;
    analyze_book(db.sea_orm_connection(), book1.id, false, None).await?;

    // Verify series metadata was populated from the first book
    let metadata = SeriesMetadataRepository::get_by_series_id(db.sea_orm_connection(), series.id)
        .await?
        .expect("Metadata should exist");

    assert_eq!(metadata.summary, Some("Test Series Summary".to_string()));
    assert_eq!(metadata.publisher, Some("Marvel Comics".to_string()));
    assert_eq!(metadata.year, Some(2024));

    // Create second book with different metadata
    let file_path2 = temp_dir.path().join("book2.cbz");
    let file = fs::File::create(&file_path2)?;
    let mut zip = ZipWriter::new(file);

    let comic_info_xml2 = r#"<?xml version="1.0"?>
<ComicInfo>
    <Title>Test Book 2</Title>
    <Series>Test Series</Series>
    <Number>2</Number>
    <Summary>Different Summary</Summary>
    <Publisher>DC Comics</Publisher>
    <Year>2025</Year>
</ComicInfo>"#;

    zip.start_file("ComicInfo.xml", SimpleFileOptions::default())?;
    zip.write_all(comic_info_xml2.as_bytes())?;

    zip.start_file("page001.png", SimpleFileOptions::default())?;
    zip.write_all(&create_test_png(10, 10))?;

    zip.finish()?;

    // Create second book directly attached to our series
    let book2 = books::Model {
        id: Uuid::new_v4(),
        series_id: series.id,
        library_id: library.id,
        file_path: file_path2.to_string_lossy().to_string(),
        file_name: "book2.cbz".to_string(),
        file_size: 0,
        file_hash: "test_hash2".to_string(),
        partial_hash: String::new(),
        format: "cbz".to_string(),
        page_count: 0,
        deleted: false,
        analyzed: false,
        analysis_error: None,
        analysis_errors: None,
        modified_at: now,
        created_at: now,
        updated_at: now,
        thumbnail_path: None,
        thumbnail_generated_at: None,
        koreader_hash: None,
        epub_positions: None,
        epub_spine_items: None,
    };

    BookRepository::create(db.sea_orm_connection(), &book2, None).await?;
    analyze_book(db.sea_orm_connection(), book2.id, false, None).await?;

    // Verify series metadata was NOT overwritten by the second book
    let metadata = SeriesMetadataRepository::get_by_series_id(db.sea_orm_connection(), series.id)
        .await?
        .expect("Metadata should exist");

    assert_eq!(metadata.summary, Some("Test Series Summary".to_string()));
    assert_eq!(metadata.publisher, Some("Marvel Comics".to_string()));
    assert_eq!(metadata.year, Some(2024));

    Ok(())
}

#[tokio::test]
async fn test_series_metadata_respects_manual_changes() -> Result<()> {
    // Create test database and library
    let (db, temp_dir) = setup_test_db_wrapper().await;
    let library_path = temp_dir.path().join("library");
    fs::create_dir_all(&library_path)?;

    let library = LibraryRepository::create(
        db.sea_orm_connection(),
        "Test Library",
        library_path.to_str().unwrap(),
        ScanningStrategy::Default,
    )
    .await?;

    // Create a series with manually set metadata
    let series =
        SeriesRepository::create(db.sea_orm_connection(), library.id, "Test Series", None).await?;

    // Manually set series metadata (simulating user edit) and lock the fields
    SeriesMetadataRepository::update_summary(
        db.sea_orm_connection(),
        series.id,
        Some("Manually Set Summary".to_string()),
    )
    .await?;
    SeriesMetadataRepository::update_publisher(
        db.sea_orm_connection(),
        series.id,
        Some("Custom Publisher".to_string()),
        None,
    )
    .await?;
    SeriesMetadataRepository::update_year(db.sea_orm_connection(), series.id, Some(2020)).await?;
    // Lock the fields to prevent auto-refresh from overwriting
    SeriesMetadataRepository::set_lock(db.sea_orm_connection(), series.id, "summary", true).await?;
    SeriesMetadataRepository::set_lock(db.sea_orm_connection(), series.id, "publisher", true)
        .await?;
    SeriesMetadataRepository::set_lock(db.sea_orm_connection(), series.id, "year", true).await?;

    // Create a book with different metadata
    let file_path = temp_dir.path().join("book1.cbz");
    let file = fs::File::create(&file_path)?;
    let mut zip = ZipWriter::new(file);

    let comic_info_xml = r#"<?xml version="1.0"?>
<ComicInfo>
    <Title>Test Book</Title>
    <Series>Test Series</Series>
    <Number>1</Number>
    <Summary>Book Summary</Summary>
    <Publisher>Book Publisher</Publisher>
    <Year>2024</Year>
</ComicInfo>"#;

    zip.start_file("ComicInfo.xml", SimpleFileOptions::default())?;
    zip.write_all(comic_info_xml.as_bytes())?;

    zip.start_file("page001.png", SimpleFileOptions::default())?;
    zip.write_all(&create_test_png(10, 10))?;

    zip.finish()?;

    let (book, _) = create_test_book(&db, file_path.to_str().unwrap()).await?;
    analyze_book(db.sea_orm_connection(), book.id, false, None).await?;

    // Verify series metadata was NOT overwritten (locked fields preserved)
    let metadata = SeriesMetadataRepository::get_by_series_id(db.sea_orm_connection(), series.id)
        .await?
        .expect("Metadata should exist");

    assert_eq!(metadata.summary, Some("Manually Set Summary".to_string()));
    assert_eq!(metadata.publisher, Some("Custom Publisher".to_string()));
    assert_eq!(metadata.year, Some(2020));

    Ok(())
}

#[tokio::test]
async fn test_series_title_sort_populated_from_title() -> Result<()> {
    // Create test database and library
    let (db, temp_dir) = setup_test_db_wrapper().await;
    let library_path = temp_dir.path().join("library");
    fs::create_dir_all(&library_path)?;

    let library = LibraryRepository::create(
        db.sea_orm_connection(),
        "Test Library",
        library_path.to_str().unwrap(),
        ScanningStrategy::Default,
    )
    .await?;

    // Create a series
    let series = SeriesRepository::create(
        db.sea_orm_connection(),
        library.id,
        "Amazing Spider-Man",
        None,
    )
    .await?;

    // Verify series metadata has title but no title_sort initially
    let metadata = SeriesMetadataRepository::get_by_series_id(db.sea_orm_connection(), series.id)
        .await?
        .expect("Metadata should exist");
    assert_eq!(metadata.title, "Amazing Spider-Man".to_string());
    assert_eq!(metadata.title_sort, None);

    // Create a book
    let file_path = temp_dir.path().join("book1.cbz");
    let file = fs::File::create(&file_path)?;
    let mut zip = ZipWriter::new(file);

    let comic_info_xml = r#"<?xml version="1.0"?>
<ComicInfo>
    <Title>Test Book 1</Title>
    <Series>Amazing Spider-Man</Series>
    <Number>1</Number>
</ComicInfo>"#;

    zip.start_file("ComicInfo.xml", SimpleFileOptions::default())?;
    zip.write_all(comic_info_xml.as_bytes())?;

    zip.start_file("page001.png", SimpleFileOptions::default())?;
    zip.write_all(&create_test_png(10, 10))?;

    zip.finish()?;

    // Create book directly attached to our series
    let now = Utc::now();
    let book = books::Model {
        id: Uuid::new_v4(),
        series_id: series.id,
        library_id: library.id,
        file_path: file_path.to_string_lossy().to_string(),
        file_name: "book1.cbz".to_string(),
        file_size: 0,
        file_hash: "test_hash".to_string(),
        partial_hash: String::new(),
        format: "cbz".to_string(),
        page_count: 0,
        deleted: false,
        analyzed: false,
        analysis_error: None,
        analysis_errors: None,
        modified_at: now,
        created_at: now,
        updated_at: now,
        thumbnail_path: None,
        thumbnail_generated_at: None,
        koreader_hash: None,
        epub_positions: None,
        epub_spine_items: None,
    };

    BookRepository::create(db.sea_orm_connection(), &book, None).await?;
    analyze_book(db.sea_orm_connection(), book.id, false, None).await?;

    // Verify series title_sort was populated from title
    let metadata = SeriesMetadataRepository::get_by_series_id(db.sea_orm_connection(), series.id)
        .await?
        .expect("Metadata should exist");

    assert_eq!(
        metadata.title_sort,
        Some("Amazing Spider-Man".to_string()),
        "title_sort should be populated from title during analysis"
    );

    Ok(())
}

#[tokio::test]
async fn test_series_title_sort_respects_lock() -> Result<()> {
    // Create test database and library
    let (db, temp_dir) = setup_test_db_wrapper().await;
    let library_path = temp_dir.path().join("library");
    fs::create_dir_all(&library_path)?;

    let library = LibraryRepository::create(
        db.sea_orm_connection(),
        "Test Library",
        library_path.to_str().unwrap(),
        ScanningStrategy::Default,
    )
    .await?;

    // Create a series
    let series = SeriesRepository::create(
        db.sea_orm_connection(),
        library.id,
        "The Amazing Spider-Man",
        None,
    )
    .await?;

    // Lock the title_sort field (simulating user wanting to keep it empty or set their own)
    SeriesMetadataRepository::set_lock(db.sea_orm_connection(), series.id, "title_sort", true)
        .await?;

    // Verify title_sort is None and locked
    let metadata = SeriesMetadataRepository::get_by_series_id(db.sea_orm_connection(), series.id)
        .await?
        .expect("Metadata should exist");
    assert_eq!(metadata.title_sort, None);
    assert!(metadata.title_sort_lock);

    // Create a book
    let file_path = temp_dir.path().join("book1.cbz");
    let file = fs::File::create(&file_path)?;
    let mut zip = ZipWriter::new(file);

    let comic_info_xml = r#"<?xml version="1.0"?>
<ComicInfo>
    <Title>Test Book 1</Title>
    <Series>The Amazing Spider-Man</Series>
    <Number>1</Number>
</ComicInfo>"#;

    zip.start_file("ComicInfo.xml", SimpleFileOptions::default())?;
    zip.write_all(comic_info_xml.as_bytes())?;

    zip.start_file("page001.png", SimpleFileOptions::default())?;
    zip.write_all(&create_test_png(10, 10))?;

    zip.finish()?;

    // Create book directly attached to our series
    let now = Utc::now();
    let book = books::Model {
        id: Uuid::new_v4(),
        series_id: series.id,
        library_id: library.id,
        file_path: file_path.to_string_lossy().to_string(),
        file_name: "book1.cbz".to_string(),
        file_size: 0,
        file_hash: "test_hash".to_string(),
        partial_hash: String::new(),
        format: "cbz".to_string(),
        page_count: 0,
        deleted: false,
        analyzed: false,
        analysis_error: None,
        analysis_errors: None,
        modified_at: now,
        created_at: now,
        updated_at: now,
        thumbnail_path: None,
        thumbnail_generated_at: None,
        koreader_hash: None,
        epub_positions: None,
        epub_spine_items: None,
    };

    BookRepository::create(db.sea_orm_connection(), &book, None).await?;
    analyze_book(db.sea_orm_connection(), book.id, false, None).await?;

    // Verify series title_sort was NOT updated because it's locked
    let metadata = SeriesMetadataRepository::get_by_series_id(db.sea_orm_connection(), series.id)
        .await?
        .expect("Metadata should exist");

    assert_eq!(
        metadata.title_sort, None,
        "title_sort should remain None when locked"
    );
    assert!(metadata.title_sort_lock, "lock should still be set");

    Ok(())
}

#[tokio::test]
async fn test_series_title_sort_not_overwritten_if_already_set() -> Result<()> {
    // Create test database and library
    let (db, temp_dir) = setup_test_db_wrapper().await;
    let library_path = temp_dir.path().join("library");
    fs::create_dir_all(&library_path)?;

    let library = LibraryRepository::create(
        db.sea_orm_connection(),
        "Test Library",
        library_path.to_str().unwrap(),
        ScanningStrategy::Default,
    )
    .await?;

    // Create a series
    let series = SeriesRepository::create(
        db.sea_orm_connection(),
        library.id,
        "The Amazing Spider-Man",
        None,
    )
    .await?;

    // Set a custom title_sort
    SeriesMetadataRepository::update_title(
        db.sea_orm_connection(),
        series.id,
        "The Amazing Spider-Man".to_string(),
        Some("Amazing Spider-Man, The".to_string()),
    )
    .await?;

    // Verify title_sort is set
    let metadata = SeriesMetadataRepository::get_by_series_id(db.sea_orm_connection(), series.id)
        .await?
        .expect("Metadata should exist");
    assert_eq!(
        metadata.title_sort,
        Some("Amazing Spider-Man, The".to_string())
    );

    // Create a book
    let file_path = temp_dir.path().join("book1.cbz");
    let file = fs::File::create(&file_path)?;
    let mut zip = ZipWriter::new(file);

    let comic_info_xml = r#"<?xml version="1.0"?>
<ComicInfo>
    <Title>Test Book 1</Title>
    <Series>The Amazing Spider-Man</Series>
    <Number>1</Number>
</ComicInfo>"#;

    zip.start_file("ComicInfo.xml", SimpleFileOptions::default())?;
    zip.write_all(comic_info_xml.as_bytes())?;

    zip.start_file("page001.png", SimpleFileOptions::default())?;
    zip.write_all(&create_test_png(10, 10))?;

    zip.finish()?;

    // Create book directly attached to our series
    let now = Utc::now();
    let book = books::Model {
        id: Uuid::new_v4(),
        series_id: series.id,
        library_id: library.id,
        file_path: file_path.to_string_lossy().to_string(),
        file_name: "book1.cbz".to_string(),
        file_size: 0,
        file_hash: "test_hash".to_string(),
        partial_hash: String::new(),
        format: "cbz".to_string(),
        page_count: 0,
        deleted: false,
        analyzed: false,
        analysis_error: None,
        analysis_errors: None,
        modified_at: now,
        created_at: now,
        updated_at: now,
        thumbnail_path: None,
        thumbnail_generated_at: None,
        koreader_hash: None,
        epub_positions: None,
        epub_spine_items: None,
    };

    BookRepository::create(db.sea_orm_connection(), &book, None).await?;
    analyze_book(db.sea_orm_connection(), book.id, false, None).await?;

    // Verify series title_sort was NOT overwritten
    let metadata = SeriesMetadataRepository::get_by_series_id(db.sea_orm_connection(), series.id)
        .await?
        .expect("Metadata should exist");

    assert_eq!(
        metadata.title_sort,
        Some("Amazing Spider-Man, The".to_string()),
        "title_sort should not be overwritten if already set"
    );

    Ok(())
}

#[tokio::test]
async fn test_series_title_sort_populated_without_comic_info() -> Result<()> {
    // Create test database and library
    let (db, temp_dir) = setup_test_db_wrapper().await;
    let library_path = temp_dir.path().join("library");
    fs::create_dir_all(&library_path)?;

    let library = LibraryRepository::create(
        db.sea_orm_connection(),
        "Test Library",
        library_path.to_str().unwrap(),
        ScanningStrategy::Default,
    )
    .await?;

    // Create a series
    let series =
        SeriesRepository::create(db.sea_orm_connection(), library.id, "Batman", None).await?;

    // Verify title_sort is None initially
    let metadata = SeriesMetadataRepository::get_by_series_id(db.sea_orm_connection(), series.id)
        .await?
        .expect("Metadata should exist");
    assert_eq!(metadata.title_sort, None);

    // Create a book WITHOUT ComicInfo.xml
    let file_path = temp_dir.path().join("book1.cbz");
    let file = fs::File::create(&file_path)?;
    let mut zip = ZipWriter::new(file);

    // Only add image pages, no ComicInfo.xml
    zip.start_file("page001.png", SimpleFileOptions::default())?;
    zip.write_all(&create_test_png(10, 10))?;

    zip.finish()?;

    // Create book directly attached to our series
    let now = Utc::now();
    let book = books::Model {
        id: Uuid::new_v4(),
        series_id: series.id,
        library_id: library.id,
        file_path: file_path.to_string_lossy().to_string(),
        file_name: "book1.cbz".to_string(),
        file_size: 0,
        file_hash: "test_hash".to_string(),
        partial_hash: String::new(),
        format: "cbz".to_string(),
        page_count: 0,
        deleted: false,
        analyzed: false,
        analysis_error: None,
        analysis_errors: None,
        modified_at: now,
        created_at: now,
        updated_at: now,
        thumbnail_path: None,
        thumbnail_generated_at: None,
        koreader_hash: None,
        epub_positions: None,
        epub_spine_items: None,
    };

    BookRepository::create(db.sea_orm_connection(), &book, None).await?;
    analyze_book(db.sea_orm_connection(), book.id, false, None).await?;

    // Verify series title_sort was populated from title even without ComicInfo
    let metadata = SeriesMetadataRepository::get_by_series_id(db.sea_orm_connection(), series.id)
        .await?
        .expect("Metadata should exist");

    assert_eq!(
        metadata.title_sort,
        Some("Batman".to_string()),
        "title_sort should be populated from title even without ComicInfo"
    );

    Ok(())
}
