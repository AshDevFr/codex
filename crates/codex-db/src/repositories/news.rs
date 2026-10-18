//! Repositories for imported news entries and per-feed fetch bookkeeping.

use crate::entities::{news_entries, news_feeds};
use anyhow::Result;
use chrono::{DateTime, Utc};
use sea_orm::sea_query::{Expr, OnConflict};
use sea_orm::*;

/// A feed entry to store.
#[derive(Debug, Clone)]
pub struct NewNewsEntry {
    pub id: String,
    pub project: String,
    pub kind: String,
    pub title: String,
    pub version: Option<String>,
    pub summary: String,
    pub body: String,
    pub url: Option<String>,
    pub published_at: DateTime<Utc>,
}

pub struct NewsEntryRepository;

impl NewsEntryRepository {
    /// Insert new entries and refresh the content of existing ones.
    ///
    /// `first_seen_at` is written on insert only: an update must not move it,
    /// because the earliest value is what marks when this instance started
    /// reading the feed.
    pub async fn upsert_many(
        db: &impl ConnectionTrait,
        entries: &[NewNewsEntry],
        now: DateTime<Utc>,
    ) -> Result<()> {
        if entries.is_empty() {
            return Ok(());
        }
        let models = entries.iter().cloned().map(|e| news_entries::ActiveModel {
            id: Set(e.id),
            project: Set(e.project),
            kind: Set(e.kind),
            title: Set(e.title),
            version: Set(e.version),
            summary: Set(e.summary),
            body: Set(e.body),
            url: Set(e.url),
            published_at: Set(e.published_at),
            first_seen_at: Set(now),
            updated_at: Set(now),
        });
        news_entries::Entity::insert_many(models)
            .on_conflict(
                OnConflict::column(news_entries::Column::Id)
                    .update_columns([
                        news_entries::Column::Project,
                        news_entries::Column::Kind,
                        news_entries::Column::Title,
                        news_entries::Column::Version,
                        news_entries::Column::Summary,
                        news_entries::Column::Body,
                        news_entries::Column::Url,
                        news_entries::Column::PublishedAt,
                        news_entries::Column::UpdatedAt,
                    ])
                    .to_owned(),
            )
            .exec_without_returning(db)
            .await?;
        Ok(())
    }

    /// Newest entries from `projects`, newest first. The id breaks ties so a
    /// page boundary between two same-instant entries is stable.
    pub async fn list_recent(
        db: &impl ConnectionTrait,
        projects: &[&str],
        limit: u64,
    ) -> Result<Vec<news_entries::Model>> {
        Ok(news_entries::Entity::find()
            .filter(news_entries::Column::Project.is_in(projects.iter().copied()))
            .order_by_desc(news_entries::Column::PublishedAt)
            .order_by_desc(news_entries::Column::Id)
            .limit(limit)
            .all(db)
            .await?)
    }

    /// Every release entry of `project` that carries a version.
    pub async fn list_releases(
        db: &impl ConnectionTrait,
        project: &str,
    ) -> Result<Vec<news_entries::Model>> {
        Ok(news_entries::Entity::find()
            .filter(news_entries::Column::Project.eq(project))
            .filter(news_entries::Column::Kind.eq("release"))
            .filter(news_entries::Column::Version.is_not_null())
            .order_by_desc(news_entries::Column::PublishedAt)
            .order_by_desc(news_entries::Column::Id)
            .all(db)
            .await?)
    }

    /// When this instance stored its first entry, if it ever has.
    pub async fn earliest_first_seen(db: &impl ConnectionTrait) -> Result<Option<DateTime<Utc>>> {
        Ok(news_entries::Entity::find()
            .select_only()
            .column_as(Expr::col(news_entries::Column::FirstSeenAt).min(), "min")
            .into_tuple::<Option<DateTime<Utc>>>()
            .one(db)
            .await?
            .flatten())
    }
}

pub struct NewsFeedRepository;

impl NewsFeedRepository {
    pub async fn get(
        db: &impl ConnectionTrait,
        project: &str,
    ) -> Result<Option<news_feeds::Model>> {
        Ok(news_feeds::Entity::find_by_id(project).one(db).await?)
    }

    /// Record a successful fetch (200 or 304) and the ETag to send next time.
    pub async fn record_success(
        db: &impl ConnectionTrait,
        project: &str,
        etag: Option<String>,
        now: DateTime<Utc>,
    ) -> Result<()> {
        news_feeds::Entity::insert(news_feeds::ActiveModel {
            project: Set(project.to_string()),
            etag: Set(etag),
            last_success_at: Set(Some(now)),
            updated_at: Set(now),
        })
        .on_conflict(
            OnConflict::column(news_feeds::Column::Project)
                .update_columns([
                    news_feeds::Column::Etag,
                    news_feeds::Column::LastSuccessAt,
                    news_feeds::Column::UpdatedAt,
                ])
                .to_owned(),
        )
        .exec_without_returning(db)
        .await?;
        Ok(())
    }
}
