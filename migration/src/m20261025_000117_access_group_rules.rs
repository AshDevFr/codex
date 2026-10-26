//! Access group rules on regular tags, genres and libraries.
//!
//! Until now an access group could only allow or deny sharing tags. These
//! tables let it allow or deny regular tags, genres and libraries as well,
//! each rule `allow` or `deny`, one rule per target per group.
//!
//! Tag and genre rules store the normalized name, not a foreign key. Unused
//! tags and genres are deleted by cleanup, and the next series to carry the
//! name recreates it under a new id. A foreign key would cascade the rule away
//! with the old row and silently drop a deny. Matching on the normalized name
//! survives that. The display name is kept alongside for the UI.
//!
//! Library rules keep a foreign key: libraries are only ever deleted by an
//! admin, and their series go with them.
//!
//! The three match columns on `access_groups` choose, per type, whether a
//! group's allowed values combine as "any of" (the default, and what sharing
//! tags have always done) or "all of". Libraries have no switch: a series is
//! in exactly one library, so "all of" two libraries could never match.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        for column in [
            AccessGroups::SharingTagMatch,
            AccessGroups::TagMatch,
            AccessGroups::GenreMatch,
        ] {
            manager
                .alter_table(
                    Table::alter()
                        .table(AccessGroups::Table)
                        .add_column(
                            ColumnDef::new(column)
                                .string_len(10)
                                .not_null()
                                .default("any"),
                        )
                        .to_owned(),
                )
                .await?;
        }

        create_name_rule_table(
            manager,
            AccessGroupTagRules::Table,
            "access_group_tag_rules",
        )
        .await?;
        create_name_rule_table(
            manager,
            AccessGroupGenreRules::Table,
            "access_group_genre_rules",
        )
        .await?;

        manager
            .create_table(
                Table::create()
                    .table(AccessGroupLibraryRules::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(RuleColumns::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(RuleColumns::AccessGroupId).uuid().not_null())
                    .col(
                        ColumnDef::new(AccessGroupLibraryRules::LibraryId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(RuleColumns::AccessMode)
                            .string_len(10)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(RuleColumns::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_access_group_library_rules_group_id")
                            .from(AccessGroupLibraryRules::Table, RuleColumns::AccessGroupId)
                            .to(AccessGroups::Table, AccessGroups::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::NoAction),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_access_group_library_rules_library_id")
                            .from(
                                AccessGroupLibraryRules::Table,
                                AccessGroupLibraryRules::LibraryId,
                            )
                            .to(Libraries::Table, Libraries::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::NoAction),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_access_group_library_rules_group_library")
                    .table(AccessGroupLibraryRules::Table)
                    .col(RuleColumns::AccessGroupId)
                    .col(AccessGroupLibraryRules::LibraryId)
                    .unique()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        for table in [
            AccessGroupLibraryRules::Table.into_iden(),
            AccessGroupGenreRules::Table.into_iden(),
            AccessGroupTagRules::Table.into_iden(),
        ] {
            manager
                .drop_table(Table::drop().table(table).if_exists().to_owned())
                .await?;
        }

        for column in [
            AccessGroups::GenreMatch,
            AccessGroups::TagMatch,
            AccessGroups::SharingTagMatch,
        ] {
            manager
                .alter_table(
                    Table::alter()
                        .table(AccessGroups::Table)
                        .drop_column(column)
                        .to_owned(),
                )
                .await?;
        }

        Ok(())
    }
}

/// Create a tag-style rule table keyed by normalized name.
async fn create_name_rule_table<T>(
    manager: &SchemaManager<'_>,
    table: T,
    name: &str,
) -> Result<(), DbErr>
where
    T: IntoIden + Copy + 'static,
{
    manager
        .create_table(
            Table::create()
                .table(table)
                .if_not_exists()
                .col(
                    ColumnDef::new(RuleColumns::Id)
                        .uuid()
                        .not_null()
                        .primary_key(),
                )
                .col(ColumnDef::new(RuleColumns::AccessGroupId).uuid().not_null())
                .col(ColumnDef::new(RuleColumns::Name).string().not_null())
                .col(
                    ColumnDef::new(RuleColumns::NormalizedName)
                        .string()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(RuleColumns::AccessMode)
                        .string_len(10)
                        .not_null(),
                )
                .col(
                    ColumnDef::new(RuleColumns::CreatedAt)
                        .timestamp_with_time_zone()
                        .not_null(),
                )
                .foreign_key(
                    ForeignKey::create()
                        .name(format!("fk_{name}_group_id"))
                        .from(table, RuleColumns::AccessGroupId)
                        .to(AccessGroups::Table, AccessGroups::Id)
                        .on_delete(ForeignKeyAction::Cascade)
                        .on_update(ForeignKeyAction::NoAction),
                )
                .to_owned(),
        )
        .await?;

    manager
        .create_index(
            Index::create()
                .name(format!("idx_{name}_group_name"))
                .table(table)
                .col(RuleColumns::AccessGroupId)
                .col(RuleColumns::NormalizedName)
                .unique()
                .to_owned(),
        )
        .await
}

#[derive(DeriveIden)]
enum AccessGroups {
    Table,
    Id,
    SharingTagMatch,
    TagMatch,
    GenreMatch,
}

#[derive(DeriveIden, Clone, Copy)]
enum AccessGroupTagRules {
    Table,
}

#[derive(DeriveIden, Clone, Copy)]
enum AccessGroupGenreRules {
    Table,
}

#[derive(DeriveIden)]
enum AccessGroupLibraryRules {
    Table,
    LibraryId,
}

/// Columns shared by every rule table.
#[derive(DeriveIden)]
enum RuleColumns {
    Id,
    AccessGroupId,
    Name,
    NormalizedName,
    AccessMode,
    CreatedAt,
}

#[derive(DeriveIden)]
enum Libraries {
    Table,
    Id,
}
