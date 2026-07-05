//! Safety gate for destructive loads.
//!
//! `import` and `copy` always truncate the target (a faithful mirror must
//! overwrite migration-seeded rows), so before doing that we refuse a target
//! that already holds *user* data unless the caller passed `--replace`.
//!
//! "User data" is deliberately a small, high-signal set of top-level tables.
//! Migration-seeded tables (e.g. `settings`) are non-empty on any freshly
//! migrated database and must NOT be treated as user data, or a fresh target
//! would always look occupied.

use anyhow::Result;
use codex_db::entities::{books, libraries, series, users};
use sea_orm::ConnectionTrait;

use crate::engine::count_table;

/// Returns `true` if the target already contains user-owned content
/// (libraries, series, books, or users). A freshly migrated, unused database
/// returns `false`.
pub async fn has_user_data<C: ConnectionTrait>(conn: &C) -> Result<bool> {
    Ok(count_table::<libraries::Entity, _>(conn).await? > 0
        || count_table::<series::Entity, _>(conn).await? > 0
        || count_table::<books::Entity, _>(conn).await? > 0
        || count_table::<users::Entity, _>(conn).await? > 0)
}
