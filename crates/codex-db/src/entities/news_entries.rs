//! One entry from a Codex news feed, as stored by this instance.
//!
//! The primary key is the feed's own entry id (`codex-v2.6.3`), which the feed
//! promises never to change, so re-importing an entry updates it in place.

use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "news_entries")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    /// Feed the entry came from: `codex` or `codex-ios`.
    pub project: String,
    /// `release` or `post`.
    pub kind: String,
    pub title: String,
    /// Releases only. Not guaranteed to be semver outside the `codex` feed.
    pub version: Option<String>,
    /// Plain text, at most a few hundred characters.
    pub summary: String,
    /// Markdown.
    pub body: String,
    pub url: Option<String>,
    pub published_at: DateTime<Utc>,
    /// When this instance first stored the entry. Never updated.
    pub first_seen_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
