//! Rewrite on-disk file paths stored in the database after an import.
//!
//! Codex stores image paths with the configured base directory baked in and
//! reads them back with `fs::read(&path)` (no re-join). When an archive is
//! imported into an instance whose `files.*_dir` differ from the source's, the
//! stored paths must be re-rooted from the source base dir (recorded in the
//! manifest) to the target base dir, or the images won't resolve.
//!
//! Affected columns:
//! - `books.thumbnail_path`  — under `files.thumbnail_dir`
//! - `book_covers.path`      — under `files.uploads_dir`
//! - `series_covers.path`    — under `files.uploads_dir`

use anyhow::Result;
use codex_db::entities::{book_covers, books, series_covers};
use sea_orm::sea_query::Expr;
use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QuerySelect};
use uuid::Uuid;

/// How many stored paths were rewritten, per column family.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RerootStats {
    pub thumbnails: u64,
    pub covers: u64,
}

/// Rewrite `path` if it sits under `from`, replacing that prefix with `to`.
/// Returns `None` when there is nothing to do (prefix doesn't match, or the
/// base dirs are identical). Matching respects path boundaries so
/// `data/thumb` does not match `data/thumbnails`.
pub fn reroot_path(path: &str, from: &str, to: &str) -> Option<String> {
    let from = from.trim_end_matches('/');
    let to = to.trim_end_matches('/');
    if from == to || from.is_empty() {
        return None;
    }
    let rest = path.strip_prefix(from)?;
    if rest.is_empty() || rest.starts_with('/') {
        Some(format!("{to}{rest}"))
    } else {
        None
    }
}

/// Re-root all affected columns. `thumbnail` and `uploads` are each an optional
/// `(from, to)` base-dir pair; `None` skips that family.
pub async fn reroot_all<C: ConnectionTrait>(
    conn: &C,
    thumbnail: Option<(&str, &str)>,
    uploads: Option<(&str, &str)>,
) -> Result<RerootStats> {
    let mut stats = RerootStats::default();

    if let Some((from, to)) = thumbnail {
        stats.thumbnails = reroot_books_thumbnails(conn, from, to).await?;
    }
    if let Some((from, to)) = uploads {
        stats.covers = reroot_book_covers(conn, from, to).await?
            + reroot_series_covers(conn, from, to).await?;
    }

    Ok(stats)
}

async fn reroot_books_thumbnails<C: ConnectionTrait>(
    conn: &C,
    from: &str,
    to: &str,
) -> Result<u64> {
    let rows: Vec<(Uuid, Option<String>)> = books::Entity::find()
        .select_only()
        .column(books::Column::Id)
        .column(books::Column::ThumbnailPath)
        .filter(books::Column::ThumbnailPath.is_not_null())
        .into_tuple()
        .all(conn)
        .await?;

    let mut n = 0;
    for (id, path) in rows {
        let Some(path) = path else { continue };
        if let Some(new) = reroot_path(&path, from, to) {
            books::Entity::update_many()
                .col_expr(books::Column::ThumbnailPath, Expr::value(new))
                .filter(books::Column::Id.eq(id))
                .exec(conn)
                .await?;
            n += 1;
        }
    }
    Ok(n)
}

async fn reroot_book_covers<C: ConnectionTrait>(conn: &C, from: &str, to: &str) -> Result<u64> {
    let rows: Vec<(Uuid, String)> = book_covers::Entity::find()
        .select_only()
        .column(book_covers::Column::Id)
        .column(book_covers::Column::Path)
        .into_tuple()
        .all(conn)
        .await?;

    let mut n = 0;
    for (id, path) in rows {
        if let Some(new) = reroot_path(&path, from, to) {
            book_covers::Entity::update_many()
                .col_expr(book_covers::Column::Path, Expr::value(new))
                .filter(book_covers::Column::Id.eq(id))
                .exec(conn)
                .await?;
            n += 1;
        }
    }
    Ok(n)
}

async fn reroot_series_covers<C: ConnectionTrait>(conn: &C, from: &str, to: &str) -> Result<u64> {
    let rows: Vec<(Uuid, String)> = series_covers::Entity::find()
        .select_only()
        .column(series_covers::Column::Id)
        .column(series_covers::Column::Path)
        .into_tuple()
        .all(conn)
        .await?;

    let mut n = 0;
    for (id, path) in rows {
        if let Some(new) = reroot_path(&path, from, to) {
            series_covers::Entity::update_many()
                .col_expr(series_covers::Column::Path, Expr::value(new))
                .filter(series_covers::Column::Id.eq(id))
                .exec(conn)
                .await?;
            n += 1;
        }
    }
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::reroot_path;

    #[test]
    fn rewrites_matching_prefix() {
        assert_eq!(
            reroot_path(
                "data/thumbnails/books/ab/x.jpg",
                "data/thumbnails",
                "/srv/thumbs"
            ),
            Some("/srv/thumbs/books/ab/x.jpg".to_string())
        );
    }

    #[test]
    fn respects_path_boundaries() {
        // `data/thumb` must NOT match `data/thumbnails/...`.
        assert_eq!(
            reroot_path("data/thumbnails/x.jpg", "data/thumb", "/new"),
            None
        );
    }

    #[test]
    fn ignores_trailing_slashes_and_identical_dirs() {
        assert_eq!(
            reroot_path("/a/b/c.jpg", "/a/b/", "/x/y"),
            Some("/x/y/c.jpg".to_string())
        );
        assert_eq!(reroot_path("/a/b/c.jpg", "/a/b", "/a/b"), None);
    }

    #[test]
    fn returns_none_when_prefix_absent() {
        assert_eq!(reroot_path("/other/c.jpg", "/a/b", "/x/y"), None);
    }
}
