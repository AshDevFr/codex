//! Generic, entity-driven transfer primitives.
//!
//! Every function here is generic over a SeaORM [`EntityTrait`]. Because
//! SeaORM performs the engine-specific type mapping (UUID blob ↔ native
//! `uuid`, text JSON ↔ JSONB, 0/1 ↔ `bool`, timestamps) when it materializes a
//! typed `Model`, moving rows as `Model` values is correct by construction
//! across SQLite and PostgreSQL — no hand-written cast rules, no raw bytes.

use std::io::{BufRead, Write};

use anyhow::Result;
use futures::stream::BoxStream;
use futures::{StreamExt, TryStreamExt};
use sea_orm::sea_query::Iden;
use sea_orm::{
    ColumnTrait, ColumnType, ConnectionTrait, DatabaseBackend, DbErr, EntityTrait, IntoActiveModel,
    Iterable, PaginatorTrait, Statement, StreamTrait,
};
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::progress::{Progress, ROW_REPORT_INTERVAL};

/// Default insert batch size. Bounds memory and round-trips while staying well
/// under parameter limits for the widest tables.
pub const DEFAULT_BATCH_SIZE: usize = 1000;

/// Open a `Model` stream over `E`'s rows, normalizing UUID storage on SQLite.
///
/// SeaORM (via sqlx) reads a SQLite `Uuid` column strictly as a 16-byte blob.
/// Databases written by older toolchains may instead store UUIDs as 36-char
/// hyphenated text, which then fails to decode. For a SQLite source we
/// therefore read through a query that coerces each UUID column back to a blob
/// (text → `unhex(replace(col,'-',''))`, blobs pass through untouched), so both
/// storage formats decode identically. Other backends use the normal path.
async fn open_source_stream<'a, E, C>(
    conn: &'a C,
) -> Result<BoxStream<'a, std::result::Result<E::Model, DbErr>>>
where
    E: EntityTrait,
    C: ConnectionTrait + StreamTrait,
{
    if conn.get_database_backend() == DatabaseBackend::Sqlite {
        let stmt = sqlite_uuid_normalizing_select::<E>();
        Ok(E::find().from_raw_sql(stmt).stream(conn).await?.boxed())
    } else {
        Ok(E::find().stream(conn).await?.boxed())
    }
}

/// Build `SELECT <cols> FROM <table>` for SQLite where every UUID column is
/// coerced to a 16-byte blob regardless of whether it was stored as a blob or
/// as hyphenated text. Requires SQLite ≥ 3.41 for `unhex` (bundled with sqlx).
fn sqlite_uuid_normalizing_select<E: EntityTrait>() -> Statement {
    let cols: Vec<String> = <E::Column as Iterable>::iter()
        .map(|col| {
            let name = iden_string(&col);
            if matches!(col.def().get_column_type(), ColumnType::Uuid) {
                format!(
                    "CASE WHEN typeof(\"{name}\") = 'text' \
                     THEN unhex(replace(\"{name}\", '-', '')) ELSE \"{name}\" END AS \"{name}\""
                )
            } else {
                format!("\"{name}\"")
            }
        })
        .collect();

    let table = iden_string(&E::default());
    let sql = format!("SELECT {} FROM \"{table}\"", cols.join(", "));
    Statement::from_string(DatabaseBackend::Sqlite, sql)
}

/// The unquoted identifier string for a column or table.
fn iden_string<I: Iden>(iden: &I) -> String {
    let mut buf = String::new();
    iden.unquoted(&mut buf);
    buf
}

/// Stream every row of `E` and write it as one NDJSON line to `out`.
/// Returns the number of rows written.
pub async fn dump_table<E, C, W>(conn: &C, out: &mut W, progress: Progress) -> Result<u64>
where
    E: EntityTrait,
    E::Model: Serialize,
    C: ConnectionTrait + StreamTrait,
    W: Write,
{
    let table = iden_string(&E::default());
    let mut stream = open_source_stream::<E, C>(conn).await?;
    let mut count = 0u64;
    let mut next_report = ROW_REPORT_INTERVAL;
    while let Some(model) = stream.try_next().await? {
        serde_json::to_writer(&mut *out, &model)?;
        out.write_all(b"\n")?;
        count += 1;
        if count >= next_report {
            progress.table_rows(&table, count);
            next_report += ROW_REPORT_INTERVAL;
        }
    }
    Ok(count)
}

/// Read NDJSON lines from `reader`, deserialize each into `E::Model`, and
/// insert them into `conn` in batches of `batch_size`. Returns rows inserted.
pub async fn load_table<E, C, R>(
    conn: &C,
    reader: R,
    batch_size: usize,
    progress: Progress,
) -> Result<u64>
where
    E: EntityTrait,
    E::Model: DeserializeOwned + IntoActiveModel<E::ActiveModel>,
    C: ConnectionTrait,
    R: BufRead,
{
    let table = iden_string(&E::default());
    let batch_size = safe_batch_size::<E>(conn.get_database_backend(), batch_size);
    let mut batch: Vec<E::ActiveModel> = Vec::with_capacity(batch_size);
    let mut count = 0u64;
    let mut next_report = ROW_REPORT_INTERVAL;
    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let model: E::Model = serde_json::from_str(&line)?;
        batch.push(model.into_active_model());
        if batch.len() >= batch_size {
            count += insert_batch::<E, C>(conn, std::mem::take(&mut batch)).await?;
            if count >= next_report {
                progress.table_rows(&table, count);
                next_report += ROW_REPORT_INTERVAL;
            }
        }
    }
    if !batch.is_empty() {
        count += insert_batch::<E, C>(conn, batch).await?;
    }
    Ok(count)
}

/// Stream every row of `E` directly from `src` into `dst` in batches, without
/// an intermediate serialized form. This is the path used by the direct
/// database-to-database `copy`.
pub async fn copy_table<E, S, D>(
    src: &S,
    dst: &D,
    batch_size: usize,
    progress: Progress,
) -> Result<u64>
where
    E: EntityTrait,
    E::Model: IntoActiveModel<E::ActiveModel>,
    S: ConnectionTrait + StreamTrait,
    D: ConnectionTrait,
{
    let table = iden_string(&E::default());
    let batch_size = safe_batch_size::<E>(dst.get_database_backend(), batch_size);
    let mut stream = open_source_stream::<E, S>(src).await?;
    let mut batch: Vec<E::ActiveModel> = Vec::with_capacity(batch_size);
    let mut count = 0u64;
    let mut next_report = ROW_REPORT_INTERVAL;
    while let Some(model) = stream.try_next().await? {
        batch.push(model.into_active_model());
        if batch.len() >= batch_size {
            count += insert_batch::<E, D>(dst, std::mem::take(&mut batch)).await?;
            if count >= next_report {
                progress.table_rows(&table, count);
                next_report += ROW_REPORT_INTERVAL;
            }
        }
    }
    if !batch.is_empty() {
        count += insert_batch::<E, D>(dst, batch).await?;
    }
    Ok(count)
}

/// Count rows in `E`.
pub async fn count_table<E, C>(conn: &C) -> Result<u64>
where
    E: EntityTrait,
    E::Model: Send + Sync,
    C: ConnectionTrait,
{
    Ok(E::find().count(conn).await?)
}

/// Delete every row of `E`. Used by `--replace` before a load. Callers must
/// have foreign-key enforcement disabled (see [`crate::fk`]).
pub async fn truncate_table<E, C>(conn: &C) -> Result<u64>
where
    E: EntityTrait,
    C: ConnectionTrait,
{
    Ok(E::delete_many().exec(conn).await?.rows_affected)
}

/// Cap the batch so a multi-row insert can't exceed the destination's bind
/// parameter limit. A batch binds `rows × columns` parameters; PostgreSQL caps
/// a statement at 65535 and SQLite at 32766, so a wide table (e.g.
/// `book_metadata`, ~66 columns) overflows a naive 1000-row batch.
fn safe_batch_size<E: EntityTrait>(backend: DatabaseBackend, requested: usize) -> usize {
    let columns = <E::Column as Iterable>::iter().count().max(1);
    let param_limit = match backend {
        DatabaseBackend::Postgres | DatabaseBackend::MySql => 65535,
        DatabaseBackend::Sqlite => 32766,
    };
    requested.min((param_limit / columns).max(1))
}

/// Insert one batch, using `exec_without_returning` to avoid a per-row
/// RETURNING clause on bulk loads. No-op (and no DB round-trip) when empty.
async fn insert_batch<E, C>(conn: &C, batch: Vec<E::ActiveModel>) -> Result<u64>
where
    E: EntityTrait,
    E::Model: IntoActiveModel<E::ActiveModel>,
    C: ConnectionTrait,
{
    let n = batch.len() as u64;
    if n == 0 {
        return Ok(0);
    }
    E::insert_many(batch).exec_without_returning(conn).await?;
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::safe_batch_size;
    use sea_orm::DatabaseBackend;

    #[test]
    fn caps_wide_table_under_postgres_param_limit() {
        // book_metadata has 66 columns; a naive 1000-row batch would bind
        // 66000 parameters, over PostgreSQL's 65535 limit.
        let n = safe_batch_size::<codex_db::entities::book_metadata::Entity>(
            DatabaseBackend::Postgres,
            1000,
        );
        assert!(n < 1000, "wide table should be capped, got {n}");
        assert!(n * 66 <= 65535, "batch {n} still exceeds the limit");
    }

    #[test]
    fn keeps_requested_size_for_narrow_table() {
        // genres has 4 columns; 1000 rows is well within the limit.
        let n =
            safe_batch_size::<codex_db::entities::genres::Entity>(DatabaseBackend::Postgres, 1000);
        assert_eq!(n, 1000);
    }
}
