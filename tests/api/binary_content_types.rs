//! Conformance between the media types the document declares and the ones the
//! server actually writes, for every endpoint whose body is not JSON.
//!
//! This invariant cannot be checked from inside the OpenAPI document: nothing in
//! it is inconsistent, because the document has no idea what the handler sets on
//! the `Content-Type` header. It has to be asserted from the other side, by
//! making the request and comparing the answer against what was declared.
//!
//! A strict generator (`swift-openapi-generator` emits
//! `converter.validateContentType(in:matching:)`) throws on a response whose
//! media type is not among the declared ones, so a mismatch here is a hard
//! client failure on a response the server considers successful.
//!
//! The cases picked below are deliberately the *unhappy* ones — the SVG
//! placeholder a thumbnail serves before its generation task finishes, a PNG
//! page rather than a JPEG one, a PDF rather than a CBZ. The happy path was
//! always going to match; these are the ones that fired in the field.

#[path = "../common/mod.rs"]
mod common;

use codex::api::ApiDoc;
use codex::db::ScanningStrategy;
use codex::db::repositories::{
    BookRepository, LibraryRepository, PageRepository, SeriesExportRepository, SeriesRepository,
    UserRepository,
};
use codex::utils::password;
use common::*;
use hyper::StatusCode;
use serde_json::{Value, json};
use std::path::Path;
use tempfile::TempDir;
use utoipa::OpenApi;

// ============================================================================
// Document lookup
// ============================================================================

/// The media types the document declares on `operation_id`'s 200 response.
///
/// Panics if the operation is absent or declares no 200 content, because both
/// mean the assertion below would silently pass on nothing.
fn declared_200_content_types(operation_id: &str) -> Vec<String> {
    let spec: Value = serde_json::to_value(ApiDoc::openapi()).expect("spec should serialize");

    let content =
        spec["paths"]
            .as_object()
            .expect("paths object")
            .values()
            .flat_map(|item| item.as_object().expect("path item object").values())
            .find(|op| op.get("operationId").and_then(Value::as_str) == Some(operation_id))
            .unwrap_or_else(|| panic!("no operation with operationId {operation_id}"))["responses"]
            ["200"]["content"]
            .as_object()
            .unwrap_or_else(|| panic!("{operation_id} declares no 200 content"))
            .keys()
            .cloned()
            .collect::<Vec<_>>();

    assert!(
        !content.is_empty(),
        "{operation_id} declares an empty 200 content map"
    );
    content
}

/// True when `actual` is covered by `declared`, per the OpenAPI media-type
/// matching rules a strict client applies.
///
/// Two things a naive string compare gets wrong: a declared `image/*` covers
/// every image subtype, and a header may carry parameters (`text/csv;
/// charset=utf-8`) that are not part of the media type key in the document.
fn media_type_matches(declared: &str, actual: &str) -> bool {
    let actual = actual.split(';').next().unwrap_or("").trim();

    if let Some(declared_type) = declared.strip_suffix("/*") {
        return actual
            .split('/')
            .next()
            .is_some_and(|actual_type| actual_type.eq_ignore_ascii_case(declared_type));
    }

    declared.eq_ignore_ascii_case(actual)
}

/// Assert the response's `Content-Type` is one the document declares for this
/// operation, and that it is the specific one this test set out to exercise.
///
/// The second half matters: without it a test that stopped reaching the
/// placeholder path would keep passing while covering nothing.
fn assert_declared_content_type(
    operation_id: &str,
    headers: &hyper::HeaderMap,
    expected_actual: &str,
) {
    let actual = headers
        .get(hyper::header::CONTENT_TYPE)
        .unwrap_or_else(|| panic!("{operation_id} sent no Content-Type"))
        .to_str()
        .expect("Content-Type is valid ASCII");

    assert!(
        media_type_matches(expected_actual, actual),
        "{operation_id} was expected to exercise {expected_actual}, but sent {actual}; \
         the test no longer covers the path it was written for"
    );

    let declared = declared_200_content_types(operation_id);
    assert!(
        declared.iter().any(|d| media_type_matches(d, actual)),
        "{operation_id} sent Content-Type {actual}, which none of its declared media \
         types {declared:?} covers. A strict generated client throws on this response \
         instead of decoding it."
    );
}

// ============================================================================
// Fixtures
// ============================================================================

async fn admin_token(
    db: &sea_orm::DatabaseConnection,
    state: &codex::api::extractors::AppState,
) -> String {
    let password_hash = password::hash_password("admin123").unwrap();
    let user = create_test_user("admin", "admin@example.com", &password_hash, true);
    let created = UserRepository::create(db, &user).await.unwrap();

    state
        .jwt_service
        .generate_token(created.id, created.username.clone(), created.get_role())
        .unwrap()
}

fn book_model(
    series_id: uuid::Uuid,
    library_id: uuid::Uuid,
    path: &str,
    file_name: &str,
    format: &str,
    page_count: i32,
) -> codex::db::entities::books::Model {
    use chrono::Utc;
    codex::db::entities::books::Model {
        id: uuid::Uuid::new_v4(),
        series_id,
        library_id,
        path: path.to_string(),
        file_name: file_name.to_string(),
        file_size: 1024,
        file_hash: format!("hash_{}", uuid::Uuid::new_v4()),
        partial_hash: String::new(),
        format: format.to_string(),
        page_count,
        deleted: false,
        analyzed: page_count > 0,
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
    }
}

/// Register a book whose file already exists on disk, and return its id.
async fn seed_book(
    db: &sea_orm::DatabaseConnection,
    dir: &Path,
    file: &Path,
    format: &str,
    page_count: i32,
) -> uuid::Uuid {
    let library = LibraryRepository::create(
        db,
        "Test Library",
        dir.to_str().unwrap(),
        ScanningStrategy::Default,
    )
    .await
    .unwrap();

    let series = SeriesRepository::create(db, library.id, "Test Series", None)
        .await
        .unwrap();

    let book = book_model(
        series.id,
        library.id,
        file.to_str().unwrap(),
        file.file_name().unwrap().to_str().unwrap(),
        format,
        page_count,
    );

    BookRepository::create(db, &book, None).await.unwrap().id
}

// ============================================================================
// get_book_file: one concrete type per book format
// ============================================================================

/// Every format the handler's match arm names, paired with the media type it
/// sets for that format. The handler never opens the archive on this route, so
/// the bytes on disk only have to exist.
async fn assert_book_file_content_type(format: &str, extension: &str, expected: &str) {
    let (db, _temp_dir) = setup_test_db().await;
    let dir = TempDir::new().unwrap();
    let file = dir.path().join(format!("volume.{extension}"));
    std::fs::write(&file, b"not really an archive, but it is bytes").unwrap();

    let book_id = seed_book(&db, dir.path(), &file, format, 1).await;

    let state = create_test_app_state(db.clone()).await;
    let token = admin_token(&db, &state).await;
    let app = create_test_router_with_app_state(state);

    let request = get_request_with_auth(&format!("/api/v1/books/{book_id}/file"), &token);
    let (status, headers, _body) = make_full_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    assert_declared_content_type("get_book_file", &headers, expected);
}

#[tokio::test]
async fn book_file_declares_the_zip_type_it_sends_for_a_cbz() {
    assert_book_file_content_type("cbz", "cbz", "application/zip").await;
}

#[tokio::test]
async fn book_file_declares_the_rar_type_it_sends_for_a_cbr() {
    assert_book_file_content_type("cbr", "cbr", "application/x-rar-compressed").await;
}

#[tokio::test]
async fn book_file_declares_the_epub_type_it_sends_for_an_epub() {
    assert_book_file_content_type("epub", "epub", "application/epub+zip").await;
}

#[tokio::test]
async fn book_file_declares_the_pdf_type_it_sends_for_a_pdf() {
    assert_book_file_content_type("pdf", "pdf", "application/pdf").await;
}

/// The handler's catch-all arm. Unreachable through the scanner, which only
/// writes the four formats above, but reachable through any other writer of
/// `books.format`, and the document has to describe it or a client throws.
#[tokio::test]
async fn book_file_declares_the_octet_stream_type_it_sends_for_an_unknown_format() {
    assert_book_file_content_type("djvu", "djvu", "application/octet-stream").await;
}

// ============================================================================
// get_page_image and the OPDS-PSE copy: whatever is in the archive
// ============================================================================

/// The CBZ fixture holds PNG pages, so this is the non-JPEG path that
/// `image/jpeg` failed to describe.
async fn seed_png_page_book(db: &sea_orm::DatabaseConnection, dir: &TempDir) -> uuid::Uuid {
    let cbz = create_test_cbz(dir, 3, true);
    let book_id = seed_book(db, dir.path(), &cbz, "cbz", 3).await;

    let page = codex::db::entities::pages::Model {
        id: uuid::Uuid::new_v4(),
        book_id,
        page_number: 1,
        file_name: "page001.png".to_string(),
        format: "png".to_string(),
        width: 800,
        height: 1200,
        file_size: 50_000,
        created_at: chrono::Utc::now(),
    };
    PageRepository::create(db, &page).await.unwrap();

    book_id
}

#[tokio::test]
async fn page_image_declares_the_png_type_it_sends_for_a_png_page() {
    let (db, _temp_dir) = setup_test_db().await;
    let dir = TempDir::new().unwrap();
    let book_id = seed_png_page_book(&db, &dir).await;

    let state = create_test_app_state(db.clone()).await;
    let token = admin_token(&db, &state).await;
    let app = create_test_router_with_app_state(state);

    let request = get_request_with_auth(&format!("/api/v1/books/{book_id}/pages/1"), &token);
    let (status, headers, _body) = make_full_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    assert_declared_content_type("get_page_image", &headers, "image/png");
}

/// The OPDS page-serving route delegates straight to `get_page_image`, so it
/// answers with the same media types and needs the same declaration.
#[tokio::test]
async fn opds_page_image_declares_the_png_type_it_sends_for_a_png_page() {
    let (db, _temp_dir) = setup_test_db().await;
    let dir = TempDir::new().unwrap();
    let book_id = seed_png_page_book(&db, &dir).await;

    let state = create_test_app_state(db.clone()).await;
    let token = admin_token(&db, &state).await;
    let app = create_test_router_with_app_state(state);

    let request = get_request_with_auth(&format!("/opds/books/{book_id}/pages/1"), &token);
    let (status, headers, _body) = make_full_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    assert_declared_content_type("opds_book_page_image", &headers, "image/png");
}

// ============================================================================
// Thumbnails: the SVG placeholder, which fires on timing rather than content
// ============================================================================

#[tokio::test]
async fn book_thumbnail_declares_the_svg_type_it_sends_for_a_placeholder() {
    let (db, _temp_dir) = setup_test_db().await;
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("volume.cbz");
    std::fs::write(&file, b"bytes").unwrap();

    // page_count 0 is the "nothing to render a cover from yet" state a book is
    // in between being discovered and being analysed.
    let book_id = seed_book(&db, dir.path(), &file, "cbz", 0).await;

    let state = create_test_app_state(db.clone()).await;
    let token = admin_token(&db, &state).await;
    let app = create_test_router_with_app_state(state);

    let request = get_request_with_auth(&format!("/api/v1/books/{book_id}/thumbnail"), &token);
    let (status, headers, _body) = make_full_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    assert_declared_content_type("get_book_thumbnail", &headers, "image/svg+xml");
}

#[tokio::test]
async fn series_thumbnail_declares_the_svg_type_it_sends_for_a_placeholder() {
    let (db, _temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(
        &db,
        "Test Library",
        "/tmp/does-not-need-to-exist",
        ScanningStrategy::Default,
    )
    .await
    .unwrap();
    let series = SeriesRepository::create(&db, library.id, "Test Series", None)
        .await
        .unwrap();

    let state = create_test_app_state(db.clone()).await;
    let token = admin_token(&db, &state).await;
    let app = create_test_router_with_app_state(state);

    let request = get_request_with_auth(&format!("/api/v1/series/{}/thumbnail", series.id), &token);
    let (status, headers, _body) = make_full_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    assert_declared_content_type("get_series_thumbnail", &headers, "image/svg+xml");
}

// ============================================================================
// Series export download: a text type behind an octet-stream declaration
// ============================================================================

#[tokio::test]
async fn export_download_declares_the_csv_type_it_sends_for_a_csv_export() {
    let (db, _temp_dir) = setup_test_db().await;
    let dir = TempDir::new().unwrap();
    let export_file = dir.path().join("export.csv");
    std::fs::write(&export_file, b"name,year\nTest Series,2026\n").unwrap();

    let password_hash = password::hash_password("admin123").unwrap();
    let user = create_test_user("admin", "admin@example.com", &password_hash, true);
    let user = UserRepository::create(&db, &user).await.unwrap();

    let export = SeriesExportRepository::create(
        &db,
        user.id,
        "csv",
        "series",
        json!([]),
        json!(["name"]),
        None,
        chrono::Utc::now() + chrono::Duration::days(1),
    )
    .await
    .unwrap();
    SeriesExportRepository::mark_completed(&db, export.id, export_file.to_str().unwrap(), 24, 1)
        .await
        .unwrap();

    let state = create_test_app_state(db.clone()).await;
    let token = state
        .jwt_service
        .generate_token(user.id, user.username.clone(), user.get_role())
        .unwrap();
    let app = create_test_router_with_app_state(state);

    let request = get_request_with_auth(
        &format!("/api/v1/user/exports/series/{}/download", export.id),
        &token,
    );
    let (status, headers, _body) = make_full_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    assert_declared_content_type("download_export", &headers, "text/csv");
}

// ============================================================================
// The matcher itself
// ============================================================================

#[test]
fn wildcard_media_types_cover_their_subtypes() {
    assert!(media_type_matches("image/*", "image/svg+xml"));
    assert!(media_type_matches("image/*", "image/jpeg"));
    assert!(!media_type_matches("image/*", "application/octet-stream"));
}

#[test]
fn media_type_parameters_are_not_part_of_the_match() {
    assert!(media_type_matches("text/csv", "text/csv; charset=utf-8"));
    assert!(!media_type_matches(
        "text/csv",
        "text/markdown; charset=utf-8"
    ));
}
