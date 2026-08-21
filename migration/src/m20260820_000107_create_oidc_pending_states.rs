//! Create the `oidc_pending_states` table: the server-side half of an
//! in-flight OIDC login, held between the authorization request and the
//! callback.
//!
//! These two legs are separate HTTP requests. Any deployment running more than
//! one `codex serve` behind a load balancer without session affinity routes
//! them to different processes, and round-robin does so essentially every time
//! because the two requests are consecutive. Holding the state in process
//! memory therefore fails every login rather than some of them.
//!
//! `state` is the CSRF token and is the primary key, so a duplicate insert
//! collides instead of silently replacing a live flow. Consumption is a read
//! followed by a targeted delete, and the caller whose delete reports one
//! affected row is the one that wins: that is what keeps the CSRF token
//! single-use when two callbacks race.
//!
//! `pkce_verifier` and `nonce` are stored in plaintext, as
//! `email_verification_tokens.token` already is. Encrypting them would route
//! every login through `CredentialEncryption::global()`, which resolves the
//! optional `CODEX_ENCRYPTION_KEY` and errors when it is unset. Login works
//! today with no key configured, and turning a missing optional env var into a
//! total login outage is a worse failure than the one this table fixes. The
//! row lives at most five minutes and is deleted on use.
//!
//! `expires_at` is stored rather than derived so the TTL in force when the
//! flow began stays attached to the row, and so the sweep is one indexed range
//! delete.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(OidcPendingStates::Table)
                    .if_not_exists()
                    // The CSRF state token itself.
                    .col(
                        ColumnDef::new(OidcPendingStates::State)
                            .string()
                            .not_null()
                            .primary_key(),
                    )
                    // Checked against the provider in the callback path, so a
                    // state minted for one provider cannot be redeemed at
                    // another.
                    .col(
                        ColumnDef::new(OidcPendingStates::ProviderName)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(OidcPendingStates::PkceVerifier)
                            .string()
                            .not_null(),
                    )
                    .col(ColumnDef::new(OidcPendingStates::Nonce).string().not_null())
                    // NULL means the built-in web completion page. Validated
                    // against the allowlist before it is written, and kept
                    // server-side rather than round-tripped through the IdP,
                    // where it would come back attacker-controlled.
                    .col(
                        ColumnDef::new(OidcPendingStates::RedirectUri)
                            .string()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(OidcPendingStates::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(OidcPendingStates::ExpiresAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        // Serves the periodic sweep. The consume path hits the primary key and
        // needs no index of its own.
        manager
            .create_index(
                Index::create()
                    .name("idx_oidc_pending_states_expires_at")
                    .table(OidcPendingStates::Table)
                    .col(OidcPendingStates::ExpiresAt)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(OidcPendingStates::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
pub enum OidcPendingStates {
    Table,
    State,
    ProviderName,
    PkceVerifier,
    Nonce,
    RedirectUri,
    CreatedAt,
    ExpiresAt,
}
