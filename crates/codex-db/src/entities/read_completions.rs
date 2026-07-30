//! `SeaORM` Entity for the read_completions table
//!
//! An append-only log of finished read-throughs: one row per completed pass of
//! one book by one user. Rows are inserted when a book is completed and only
//! ever removed by an explicit history reset (or by a cascade when the user or
//! book is deleted). Nothing updates them.
//!
//! This is deliberately separate from `read_progress`, which tracks the
//! *current* pass and is deleted when a book is marked unread. Keeping the
//! completion log out of that row is what lets marking a series unread reset
//! progress without erasing the fact that it was read.

use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "read_completions")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub user_id: Uuid,
    pub book_id: Uuid,
    /// When this pass started, copied from the `read_progress` row that was
    /// current when the book completed.
    pub started_at: DateTime<Utc>,
    /// When this pass finished. Never null: the row exists because it finished.
    pub completed_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::books::Entity",
        from = "Column::BookId",
        to = "super::books::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    Books,
    #[sea_orm(
        belongs_to = "super::users::Entity",
        from = "Column::UserId",
        to = "super::users::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    Users,
}

impl Related<super::books::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Books.def()
    }
}

impl Related<super::users::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Users.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
