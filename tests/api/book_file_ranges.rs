//! Range, conditional, and resume behaviour on `GET /api/v1/books/{id}/file`.
//!
//! Two capabilities motivate these, and they fail independently. A download
//! that drops partway has to resume rather than restart, which is `Range` plus
//! a validator. And a client that can read a ZIP central directory with
//! `bytes=-65536` can show page one of a large volume without fetching the rest,
//! which is the suffix form specifically — an implementation that handles only
//! `bytes=a-b` satisfies the first and none of the second.
//!
//! The assertions that matter most are the ones about bytes rather than status
//! codes: a 206 with the right `Content-Range` and the wrong slice of the file
//! is worse than no range support at all, because it corrupts a resumed
//! download silently.

#[path = "../common/mod.rs"]
mod common;

use codex::db::ScanningStrategy;
use codex::db::entities::user_sharing_tags::AccessMode;
use codex::db::repositories::{
    BookRepository, LibraryRepository, SeriesRepository, SharingTagRepository, UserRepository,
};
use codex::utils::password;
use common::*;
use hyper::StatusCode;
use hyper::header;
use tempfile::TempDir;

/// Big enough that a suffix range is a meaningfully small slice of it, and
/// patterned so a wrong offset shows up as wrong bytes rather than as a
/// plausible-looking blob.
const FILE_LEN: usize = 8192;

fn file_bytes() -> Vec<u8> {
    (0..FILE_LEN).map(|i| (i % 251) as u8).collect()
}

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

struct Fixture {
    db: sea_orm::DatabaseConnection,
    book_id: uuid::Uuid,
    file_hash: String,
    token: String,
    state: std::sync::Arc<codex::api::extractors::AppState>,
    _temp_db: TempDir,
    _dir: TempDir,
}

impl Fixture {
    fn app(&self) -> axum::Router {
        create_test_router_with_app_state(self.state.clone())
    }
}

/// A book whose file is on disk, with a known `file_hash` so the ETag is
/// predictable.
async fn fixture_with_name(file_name: &str) -> Fixture {
    let (db, _temp_db) = setup_test_db().await;
    let dir = TempDir::new().unwrap();
    let path = dir.path().join(file_name);
    std::fs::write(&path, file_bytes()).unwrap();

    let library = LibraryRepository::create(
        &db,
        "Test Library",
        dir.path().to_str().unwrap(),
        ScanningStrategy::Default,
    )
    .await
    .unwrap();
    let series = SeriesRepository::create(&db, library.id, "Test Series", None)
        .await
        .unwrap();

    let file_hash = "0123456789abcdef0123456789abcdef".to_string();
    let book = codex::db::entities::books::Model {
        id: uuid::Uuid::new_v4(),
        series_id: series.id,
        library_id: library.id,
        path: path.to_str().unwrap().to_string(),
        file_name: file_name.to_string(),
        file_size: FILE_LEN as i64,
        file_hash: file_hash.clone(),
        partial_hash: String::new(),
        format: "cbz".to_string(),
        page_count: 1,
        deleted: false,
        analyzed: true,
        analysis_error: None,
        analysis_errors: None,
        modified_at: chrono::Utc::now(),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
        thumbnail_path: None,
        thumbnail_generated_at: None,
        koreader_hash: None,
        epub_positions: None,
        epub_spine_items: None,
    };
    let book = BookRepository::create(&db, &book, None).await.unwrap();

    let state = create_test_app_state(db.clone()).await;
    let token = admin_token(&db, &state).await;

    Fixture {
        db,
        book_id: book.id,
        file_hash,
        token,
        state,
        _temp_db,
        _dir: dir,
    }
}

async fn fixture() -> Fixture {
    fixture_with_name("volume.cbz").await
}

/// Issue a request with an arbitrary set of extra headers.
async fn get_file_with(
    f: &Fixture,
    extra: &[(header::HeaderName, &str)],
) -> (StatusCode, hyper::HeaderMap, Vec<u8>) {
    let mut request = get_request_with_auth(&format!("/api/v1/books/{}/file", f.book_id), &f.token);
    for (name, value) in extra {
        request
            .headers_mut()
            .insert(name.clone(), value.parse().unwrap());
    }
    make_full_request(f.app(), request).await
}

fn header_str(headers: &hyper::HeaderMap, name: header::HeaderName) -> Option<String> {
    headers
        .get(name)
        .map(|v| v.to_str().expect("header is ASCII").to_string())
}

// ============================================================================
// The unranged response, which must not have moved
// ============================================================================

#[tokio::test]
async fn a_request_without_a_range_still_returns_the_whole_file() {
    let f = fixture().await;
    let (status, headers, body) = get_file_with(&f, &[]).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, file_bytes());
    assert_eq!(
        header_str(&headers, header::CONTENT_LENGTH),
        Some(FILE_LEN.to_string())
    );
    assert_eq!(
        header_str(&headers, header::CONTENT_TYPE),
        Some("application/zip".to_string())
    );
    assert!(
        headers.get(header::CONTENT_RANGE).is_none(),
        "a 200 must not carry Content-Range"
    );
}

/// The capability is a property of the resource, so it is advertised whether or
/// not the request asked for a range. A client that cannot see `Accept-Ranges`
/// on the plain response will not try to resume.
#[tokio::test]
async fn every_response_advertises_range_support_and_a_validator() {
    let f = fixture().await;

    for extra in [vec![], vec![(header::RANGE, "bytes=0-9")]] {
        let (_, headers, _) = get_file_with(&f, &extra).await;
        assert_eq!(
            header_str(&headers, header::ACCEPT_RANGES),
            Some("bytes".to_string()),
            "{extra:?}"
        );
        assert!(
            headers.get(header::ETAG).is_some(),
            "no ETag, so a resume has nothing to pin to: {extra:?}"
        );
        assert!(headers.get(header::LAST_MODIFIED).is_some(), "{extra:?}");
    }
}

/// `books.file_hash` is a non-null column the scanner already computes, so it
/// costs no I/O, survives a rescan, and survives the file moving on disk. An
/// mtime validator would do none of those.
#[tokio::test]
async fn the_etag_is_the_stored_file_hash() {
    let f = fixture().await;
    let (_, headers, _) = get_file_with(&f, &[]).await;

    assert_eq!(
        header_str(&headers, header::ETAG),
        Some(format!("\"{}\"", f.file_hash))
    );
}

// ============================================================================
// The three range forms
// ============================================================================

#[tokio::test]
async fn a_closed_range_returns_exactly_those_bytes() {
    let f = fixture().await;
    let (status, headers, body) = get_file_with(&f, &[(header::RANGE, "bytes=100-199")]).await;

    assert_eq!(status, StatusCode::PARTIAL_CONTENT);
    assert_eq!(
        header_str(&headers, header::CONTENT_RANGE),
        Some(format!("bytes 100-199/{FILE_LEN}"))
    );
    assert_eq!(
        header_str(&headers, header::CONTENT_LENGTH),
        Some("100".to_string())
    );
    assert_eq!(body, file_bytes()[100..=199]);
}

#[tokio::test]
async fn an_open_ended_range_runs_to_the_end_of_the_file() {
    let f = fixture().await;
    let (status, headers, body) = get_file_with(&f, &[(header::RANGE, "bytes=8000-")]).await;

    assert_eq!(status, StatusCode::PARTIAL_CONTENT);
    assert_eq!(
        header_str(&headers, header::CONTENT_RANGE),
        Some(format!("bytes 8000-{}/{FILE_LEN}", FILE_LEN - 1))
    );
    assert_eq!(body, file_bytes()[8000..]);
}

/// The form that reads a ZIP central directory without downloading the archive.
#[tokio::test]
async fn a_suffix_range_returns_the_tail_of_the_file() {
    let f = fixture().await;
    let (status, headers, body) = get_file_with(&f, &[(header::RANGE, "bytes=-512")]).await;

    assert_eq!(status, StatusCode::PARTIAL_CONTENT);
    assert_eq!(
        header_str(&headers, header::CONTENT_RANGE),
        Some(format!(
            "bytes {}-{}/{FILE_LEN}",
            FILE_LEN - 512,
            FILE_LEN - 1
        ))
    );
    assert_eq!(body, file_bytes()[FILE_LEN - 512..]);
}

/// The assertion that actually protects a resumed download: several ranged
/// reads, concatenated, must be byte-identical to the whole file. Offsets that
/// are individually plausible but collectively wrong show up here and nowhere
/// else.
#[tokio::test]
async fn ranged_reads_reassemble_into_the_original_file() {
    let f = fixture().await;
    let mut assembled = Vec::new();

    for spec in ["bytes=0-2047", "bytes=2048-6143", "bytes=6144-"] {
        let (status, _, body) = get_file_with(&f, &[(header::RANGE, spec)]).await;
        assert_eq!(status, StatusCode::PARTIAL_CONTENT, "{spec}");
        assembled.extend_from_slice(&body);
    }

    assert_eq!(assembled, file_bytes());
}

#[tokio::test]
async fn a_last_byte_past_the_end_is_clamped_rather_than_rejected() {
    let f = fixture().await;
    let (status, headers, body) = get_file_with(&f, &[(header::RANGE, "bytes=8100-99999")]).await;

    assert_eq!(status, StatusCode::PARTIAL_CONTENT);
    assert_eq!(
        header_str(&headers, header::CONTENT_RANGE),
        Some(format!("bytes 8100-{}/{FILE_LEN}", FILE_LEN - 1))
    );
    assert_eq!(body, file_bytes()[8100..]);
}

// ============================================================================
// Ranges that cannot be served
// ============================================================================

#[tokio::test]
async fn an_unsatisfiable_range_returns_416_with_the_full_length() {
    let f = fixture().await;
    let (status, headers, _) = get_file_with(&f, &[(header::RANGE, "bytes=99999-")]).await;

    assert_eq!(status, StatusCode::RANGE_NOT_SATISFIABLE);
    assert_eq!(
        header_str(&headers, header::CONTENT_RANGE),
        Some(format!("bytes */{FILE_LEN}")),
        "416 must tell the client the real length so it can retry correctly"
    );
}

/// RFC 9110 permits ignoring a `Range` the server does not wish to satisfy, and
/// a multi-range request would need a multipart/byteranges body no client here
/// asks for. The whole file remains a correct answer.
#[tokio::test]
async fn a_multi_range_request_falls_back_to_the_whole_file() {
    let f = fixture().await;
    let (status, headers, body) = get_file_with(&f, &[(header::RANGE, "bytes=0-99,200-299")]).await;

    assert_eq!(status, StatusCode::OK);
    assert!(headers.get(header::CONTENT_RANGE).is_none());
    assert_eq!(body, file_bytes());
}

#[tokio::test]
async fn a_malformed_range_falls_back_to_the_whole_file() {
    let f = fixture().await;
    for spec in ["bytes=not-a-range", "chapters=1-2", "bytes=500-100"] {
        let (status, _, body) = get_file_with(&f, &[(header::RANGE, spec)]).await;
        assert_eq!(status, StatusCode::OK, "{spec}");
        assert_eq!(body, file_bytes(), "{spec}");
    }
}

// ============================================================================
// Conditional requests
// ============================================================================

#[tokio::test]
async fn a_current_validator_returns_304_without_a_body() {
    let f = fixture().await;
    let etag = format!("\"{}\"", f.file_hash);
    let (status, headers, body) = get_file_with(&f, &[(header::IF_NONE_MATCH, &etag)]).await;

    assert_eq!(status, StatusCode::NOT_MODIFIED);
    assert!(body.is_empty());
    assert_eq!(header_str(&headers, header::ETAG), Some(etag));
}

#[tokio::test]
async fn a_stale_validator_returns_the_file() {
    let f = fixture().await;
    let (status, _, body) = get_file_with(&f, &[(header::IF_NONE_MATCH, "\"not-the-hash\"")]).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, file_bytes());
}

#[tokio::test]
async fn if_range_with_a_current_validator_serves_the_range() {
    let f = fixture().await;
    let etag = format!("\"{}\"", f.file_hash);
    let (status, headers, body) = get_file_with(
        &f,
        &[(header::RANGE, "bytes=0-99"), (header::IF_RANGE, &etag)],
    )
    .await;

    assert_eq!(status, StatusCode::PARTIAL_CONTENT);
    assert_eq!(
        header_str(&headers, header::CONTENT_RANGE),
        Some(format!("bytes 0-99/{FILE_LEN}"))
    );
    assert_eq!(body, file_bytes()[0..=99]);
}

/// The case that keeps a resume from corrupting a download. If the file changed
/// since the client started, splicing fresh bytes into its partial copy would
/// produce a file that is neither version, so the whole representation has to go
/// out instead.
#[tokio::test]
async fn if_range_with_a_stale_validator_returns_the_whole_file() {
    let f = fixture().await;
    let (status, headers, body) = get_file_with(
        &f,
        &[
            (header::RANGE, "bytes=0-99"),
            (header::IF_RANGE, "\"a-hash-from-before-the-rescan\""),
        ],
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert!(headers.get(header::CONTENT_RANGE).is_none());
    assert_eq!(body, file_bytes());
}

// ============================================================================
// Access control still runs first
// ============================================================================

#[tokio::test]
async fn a_range_request_still_requires_authentication() {
    let f = fixture().await;
    let mut request = get_request(&format!("/api/v1/books/{}/file", f.book_id));
    request
        .headers_mut()
        .insert(header::RANGE, "bytes=0-99".parse().unwrap());
    let (status, _) = make_request(f.app(), request).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

/// A 206 must never be a way around the checks a 200 has to pass. The book is
/// hidden from this user by the content filter, so the range must not be served
/// even though the bytes are on disk.
#[tokio::test]
async fn a_range_request_cannot_see_a_book_the_content_filter_hides() {
    let f = fixture().await;

    let password_hash = password::hash_password("user123").unwrap();
    let restricted = create_test_user("reader", "reader@example.com", &password_hash, false);
    let restricted = UserRepository::create(&f.db, &restricted).await.unwrap();

    // Tag the series and deny this user that tag, which is how the content
    // filter hides a book that is otherwise perfectly readable.
    let tag = SharingTagRepository::create(&f.db, "restricted", None)
        .await
        .unwrap();
    let book = BookRepository::get_by_id(&f.db, f.book_id)
        .await
        .unwrap()
        .unwrap();
    SharingTagRepository::add_tag_to_series(&f.db, book.series_id, tag.id)
        .await
        .unwrap();
    SharingTagRepository::set_user_grant(&f.db, restricted.id, tag.id, AccessMode::Deny)
        .await
        .unwrap();

    let token = f
        .state
        .jwt_service
        .generate_token(
            restricted.id,
            restricted.username.clone(),
            restricted.get_role(),
        )
        .unwrap();

    let mut request = get_request_with_auth(&format!("/api/v1/books/{}/file", f.book_id), &token);
    request
        .headers_mut()
        .insert(header::RANGE, "bytes=0-99".parse().unwrap());
    let (status, _) = make_request(f.app(), request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

// ============================================================================
// Content-Disposition
// ============================================================================

/// The v1 route emitted a bare `filename="..."`, which mangles any non-ASCII
/// name. The Komga copy already encoded per RFC 5987; this route now does too.
#[tokio::test]
async fn a_non_ascii_filename_is_encoded_rather_than_mangled() {
    let f = fixture_with_name("漫画 Vol 1.cbz").await;
    let (status, headers, _) = get_file_with(&f, &[]).await;

    assert_eq!(status, StatusCode::OK);
    let disposition = header_str(&headers, header::CONTENT_DISPOSITION).expect("disposition");
    assert!(
        disposition.contains("filename*=UTF-8''"),
        "expected an RFC 5987 encoded name, got {disposition}"
    );
    assert!(
        disposition.contains("%E6%BC%AB%E7%94%BB"),
        "expected the name percent-encoded, got {disposition}"
    );
}

#[tokio::test]
async fn a_partial_response_carries_the_same_disposition_as_a_full_one() {
    let f = fixture().await;
    let (_, full, _) = get_file_with(&f, &[]).await;
    let (_, partial, _) = get_file_with(&f, &[(header::RANGE, "bytes=0-9")]).await;

    assert_eq!(
        header_str(&full, header::CONTENT_DISPOSITION),
        header_str(&partial, header::CONTENT_DISPOSITION),
        "a resumed download must name the same file as the one it resumes"
    );
}
