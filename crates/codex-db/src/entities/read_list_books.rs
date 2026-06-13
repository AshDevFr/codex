//! `SeaORM` Entity for read_list_books junction table
//!
//! Ordered membership linking read lists to books (many-to-many).

use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "read_list_books")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub read_list_id: Uuid,
    pub book_id: Uuid,
    /// Honored only when the parent read list's `ordered` flag is true.
    pub position: i32,
    pub created_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::read_lists::Entity",
        from = "Column::ReadListId",
        to = "super::read_lists::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    ReadList,
    #[sea_orm(
        belongs_to = "super::books::Entity",
        from = "Column::BookId",
        to = "super::books::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    Book,
}

impl Related<super::read_lists::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ReadList.def()
    }
}

impl Related<super::books::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Book.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
