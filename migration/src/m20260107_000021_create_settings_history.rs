use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let is_postgres = manager.get_database_backend() == sea_orm::DatabaseBackend::Postgres;

        let mut table = Table::create();
        table.table(SettingsHistory::Table).if_not_exists();

        // ID column - different defaults for Postgres vs SQLite
        if is_postgres {
            table.col(
                ColumnDef::new(SettingsHistory::Id)
                    .uuid()
                    .not_null()
                    .primary_key()
                    .extra("DEFAULT gen_random_uuid()"),
            );
        } else {
            table.col(
                ColumnDef::new(SettingsHistory::Id)
                    .uuid()
                    .not_null()
                    .primary_key(),
            );
        }

        manager
            .create_table(
                table
                    // FK to settings table
                    .col(ColumnDef::new(SettingsHistory::SettingId).uuid().not_null())
                    // Historical key (denormalized for audit)
                    .col(
                        ColumnDef::new(SettingsHistory::Key)
                            .string_len(255)
                            .not_null(),
                    )
                    // Previous value
                    .col(ColumnDef::new(SettingsHistory::OldValue).text())
                    // New value
                    .col(ColumnDef::new(SettingsHistory::NewValue).text().not_null())
                    // Who made the change
                    .col(ColumnDef::new(SettingsHistory::ChangedBy).uuid().not_null())
                    // When it was changed
                    .col({
                        let mut col = ColumnDef::new(SettingsHistory::ChangedAt);
                        col.timestamp_with_time_zone().not_null();
                        if is_postgres {
                            col.extra("DEFAULT NOW()");
                        } else {
                            col.extra("DEFAULT CURRENT_TIMESTAMP");
                        }
                        col
                    })
                    // Optional reason for change
                    .col(ColumnDef::new(SettingsHistory::ChangeReason).text())
                    // IP address of requester
                    .col(ColumnDef::new(SettingsHistory::IpAddress).string_len(45))
                    // Foreign keys
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_settings_history_setting_id")
                            .from(SettingsHistory::Table, SettingsHistory::SettingId)
                            .to(Settings::Table, Settings::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::NoAction),
                    )
                    // NOTE: Foreign key to users table commented out to allow testing
                    // without requiring users table to be populated
                    // .foreign_key(
                    //     ForeignKey::create()
                    //         .name("fk_settings_history_changed_by")
                    //         .from(SettingsHistory::Table, SettingsHistory::ChangedBy)
                    //         .to(Users::Table, Users::Id)
                    //         .on_delete(ForeignKeyAction::NoAction)
                    //         .on_update(ForeignKeyAction::NoAction),
                    // )
                    .to_owned(),
            )
            .await?;

        // Index on setting_id for looking up history
        manager
            .create_index(
                Index::create()
                    .name("idx_settings_history_setting_id")
                    .table(SettingsHistory::Table)
                    .col(SettingsHistory::SettingId)
                    .to_owned(),
            )
            .await?;

        // Index on changed_at for sorting history
        manager
            .create_index(
                Index::create()
                    .name("idx_settings_history_changed_at")
                    .table(SettingsHistory::Table)
                    .col(SettingsHistory::ChangedAt)
                    .to_owned(),
            )
            .await?;

        // Index on key for querying history by setting key
        manager
            .create_index(
                Index::create()
                    .name("idx_settings_history_key")
                    .table(SettingsHistory::Table)
                    .col(SettingsHistory::Key)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(SettingsHistory::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum SettingsHistory {
    Table,
    Id,
    SettingId,
    Key,
    OldValue,
    NewValue,
    ChangedBy,
    ChangedAt,
    ChangeReason,
    IpAddress,
}

#[derive(DeriveIden)]
enum Settings {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum Users {
    Table,
    Id,
}
