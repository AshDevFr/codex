use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let is_postgres = manager.get_database_backend() == sea_orm::DatabaseBackend::Postgres;

        let mut table = Table::create();
        table.table(Settings::Table).if_not_exists();

        // ID column - different defaults for Postgres vs SQLite
        if is_postgres {
            table.col(
                ColumnDef::new(Settings::Id)
                    .uuid()
                    .not_null()
                    .primary_key()
                    .extra("DEFAULT gen_random_uuid()"),
            );
        } else {
            table.col(ColumnDef::new(Settings::Id).uuid().not_null().primary_key());
        }

        manager
            .create_table(
                table
                    // Hierarchical key (e.g., "scanner.max_concurrent_scans")
                    .col(
                        ColumnDef::new(Settings::Key)
                            .string_len(255)
                            .not_null()
                            .unique_key(),
                    )
                    // JSON-serialized value for flexibility
                    .col(ColumnDef::new(Settings::Value).text().not_null())
                    // Type information for validation
                    .col(
                        ColumnDef::new(Settings::ValueType)
                            .string_len(20)
                            .not_null(),
                    )
                    // Category for UI grouping
                    .col(ColumnDef::new(Settings::Category).string_len(50).not_null())
                    // Human-readable description for admin UI
                    .col(ColumnDef::new(Settings::Description).text().not_null())
                    // Mask in UI/logs if true
                    .col(
                        ColumnDef::new(Settings::IsSensitive)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    // Fallback value if setting is deleted
                    .col(ColumnDef::new(Settings::DefaultValue).text().not_null())
                    // Validation constraints (JSON schema or simple rules)
                    .col(ColumnDef::new(Settings::ValidationRules).text())
                    // Minimum value (for numeric types)
                    .col(ColumnDef::new(Settings::MinValue).big_integer())
                    // Maximum value (for numeric types)
                    .col(ColumnDef::new(Settings::MaxValue).big_integer())
                    // Audit trail - timestamps
                    .col({
                        let mut col = ColumnDef::new(Settings::UpdatedAt);
                        col.timestamp_with_time_zone().not_null();
                        if is_postgres {
                            col.extra("DEFAULT NOW()");
                        } else {
                            col.extra("DEFAULT CURRENT_TIMESTAMP");
                        }
                        col
                    })
                    // Audit trail - user who updated
                    .col(ColumnDef::new(Settings::UpdatedBy).uuid())
                    // Optimistic locking
                    .col(
                        ColumnDef::new(Settings::Version)
                            .integer()
                            .not_null()
                            .default(1),
                    )
                    // Soft delete support
                    .col(ColumnDef::new(Settings::DeletedAt).timestamp_with_time_zone())
                    // NOTE: Foreign key to users table commented out to allow system updates
                    // without requiring a user (e.g., during seeding, tests)
                    // In production, updated_by should be set when changes are made via API
                    // .foreign_key(
                    //     ForeignKey::create()
                    //         .name("fk_settings_updated_by")
                    //         .from(Settings::Table, Settings::UpdatedBy)
                    //         .to(Users::Table, Users::Id)
                    //         .on_delete(ForeignKeyAction::SetNull)
                    //         .on_update(ForeignKeyAction::NoAction),
                    // )
                    .to_owned(),
            )
            .await?;

        // Unique index on key (excluding soft-deleted records)
        if is_postgres {
            manager
                .get_connection()
                .execute_unprepared(
                    r#"
                    CREATE UNIQUE INDEX idx_settings_key ON settings(key)
                    WHERE deleted_at IS NULL
                    "#,
                )
                .await?;
        } else {
            // SQLite doesn't support WHERE in CREATE INDEX via SeaORM schema builder
            // So we use a regular unique index on the key column (already done above)
            manager
                .create_index(
                    Index::create()
                        .name("idx_settings_key")
                        .table(Settings::Table)
                        .col(Settings::Key)
                        .unique()
                        .to_owned(),
                )
                .await?;
        }

        // Index on category for filtering
        manager
            .create_index(
                Index::create()
                    .name("idx_settings_category")
                    .table(Settings::Table)
                    .col(Settings::Category)
                    .to_owned(),
            )
            .await?;

        // Index on updated_at for sorting
        manager
            .create_index(
                Index::create()
                    .name("idx_settings_updated_at")
                    .table(Settings::Table)
                    .col(Settings::UpdatedAt)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Settings::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum Settings {
    Table,
    Id,
    Key,
    Value,
    ValueType,
    Category,
    Description,
    IsSensitive,
    DefaultValue,
    ValidationRules,
    MinValue,
    MaxValue,
    UpdatedAt,
    UpdatedBy,
    Version,
    DeletedAt,
}
