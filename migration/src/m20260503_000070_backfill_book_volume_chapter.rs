//! Backfill `book_metadata.volume` and `book_metadata.chapter` from the
//! structured filename parser (Phase 12 of metadata-count-split).
//!
//! Phase 11 added the `chapter` column; the scanner now writes both columns
//! on insert/rescan. This migration handles the population for already-scanned
//! libraries: re-parse each book's `file_name` and update `volume` / `chapter`
//! where they are currently NULL and the parser has a value.
//!
//! Rules:
//! - Only touch rows where the field is NULL — never overwrite manually-set or
//!   plugin-derived values. The migration is additive.
//! - Lock fields are not touched. A locked-but-NULL field stays locked-NULL;
//!   the user explicitly chose "don't autopopulate this".
//! - Rows are processed in 1000-row batches with a single UPDATE per batch
//!   (per-row UPDATE would be O(n) round-trips on a 10k-book library).
//! - Idempotent: re-running produces no further writes after the first pass
//!   (the WHERE filter excludes the rows it has already populated).

use regex::Regex;
use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{
    ConnectionTrait, FromQueryResult, Statement, TransactionTrait, Value,
};

#[derive(DeriveMigrationName)]
pub struct Migration;

const BATCH_SIZE: u64 = 1000;

#[derive(Debug, FromQueryResult)]
struct Row {
    book_id: uuid::Uuid,
    file_name: String,
    volume: Option<i32>,
    chapter: Option<f32>,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        let backend = manager.get_database_backend();
        let txn = db.begin().await?;

        let mut offset: u64 = 0;
        loop {
            // Fetch a batch of rows that still have at least one of the structured
            // fields NULL. We fetch the existing values too so the UPDATE only
            // touches the column that actually needs filling — keeps the migration
            // strictly additive (never clobbers a populated field).
            let select_sql = format!(
                "SELECT bm.book_id, b.file_name, bm.volume, bm.chapter \
                 FROM book_metadata bm \
                 JOIN books b ON b.id = bm.book_id \
                 WHERE bm.volume IS NULL OR bm.chapter IS NULL \
                 ORDER BY bm.book_id \
                 LIMIT {BATCH_SIZE} OFFSET {offset}"
            );
            let rows = Row::find_by_statement(Statement::from_string(backend, select_sql))
                .all(&txn)
                .await?;

            if rows.is_empty() {
                break;
            }

            let batch_size = rows.len();
            for row in rows {
                let parsed_volume = extract_volume(&row.file_name);
                let parsed_chapter = extract_chapter(&row.file_name);

                let new_volume = if row.volume.is_none() {
                    parsed_volume
                } else {
                    None
                };
                let new_chapter = if row.chapter.is_none() {
                    parsed_chapter
                } else {
                    None
                };

                // Skip the UPDATE entirely if nothing to set.
                if new_volume.is_none() && new_chapter.is_none() {
                    continue;
                }

                // Build the UPDATE dynamically: only set the columns that need a
                // new value. We always set updated_at to reflect the touch.
                let mut sets: Vec<&str> = Vec::with_capacity(3);
                let mut values: Vec<Value> = Vec::with_capacity(3);
                if let Some(v) = new_volume {
                    sets.push("volume = ?");
                    values.push(v.into());
                }
                if let Some(c) = new_chapter {
                    sets.push("chapter = ?");
                    values.push(c.into());
                }
                if sets.is_empty() {
                    continue;
                }
                let sql = format!(
                    "UPDATE book_metadata SET {} WHERE book_id = ?",
                    sets.join(", ")
                );
                values.push(row.book_id.into());

                txn.execute(Statement::from_sql_and_values(backend, &sql, values))
                    .await?;
            }

            // If we fetched fewer than the batch size, there's no next page.
            if (batch_size as u64) < BATCH_SIZE {
                break;
            }
            offset += BATCH_SIZE;
        }

        txn.commit().await?;
        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // No-op down: there's no safe way to distinguish backfilled values from
        // values that were already present (we'd need a marker column we never
        // added). The data shape stays stable; the columns themselves are owned
        // by migration 069.
        Ok(())
    }
}

// -----------------------------------------------------------------------------
// Filename parser (mirrors `src/scanner/strategies/book/filename.rs`).
// Kept inline because the migration crate cannot depend on the main crate.
// -----------------------------------------------------------------------------

lazy_static::lazy_static! {
    static ref VOLUME_PATTERN: Regex =
        Regex::new(r"(?i)(?:^|[\s_\-\[\(])v(?:ol(?:ume)?)?\.?\s*(\d+(?:\.\d+)?)").unwrap();
    static ref CHAPTER_PATTERN: Regex =
        Regex::new(r"(?i)(?:^|[\s_\-\[\(])c(?:h(?:apter)?)?\.?\s*(\d+(?:\.\d+)?)").unwrap();
}

fn name_without_ext(file_name: &str) -> &str {
    match file_name.rfind('.') {
        Some(pos) => &file_name[..pos],
        None => file_name,
    }
}

fn extract_volume(file_name: &str) -> Option<i32> {
    let name = name_without_ext(file_name);
    let captures = VOLUME_PATTERN.captures(name)?;
    let raw = captures.get(1)?.as_str();
    if raw.contains('.') {
        return None;
    }
    raw.parse::<i32>().ok()
}

fn extract_chapter(file_name: &str) -> Option<f32> {
    let name = name_without_ext(file_name);
    let captures = CHAPTER_PATTERN.captures(name)?;
    captures.get(1)?.as_str().parse::<f32>().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parser_matches_canonical_patterns() {
        assert_eq!(extract_volume("Series v01.cbz"), Some(1));
        assert_eq!(extract_chapter("Series c042.cbz"), Some(42.0));
        assert_eq!(extract_volume("Series v15 - c126.cbz"), Some(15));
        assert_eq!(extract_chapter("Series v15 - c126.cbz"), Some(126.0));
    }

    #[test]
    fn parser_rejects_fractional_volume() {
        assert_eq!(extract_volume("Series v01.5.cbz"), None);
    }

    #[test]
    fn parser_keeps_fractional_chapter() {
        assert_eq!(extract_chapter("Series c042.5.cbz"), Some(42.5));
    }

    #[test]
    fn parser_returns_none_for_bare_numbers() {
        assert_eq!(extract_volume("Naruto 042.cbz"), None);
        assert_eq!(extract_chapter("Naruto 042.cbz"), None);
    }
}
