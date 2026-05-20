//! `SeaORM` Entity for series_duplicates table
//!
//! Stores groups of series flagged as potential duplicates. Detection runs as part
//! of the `find_duplicates` task and produces one row per group. Two match types
//! are supported:
//!
//! - `external_id`: two or more series share the same `(source, external_id)` in
//!   `series_external_ids`. High confidence; matched globally across libraries.
//! - `title`: two or more series in the same library share the same normalized
//!   `series_metadata.search_title`. Medium confidence; scoped to one library so
//!   we do not flag a "Naruto" comic and a "Naruto" manga in separate libraries.

use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Match type for a series duplicate group.
pub const MATCH_TYPE_EXTERNAL_ID: &str = "external_id";
pub const MATCH_TYPE_TITLE: &str = "title";

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "series_duplicates")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    /// `external_id` or `title` (see module-level constants).
    pub match_type: String,
    /// For `external_id`: `"<source>:<external_id>"`. For `title`: normalized title.
    pub match_key: String,
    /// Null for `external_id` matches (global); set for `title` matches.
    pub library_id: Option<Uuid>,
    #[sea_orm(column_type = "Text")]
    pub series_ids: String, // JSON string of Vec<Uuid>
    pub duplicate_count: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
