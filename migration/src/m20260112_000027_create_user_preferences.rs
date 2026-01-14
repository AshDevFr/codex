use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let is_postgres = manager.get_database_backend() == sea_orm::DatabaseBackend::Postgres;

        let mut table = Table::create();
        table.table(UserPreferences::Table).if_not_exists();

        // ID column - different defaults for Postgres vs SQLite
        if is_postgres {
            table.col(
                ColumnDef::new(UserPreferences::Id)
                    .uuid()
                    .not_null()
                    .primary_key()
                    .extra("DEFAULT gen_random_uuid()"),
            );
        } else {
            table.col(
                ColumnDef::new(UserPreferences::Id)
                    .uuid()
                    .not_null()
                    .primary_key(),
            );
        }

        manager
            .create_table(
                table
                    // User reference
                    .col(ColumnDef::new(UserPreferences::UserId).uuid().not_null())
                    // Hierarchical key (e.g., "ui.theme", "reader.default_zoom")
                    .col(
                        ColumnDef::new(UserPreferences::Key)
                            .string_len(255)
                            .not_null(),
                    )
                    // JSON-serialized value for flexibility
                    .col(ColumnDef::new(UserPreferences::Value).text().not_null())
                    // Type information for validation: string, integer, float, boolean, json
                    .col(
                        ColumnDef::new(UserPreferences::ValueType)
                            .string_len(20)
                            .not_null()
                            .default("string"),
                    )
                    // Timestamps
                    .col({
                        let mut col = ColumnDef::new(UserPreferences::CreatedAt);
                        col.timestamp_with_time_zone().not_null();
                        if is_postgres {
                            col.extra("DEFAULT NOW()");
                        } else {
                            col.extra("DEFAULT CURRENT_TIMESTAMP");
                        }
                        col
                    })
                    .col({
                        let mut col = ColumnDef::new(UserPreferences::UpdatedAt);
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
                            .name("fk_user_preferences_user_id")
                            .from(UserPreferences::Table, UserPreferences::UserId)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::NoAction),
                    )
                    .to_owned(),
            )
            .await?;

        // Unique constraint on (user_id, key)
        manager
            .create_index(
                Index::create()
                    .name("idx_user_preferences_user_key")
                    .table(UserPreferences::Table)
                    .col(UserPreferences::UserId)
                    .col(UserPreferences::Key)
                    .unique()
                    .to_owned(),
            )
            .await?;

        // Index on user_id for fetching all preferences for a user
        manager
            .create_index(
                Index::create()
                    .name("idx_user_preferences_user_id")
                    .table(UserPreferences::Table)
                    .col(UserPreferences::UserId)
                    .to_owned(),
            )
            .await?;

        // Index on key for finding all users with a specific preference
        manager
            .create_index(
                Index::create()
                    .name("idx_user_preferences_key")
                    .table(UserPreferences::Table)
                    .col(UserPreferences::Key)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(UserPreferences::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum UserPreferences {
    Table,
    Id,
    UserId,
    Key,
    Value,
    ValueType,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum Users {
    Table,
    Id,
}
