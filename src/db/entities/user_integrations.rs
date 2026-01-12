//! User integrations entity for per-user external service connections

use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "user_integrations")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub user_id: Uuid,
    pub integration_name: String,
    pub display_name: Option<String>,
    #[serde(skip_serializing)] // Never serialize credentials
    pub credentials: Vec<u8>,
    pub settings: serde_json::Value,
    pub enabled: bool,
    pub last_sync_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
    pub sync_status: String,
    pub external_user_id: Option<String>,
    pub external_username: Option<String>,
    pub token_expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
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
    User,
}

impl Related<super::users::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::User.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

/// Sync status values for user integrations
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncStatus {
    Idle,
    Syncing,
    Error,
    RateLimited,
}

impl SyncStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            SyncStatus::Idle => "idle",
            SyncStatus::Syncing => "syncing",
            SyncStatus::Error => "error",
            SyncStatus::RateLimited => "rate_limited",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "idle" => Some(SyncStatus::Idle),
            "syncing" => Some(SyncStatus::Syncing),
            "error" => Some(SyncStatus::Error),
            "rate_limited" => Some(SyncStatus::RateLimited),
            _ => None,
        }
    }
}

impl std::fmt::Display for SyncStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Known integration providers
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IntegrationProvider {
    Anilist,
    MyAnimeList,
    Kitsu,
    MangaDex,
    Kavita,
}

impl IntegrationProvider {
    pub fn as_str(&self) -> &'static str {
        match self {
            IntegrationProvider::Anilist => "anilist",
            IntegrationProvider::MyAnimeList => "myanimelist",
            IntegrationProvider::Kitsu => "kitsu",
            IntegrationProvider::MangaDex => "mangadex",
            IntegrationProvider::Kavita => "kavita",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "anilist" => Some(IntegrationProvider::Anilist),
            "myanimelist" => Some(IntegrationProvider::MyAnimeList),
            "kitsu" => Some(IntegrationProvider::Kitsu),
            "mangadex" => Some(IntegrationProvider::MangaDex),
            "kavita" => Some(IntegrationProvider::Kavita),
            _ => None,
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            IntegrationProvider::Anilist => "AniList",
            IntegrationProvider::MyAnimeList => "MyAnimeList",
            IntegrationProvider::Kitsu => "Kitsu",
            IntegrationProvider::MangaDex => "MangaDex",
            IntegrationProvider::Kavita => "Kavita",
        }
    }

    pub fn auth_type(&self) -> &'static str {
        match self {
            IntegrationProvider::Anilist => "oauth2",
            IntegrationProvider::MyAnimeList => "oauth2",
            IntegrationProvider::Kitsu => "oauth2",
            IntegrationProvider::MangaDex => "api_key",
            IntegrationProvider::Kavita => "api_key",
        }
    }

    pub fn features(&self) -> Vec<&'static str> {
        match self {
            IntegrationProvider::Anilist => {
                vec!["sync_progress", "sync_ratings", "import_lists"]
            }
            IntegrationProvider::MyAnimeList => {
                vec!["sync_progress", "sync_ratings", "import_lists"]
            }
            IntegrationProvider::Kitsu => {
                vec!["sync_progress", "sync_ratings"]
            }
            IntegrationProvider::MangaDex => {
                vec!["sync_progress"]
            }
            IntegrationProvider::Kavita => {
                vec!["sync_progress", "sync_ratings"]
            }
        }
    }

    pub fn all() -> Vec<Self> {
        vec![
            IntegrationProvider::Anilist,
            IntegrationProvider::MyAnimeList,
            IntegrationProvider::Kitsu,
            IntegrationProvider::MangaDex,
            IntegrationProvider::Kavita,
        ]
    }
}

impl std::fmt::Display for IntegrationProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}
