//! `SeaORM` Entity for read_lists table
//!
//! A read list is a shared, ordered grouping of books across series (Komga-style
//! "playlist for books"). Membership and order live in the `read_list_books`
//! junction.

use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "read_lists")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    #[sea_orm(unique)]
    pub name: String,
    #[sea_orm(unique)]
    pub normalized_name: String,
    #[sea_orm(column_type = "Text", nullable)]
    pub summary: Option<String>,
    /// true (default) => manual reading order; false => sort members by release date.
    pub ordered: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::read_list_books::Entity")]
    ReadListBooks,
}

impl Related<super::read_list_books::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ReadListBooks.def()
    }
}

impl Related<super::books::Entity> for Entity {
    fn to() -> RelationDef {
        super::read_list_books::Relation::Book.def()
    }
    fn via() -> Option<RelationDef> {
        Some(super::read_list_books::Relation::ReadList.def().rev())
    }
}

impl ActiveModelBehavior for ActiveModel {}
