//! System integrations entity for app-wide external service connections
//!
//! TODO: Remove allow(dead_code) once integration features are implemented

#![allow(dead_code)]

use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "system_integrations")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub name: String,
    pub display_name: String,
    pub integration_type: String,
    #[serde(skip_serializing)] // Never serialize credentials
    pub credentials: Option<Vec<u8>>,
    pub config: serde_json::Value,
    pub enabled: bool,
    pub health_status: String,
    pub last_health_check_at: Option<DateTime<Utc>>,
    pub last_sync_at: Option<DateTime<Utc>>,
    pub error_message: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub created_by: Option<Uuid>,
    pub updated_by: Option<Uuid>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::users::Entity",
        from = "Column::CreatedBy",
        to = "super::users::Column::Id",
        on_update = "NoAction",
        on_delete = "SetNull"
    )]
    CreatedByUser,
    #[sea_orm(
        belongs_to = "super::users::Entity",
        from = "Column::UpdatedBy",
        to = "super::users::Column::Id",
        on_update = "NoAction",
        on_delete = "SetNull"
    )]
    UpdatedByUser,
}

impl ActiveModelBehavior for ActiveModel {}

/// Integration types for categorizing integrations
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IntegrationType {
    MetadataProvider,
    Notification,
    Storage,
    Sync,
}

impl IntegrationType {
    pub fn as_str(&self) -> &'static str {
        match self {
            IntegrationType::MetadataProvider => "metadata_provider",
            IntegrationType::Notification => "notification",
            IntegrationType::Storage => "storage",
            IntegrationType::Sync => "sync",
        }
    }
}

impl FromStr for IntegrationType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "metadata_provider" => Ok(IntegrationType::MetadataProvider),
            "notification" => Ok(IntegrationType::Notification),
            "storage" => Ok(IntegrationType::Storage),
            "sync" => Ok(IntegrationType::Sync),
            _ => Err(format!("Unknown integration type: {}", s)),
        }
    }
}

impl std::fmt::Display for IntegrationType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Health status values for integration health checks
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HealthStatus {
    Unknown,
    Healthy,
    Degraded,
    Unhealthy,
    Disabled,
}

impl HealthStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            HealthStatus::Unknown => "unknown",
            HealthStatus::Healthy => "healthy",
            HealthStatus::Degraded => "degraded",
            HealthStatus::Unhealthy => "unhealthy",
            HealthStatus::Disabled => "disabled",
        }
    }
}

impl FromStr for HealthStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "unknown" => Ok(HealthStatus::Unknown),
            "healthy" => Ok(HealthStatus::Healthy),
            "degraded" => Ok(HealthStatus::Degraded),
            "unhealthy" => Ok(HealthStatus::Unhealthy),
            "disabled" => Ok(HealthStatus::Disabled),
            _ => Err(format!("Unknown health status: {}", s)),
        }
    }
}

impl std::fmt::Display for HealthStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}
