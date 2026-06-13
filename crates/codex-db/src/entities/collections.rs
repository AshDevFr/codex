//! `SeaORM` Entity for collections table
//!
//! A collection is a shared, named grouping of series (Komga-style). Membership
//! and order live in the `collection_series` junction.

use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "collections")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    #[sea_orm(unique)]
    pub name: String,
    #[sea_orm(unique)]
    pub normalized_name: String,
    /// false => members sorted by series title; true => use `position`.
    pub ordered: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::collection_series::Entity")]
    CollectionSeries,
}

impl Related<super::collection_series::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::CollectionSeries.def()
    }
}

impl Related<super::series::Entity> for Entity {
    fn to() -> RelationDef {
        super::collection_series::Relation::Series.def()
    }
    fn via() -> Option<RelationDef> {
        Some(super::collection_series::Relation::Collection.def().rev())
    }
}

impl ActiveModelBehavior for ActiveModel {}
