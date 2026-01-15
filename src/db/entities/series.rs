//! `SeaORM` Entity for series table
//!
//! This table contains core series identity fields only.
//! Rich metadata is stored in series_metadata table (1:1 relationship).

use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "series")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub library_id: Uuid,
    pub fingerprint: Option<String>,
    pub path: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::books::Entity")]
    Books,
    #[sea_orm(
        belongs_to = "super::libraries::Entity",
        from = "Column::LibraryId",
        to = "super::libraries::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    Libraries,
    #[sea_orm(has_many = "super::metadata_sources::Entity")]
    MetadataSources,
    // Series metadata enhancement relations
    #[sea_orm(has_one = "super::series_metadata::Entity")]
    SeriesMetadata,
    #[sea_orm(has_many = "super::series_genres::Entity")]
    SeriesGenres,
    #[sea_orm(has_many = "super::series_tags::Entity")]
    SeriesTags,
    #[sea_orm(has_many = "super::series_alternate_titles::Entity")]
    SeriesAlternateTitles,
    #[sea_orm(has_many = "super::series_external_ratings::Entity")]
    SeriesExternalRatings,
    #[sea_orm(has_many = "super::series_external_links::Entity")]
    SeriesExternalLinks,
    #[sea_orm(has_many = "super::series_covers::Entity")]
    SeriesCovers,
    #[sea_orm(has_many = "super::user_series_ratings::Entity")]
    UserSeriesRatings,
}

impl Related<super::books::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Books.def()
    }
}

impl Related<super::libraries::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Libraries.def()
    }
}

impl Related<super::metadata_sources::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::MetadataSources.def()
    }
}

impl Related<super::series_metadata::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::SeriesMetadata.def()
    }
}

impl Related<super::series_genres::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::SeriesGenres.def()
    }
}

impl Related<super::series_tags::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::SeriesTags.def()
    }
}

impl Related<super::series_alternate_titles::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::SeriesAlternateTitles.def()
    }
}

impl Related<super::series_external_ratings::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::SeriesExternalRatings.def()
    }
}

impl Related<super::series_external_links::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::SeriesExternalLinks.def()
    }
}

impl Related<super::series_covers::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::SeriesCovers.def()
    }
}

impl Related<super::user_series_ratings::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::UserSeriesRatings.def()
    }
}

// Many-to-many relationships via junction tables
impl Related<super::genres::Entity> for Entity {
    fn to() -> RelationDef {
        super::series_genres::Relation::Genre.def()
    }
    fn via() -> Option<RelationDef> {
        Some(super::series_genres::Relation::Series.def().rev())
    }
}

impl Related<super::tags::Entity> for Entity {
    fn to() -> RelationDef {
        super::series_tags::Relation::Tag.def()
    }
    fn via() -> Option<RelationDef> {
        Some(super::series_tags::Relation::Series.def().rev())
    }
}

impl ActiveModelBehavior for ActiveModel {}
