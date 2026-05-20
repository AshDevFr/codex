use sea_orm_migration::prelude::*;

use crate::m20260103_000002_create_users::Users;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let is_postgres = manager.get_database_backend() == sea_orm::DatabaseBackend::Postgres;

        let mut table = Table::create();
        table.table(FilterPresets::Table).if_not_exists();

        if is_postgres {
            table.col(
                ColumnDef::new(FilterPresets::Id)
                    .uuid()
                    .not_null()
                    .primary_key()
                    .extra("DEFAULT gen_random_uuid()"),
            );
        } else {
            table.col(
                ColumnDef::new(FilterPresets::Id)
                    .uuid()
                    .not_null()
                    .primary_key(),
            );
        }

        manager
            .create_table(
                table
                    .col(ColumnDef::new(FilterPresets::UserId).uuid().not_null())
                    .col(ColumnDef::new(FilterPresets::LibraryId).uuid())
                    .col(
                        ColumnDef::new(FilterPresets::Name)
                            .string_len(100)
                            .not_null(),
                    )
                    // 'list' | 'search'
                    .col(
                        ColumnDef::new(FilterPresets::Scope)
                            .string_len(16)
                            .not_null(),
                    )
                    // 'series' | 'books'
                    .col(
                        ColumnDef::new(FilterPresets::Target)
                            .string_len(16)
                            .not_null(),
                    )
                    // Serialized SeriesCondition or BookCondition (per `target`)
                    .col(
                        ColumnDef::new(FilterPresets::Condition)
                            .json_binary()
                            .not_null(),
                    )
                    // Optional text query (search-scope) or list-page search box value
                    .col(ColumnDef::new(FilterPresets::Query).text())
                    // Sort key, mirrors URL sort parameter (e.g. "title:asc")
                    .col(ColumnDef::new(FilterPresets::Sort).text())
                    .col({
                        let mut col = ColumnDef::new(FilterPresets::CreatedAt);
                        col.timestamp_with_time_zone().not_null();
                        if is_postgres {
                            col.extra("DEFAULT NOW()");
                        } else {
                            col.extra("DEFAULT CURRENT_TIMESTAMP");
                        }
                        col
                    })
                    .col({
                        let mut col = ColumnDef::new(FilterPresets::UpdatedAt);
                        col.timestamp_with_time_zone().not_null();
                        if is_postgres {
                            col.extra("DEFAULT NOW()");
                        } else {
                            col.extra("DEFAULT CURRENT_TIMESTAMP");
                        }
                        col
                    })
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_filter_presets_user_id")
                            .from(FilterPresets::Table, FilterPresets::UserId)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::NoAction),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_filter_presets_library_id")
                            .from(FilterPresets::Table, FilterPresets::LibraryId)
                            .to(Libraries::Table, Libraries::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::NoAction),
                    )
                    .to_owned(),
            )
            .await?;

        // Index for listing a user's presets
        manager
            .create_index(
                Index::create()
                    .name("idx_filter_presets_user")
                    .table(FilterPresets::Table)
                    .col(FilterPresets::UserId)
                    .to_owned(),
            )
            .await?;

        // Index for the common (user, scope, target) lookup pattern
        manager
            .create_index(
                Index::create()
                    .name("idx_filter_presets_lookup")
                    .table(FilterPresets::Table)
                    .col(FilterPresets::UserId)
                    .col(FilterPresets::Scope)
                    .col(FilterPresets::Target)
                    .to_owned(),
            )
            .await?;

        // Partial index for cascade lookups when a library is dropped
        manager
            .get_connection()
            .execute_unprepared(
                r#"
                CREATE INDEX idx_filter_presets_library
                ON filter_presets(library_id)
                WHERE library_id IS NOT NULL
                "#,
            )
            .await?;

        // Uniqueness: a given user cannot have two presets with the same
        // (scope, target, library_id, name). NULL library_ids are split into
        // their own partial index so two distinct users-or-scopes can both
        // own a "global" preset of the same name, but a single user cannot.
        manager
            .get_connection()
            .execute_unprepared(
                r#"
                CREATE UNIQUE INDEX uq_filter_presets_name_scoped
                ON filter_presets(user_id, scope, target, library_id, name)
                WHERE library_id IS NOT NULL
                "#,
            )
            .await?;

        manager
            .get_connection()
            .execute_unprepared(
                r#"
                CREATE UNIQUE INDEX uq_filter_presets_name_global
                ON filter_presets(user_id, scope, target, name)
                WHERE library_id IS NULL
                "#,
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(FilterPresets::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum FilterPresets {
    Table,
    Id,
    UserId,
    LibraryId,
    Name,
    Scope,
    Target,
    Condition,
    Query,
    Sort,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum Libraries {
    Table,
    Id,
}
