//! Content visibility for handlers.
//!
//! Every handler that returns, streams or changes something tied to a series
//! or a book goes through these helpers, so what a restricted reader can see
//! is decided in one place. A hidden item answers exactly like a missing one
//! (404), so its existence cannot be probed.
//!
//! The filter comes from the per-process cache (see
//! `codex_services::content_filter_cache`), which matters on hot paths such as
//! page images.

use crate::error::ApiError;
use codex_db::entities::{books, series};
use codex_db::repositories::{BookRepository, SeriesRepository, SeriesVisibility};
use codex_services::content_filter::ContentFilter;
use sea_orm::DatabaseConnection;
use std::sync::Arc;
use uuid::Uuid;

/// The user's content filter.
pub async fn content_filter(
    db: &DatabaseConnection,
    user_id: Uuid,
) -> Result<Arc<ContentFilter>, ApiError> {
    ContentFilter::cached_for_user(db, user_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to load content filter: {}", e)))
}

/// The SQL-level visibility constraint for the user, or `None` when they see
/// everything.
pub async fn user_visibility(
    db: &DatabaseConnection,
    user_id: Uuid,
) -> Result<Option<SeriesVisibility>, ApiError> {
    Ok(content_filter(db, user_id).await?.to_visibility())
}

/// Fail with 404 unless the series exists and the user can see it.
pub async fn require_visible_series(
    db: &DatabaseConnection,
    user_id: Uuid,
    series_id: Uuid,
) -> Result<series::Model, ApiError> {
    let series = SeriesRepository::get_by_id(db, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch series: {}", e)))?
        .ok_or_else(series_not_found)?;
    if !content_filter(db, user_id)
        .await?
        .is_series_visible(series.id)
    {
        return Err(series_not_found());
    }
    Ok(series)
}

/// Fail with 404 unless the book exists and the user can see its series.
pub async fn require_visible_book(
    db: &DatabaseConnection,
    user_id: Uuid,
    book_id: Uuid,
) -> Result<books::Model, ApiError> {
    let book = BookRepository::get_by_id(db, book_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch book: {}", e)))?
        .ok_or_else(book_not_found)?;
    if !content_filter(db, user_id)
        .await?
        .is_book_visible(book.series_id)
    {
        return Err(book_not_found());
    }
    Ok(book)
}

/// Fail with 404 unless the user can see the book, when the handler does not
/// need the book itself.
///
/// For a user with no restrictions this runs no query at all (the filter is
/// cached), which matters on paths that are meant to stay off the database,
/// such as thumbnails served from the disk cache. A book that does not exist
/// is left for the handler to report.
pub async fn ensure_book_id_visible(
    db: &DatabaseConnection,
    user_id: Uuid,
    book_id: Uuid,
) -> Result<(), ApiError> {
    let filter = content_filter(db, user_id).await?;
    if !filter.has_restrictions {
        return Ok(());
    }
    let book = BookRepository::get_by_id(db, book_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch book: {}", e)))?;
    match book {
        Some(book) if !filter.is_book_visible(book.series_id) => Err(book_not_found()),
        _ => Ok(()),
    }
}

/// Fail with 404 unless the user can see this already-loaded book.
pub async fn ensure_book_visible(
    db: &DatabaseConnection,
    user_id: Uuid,
    book: &books::Model,
) -> Result<(), ApiError> {
    if content_filter(db, user_id)
        .await?
        .is_book_visible(book.series_id)
    {
        Ok(())
    } else {
        Err(book_not_found())
    }
}

/// Fail with 404 unless the user can see this series id (without loading it).
pub async fn ensure_series_visible(
    db: &DatabaseConnection,
    user_id: Uuid,
    series_id: Uuid,
) -> Result<(), ApiError> {
    if content_filter(db, user_id)
        .await?
        .is_series_visible(series_id)
    {
        Ok(())
    } else {
        Err(series_not_found())
    }
}

/// Fail with 404 unless the user can see this library.
pub async fn ensure_library_visible(
    db: &DatabaseConnection,
    user_id: Uuid,
    library_id: Uuid,
) -> Result<(), ApiError> {
    if content_filter(db, user_id)
        .await?
        .is_library_visible(library_id)
    {
        Ok(())
    } else {
        Err(ApiError::NotFound("Library not found".to_string()))
    }
}

/// Whether an entity change event may be sent to this user.
///
/// An event about a series, book or library the user cannot see is withheld,
/// so the event streams never announce hidden content. Fails closed: if
/// visibility cannot be determined, the event is withheld.
pub async fn entity_event_visible(
    db: &DatabaseConnection,
    user_id: Uuid,
    event: &codex_events::EntityChangeEvent,
) -> bool {
    let Ok(filter) = content_filter(db, user_id).await else {
        return false;
    };
    if !filter.has_restrictions {
        return true;
    }
    content_visible(
        db,
        &filter,
        event.library_id(),
        event.series_id(),
        event.book_id(),
    )
    .await
}

/// Whether a task progress event may be sent to this user. Same rules as
/// [`entity_event_visible`].
pub async fn task_event_visible(
    db: &DatabaseConnection,
    user_id: Uuid,
    event: &codex_events::TaskProgressEvent,
) -> bool {
    let Ok(filter) = content_filter(db, user_id).await else {
        return false;
    };
    if !filter.has_restrictions {
        return true;
    }
    content_visible(
        db,
        &filter,
        event.library_id,
        event.series_id,
        event.book_id,
    )
    .await
}

async fn content_visible(
    db: &DatabaseConnection,
    filter: &ContentFilter,
    library_id: Option<Uuid>,
    series_id: Option<Uuid>,
    book_id: Option<Uuid>,
) -> bool {
    if let Some(library_id) = library_id
        && !filter.is_library_visible(library_id)
    {
        return false;
    }
    if let Some(series_id) = series_id {
        return filter.is_series_visible(series_id);
    }
    if let Some(book_id) = book_id {
        // Only book cover events name a book without its series.
        return match BookRepository::get_by_id(db, book_id).await {
            Ok(Some(book)) => filter.is_book_visible(book.series_id),
            _ => false,
        };
    }
    true
}

fn series_not_found() -> ApiError {
    ApiError::NotFound("Series not found".to_string())
}

fn book_not_found() -> ApiError {
    ApiError::NotFound("Book not found".to_string())
}

#[cfg(test)]
mod guard {
    //! Keeps the rule "every handler that takes a series, book or library id
    //! checks visibility" true as handlers are added. Visibility used to be a
    //! per-handler convention that most handlers forgot; this test is what
    //! stops that from coming back.

    use std::path::{Path, PathBuf};

    /// Names that, appearing in a handler body, mean it enforces visibility.
    const CHECKS: &[&str] = &[
        "ContentFilter",
        "visibility::",
        "require_visible_",
        "ensure_series_visible",
        "ensure_book_visible",
        "ensure_book_id_visible",
        "ensure_library_visible",
        "user_visibility",
        "to_visibility",
        "find_book_by_hash",
    ];

    /// Path parameters that name content.
    const CONTENT_IDS: &[&str] = &["series_id", "book_id", "library_id"];

    fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                rust_files(&path, out);
            } else if path.extension().is_some_and(|e| e == "rs") {
                out.push(path);
            }
        }
    }

    /// The names bound by the handler's `Path(...)` extractor, if any.
    fn path_params(signature: &str) -> Vec<String> {
        let Some(start) = signature.find("Path(") else {
            return Vec::new();
        };
        let rest = &signature[start + "Path(".len()..];
        let Some(end) = rest.find("):") else {
            return Vec::new();
        };
        rest[..end]
            .trim_start_matches('(')
            .trim_end_matches(')')
            .split(',')
            .map(|n| n.trim().to_string())
            .collect()
    }

    #[test]
    fn every_content_id_handler_checks_visibility() {
        let routes = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/routes");
        let mut files = Vec::new();
        rust_files(&routes, &mut files);

        let mut missing = Vec::new();
        for file in files {
            let source = std::fs::read_to_string(&file).unwrap();
            let mut rest = source.as_str();
            while let Some(at) = rest.find("pub async fn ") {
                let from = &rest[at..];
                let Some(body_start) = from.find("{\n") else {
                    break;
                };
                let body_end = from[body_start..]
                    .find("\n}\n")
                    .map(|e| body_start + e)
                    .unwrap_or(from.len());
                let signature = &from[..body_start];
                let body = &from[body_start..body_end];
                let name: String = from["pub async fn ".len()..]
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .collect();

                let takes_content_id = path_params(signature)
                    .iter()
                    .any(|p| CONTENT_IDS.contains(&p.as_str()));
                // Admins are never restricted, so admin-only handlers are exempt.
                let admin_only = body.contains("Permission::SystemAdmin");
                let checks = CHECKS.iter().any(|c| body.contains(c));
                if takes_content_id && !admin_only && !checks {
                    let relative = file.strip_prefix(&routes).unwrap_or(&file);
                    missing.push(format!("{}: {}", relative.display(), name));
                }
                rest = &from[body_end.min(from.len())..];
            }
        }

        assert!(
            missing.is_empty(),
            "These handlers take a series, book or library id but never check that the user \
             can see it. Call crate::visibility::ensure_series_visible / \
             ensure_book_id_visible / ensure_library_visible first:\n{}",
            missing.join("\n")
        );
    }
}
