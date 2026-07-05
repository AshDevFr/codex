//! The entity registry: every table listed exactly once.
//!
//! [`for_each_entity!`] is an x-macro that expands a caller-supplied per-entity
//! macro once for each entity module. All collective operations
//! (count / copy / dump / load / truncate) are generated from this single list,
//! so adding a table means adding one line here — and the drift-guard test
//! fails loudly if a migration adds a table that is missing from this list.

use std::path::Path;

use anyhow::Result;
use sea_orm::{ConnectionTrait, EntityName, StreamTrait};

use crate::engine;
use crate::progress::Progress;

/// Row count for a single table, produced by the collective operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableRows {
    pub table: String,
    pub rows: u64,
}

/// Expand `$op!(module_name)` once per entity, in `entities/mod.rs` order.
///
/// Order is irrelevant to correctness: loads run with foreign-key enforcement
/// disabled (see [`crate::fk`]), so parents and children may be inserted in any
/// order.
macro_rules! for_each_entity {
    ($op:ident) => {
        $op!(api_keys);
        $op!(book_covers);
        $op!(book_duplicates);
        $op!(book_external_ids);
        $op!(book_external_links);
        $op!(book_genres);
        $op!(book_metadata);
        $op!(book_tags);
        $op!(books);
        $op!(email_verification_tokens);
        $op!(libraries);
        $op!(library_jobs);
        $op!(metadata_sources);
        $op!(pages);
        $op!(plugin_failures);
        $op!(plugins);
        $op!(read_progress);
        $op!(refresh_tokens);
        $op!(scheduled_firing_claims);
        $op!(series);
        $op!(settings);
        $op!(settings_history);
        $op!(task_metrics);
        $op!(tasks);
        $op!(users);
        $op!(oidc_connections);
        $op!(plugin_data);
        $op!(user_plugin_data);
        $op!(user_plugins);
        $op!(filter_presets);
        $op!(genres);
        $op!(release_ledger);
        $op!(release_sources);
        $op!(series_aliases);
        $op!(series_alternate_titles);
        $op!(series_covers);
        $op!(series_duplicates);
        $op!(series_exports);
        $op!(series_external_ids);
        $op!(series_external_links);
        $op!(series_external_ratings);
        $op!(series_genres);
        $op!(series_metadata);
        $op!(series_tags);
        $op!(series_tracking);
        $op!(tags);
        $op!(user_preferences);
        $op!(user_series_ratings);
        $op!(series_sharing_tags);
        $op!(sharing_tags);
        $op!(user_sharing_tags);
        $op!(access_group_oidc_mappings);
        $op!(access_group_sharing_tags);
        $op!(access_groups);
        $op!(user_access_groups);
        $op!(collection_series);
        $op!(collections);
        $op!(read_list_books);
        $op!(read_lists);
        $op!(want_to_read);
    };
}

/// The table name of every registered entity, in registry order.
pub fn table_names() -> Vec<String> {
    let mut names = Vec::new();
    macro_rules! push_name {
        ($ent:ident) => {{
            names.push(
                <codex_db::entities::$ent::Entity as Default>::default()
                    .table_name()
                    .to_string(),
            );
        }};
    }
    for_each_entity!(push_name);
    names
}

/// Count rows in every table.
pub async fn count_all<C>(conn: &C) -> Result<Vec<TableRows>>
where
    C: ConnectionTrait,
{
    let mut rows = Vec::new();
    macro_rules! count_one {
        ($ent:ident) => {{
            type E = codex_db::entities::$ent::Entity;
            let table = <E as Default>::default().table_name().to_string();
            let n = engine::count_table::<E, C>(conn).await?;
            rows.push(TableRows { table, rows: n });
        }};
    }
    for_each_entity!(count_one);
    Ok(rows)
}

/// Delete every row of every table. Callers must disable FK enforcement first
/// (used by `--replace`).
pub async fn truncate_all<C>(conn: &C) -> Result<()>
where
    C: ConnectionTrait,
{
    macro_rules! truncate_one {
        ($ent:ident) => {{
            type E = codex_db::entities::$ent::Entity;
            engine::truncate_table::<E, C>(conn).await?;
        }};
    }
    for_each_entity!(truncate_one);
    Ok(())
}

/// Stream every table directly from `src` into `dst` (the direct copy path).
pub async fn copy_all<S, D>(
    src: &S,
    dst: &D,
    batch_size: usize,
    progress: Progress,
) -> Result<Vec<TableRows>>
where
    S: ConnectionTrait + StreamTrait,
    D: ConnectionTrait,
{
    let mut rows = Vec::new();
    macro_rules! copy_one {
        ($ent:ident) => {{
            type E = codex_db::entities::$ent::Entity;
            let table = <E as Default>::default().table_name().to_string();
            progress.table_start(&table);
            let n = engine::copy_table::<E, S, D>(src, dst, batch_size, progress).await?;
            progress.table_done(&table, n);
            rows.push(TableRows { table, rows: n });
        }};
    }
    for_each_entity!(copy_one);
    Ok(rows)
}

/// Dump every table to `dir` as one `<table>.ndjson` file each (the archive
/// payload). `dir` must already exist.
pub async fn dump_all_to_dir<C>(conn: &C, dir: &Path, progress: Progress) -> Result<Vec<TableRows>>
where
    C: ConnectionTrait + StreamTrait,
{
    use std::io::Write as _;
    let mut rows = Vec::new();
    macro_rules! dump_one {
        ($ent:ident) => {{
            type E = codex_db::entities::$ent::Entity;
            let table = <E as Default>::default().table_name().to_string();
            progress.table_start(&table);
            let path = dir.join(format!("{table}.ndjson"));
            let mut writer = std::io::BufWriter::new(std::fs::File::create(&path)?);
            let n = engine::dump_table::<E, C, _>(conn, &mut writer, progress).await?;
            writer.flush()?;
            progress.table_done(&table, n);
            rows.push(TableRows { table, rows: n });
        }};
    }
    for_each_entity!(dump_one);
    Ok(rows)
}

/// Load every table from a directory of `<table>.ndjson` files. Missing files
/// are treated as empty tables.
pub async fn load_all_from_dir<C>(
    conn: &C,
    dir: &Path,
    batch_size: usize,
    progress: Progress,
) -> Result<Vec<TableRows>>
where
    C: ConnectionTrait,
{
    let mut rows = Vec::new();
    macro_rules! load_one {
        ($ent:ident) => {{
            type E = codex_db::entities::$ent::Entity;
            let table = <E as Default>::default().table_name().to_string();
            let path = dir.join(format!("{table}.ndjson"));
            let n = if path.exists() {
                progress.table_start(&table);
                let reader = std::io::BufReader::new(std::fs::File::open(&path)?);
                let n = engine::load_table::<E, C, _>(conn, reader, batch_size, progress).await?;
                progress.table_done(&table, n);
                n
            } else {
                0
            };
            rows.push(TableRows { table, rows: n });
        }};
    }
    for_each_entity!(load_one);
    Ok(rows)
}
