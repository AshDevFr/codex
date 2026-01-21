use sea_orm_migration::prelude::*;

use crate::m20260103_000002_create_users::Users;
use crate::m20260103_000003_create_series::Series;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Create sharing_tags table - tags for controlling content access
        manager
            .create_table(
                Table::create()
                    .table(SharingTags::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(SharingTags::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(SharingTags::Name)
                            .string_len(100)
                            .not_null()
                            .unique_key(),
                    )
                    .col(
                        ColumnDef::new(SharingTags::NormalizedName)
                            .string_len(100)
                            .not_null()
                            .unique_key(),
                    )
                    .col(ColumnDef::new(SharingTags::Description).text())
                    .col(
                        ColumnDef::new(SharingTags::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SharingTags::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        // Index for fast lookup by normalized name
        manager
            .create_index(
                Index::create()
                    .name("idx_sharing_tags_normalized_name")
                    .table(SharingTags::Table)
                    .col(SharingTags::NormalizedName)
                    .to_owned(),
            )
            .await?;

        // Create series_sharing_tags junction table
        manager
            .create_table(
                Table::create()
                    .table(SeriesSharingTags::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(SeriesSharingTags::SeriesId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SeriesSharingTags::SharingTagId)
                            .uuid()
                            .not_null(),
                    )
                    .primary_key(
                        Index::create()
                            .col(SeriesSharingTags::SeriesId)
                            .col(SeriesSharingTags::SharingTagId),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_series_sharing_tags_series_id")
                            .from(SeriesSharingTags::Table, SeriesSharingTags::SeriesId)
                            .to(Series::Table, Series::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::NoAction),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_series_sharing_tags_sharing_tag_id")
                            .from(SeriesSharingTags::Table, SeriesSharingTags::SharingTagId)
                            .to(SharingTags::Table, SharingTags::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::NoAction),
                    )
                    .to_owned(),
            )
            .await?;

        // Index for filtering by sharing tag
        manager
            .create_index(
                Index::create()
                    .name("idx_series_sharing_tags_sharing_tag_id")
                    .table(SeriesSharingTags::Table)
                    .col(SeriesSharingTags::SharingTagId)
                    .to_owned(),
            )
            .await?;

        // Create user_sharing_tags table - user grants for sharing tags
        // access_mode: 'allow' = user can see content with this tag
        //              'deny' = user cannot see content with this tag (overrides allow)
        manager
            .create_table(
                Table::create()
                    .table(UserSharingTags::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(UserSharingTags::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(UserSharingTags::UserId).uuid().not_null())
                    .col(
                        ColumnDef::new(UserSharingTags::SharingTagId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(UserSharingTags::AccessMode)
                            .string_len(10)
                            .not_null()
                            .default("allow"),
                    )
                    .col(
                        ColumnDef::new(UserSharingTags::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_user_sharing_tags_user_id")
                            .from(UserSharingTags::Table, UserSharingTags::UserId)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::NoAction),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_user_sharing_tags_sharing_tag_id")
                            .from(UserSharingTags::Table, UserSharingTags::SharingTagId)
                            .to(SharingTags::Table, SharingTags::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::NoAction),
                    )
                    .to_owned(),
            )
            .await?;

        // Unique constraint: one grant per user per tag
        manager
            .create_index(
                Index::create()
                    .name("idx_user_sharing_tags_user_tag")
                    .table(UserSharingTags::Table)
                    .col(UserSharingTags::UserId)
                    .col(UserSharingTags::SharingTagId)
                    .unique()
                    .to_owned(),
            )
            .await?;

        // Index for fast user lookup
        manager
            .create_index(
                Index::create()
                    .name("idx_user_sharing_tags_user_id")
                    .table(UserSharingTags::Table)
                    .col(UserSharingTags::UserId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(UserSharingTags::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(SeriesSharingTags::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(SharingTags::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
pub enum SharingTags {
    Table,
    Id,
    Name,
    NormalizedName,
    Description,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
pub enum SeriesSharingTags {
    Table,
    SeriesId,
    SharingTagId,
}

#[derive(DeriveIden)]
pub enum UserSharingTags {
    Table,
    Id,
    UserId,
    SharingTagId,
    AccessMode,
    CreatedAt,
}
