//! Server-side state for an OIDC login that is in flight.
//!
//! Written when the authorization URL is generated and deleted when the
//! callback consumes it. Lives in the database rather than in process memory
//! because the two legs of the flow are separate HTTP requests that a load
//! balancer is free to route to different `codex serve` processes.

use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "oidc_pending_states")]
pub struct Model {
    /// The CSRF state token. Primary key, so a duplicate insert collides
    /// rather than replacing a live flow.
    #[sea_orm(primary_key, auto_increment = false)]
    pub state: String,
    /// Provider this state was minted for. The callback rejects a mismatch.
    pub provider_name: String,
    /// PKCE code verifier, needed to complete the token exchange. Plaintext by
    /// deliberate choice: see the migration that creates this table.
    pub pkce_verifier: String,
    /// Nonce, checked against the ID token to block replay.
    pub nonce: String,
    /// Where to send the browser once the flow completes. `None` means the
    /// built-in web completion page.
    pub redirect_uri: Option<String>,
    pub created_at: DateTime<Utc>,
    /// When this state stops being redeemable. Stored rather than derived so
    /// the TTL in force at creation stays attached to the row.
    pub expires_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
