//! Consolidate individual author columns into authors_json
//!
//! Book metadata previously had 7 individual author columns (writer, penciller,
//! inker, colorist, letterer, cover_artist, editor) plus their lock fields.
//! This migration:
//! 1. Adds authors_json + authors_json_lock to series_metadata
//! 2. Backfills book_metadata.authors_json from individual columns
//! 3. Consolidates individual lock fields into authors_json_lock
//! 4. Drops the 14 individual author/lock columns from book_metadata

use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{ConnectionTrait, DatabaseBackend, Statement};

use crate::m20260103_000006_create_series_metadata::SeriesMetadata;
use crate::m20260103_000014_create_book_metadata::BookMetadata;

/// Check if a column exists on a table (works for both SQLite and PostgreSQL).
/// This makes the migration idempotent in case a previous run partially applied.
async fn has_column(
    db: &impl ConnectionTrait,
    backend: DatabaseBackend,
    table: &str,
    column: &str,
) -> Result<bool, DbErr> {
    let sql = match backend {
        DatabaseBackend::Sqlite => {
            format!(
                "SELECT COUNT(*) as cnt FROM pragma_table_info('{table}') WHERE name = '{column}'"
            )
        }
        DatabaseBackend::Postgres => {
            format!(
                "SELECT CAST(COUNT(*) AS INT) as cnt FROM information_schema.columns WHERE table_name = '{table}' AND column_name = '{column}'"
            )
        }
        _ => return Err(DbErr::Custom("Unsupported database backend".to_owned())),
    };
    let row = db
        .query_one(Statement::from_string(backend, sql))
        .await?
        .ok_or_else(|| DbErr::Custom("Expected a row from column check query".to_owned()))?;
    let count: i32 = row.try_get("", "cnt")?;
    Ok(count > 0)
}

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        let backend = manager.get_database_backend();

        // Step 1: Add authors_json + authors_json_lock to series_metadata
        // (idempotent — skip if columns already exist from a partial previous run)
        if !has_column(db, backend, "series_metadata", "authors_json").await? {
            manager
                .alter_table(
                    Table::alter()
                        .table(SeriesMetadata::Table)
                        .add_column(ColumnDef::new(Alias::new("authors_json")).text())
                        .to_owned(),
                )
                .await?;
        }

        if !has_column(db, backend, "series_metadata", "authors_json_lock").await? {
            manager
                .alter_table(
                    Table::alter()
                        .table(SeriesMetadata::Table)
                        .add_column(
                            ColumnDef::new(Alias::new("authors_json_lock"))
                                .boolean()
                                .not_null()
                                .default(false),
                        )
                        .to_owned(),
                )
                .await?;
        }

        // Step 2: Backfill authors_json from individual columns using Rust logic
        // We query rows that have individual author fields but no authors_json,
        // then build JSON from the individual fields (handling comma-separated names).
        // (only run if individual columns still exist — they may have been dropped in a partial run)
        let has_individual_columns = has_column(db, backend, "book_metadata", "writer").await?;

        if !has_individual_columns {
            // Individual columns already dropped — backfill was completed in a prior partial run.
            // Skip steps 2-4 entirely.
            return Ok(());
        }

        // SQLite stores UUIDs as 16-byte blobs; CAST(... AS TEXT) produces invalid UTF-8.
        // Use HEX() on SQLite to get a readable hex string, CAST on PostgreSQL where UUID is native.
        let book_id_expr = match backend {
            DatabaseBackend::Sqlite => "HEX(book_id)",
            _ => "CAST(book_id AS TEXT)",
        };
        let rows = db
            .query_all(Statement::from_string(
                backend,
                format!(
                    "SELECT {book_id_expr} AS book_id, writer, penciller, inker, colorist, letterer, cover_artist, editor
                     FROM book_metadata
                     WHERE authors_json IS NULL
                       AND (writer IS NOT NULL OR penciller IS NOT NULL OR inker IS NOT NULL
                            OR colorist IS NOT NULL OR letterer IS NOT NULL OR cover_artist IS NOT NULL
                            OR editor IS NOT NULL)"
                ),
            ))
            .await?;

        for row in &rows {
            let book_id_raw: String = row.try_get("", "book_id")?;
            let writer: Option<String> = row.try_get("", "writer").ok().flatten();
            let penciller: Option<String> = row.try_get("", "penciller").ok().flatten();
            let inker: Option<String> = row.try_get("", "inker").ok().flatten();
            let colorist: Option<String> = row.try_get("", "colorist").ok().flatten();
            let letterer: Option<String> = row.try_get("", "letterer").ok().flatten();
            let cover_artist: Option<String> = row.try_get("", "cover_artist").ok().flatten();
            let editor: Option<String> = row.try_get("", "editor").ok().flatten();

            let role_fields: &[(&str, &Option<String>)] = &[
                ("writer", &writer),
                ("penciller", &penciller),
                ("inker", &inker),
                ("colorist", &colorist),
                ("letterer", &letterer),
                ("cover_artist", &cover_artist),
                ("editor", &editor),
            ];

            let mut authors = Vec::new();
            for (role, value) in role_fields {
                if let Some(names) = value {
                    for name in names.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()) {
                        // Build JSON object manually to avoid serde dependency in migration
                        let escaped_name = name.replace('\\', "\\\\").replace('"', "\\\"");
                        authors.push(format!(
                            r#"{{"name":"{}","role":"{}"}}"#,
                            escaped_name, role
                        ));
                    }
                }
            }

            if !authors.is_empty() {
                let json = format!("[{}]", authors.join(","));
                let escaped_json = json.replace('\'', "''");
                // SQLite: book_id is a blob, so match with X'...' hex literal.
                // PostgreSQL: book_id is a native UUID, so match with '...'::uuid.
                let where_clause = match backend {
                    DatabaseBackend::Sqlite => format!("book_id = X'{book_id_raw}'"),
                    _ => format!("book_id = '{book_id_raw}'"),
                };
                let sql = format!(
                    "UPDATE book_metadata SET authors_json = '{escaped_json}' WHERE {where_clause}"
                );
                db.execute(Statement::from_string(backend, sql)).await?;
            }
        }

        // Step 3: Consolidate locks — if any individual lock is true, set authors_json_lock
        // (only run if old lock columns still exist)
        if has_column(db, backend, "book_metadata", "writer_lock").await? {
            db.execute(Statement::from_string(
                backend,
                "UPDATE book_metadata SET authors_json_lock = TRUE
                 WHERE authors_json_lock = FALSE
                   AND (writer_lock = TRUE OR penciller_lock = TRUE OR inker_lock = TRUE
                        OR colorist_lock = TRUE OR letterer_lock = TRUE
                        OR cover_artist_lock = TRUE OR editor_lock = TRUE)"
                    .to_owned(),
            ))
            .await?;
        }

        // Step 4: Drop 14 individual author columns (7 lock + 7 data)
        // (idempotent — skip columns already dropped from a partial previous run)
        let columns_to_drop = [
            "writer_lock",
            "penciller_lock",
            "inker_lock",
            "colorist_lock",
            "letterer_lock",
            "cover_artist_lock",
            "editor_lock",
            "writer",
            "penciller",
            "inker",
            "colorist",
            "letterer",
            "cover_artist",
            "editor",
        ];

        for col in &columns_to_drop {
            if has_column(db, backend, "book_metadata", col).await? {
                manager
                    .alter_table(
                        Table::alter()
                            .table(BookMetadata::Table)
                            .drop_column(Alias::new(*col))
                            .to_owned(),
                    )
                    .await?;
            }
        }

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Re-add individual author columns
        let author_columns = [
            "writer",
            "penciller",
            "inker",
            "colorist",
            "letterer",
            "cover_artist",
            "editor",
        ];

        for col in &author_columns {
            manager
                .alter_table(
                    Table::alter()
                        .table(BookMetadata::Table)
                        .add_column(ColumnDef::new(Alias::new(*col)).string())
                        .to_owned(),
                )
                .await?;
        }

        // Re-add lock columns
        for col in &author_columns {
            let lock_col = format!("{}_lock", col);
            manager
                .alter_table(
                    Table::alter()
                        .table(BookMetadata::Table)
                        .add_column(
                            ColumnDef::new(Alias::new(&lock_col))
                                .boolean()
                                .not_null()
                                .default(false),
                        )
                        .to_owned(),
                )
                .await?;
        }

        // Drop series_metadata author columns
        manager
            .alter_table(
                Table::alter()
                    .table(SeriesMetadata::Table)
                    .drop_column(Alias::new("authors_json_lock"))
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(SeriesMetadata::Table)
                    .drop_column(Alias::new("authors_json"))
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}
