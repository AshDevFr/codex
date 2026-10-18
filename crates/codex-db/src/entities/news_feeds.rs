//! Fetch bookkeeping for one news feed.

use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "news_feeds")]
pub struct Model {
    /// Feed name, e.g. `codex`.
    #[sea_orm(primary_key, auto_increment = false)]
    pub project: String,
    /// ETag of the last successful response, sent back as `If-None-Match`.
    pub etag: Option<String>,
    /// Last fetch that returned 200 or 304. `None` until the first success.
    pub last_success_at: Option<DateTime<Utc>>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
