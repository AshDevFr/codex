//! Default access group.
//!
//! At most one access group may be the default. New users are added to it
//! when their account is created (membership source `default`, which fits the
//! existing `varchar(10)` source column). The membership is an ordinary row an
//! admin can remove later, not a rule that applies to everyone.
//!
//! "At most one" is a partial unique index over the `true` rows, which both
//! SQLite and PostgreSQL support. SeaQuery's index builder has no partial
//! form, so it is raw SQL; the statement is identical on both backends.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const SINGLE_DEFAULT_INDEX: &str = "idx_access_groups_single_default";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(AccessGroups::Table)
                    .add_column(
                        ColumnDef::new(AccessGroups::IsDefault)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .get_connection()
            .execute_unprepared(&format!(
                "CREATE UNIQUE INDEX {SINGLE_DEFAULT_INDEX} \
                 ON access_groups (is_default) WHERE is_default"
            ))
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name(SINGLE_DEFAULT_INDEX)
                    .table(AccessGroups::Table)
                    .to_owned(),
            )
            .await?;

        // Default memberships stay, as manual ones: the user is still in the
        // group, only the reason is gone.
        manager
            .get_connection()
            .execute_unprepared(
                "UPDATE user_access_groups SET source = 'manual' WHERE source = 'default'",
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(AccessGroups::Table)
                    .drop_column(AccessGroups::IsDefault)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
enum AccessGroups {
    Table,
    IsDefault,
}
