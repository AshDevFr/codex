//! Refresh tokens: persist a hashed, rotatable refresh token alongside each access
//! token issued at login. `family_id` groups every rotation of one login so theft
//! detection can revoke the whole chain with a single update.

use sea_orm_migration::prelude::*;

use crate::m20260103_000002_create_users::Users;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(RefreshTokens::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(RefreshTokens::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(RefreshTokens::UserId).uuid().not_null())
                    .col(ColumnDef::new(RefreshTokens::FamilyId).uuid().not_null())
                    .col(
                        ColumnDef::new(RefreshTokens::TokenHash)
                            .string_len(64)
                            .not_null()
                            .unique_key(),
                    )
                    .col(
                        ColumnDef::new(RefreshTokens::IssuedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(RefreshTokens::ExpiresAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(ColumnDef::new(RefreshTokens::RevokedAt).timestamp_with_time_zone())
                    .col(ColumnDef::new(RefreshTokens::ReplacedBy).uuid())
                    .col(ColumnDef::new(RefreshTokens::UserAgent).string_len(512))
                    .col(ColumnDef::new(RefreshTokens::IpAddress).string_len(64))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_refresh_tokens_user_id")
                            .from(RefreshTokens::Table, RefreshTokens::UserId)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_refresh_tokens_replaced_by")
                            .from(RefreshTokens::Table, RefreshTokens::ReplacedBy)
                            .to(RefreshTokens::Table, RefreshTokens::Id)
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_refresh_tokens_user_id")
                    .table(RefreshTokens::Table)
                    .col(RefreshTokens::UserId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_refresh_tokens_family_id")
                    .table(RefreshTokens::Table)
                    .col(RefreshTokens::FamilyId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_refresh_tokens_expires_at")
                    .table(RefreshTokens::Table)
                    .col(RefreshTokens::ExpiresAt)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(RefreshTokens::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
pub enum RefreshTokens {
    Table,
    Id,
    UserId,
    FamilyId,
    TokenHash,
    IssuedAt,
    ExpiresAt,
    RevokedAt,
    ReplacedBy,
    UserAgent,
    IpAddress,
}
