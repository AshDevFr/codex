//! Scheduled-firing claims: a distributed mutex for cron occurrences.
//!
//! Every `serve` replica runs its own in-process scheduler, so in a
//! horizontally-scaled deployment a cron fires once per replica. For jobs whose
//! firing does real work (e.g. fanning out per-user plugin syncs), each replica
//! claims the firing here before acting; the composite primary key
//! `(job_key, fire_slot)` lets exactly one INSERT win, and the rest skip.
//!
//! `job_key` identifies the logical job (e.g. `"plugin_sync:<plugin_uuid>"`) and
//! `fire_slot` is the firing instant truncated to the cron's granularity so all
//! replicas firing for the same occurrence compute the same key.

use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "scheduled_firing_claims")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub job_key: String,
    #[sea_orm(primary_key, auto_increment = false)]
    pub fire_slot: DateTime<Utc>,
    pub claimed_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
