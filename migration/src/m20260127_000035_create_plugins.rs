//! Create plugins table for external plugin processes
//!
//! Plugins are external processes that communicate with Codex via JSON-RPC over stdio.
//! This table stores plugin configuration, permissions, and health status.
//!
//! Plugin types:
//! - `system`: Admin-configured plugins for metadata fetching (e.g., MangaBaka)
//! - `user`: User-configured plugins for sync/recommendations (e.g., AniList sync)

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let is_postgres = manager.get_database_backend() == sea_orm::DatabaseBackend::Postgres;

        let mut table = Table::create();
        table.table(Plugins::Table).if_not_exists();

        // ID column - different defaults for Postgres vs SQLite
        if is_postgres {
            table.col(
                ColumnDef::new(Plugins::Id)
                    .uuid()
                    .not_null()
                    .primary_key()
                    .extra("DEFAULT gen_random_uuid()"),
            );
        } else {
            table.col(ColumnDef::new(Plugins::Id).uuid().not_null().primary_key());
        }

        manager
            .create_table(
                table
                    // Identity
                    .col(
                        ColumnDef::new(Plugins::Name)
                            .string_len(100)
                            .not_null()
                            .unique_key(),
                    )
                    .col(
                        ColumnDef::new(Plugins::DisplayName)
                            .string_len(255)
                            .not_null(),
                    )
                    .col(ColumnDef::new(Plugins::Description).text())
                    // Plugin type: 'system' (admin-configured) or 'user' (per-user instances)
                    .col(
                        ColumnDef::new(Plugins::PluginType)
                            .string_len(20)
                            .not_null()
                            .default("system"),
                    )
                    // Execution
                    .col(ColumnDef::new(Plugins::Command).text().not_null())
                    .col(
                        ColumnDef::new(Plugins::Args)
                            .json()
                            .not_null()
                            .default("[]"),
                    )
                    .col(ColumnDef::new(Plugins::Env).json().not_null().default("{}"))
                    .col(ColumnDef::new(Plugins::WorkingDirectory).text())
                    // Permissions (RBAC)
                    .col(
                        ColumnDef::new(Plugins::Permissions)
                            .json()
                            .not_null()
                            .default("[]"),
                    )
                    // Scopes (where plugin can be invoked)
                    .col(
                        ColumnDef::new(Plugins::Scopes)
                            .json()
                            .not_null()
                            .default("[]"),
                    )
                    // Library filtering (restrict plugin to specific libraries)
                    // Empty array = all libraries, non-empty = only these library UUIDs
                    .col(
                        ColumnDef::new(Plugins::LibraryIds)
                            .json()
                            .not_null()
                            .default("[]"),
                    )
                    // Credentials (encrypted, passed as env vars or init message)
                    .col(ColumnDef::new(Plugins::Credentials).binary())
                    .col(
                        ColumnDef::new(Plugins::CredentialDelivery)
                            .string_len(20)
                            .not_null()
                            .default("env"),
                    )
                    // Plugin configuration
                    .col(
                        ColumnDef::new(Plugins::Config)
                            .json()
                            .not_null()
                            .default("{}"),
                    )
                    // Manifest (cached from plugin after first connection)
                    .col(ColumnDef::new(Plugins::Manifest).json())
                    // State
                    .col(
                        ColumnDef::new(Plugins::Enabled)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .col(
                        ColumnDef::new(Plugins::HealthStatus)
                            .string_len(20)
                            .not_null()
                            .default("unknown"),
                    )
                    .col(
                        ColumnDef::new(Plugins::FailureCount)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(ColumnDef::new(Plugins::LastFailureAt).timestamp_with_time_zone())
                    .col(ColumnDef::new(Plugins::LastSuccessAt).timestamp_with_time_zone())
                    .col(ColumnDef::new(Plugins::DisabledReason).text())
                    // Rate limiting
                    .col(
                        ColumnDef::new(Plugins::RateLimitRequestsPerMinute)
                            .integer()
                            .default(60),
                    )
                    // Timestamps
                    .col({
                        let mut col = ColumnDef::new(Plugins::CreatedAt);
                        col.timestamp_with_time_zone().not_null();
                        if is_postgres {
                            col.extra("DEFAULT NOW()");
                        } else {
                            col.extra("DEFAULT CURRENT_TIMESTAMP");
                        }
                        col
                    })
                    .col({
                        let mut col = ColumnDef::new(Plugins::UpdatedAt);
                        col.timestamp_with_time_zone().not_null();
                        if is_postgres {
                            col.extra("DEFAULT NOW()");
                        } else {
                            col.extra("DEFAULT CURRENT_TIMESTAMP");
                        }
                        col
                    })
                    // Audit trail
                    .col(ColumnDef::new(Plugins::CreatedBy).uuid())
                    .col(ColumnDef::new(Plugins::UpdatedBy).uuid())
                    // Foreign keys (optional - allow null for system-created plugins)
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_plugins_created_by")
                            .from(Plugins::Table, Plugins::CreatedBy)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::SetNull)
                            .on_update(ForeignKeyAction::NoAction),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_plugins_updated_by")
                            .from(Plugins::Table, Plugins::UpdatedBy)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::SetNull)
                            .on_update(ForeignKeyAction::NoAction),
                    )
                    .to_owned(),
            )
            .await?;

        // Add CHECK constraints on enum columns to prevent data corruption
        // Note: SQLite doesn't support ALTER TABLE ADD CONSTRAINT, so we use raw SQL
        // PostgreSQL and SQLite both support CHECK constraints in raw SQL
        let check_plugin_type = if is_postgres {
            "ALTER TABLE plugins ADD CONSTRAINT chk_plugins_plugin_type CHECK (plugin_type IN ('system', 'user'))"
        } else {
            // SQLite requires recreating the table to add constraints, but we can add a trigger instead
            // For simplicity, we skip CHECK constraints in SQLite as it has less strict enforcement anyway
            ""
        };

        let check_health_status = if is_postgres {
            "ALTER TABLE plugins ADD CONSTRAINT chk_plugins_health_status CHECK (health_status IN ('unknown', 'healthy', 'degraded', 'unhealthy', 'disabled'))"
        } else {
            ""
        };

        let check_credential_delivery = if is_postgres {
            "ALTER TABLE plugins ADD CONSTRAINT chk_plugins_credential_delivery CHECK (credential_delivery IN ('env', 'stdin'))"
        } else {
            ""
        };

        // Execute CHECK constraints (PostgreSQL only)
        if is_postgres {
            let db = manager.get_connection();
            db.execute_unprepared(check_plugin_type).await?;
            db.execute_unprepared(check_health_status).await?;
            db.execute_unprepared(check_credential_delivery).await?;
        }

        // Index on enabled for finding active plugins
        manager
            .create_index(
                Index::create()
                    .name("idx_plugins_enabled")
                    .table(Plugins::Table)
                    .col(Plugins::Enabled)
                    .to_owned(),
            )
            .await?;

        // Index on health_status for filtering by health
        manager
            .create_index(
                Index::create()
                    .name("idx_plugins_health_status")
                    .table(Plugins::Table)
                    .col(Plugins::HealthStatus)
                    .to_owned(),
            )
            .await?;

        // Index on plugin_type for filtering system vs user plugins
        manager
            .create_index(
                Index::create()
                    .name("idx_plugins_plugin_type")
                    .table(Plugins::Table)
                    .col(Plugins::PluginType)
                    .to_owned(),
            )
            .await?;

        // Create plugin_failures table for time-windowed failure tracking
        let mut failures_table = Table::create();
        failures_table.table(PluginFailures::Table).if_not_exists();

        // ID column - different defaults for Postgres vs SQLite
        if is_postgres {
            failures_table.col(
                ColumnDef::new(PluginFailures::Id)
                    .uuid()
                    .not_null()
                    .primary_key()
                    .extra("DEFAULT gen_random_uuid()"),
            );
        } else {
            failures_table.col(
                ColumnDef::new(PluginFailures::Id)
                    .uuid()
                    .not_null()
                    .primary_key(),
            );
        }

        manager
            .create_table(
                failures_table
                    // Reference to plugin
                    .col(ColumnDef::new(PluginFailures::PluginId).uuid().not_null())
                    // Failure details
                    .col(
                        ColumnDef::new(PluginFailures::ErrorMessage)
                            .text()
                            .not_null(),
                    )
                    .col(ColumnDef::new(PluginFailures::ErrorCode).string_len(50))
                    .col(ColumnDef::new(PluginFailures::Method).string_len(100))
                    // Context (optional)
                    .col(ColumnDef::new(PluginFailures::RequestId).string_len(100))
                    .col(ColumnDef::new(PluginFailures::Context).json())
                    // Request summary (sanitized, sensitive fields redacted)
                    .col(ColumnDef::new(PluginFailures::RequestSummary).text())
                    // Timestamp
                    .col({
                        let mut col = ColumnDef::new(PluginFailures::OccurredAt);
                        col.timestamp_with_time_zone().not_null();
                        if is_postgres {
                            col.extra("DEFAULT NOW()");
                        } else {
                            col.extra("DEFAULT CURRENT_TIMESTAMP");
                        }
                        col
                    })
                    // TTL: Failures older than retention period are auto-deleted
                    // Default retention: 30 days
                    .col({
                        let mut col = ColumnDef::new(PluginFailures::ExpiresAt);
                        col.timestamp_with_time_zone().not_null();
                        if is_postgres {
                            col.extra("DEFAULT (NOW() + INTERVAL '30 days')");
                        } else {
                            col.extra("DEFAULT (datetime('now', '+30 days'))");
                        }
                        col
                    })
                    // Foreign key to plugins table
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_plugin_failures_plugin_id")
                            .from(PluginFailures::Table, PluginFailures::PluginId)
                            .to(Plugins::Table, Plugins::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::NoAction),
                    )
                    .to_owned(),
            )
            .await?;

        // Index on plugin_id for filtering failures by plugin
        manager
            .create_index(
                Index::create()
                    .name("idx_plugin_failures_plugin_id")
                    .table(PluginFailures::Table)
                    .col(PluginFailures::PluginId)
                    .to_owned(),
            )
            .await?;

        // Index on occurred_at for time-window queries
        manager
            .create_index(
                Index::create()
                    .name("idx_plugin_failures_occurred_at")
                    .table(PluginFailures::Table)
                    .col(PluginFailures::PluginId)
                    .col(PluginFailures::OccurredAt)
                    .to_owned(),
            )
            .await?;

        // Index on expires_at for cleanup job
        manager
            .create_index(
                Index::create()
                    .name("idx_plugin_failures_expires_at")
                    .table(PluginFailures::Table)
                    .col(PluginFailures::ExpiresAt)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Drop plugin_failures first (has FK to plugins)
        manager
            .drop_table(Table::drop().table(PluginFailures::Table).to_owned())
            .await?;

        manager
            .drop_table(Table::drop().table(Plugins::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum Plugins {
    Table,
    Id,
    // Identity
    Name,
    DisplayName,
    Description,
    PluginType,
    // Execution
    Command,
    Args,
    Env,
    WorkingDirectory,
    // Permissions & Scopes
    Permissions,
    Scopes,
    // Library filtering
    LibraryIds,
    // Credentials
    Credentials,
    CredentialDelivery,
    // Configuration
    Config,
    Manifest,
    // State
    Enabled,
    HealthStatus,
    FailureCount,
    LastFailureAt,
    LastSuccessAt,
    DisabledReason,
    // Rate limiting
    RateLimitRequestsPerMinute,
    // Timestamps
    CreatedAt,
    UpdatedAt,
    // Audit trail
    CreatedBy,
    UpdatedBy,
}

#[derive(DeriveIden)]
enum Users {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum PluginFailures {
    Table,
    Id,
    PluginId,
    ErrorMessage,
    ErrorCode,
    Method,
    RequestId,
    Context,
    RequestSummary,
    OccurredAt,
    ExpiresAt,
}
