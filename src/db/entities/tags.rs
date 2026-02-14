//! `SeaORM` Entity for tags table
//!
//! Tag taxonomy table for categorizing series and books.

use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "tags")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    #[sea_orm(unique)]
    pub name: String,
    #[sea_orm(unique)]
    pub normalized_name: String, // lowercase for matching
    pub created_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::series_tags::Entity")]
    SeriesTags,
    #[sea_orm(has_many = "super::book_tags::Entity")]
    BookTags,
}

impl Related<super::series_tags::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::SeriesTags.def()
    }
}

impl Related<super::series::Entity> for Entity {
    fn to() -> RelationDef {
        super::series_tags::Relation::Series.def()
    }
    fn via() -> Option<RelationDef> {
        Some(super::series_tags::Relation::Tag.def().rev())
    }
}

impl Related<super::book_tags::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::BookTags.def()
    }
}

impl Related<super::books::Entity> for Entity {
    fn to() -> RelationDef {
        super::book_tags::Relation::Book.def()
    }
    fn via() -> Option<RelationDef> {
        Some(super::book_tags::Relation::Tag.def().rev())
    }
}

impl ActiveModelBehavior for ActiveModel {}
