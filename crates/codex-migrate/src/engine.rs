//! Generic, entity-driven transfer primitives.
//!
//! Every function here is generic over a SeaORM [`EntityTrait`]. Because
//! SeaORM performs the engine-specific type mapping (UUID blob ↔ native
//! `uuid`, text JSON ↔ JSONB, 0/1 ↔ `bool`, timestamps) when it materializes a
//! typed `Model`, moving rows as `Model` values is correct by construction
//! across SQLite and PostgreSQL — no hand-written cast rules, no raw bytes.

use std::io::{BufRead, Write};

use anyhow::Result;
use futures::TryStreamExt;
use sea_orm::{ConnectionTrait, EntityTrait, IntoActiveModel, PaginatorTrait, StreamTrait};
use serde::Serialize;
use serde::de::DeserializeOwned;

/// Default insert batch size. Bounds memory and round-trips while staying well
/// under parameter limits for the widest tables.
pub const DEFAULT_BATCH_SIZE: usize = 1000;

/// Stream every row of `E` and write it as one NDJSON line to `out`.
/// Returns the number of rows written.
pub async fn dump_table<E, C, W>(conn: &C, out: &mut W) -> Result<u64>
where
    E: EntityTrait,
    E::Model: Serialize,
    C: ConnectionTrait + StreamTrait,
    W: Write,
{
    let mut stream = E::find().stream(conn).await?;
    let mut count = 0u64;
    while let Some(model) = stream.try_next().await? {
        serde_json::to_writer(&mut *out, &model)?;
        out.write_all(b"\n")?;
        count += 1;
    }
    Ok(count)
}

/// Read NDJSON lines from `reader`, deserialize each into `E::Model`, and
/// insert them into `conn` in batches of `batch_size`. Returns rows inserted.
pub async fn load_table<E, C, R>(conn: &C, reader: R, batch_size: usize) -> Result<u64>
where
    E: EntityTrait,
    E::Model: DeserializeOwned + IntoActiveModel<E::ActiveModel>,
    C: ConnectionTrait,
    R: BufRead,
{
    let mut batch: Vec<E::ActiveModel> = Vec::with_capacity(batch_size);
    let mut count = 0u64;
    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let model: E::Model = serde_json::from_str(&line)?;
        batch.push(model.into_active_model());
        if batch.len() >= batch_size {
            count += insert_batch::<E, C>(conn, std::mem::take(&mut batch)).await?;
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
pub async fn copy_table<E, S, D>(src: &S, dst: &D, batch_size: usize) -> Result<u64>
where
    E: EntityTrait,
    E::Model: IntoActiveModel<E::ActiveModel>,
    S: ConnectionTrait + StreamTrait,
    D: ConnectionTrait,
{
    let mut stream = E::find().stream(src).await?;
    let mut batch: Vec<E::ActiveModel> = Vec::with_capacity(batch_size);
    let mut count = 0u64;
    while let Some(model) = stream.try_next().await? {
        batch.push(model.into_active_model());
        if batch.len() >= batch_size {
            count += insert_batch::<E, D>(dst, std::mem::take(&mut batch)).await?;
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
