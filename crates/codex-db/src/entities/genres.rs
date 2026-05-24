//! `SeaORM` Entity for genres table
//!
//! Genre taxonomy table for categorizing series and books.

use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "genres")]
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
    #[sea_orm(has_many = "super::series_genres::Entity")]
    SeriesGenres,
    #[sea_orm(has_many = "super::book_genres::Entity")]
    BookGenres,
}

impl Related<super::series_genres::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::SeriesGenres.def()
    }
}

impl Related<super::series::Entity> for Entity {
    fn to() -> RelationDef {
        super::series_genres::Relation::Series.def()
    }
    fn via() -> Option<RelationDef> {
        Some(super::series_genres::Relation::Genre.def().rev())
    }
}

impl Related<super::book_genres::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::BookGenres.def()
    }
}

impl Related<super::books::Entity> for Entity {
    fn to() -> RelationDef {
        super::book_genres::Relation::Book.def()
    }
    fn via() -> Option<RelationDef> {
        Some(super::book_genres::Relation::Genre.def().rev())
    }
}

impl ActiveModelBehavior for ActiveModel {}
