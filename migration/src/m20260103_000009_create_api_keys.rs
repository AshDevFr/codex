use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(ApiKeys::Table)
                    .if_not_exists()
                    .col(uuid(ApiKeys::Id).primary_key())
                    .col(uuid(ApiKeys::UserId).not_null())
                    .col(string(ApiKeys::Name).not_null())
                    .col(string_len(ApiKeys::KeyHash, 255).not_null().unique_key())
                    .col(string_len(ApiKeys::KeyPrefix, 32).not_null())
                    .col(json(ApiKeys::Permissions).not_null())
                    .col(boolean(ApiKeys::IsActive).not_null().default(true))
                    .col(timestamp_with_time_zone_null(ApiKeys::ExpiresAt))
                    .col(timestamp_with_time_zone_null(ApiKeys::LastUsedAt))
                    .col(timestamp_with_time_zone(ApiKeys::CreatedAt).not_null())
                    .col(timestamp_with_time_zone(ApiKeys::UpdatedAt).not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_api_keys_user_id")
                            .from(ApiKeys::Table, ApiKeys::UserId)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // Create indexes
        manager
            .create_index(
                Index::create()
                    .name("idx_api_keys_user_id")
                    .table(ApiKeys::Table)
                    .col(ApiKeys::UserId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_api_keys_key_hash")
                    .table(ApiKeys::Table)
                    .col(ApiKeys::KeyHash)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_api_keys_is_active")
                    .table(ApiKeys::Table)
                    .col(ApiKeys::IsActive)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_api_keys_key_prefix")
                    .table(ApiKeys::Table)
                    .col(ApiKeys::KeyPrefix)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ApiKeys::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum ApiKeys {
    Table,
    Id,
    UserId,
    Name,
    KeyHash,
    KeyPrefix,
    Permissions,
    IsActive,
    ExpiresAt,
    LastUsedAt,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum Users {
    Table,
    Id,
}
