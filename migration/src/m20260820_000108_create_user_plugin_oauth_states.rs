//! Create the `user_plugin_oauth_states` table: the server-side half of a
//! plugin OAuth connect flow, held between the authorization request and the
//! callback.
//!
//! The same defect the `oidc_pending_states` table fixes, in a second place.
//! These two legs are separate HTTP requests, so a deployment running more
//! than one `codex serve` behind a load balancer without session affinity
//! routes them to different processes and the flow can never complete.
//!
//! Storing them here also fixes the sweep, which was broken independently of
//! the split. The manager holding the real flows was built by `serve`, while
//! the cleanup handler that swept it received an instance built by `worker`.
//! Those are separate deployments in production, so the sweep ran against a
//! map that was always empty while the flows accumulated untouched. A table
//! any process can reach removes the question of which process runs the sweep.
//!
//! `state` is the CSRF token and the primary key. Consumption is a single
//! `DELETE ... RETURNING`, so the engine decides which of two racing callbacks
//! is handed the PKCE verifier.
//!
//! `pkce_verifier` is plaintext, matching `oidc_pending_states`: making it
//! depend on the optional `CODEX_ENCRYPTION_KEY` would break plugin connect
//! wherever that key is unset. Rows live at most five minutes and are deleted
//! on use.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(UserPluginOauthStates::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(UserPluginOauthStates::State)
                            .string()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(UserPluginOauthStates::PluginId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(UserPluginOauthStates::UserId)
                            .uuid()
                            .not_null(),
                    )
                    // Absent when the plugin's OAuth config disables PKCE.
                    .col(
                        ColumnDef::new(UserPluginOauthStates::PkceVerifier)
                            .string()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(UserPluginOauthStates::PkceChallenge)
                            .string()
                            .null(),
                    )
                    // Must be replayed verbatim in the token exchange, so it is
                    // stored rather than rebuilt from config that may have
                    // changed while the user was away at the provider.
                    .col(
                        ColumnDef::new(UserPluginOauthStates::RedirectUri)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(UserPluginOauthStates::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(UserPluginOauthStates::ExpiresAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_user_plugin_oauth_states_plugin")
                            .from(
                                UserPluginOauthStates::Table,
                                UserPluginOauthStates::PluginId,
                            )
                            .to(Plugins::Table, Plugins::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_user_plugin_oauth_states_user")
                            .from(UserPluginOauthStates::Table, UserPluginOauthStates::UserId)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // Serves the periodic sweep.
        manager
            .create_index(
                Index::create()
                    .name("idx_user_plugin_oauth_states_expires_at")
                    .table(UserPluginOauthStates::Table)
                    .col(UserPluginOauthStates::ExpiresAt)
                    .to_owned(),
            )
            .await?;

        // Serves the per-user concurrent-flow limit, which counts a user's
        // live flows on every connect attempt.
        manager
            .create_index(
                Index::create()
                    .name("idx_user_plugin_oauth_states_user_id")
                    .table(UserPluginOauthStates::Table)
                    .col(UserPluginOauthStates::UserId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(UserPluginOauthStates::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
pub enum UserPluginOauthStates {
    Table,
    State,
    PluginId,
    UserId,
    PkceVerifier,
    PkceChallenge,
    RedirectUri,
    CreatedAt,
    ExpiresAt,
}

#[derive(DeriveIden)]
enum Plugins {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum Users {
    Table,
    Id,
}
