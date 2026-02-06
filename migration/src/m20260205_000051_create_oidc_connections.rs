use sea_orm_migration::prelude::*;

use crate::m20260103_000002_create_users::Users;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Create oidc_connections table
        manager
            .create_table(
                Table::create()
                    .table(OidcConnections::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(OidcConnections::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(OidcConnections::UserId).uuid().not_null())
                    .col(
                        ColumnDef::new(OidcConnections::ProviderName)
                            .string_len(100)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(OidcConnections::Subject)
                            .string_len(255)
                            .not_null(),
                    )
                    .col(ColumnDef::new(OidcConnections::Email).string_len(255))
                    .col(ColumnDef::new(OidcConnections::DisplayName).string_len(255))
                    .col(ColumnDef::new(OidcConnections::Groups).json())
                    .col(ColumnDef::new(OidcConnections::AccessTokenHash).string_len(64))
                    .col(ColumnDef::new(OidcConnections::RefreshTokenEncrypted).binary())
                    .col(ColumnDef::new(OidcConnections::TokenExpiresAt).timestamp_with_time_zone())
                    .col(
                        ColumnDef::new(OidcConnections::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(OidcConnections::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(ColumnDef::new(OidcConnections::LastUsedAt).timestamp_with_time_zone())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_oidc_connections_user")
                            .from(OidcConnections::Table, OidcConnections::UserId)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // Create unique constraint on provider_name + subject
        manager
            .create_index(
                Index::create()
                    .name("idx_oidc_connections_provider_subject")
                    .table(OidcConnections::Table)
                    .col(OidcConnections::ProviderName)
                    .col(OidcConnections::Subject)
                    .unique()
                    .to_owned(),
            )
            .await?;

        // Create index on user_id for fast lookups
        manager
            .create_index(
                Index::create()
                    .name("idx_oidc_connections_user_id")
                    .table(OidcConnections::Table)
                    .col(OidcConnections::UserId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(OidcConnections::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
pub enum OidcConnections {
    Table,
    Id,
    UserId,
    ProviderName,
    Subject,
    Email,
    DisplayName,
    Groups,
    AccessTokenHash,
    RefreshTokenEncrypted,
    TokenExpiresAt,
    CreatedAt,
    UpdatedAt,
    LastUsedAt,
}
