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
    pub name: String,
    pub normalized_name: String,
    pub sort_name: Option<String>,
    pub summary: Option<String>,
    pub publisher: Option<String>,
    pub year: Option<i32>,
    pub book_count: i32,
    pub user_rating: Option<Decimal>,
    pub external_rating: Option<Decimal>,
    pub external_rating_count: Option<i32>,
    pub external_rating_source: Option<String>,
    pub custom_metadata: Option<String>,
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

impl ActiveModelBehavior for ActiveModel {}
