//! `SeaORM` Entity for access_group_oidc_mappings table
//!
//! Maps OIDC IdP group names to access groups. During login, the OIDC
//! reconciliation step uses these mappings to auto-assign users to
//! access groups based on their IdP group claims.

use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "access_group_oidc_mappings")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub access_group_id: Uuid,
    pub oidc_group_name: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::access_groups::Entity",
        from = "Column::AccessGroupId",
        to = "super::access_groups::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    AccessGroup,
}

impl Related<super::access_groups::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::AccessGroup.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
