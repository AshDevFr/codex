use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let is_postgres = manager.get_database_backend() == sea_orm::DatabaseBackend::Postgres;

        let mut table = Table::create();
        table.table(SystemIntegrations::Table).if_not_exists();

        // ID column - different defaults for Postgres vs SQLite
        if is_postgres {
            table.col(
                ColumnDef::new(SystemIntegrations::Id)
                    .uuid()
                    .not_null()
                    .primary_key()
                    .extra("DEFAULT gen_random_uuid()"),
            );
        } else {
            table.col(
                ColumnDef::new(SystemIntegrations::Id)
                    .uuid()
                    .not_null()
                    .primary_key(),
            );
        }

        manager
            .create_table(
                table
                    // Identity
                    .col(
                        ColumnDef::new(SystemIntegrations::Name)
                            .string_len(100)
                            .not_null()
                            .unique_key(),
                    )
                    .col(
                        ColumnDef::new(SystemIntegrations::DisplayName)
                            .string_len(255)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SystemIntegrations::IntegrationType)
                            .string_len(50)
                            .not_null(),
                    )
                    // Configuration
                    .col(ColumnDef::new(SystemIntegrations::Credentials).binary()) // Encrypted JSON
                    .col(
                        ColumnDef::new(SystemIntegrations::Config)
                            .json()
                            .not_null()
                            .default("{}"),
                    )
                    // State
                    .col(
                        ColumnDef::new(SystemIntegrations::Enabled)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .col(
                        ColumnDef::new(SystemIntegrations::HealthStatus)
                            .string_len(20)
                            .not_null()
                            .default("unknown"),
                    )
                    .col(
                        ColumnDef::new(SystemIntegrations::LastHealthCheckAt)
                            .timestamp_with_time_zone(),
                    )
                    .col(ColumnDef::new(SystemIntegrations::LastSyncAt).timestamp_with_time_zone())
                    .col(ColumnDef::new(SystemIntegrations::ErrorMessage).text())
                    // Timestamps
                    .col({
                        let mut col = ColumnDef::new(SystemIntegrations::CreatedAt);
                        col.timestamp_with_time_zone().not_null();
                        if is_postgres {
                            col.extra("DEFAULT NOW()");
                        } else {
                            col.extra("DEFAULT CURRENT_TIMESTAMP");
                        }
                        col
                    })
                    .col({
                        let mut col = ColumnDef::new(SystemIntegrations::UpdatedAt);
                        col.timestamp_with_time_zone().not_null();
                        if is_postgres {
                            col.extra("DEFAULT NOW()");
                        } else {
                            col.extra("DEFAULT CURRENT_TIMESTAMP");
                        }
                        col
                    })
                    // Audit trail
                    .col(ColumnDef::new(SystemIntegrations::CreatedBy).uuid())
                    .col(ColumnDef::new(SystemIntegrations::UpdatedBy).uuid())
                    // Foreign keys (optional - allow null for system-created integrations)
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_system_integrations_created_by")
                            .from(SystemIntegrations::Table, SystemIntegrations::CreatedBy)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::SetNull)
                            .on_update(ForeignKeyAction::NoAction),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_system_integrations_updated_by")
                            .from(SystemIntegrations::Table, SystemIntegrations::UpdatedBy)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::SetNull)
                            .on_update(ForeignKeyAction::NoAction),
                    )
                    .to_owned(),
            )
            .await?;

        // Index on integration_type for filtering by type
        manager
            .create_index(
                Index::create()
                    .name("idx_system_integrations_type")
                    .table(SystemIntegrations::Table)
                    .col(SystemIntegrations::IntegrationType)
                    .to_owned(),
            )
            .await?;

        // Index on enabled for finding active integrations
        manager
            .create_index(
                Index::create()
                    .name("idx_system_integrations_enabled")
                    .table(SystemIntegrations::Table)
                    .col(SystemIntegrations::Enabled)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(SystemIntegrations::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum SystemIntegrations {
    Table,
    Id,
    Name,
    DisplayName,
    IntegrationType,
    Credentials,
    Config,
    Enabled,
    HealthStatus,
    LastHealthCheckAt,
    LastSyncAt,
    ErrorMessage,
    CreatedAt,
    UpdatedAt,
    CreatedBy,
    UpdatedBy,
}

#[derive(DeriveIden)]
enum Users {
    Table,
    Id,
}
