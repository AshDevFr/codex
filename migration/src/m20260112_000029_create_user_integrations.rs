use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let is_postgres = manager.get_database_backend() == sea_orm::DatabaseBackend::Postgres;

        let mut table = Table::create();
        table.table(UserIntegrations::Table).if_not_exists();

        // ID column - different defaults for Postgres vs SQLite
        if is_postgres {
            table.col(
                ColumnDef::new(UserIntegrations::Id)
                    .uuid()
                    .not_null()
                    .primary_key()
                    .extra("DEFAULT gen_random_uuid()"),
            );
        } else {
            table.col(
                ColumnDef::new(UserIntegrations::Id)
                    .uuid()
                    .not_null()
                    .primary_key(),
            );
        }

        manager
            .create_table(
                table
                    // User reference
                    .col(ColumnDef::new(UserIntegrations::UserId).uuid().not_null())
                    // Integration identity
                    .col(
                        ColumnDef::new(UserIntegrations::IntegrationName)
                            .string_len(100)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(UserIntegrations::DisplayName)
                            .string_len(255)
                            .null(),
                    )
                    // Encrypted credentials (OAuth tokens, API keys)
                    .col(
                        ColumnDef::new(UserIntegrations::Credentials)
                            .binary()
                            .not_null(),
                    )
                    // User preferences for this integration
                    .col(
                        ColumnDef::new(UserIntegrations::Settings)
                            .json()
                            .not_null()
                            .default("{}"),
                    )
                    // State
                    .col(
                        ColumnDef::new(UserIntegrations::Enabled)
                            .boolean()
                            .not_null()
                            .default(true),
                    )
                    .col(ColumnDef::new(UserIntegrations::LastSyncAt).timestamp_with_time_zone())
                    .col(ColumnDef::new(UserIntegrations::LastError).text())
                    .col(
                        ColumnDef::new(UserIntegrations::SyncStatus)
                            .string_len(20)
                            .not_null()
                            .default("idle"),
                    )
                    // External user info (cached from provider)
                    .col(ColumnDef::new(UserIntegrations::ExternalUserId).string_len(100))
                    .col(ColumnDef::new(UserIntegrations::ExternalUsername).string_len(255))
                    // Token expiry tracking
                    .col(
                        ColumnDef::new(UserIntegrations::TokenExpiresAt).timestamp_with_time_zone(),
                    )
                    // Timestamps
                    .col({
                        let mut col = ColumnDef::new(UserIntegrations::CreatedAt);
                        col.timestamp_with_time_zone().not_null();
                        if is_postgres {
                            col.extra("DEFAULT NOW()");
                        } else {
                            col.extra("DEFAULT CURRENT_TIMESTAMP");
                        }
                        col
                    })
                    .col({
                        let mut col = ColumnDef::new(UserIntegrations::UpdatedAt);
                        col.timestamp_with_time_zone().not_null();
                        if is_postgres {
                            col.extra("DEFAULT NOW()");
                        } else {
                            col.extra("DEFAULT CURRENT_TIMESTAMP");
                        }
                        col
                    })
                    // Foreign key to users table
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_user_integrations_user_id")
                            .from(UserIntegrations::Table, UserIntegrations::UserId)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::NoAction),
                    )
                    .to_owned(),
            )
            .await?;

        // Unique constraint on (user_id, integration_name)
        manager
            .create_index(
                Index::create()
                    .name("idx_user_integrations_user_name")
                    .table(UserIntegrations::Table)
                    .col(UserIntegrations::UserId)
                    .col(UserIntegrations::IntegrationName)
                    .unique()
                    .to_owned(),
            )
            .await?;

        // Index on user_id for fetching all integrations for a user
        manager
            .create_index(
                Index::create()
                    .name("idx_user_integrations_user_id")
                    .table(UserIntegrations::Table)
                    .col(UserIntegrations::UserId)
                    .to_owned(),
            )
            .await?;

        // Index on integration_name for finding all users with a specific integration
        manager
            .create_index(
                Index::create()
                    .name("idx_user_integrations_name")
                    .table(UserIntegrations::Table)
                    .col(UserIntegrations::IntegrationName)
                    .to_owned(),
            )
            .await?;

        // Index on enabled for finding active integrations
        manager
            .create_index(
                Index::create()
                    .name("idx_user_integrations_enabled")
                    .table(UserIntegrations::Table)
                    .col(UserIntegrations::Enabled)
                    .to_owned(),
            )
            .await?;

        // Index on sync_status for finding integrations needing sync
        manager
            .create_index(
                Index::create()
                    .name("idx_user_integrations_sync_status")
                    .table(UserIntegrations::Table)
                    .col(UserIntegrations::SyncStatus)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(UserIntegrations::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum UserIntegrations {
    Table,
    Id,
    UserId,
    IntegrationName,
    DisplayName,
    Credentials,
    Settings,
    Enabled,
    LastSyncAt,
    LastError,
    SyncStatus,
    ExternalUserId,
    ExternalUsername,
    TokenExpiresAt,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum Users {
    Table,
    Id,
}
