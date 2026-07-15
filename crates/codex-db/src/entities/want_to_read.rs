//! `SeaORM` Entity for want_to_read table
//!
//! Per-user, flat on-deck queue. Each row flags exactly one series OR one book
//! the user intends to read (enforced by a DB CHECK constraint).

use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "want_to_read")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub user_id: Uuid,
    /// Set when this entry flags a series; mutually exclusive with `book_id`.
    pub series_id: Option<Uuid>,
    /// Set when this entry flags a book; mutually exclusive with `series_id`.
    pub book_id: Option<Uuid>,
    pub added_at: DateTime<Utc>,
    /// Manual queue order, honored by the `custom` sort. New entries append at
    /// max+1; rows predating the column default to 0 and tie-break on
    /// `added_at` until the user reorders.
    pub position: i32,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::users::Entity",
        from = "Column::UserId",
        to = "super::users::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    Users,
    #[sea_orm(
        belongs_to = "super::series::Entity",
        from = "Column::SeriesId",
        to = "super::series::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    Series,
    #[sea_orm(
        belongs_to = "super::books::Entity",
        from = "Column::BookId",
        to = "super::books::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    Books,
}

impl Related<super::users::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Users.def()
    }
}

impl Related<super::series::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Series.def()
    }
}

impl Related<super::books::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Books.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
