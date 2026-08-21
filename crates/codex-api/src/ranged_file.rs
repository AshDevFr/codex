//! Conditional, range-capable responses for files served straight off disk.
//!
//! Two things depend on this that a plain 200 cannot offer. A download that
//! drops at 90% has to resume rather than start again, which needs `Range` plus
//! a validator the client can pin the resumed request to. And a CBZ is a ZIP,
//! whose central directory sits at the end of the file and whose entries are
//! independently addressable, so a client that can issue `bytes=-65536` and
//! then a handful of entry ranges can show page one of a 200 MB volume without
//! fetching the other 199 MB.
//!
//! That second case is why the suffix form is not optional: an implementation
//! that handles only `bytes=a-b` satisfies resume and none of the partial-read
//! case.

use std::path::Path;

use axum::body::Body;
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::Response;
use chrono::{DateTime, Utc};
use httpdate::fmt_http_date;
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use tokio_util::io::ReaderStream;

/// Read buffer for the file streams below.
///
/// `ReaderStream`'s default is 4 KiB, which turns a 700 KiB page into ~170
/// chunks and a 40 MiB volume into ~10,000, each one a separate poll through
/// the body machinery. Measured against `GET /books/{id}/pages/{n}`, which
/// answers from a single buffered `Vec`, the 4 KiB default was the whole of a
/// 2-3x gap on identical bytes. 64 KiB closes it and still bounds memory per
/// in-flight response.
const STREAM_CHUNK_BYTES: usize = 64 * 1024;

use crate::error::ApiError;

/// What a `Range` header asks for, resolved against a known content length.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RangeRequest {
    /// No range header, or one this server declines to satisfy. RFC 9110 lets a
    /// server ignore a `Range` it does not wish to honour, so a syntactically
    /// invalid header, an unknown unit, and a multi-range request all land here
    /// and are answered with the whole representation.
    Full,
    /// A single satisfiable byte range, inclusive at both ends.
    Partial { start: u64, end: u64 },
    /// A range that names no byte of this representation. Answered with 416.
    Unsatisfiable,
}

/// Resolve a `Range` header value against the length of the file being served.
///
/// `len` is the full representation length. `end` in the returned `Partial` is
/// always clamped to `len - 1`, so callers never have to re-check it.
pub fn parse_range(header: Option<&str>, len: u64) -> RangeRequest {
    let Some(header) = header else {
        return RangeRequest::Full;
    };

    let Some(spec) = header.trim().strip_prefix("bytes=") else {
        // An unrecognised unit is not an error; it is a range this server does
        // not implement, and the whole representation is a valid answer to it.
        return RangeRequest::Full;
    };

    // Multiple ranges would need a multipart/byteranges body. Nothing in play
    // asks for one, and the full response remains a correct answer.
    if spec.contains(',') {
        return RangeRequest::Full;
    }

    let spec = spec.trim();
    let Some((first, last)) = spec.split_once('-') else {
        return RangeRequest::Full;
    };
    let (first, last) = (first.trim(), last.trim());

    // An empty file has no byte any range could name.
    if len == 0 {
        return if first.is_empty() && last.is_empty() {
            RangeRequest::Full
        } else {
            RangeRequest::Unsatisfiable
        };
    }

    match (first, last) {
        // `bytes=-n`: the last n bytes. A zero-length suffix names nothing.
        ("", suffix) => match suffix.parse::<u64>() {
            Ok(0) => RangeRequest::Unsatisfiable,
            Ok(n) => RangeRequest::Partial {
                start: len.saturating_sub(n),
                end: len - 1,
            },
            Err(_) => RangeRequest::Full,
        },
        // `bytes=a-`: from a to the end.
        (start, "") => match start.parse::<u64>() {
            Ok(start) if start >= len => RangeRequest::Unsatisfiable,
            Ok(start) => RangeRequest::Partial {
                start,
                end: len - 1,
            },
            Err(_) => RangeRequest::Full,
        },
        // `bytes=a-b`, with b clamped to the last byte.
        (start, end) => match (start.parse::<u64>(), end.parse::<u64>()) {
            // A last-byte-pos below first-byte-pos makes the spec invalid
            // rather than unsatisfiable, so it is ignored, not rejected.
            (Ok(start), Ok(end)) if end < start => RangeRequest::Full,
            (Ok(start), Ok(_)) if start >= len => RangeRequest::Unsatisfiable,
            (Ok(start), Ok(end)) => RangeRequest::Partial {
                start,
                end: end.min(len - 1),
            },
            _ => RangeRequest::Full,
        },
    }
}

/// Build a `Content-Disposition` naming `file_name`, encoded per RFC 6266.
///
/// Both parameters are emitted. `filename*` carries the real name; the quoted
/// `filename` is an **ASCII-only fallback** for clients that do not understand
/// the extended form.
///
/// The fallback has to be transliterated rather than passed through: a header
/// value may only hold visible ASCII, so putting raw UTF-8 in the quoted
/// parameter produces a header a client reads as latin-1 — which is precisely
/// the mangling the extended parameter exists to avoid. Shared with the
/// Komga-compatible download route so the two cannot drift.
pub fn content_disposition_attachment(file_name: &str) -> String {
    let fallback: String = file_name
        .chars()
        .map(|c| match c {
            // Anything outside printable ASCII cannot travel in the quoted
            // parameter at all, and a quote or backslash would end it early.
            '"' | '\\' => '_',
            c if c.is_ascii_graphic() || c == ' ' => c,
            _ => '_',
        })
        .collect();

    format!(
        "attachment; filename=\"{fallback}\"; filename*=UTF-8''{}",
        percent_encode_filename(file_name)
    )
}

/// Percent-encode a filename for the `filename*` parameter (RFC 5987).
///
/// Unreserved characters per RFC 3986 pass through; everything else is encoded.
fn percent_encode_filename(file_name: &str) -> String {
    let mut result = String::with_capacity(file_name.len() * 3);
    for byte in file_name.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                result.push(byte as char);
            }
            _ => {
                result.push('%');
                result.push_str(&format!("{byte:02X}"));
            }
        }
    }
    result
}

/// True when a client-supplied validator names this representation.
///
/// Handles the weak prefix and missing quotes, matching how the thumbnail and
/// PDF page routes already compare ETags.
fn etag_matches(candidate: &str, etag: &str) -> bool {
    let candidate = candidate.trim().trim_start_matches("W/");
    candidate == etag || candidate.trim_matches('"') == etag.trim_matches('"')
}

/// True when any entry of an `If-None-Match` list names this representation.
fn if_none_match_matches(value: &str, etag: &str) -> bool {
    value.trim() == "*" || value.split(',').any(|entry| etag_matches(entry, etag))
}

/// Serve `path` as a conditional, range-capable response.
///
/// Answers 304 to a current `If-None-Match`, 206 to a satisfiable `Range`, 416
/// to one that names no byte, and 200 otherwise. `Accept-Ranges: bytes` goes out
/// on every one of them, because the capability is a property of the resource
/// rather than of the request that happened to arrive.
///
/// A 206 seeks and bounds the read, so it never touches more of the file than
/// the range covers.
#[allow(clippy::too_many_arguments)]
pub async fn ranged_file_response(
    headers: &HeaderMap,
    path: &Path,
    len: u64,
    etag: &str,
    last_modified: DateTime<Utc>,
    content_type: &str,
    content_disposition: &str,
) -> Result<Response, ApiError> {
    let last_modified_str = fmt_http_date(last_modified.into());

    let base = |builder: axum::http::response::Builder| {
        builder
            .header(header::ACCEPT_RANGES, "bytes")
            .header(header::ETAG, etag)
            .header(header::LAST_MODIFIED, &last_modified_str)
    };

    // A fresh cached copy short-circuits everything below, range or not.
    if let Some(value) = headers.get(header::IF_NONE_MATCH)
        && let Ok(value) = value.to_str()
        && if_none_match_matches(value, etag)
    {
        return Ok(base(Response::builder())
            .status(StatusCode::NOT_MODIFIED)
            .body(Body::empty())
            .expect("304 response is well-formed"));
    }

    // `If-Range` makes a conditional range: honour the range only if the client
    // is resuming the same representation it started on. A stale validator has
    // to yield the whole file, because splicing new bytes into a partially
    // downloaded old file is how a resume silently corrupts a download.
    let range_header = headers
        .get(header::RANGE)
        .and_then(|value| value.to_str().ok());

    let if_range_is_current = match headers.get(header::IF_RANGE) {
        None => true,
        Some(value) => value.to_str().is_ok_and(|value| etag_matches(value, etag)),
    };

    let requested = if if_range_is_current {
        parse_range(range_header, len)
    } else {
        RangeRequest::Full
    };

    match requested {
        RangeRequest::Unsatisfiable => Ok(base(Response::builder())
            .status(StatusCode::RANGE_NOT_SATISFIABLE)
            .header(header::CONTENT_RANGE, format!("bytes */{len}"))
            .body(Body::empty())
            .expect("416 response is well-formed")),

        RangeRequest::Partial { start, end } => {
            let mut file = tokio::fs::File::open(path)
                .await
                .map_err(|e| ApiError::Internal(format!("Failed to open file: {e}")))?;
            file.seek(std::io::SeekFrom::Start(start))
                .await
                .map_err(|e| ApiError::Internal(format!("Failed to seek file: {e}")))?;

            let length = end - start + 1;
            let body = Body::from_stream(ReaderStream::with_capacity(
                file.take(length),
                STREAM_CHUNK_BYTES,
            ));

            Ok(base(Response::builder())
                .status(StatusCode::PARTIAL_CONTENT)
                .header(header::CONTENT_TYPE, content_type)
                .header(header::CONTENT_LENGTH, length)
                .header(header::CONTENT_RANGE, format!("bytes {start}-{end}/{len}"))
                .header(header::CONTENT_DISPOSITION, content_disposition)
                .body(body)
                .expect("206 response is well-formed"))
        }

        RangeRequest::Full => {
            let file = tokio::fs::File::open(path)
                .await
                .map_err(|e| ApiError::Internal(format!("Failed to open file: {e}")))?;

            Ok(base(Response::builder())
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, content_type)
                .header(header::CONTENT_LENGTH, len)
                .header(header::CONTENT_DISPOSITION, content_disposition)
                .body(Body::from_stream(ReaderStream::with_capacity(
                    file,
                    STREAM_CHUNK_BYTES,
                )))
                .expect("200 response is well-formed"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LEN: u64 = 1000;

    #[test]
    fn absent_range_serves_the_whole_representation() {
        assert_eq!(parse_range(None, LEN), RangeRequest::Full);
    }

    #[test]
    fn closed_ranges_resolve_to_their_bounds() {
        for (header, start, end) in [
            ("bytes=0-99", 0, 99),
            ("bytes=0-0", 0, 0),
            ("bytes=999-999", 999, 999),
            ("bytes=500-600", 500, 600),
        ] {
            assert_eq!(
                parse_range(Some(header), LEN),
                RangeRequest::Partial { start, end },
                "{header}"
            );
        }
    }

    #[test]
    fn a_last_byte_past_the_end_is_clamped_rather_than_rejected() {
        assert_eq!(
            parse_range(Some("bytes=990-2000"), LEN),
            RangeRequest::Partial {
                start: 990,
                end: 999
            }
        );
    }

    #[test]
    fn open_ended_ranges_run_to_the_last_byte() {
        for (header, start) in [("bytes=0-", 0), ("bytes=500-", 500), ("bytes=999-", 999)] {
            assert_eq!(
                parse_range(Some(header), LEN),
                RangeRequest::Partial { start, end: 999 },
                "{header}"
            );
        }
    }

    /// The form a client uses to read a ZIP central directory without
    /// downloading the archive.
    #[test]
    fn suffix_ranges_count_back_from_the_end() {
        assert_eq!(
            parse_range(Some("bytes=-100"), LEN),
            RangeRequest::Partial {
                start: 900,
                end: 999
            }
        );
        assert_eq!(
            parse_range(Some("bytes=-1"), LEN),
            RangeRequest::Partial {
                start: 999,
                end: 999
            }
        );
    }

    #[test]
    fn a_suffix_longer_than_the_file_yields_the_whole_file() {
        assert_eq!(
            parse_range(Some("bytes=-2000"), LEN),
            RangeRequest::Partial { start: 0, end: 999 }
        );
    }

    #[test]
    fn ranges_beyond_the_end_are_unsatisfiable() {
        for header in ["bytes=1000-", "bytes=1000-1200", "bytes=5000-6000"] {
            assert_eq!(
                parse_range(Some(header), LEN),
                RangeRequest::Unsatisfiable,
                "{header}"
            );
        }
    }

    /// A zero-length suffix names no byte, so it is unsatisfiable rather than
    /// an empty success.
    #[test]
    fn a_zero_length_suffix_is_unsatisfiable() {
        assert_eq!(
            parse_range(Some("bytes=-0"), LEN),
            RangeRequest::Unsatisfiable
        );
    }

    #[test]
    fn an_empty_file_can_satisfy_no_range() {
        for header in ["bytes=0-", "bytes=0-0", "bytes=-1"] {
            assert_eq!(
                parse_range(Some(header), 0),
                RangeRequest::Unsatisfiable,
                "{header}"
            );
        }
        assert_eq!(parse_range(None, 0), RangeRequest::Full);
    }

    /// RFC 9110: a last-byte-pos below first-byte-pos makes the spec invalid,
    /// and an invalid `Range` is ignored rather than rejected.
    #[test]
    fn an_inverted_range_is_ignored_rather_than_rejected() {
        assert_eq!(parse_range(Some("bytes=100-50"), LEN), RangeRequest::Full);
    }

    #[test]
    fn malformed_and_unsupported_ranges_serve_the_whole_representation() {
        for header in [
            "bytes=abc",
            "bytes=",
            "bytes=-",
            "bytes=1-abc",
            "items=0-99",
            "0-99",
            "",
        ] {
            assert_eq!(
                parse_range(Some(header), LEN),
                RangeRequest::Full,
                "{header}"
            );
        }
    }

    /// Multiple ranges need a `multipart/byteranges` body. RFC 9110 permits
    /// ignoring a range the server does not wish to satisfy, so the whole file
    /// is a correct answer.
    #[test]
    fn multi_range_requests_serve_the_whole_representation() {
        assert_eq!(
            parse_range(Some("bytes=0-99,200-299"), LEN),
            RangeRequest::Full
        );
    }

    #[test]
    fn ascii_filenames_pass_through_the_encoder_unchanged() {
        assert_eq!(percent_encode_filename("test.cbz"), "test.cbz");
        assert_eq!(
            percent_encode_filename("my-file_v1.0.epub"),
            "my-file_v1.0.epub"
        );
    }

    #[test]
    fn spaces_and_special_characters_are_encoded() {
        assert_eq!(percent_encode_filename("My File.cbz"), "My%20File.cbz");
        assert_eq!(percent_encode_filename("file[1].cbz"), "file%5B1%5D.cbz");
    }

    #[test]
    fn non_ascii_filenames_are_encoded_rather_than_dropped() {
        let encoded = percent_encode_filename("漫画 Vol 1.cbz");
        assert!(encoded.contains('%'));
        assert!(encoded.ends_with(".cbz"));
    }

    /// Both parameters go out: an ASCII fallback for old clients, the encoded
    /// real name for everything else.
    #[test]
    fn the_disposition_carries_an_ascii_fallback_and_the_encoded_name() {
        let disposition = content_disposition_attachment("漫画.cbz");
        assert_eq!(
            disposition,
            "attachment; filename=\"__.cbz\"; filename*=UTF-8''%E6%BC%AB%E7%94%BB.cbz"
        );
    }

    /// A header value may only hold visible ASCII. A disposition that is not
    /// ASCII is one the client reads as latin-1, which is the mangling the
    /// extended parameter exists to prevent.
    #[test]
    fn the_disposition_is_always_ascii() {
        for name in [
            "漫画 Vol 1.cbz",
            "café.epub",
            "naïve\u{7f}.pdf",
            "plain.cbz",
        ] {
            let disposition = content_disposition_attachment(name);
            assert!(
                disposition.is_ascii(),
                "{name} produced a non-ASCII header value: {disposition}"
            );
        }
    }

    /// A quote or backslash would close the quoted parameter early.
    #[test]
    fn quotes_cannot_escape_the_quoted_parameter() {
        let disposition = content_disposition_attachment("a\"b\\c.cbz");
        assert!(
            disposition.starts_with("attachment; filename=\"a_b_c.cbz\""),
            "{disposition}"
        );
        assert!(disposition.contains("filename*=UTF-8''a%22b%5Cc.cbz"));
    }

    #[test]
    fn an_ascii_filename_survives_intact_in_both_parameters() {
        assert_eq!(
            content_disposition_attachment("My Volume 1.cbz"),
            "attachment; filename=\"My Volume 1.cbz\"; filename*=UTF-8''My%20Volume%201.cbz"
        );
    }

    #[test]
    fn etags_match_across_weak_prefixes_and_missing_quotes() {
        assert!(etag_matches("\"abc\"", "\"abc\""));
        assert!(etag_matches("W/\"abc\"", "\"abc\""));
        assert!(etag_matches("abc", "\"abc\""));
        assert!(!etag_matches("\"def\"", "\"abc\""));
    }

    #[test]
    fn if_none_match_accepts_a_list_and_the_wildcard() {
        assert!(if_none_match_matches("*", "\"abc\""));
        assert!(if_none_match_matches("\"xyz\", \"abc\"", "\"abc\""));
        assert!(!if_none_match_matches("\"xyz\", \"def\"", "\"abc\""));
    }
}
