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
    #[sea_orm(column_type = "Text", nullable)]
    pub summary: Option<String>,
    /// Serialized `SeriesCondition` when membership is defined by a rule.
    ///
    /// `None` means the collection is manual and its members live in
    /// `collection_series`. This is the sole discriminator between the two
    /// kinds: an automatic collection never has junction rows, and a manual one
    /// never has a condition.
    #[sea_orm(column_type = "JsonBinary", nullable)]
    pub condition: Option<Json>,
    /// Default presentation order when no sort is requested: false => by
    /// series title; true => manual `position`.
    ///
    /// Forced to `false` for rule-backed collections: there is no manual order
    /// to honour when nobody arranged the members.
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
