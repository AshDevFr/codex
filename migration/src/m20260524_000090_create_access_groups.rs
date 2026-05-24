use sea_orm_migration::prelude::*;

use crate::m20260103_000002_create_users::Users;
use crate::m20260120_000030_create_sharing_tags::SharingTags;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // 1. access_groups - the group itself
        manager
            .create_table(
                Table::create()
                    .table(AccessGroups::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(AccessGroups::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(AccessGroups::Name)
                            .string_len(100)
                            .not_null()
                            .unique_key(),
                    )
                    .col(ColumnDef::new(AccessGroups::Description).text())
                    .col(
                        ColumnDef::new(AccessGroups::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(AccessGroups::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        // 2. user_access_groups - user <-> group membership (M:N) with provenance
        manager
            .create_table(
                Table::create()
                    .table(UserAccessGroups::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(UserAccessGroups::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(UserAccessGroups::UserId).uuid().not_null())
                    .col(
                        ColumnDef::new(UserAccessGroups::AccessGroupId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(UserAccessGroups::Source)
                            .string_len(10)
                            .not_null()
                            .default("manual"),
                    )
                    .col(
                        ColumnDef::new(UserAccessGroups::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_user_access_groups_user_id")
                            .from(UserAccessGroups::Table, UserAccessGroups::UserId)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::NoAction),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_user_access_groups_access_group_id")
                            .from(UserAccessGroups::Table, UserAccessGroups::AccessGroupId)
                            .to(AccessGroups::Table, AccessGroups::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::NoAction),
                    )
                    .to_owned(),
            )
            .await?;

        // Unique constraint: one membership per user per group
        manager
            .create_index(
                Index::create()
                    .name("idx_user_access_groups_user_group")
                    .table(UserAccessGroups::Table)
                    .col(UserAccessGroups::UserId)
                    .col(UserAccessGroups::AccessGroupId)
                    .unique()
                    .to_owned(),
            )
            .await?;

        // Index for fast user lookup
        manager
            .create_index(
                Index::create()
                    .name("idx_user_access_groups_user_id")
                    .table(UserAccessGroups::Table)
                    .col(UserAccessGroups::UserId)
                    .to_owned(),
            )
            .await?;

        // Index for fast group lookup
        manager
            .create_index(
                Index::create()
                    .name("idx_user_access_groups_group_id")
                    .table(UserAccessGroups::Table)
                    .col(UserAccessGroups::AccessGroupId)
                    .to_owned(),
            )
            .await?;

        // 3. access_group_sharing_tags - group <-> sharing tag grants
        manager
            .create_table(
                Table::create()
                    .table(AccessGroupSharingTags::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(AccessGroupSharingTags::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(AccessGroupSharingTags::AccessGroupId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(AccessGroupSharingTags::SharingTagId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(AccessGroupSharingTags::AccessMode)
                            .string_len(10)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(AccessGroupSharingTags::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_access_group_sharing_tags_group_id")
                            .from(
                                AccessGroupSharingTags::Table,
                                AccessGroupSharingTags::AccessGroupId,
                            )
                            .to(AccessGroups::Table, AccessGroups::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::NoAction),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_access_group_sharing_tags_tag_id")
                            .from(
                                AccessGroupSharingTags::Table,
                                AccessGroupSharingTags::SharingTagId,
                            )
                            .to(SharingTags::Table, SharingTags::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::NoAction),
                    )
                    .to_owned(),
            )
            .await?;

        // Unique constraint: one grant per group per tag
        manager
            .create_index(
                Index::create()
                    .name("idx_access_group_sharing_tags_group_tag")
                    .table(AccessGroupSharingTags::Table)
                    .col(AccessGroupSharingTags::AccessGroupId)
                    .col(AccessGroupSharingTags::SharingTagId)
                    .unique()
                    .to_owned(),
            )
            .await?;

        // Index for fast group lookup
        manager
            .create_index(
                Index::create()
                    .name("idx_group_sharing_tags_group_id")
                    .table(AccessGroupSharingTags::Table)
                    .col(AccessGroupSharingTags::AccessGroupId)
                    .to_owned(),
            )
            .await?;

        // 4. access_group_oidc_mappings - OIDC group name -> access group mapping
        manager
            .create_table(
                Table::create()
                    .table(AccessGroupOidcMappings::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(AccessGroupOidcMappings::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(AccessGroupOidcMappings::AccessGroupId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(AccessGroupOidcMappings::OidcGroupName)
                            .string_len(255)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(AccessGroupOidcMappings::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_access_group_oidc_mappings_group_id")
                            .from(
                                AccessGroupOidcMappings::Table,
                                AccessGroupOidcMappings::AccessGroupId,
                            )
                            .to(AccessGroups::Table, AccessGroups::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::NoAction),
                    )
                    .to_owned(),
            )
            .await?;

        // Unique constraint: one mapping per group per OIDC group name
        manager
            .create_index(
                Index::create()
                    .name("idx_oidc_mappings_group_oidc_name")
                    .table(AccessGroupOidcMappings::Table)
                    .col(AccessGroupOidcMappings::AccessGroupId)
                    .col(AccessGroupOidcMappings::OidcGroupName)
                    .unique()
                    .to_owned(),
            )
            .await?;

        // Index for fast OIDC group name lookup (used during login reconciliation)
        manager
            .create_index(
                Index::create()
                    .name("idx_oidc_mappings_oidc_name")
                    .table(AccessGroupOidcMappings::Table)
                    .col(AccessGroupOidcMappings::OidcGroupName)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Drop in reverse order to respect FK dependencies
        manager
            .drop_table(
                Table::drop()
                    .table(AccessGroupOidcMappings::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(
                Table::drop()
                    .table(AccessGroupSharingTags::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(Table::drop().table(UserAccessGroups::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(AccessGroups::Table).to_owned())
            .await?;
        Ok(())
    }
}

#[derive(DeriveIden)]
pub enum AccessGroups {
    Table,
    Id,
    Name,
    Description,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
pub enum UserAccessGroups {
    Table,
    Id,
    UserId,
    AccessGroupId,
    Source,
    CreatedAt,
}

#[derive(DeriveIden)]
pub enum AccessGroupSharingTags {
    Table,
    Id,
    AccessGroupId,
    SharingTagId,
    AccessMode,
    CreatedAt,
}

#[derive(DeriveIden)]
pub enum AccessGroupOidcMappings {
    Table,
    Id,
    AccessGroupId,
    OidcGroupName,
    CreatedAt,
}
