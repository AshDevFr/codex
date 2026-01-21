//! `SeaORM` Entity for series_sharing_tags junction table
//!
//! Links series to sharing tags (many-to-many).
//! When a series has sharing tags, only users with matching grants can see it.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "series_sharing_tags")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub series_id: Uuid,
    #[sea_orm(primary_key, auto_increment = false)]
    pub sharing_tag_id: Uuid,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::series::Entity",
        from = "Column::SeriesId",
        to = "super::series::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    Series,
    #[sea_orm(
        belongs_to = "super::sharing_tags::Entity",
        from = "Column::SharingTagId",
        to = "super::sharing_tags::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    SharingTag,
}

impl Related<super::series::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Series.def()
    }
}

impl Related<super::sharing_tags::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::SharingTag.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
