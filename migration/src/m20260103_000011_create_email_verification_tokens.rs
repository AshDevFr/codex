use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(EmailVerificationTokens::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(EmailVerificationTokens::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(EmailVerificationTokens::UserId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(EmailVerificationTokens::Token)
                            .text()
                            .not_null()
                            .unique_key(),
                    )
                    .col(
                        ColumnDef::new(EmailVerificationTokens::ExpiresAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(EmailVerificationTokens::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-email_verification_tokens-user_id")
                            .from(
                                EmailVerificationTokens::Table,
                                EmailVerificationTokens::UserId,
                            )
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // Create index on user_id for faster lookups
        manager
            .create_index(
                Index::create()
                    .name("idx-email_verification_tokens-user_id")
                    .table(EmailVerificationTokens::Table)
                    .col(EmailVerificationTokens::UserId)
                    .to_owned(),
            )
            .await?;

        // Create index on expires_at for cleanup queries
        manager
            .create_index(
                Index::create()
                    .name("idx-email_verification_tokens-expires_at")
                    .table(EmailVerificationTokens::Table)
                    .col(EmailVerificationTokens::ExpiresAt)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(EmailVerificationTokens::Table)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum EmailVerificationTokens {
    Table,
    Id,
    UserId,
    Token,
    ExpiresAt,
    CreatedAt,
}

#[derive(DeriveIden)]
enum Users {
    Table,
    Id,
}
