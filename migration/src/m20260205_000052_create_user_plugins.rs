use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Create user_plugins table - per-user plugin instances
        manager
            .create_table(
                Table::create()
                    .table(UserPlugins::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(UserPlugins::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    // References
                    .col(ColumnDef::new(UserPlugins::PluginId).uuid().not_null())
                    .col(ColumnDef::new(UserPlugins::UserId).uuid().not_null())
                    // Per-user credentials (encrypted OAuth tokens, API keys)
                    .col(ColumnDef::new(UserPlugins::Credentials).binary())
                    // Per-user configuration overrides
                    .col(
                        ColumnDef::new(UserPlugins::Config)
                            .json()
                            .not_null()
                            .default("{}"),
                    )
                    // OAuth-specific fields
                    .col(ColumnDef::new(UserPlugins::OauthAccessToken).binary())
                    .col(ColumnDef::new(UserPlugins::OauthRefreshToken).binary())
                    .col(ColumnDef::new(UserPlugins::OauthExpiresAt).timestamp_with_time_zone())
                    .col(ColumnDef::new(UserPlugins::OauthScope).text())
                    // External user identity (for display)
                    .col(ColumnDef::new(UserPlugins::ExternalUserId).text())
                    .col(ColumnDef::new(UserPlugins::ExternalUsername).text())
                    .col(ColumnDef::new(UserPlugins::ExternalAvatarUrl).text())
                    // Per-user state
                    .col(
                        ColumnDef::new(UserPlugins::Enabled)
                            .boolean()
                            .not_null()
                            .default(true),
                    )
                    .col(
                        ColumnDef::new(UserPlugins::HealthStatus)
                            .string_len(20)
                            .not_null()
                            .default("unknown"),
                    )
                    .col(
                        ColumnDef::new(UserPlugins::FailureCount)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(ColumnDef::new(UserPlugins::LastFailureAt).timestamp_with_time_zone())
                    .col(ColumnDef::new(UserPlugins::LastSuccessAt).timestamp_with_time_zone())
                    .col(ColumnDef::new(UserPlugins::LastSyncAt).timestamp_with_time_zone())
                    // Timestamps
                    .col(
                        ColumnDef::new(UserPlugins::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(UserPlugins::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    // Foreign keys
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_user_plugins_plugin")
                            .from(UserPlugins::Table, UserPlugins::PluginId)
                            .to(Plugins::Table, Plugins::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_user_plugins_user")
                            .from(UserPlugins::Table, UserPlugins::UserId)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // Unique constraint: one instance per user per plugin
        manager
            .create_index(
                Index::create()
                    .name("idx_user_plugins_plugin_user")
                    .table(UserPlugins::Table)
                    .col(UserPlugins::PluginId)
                    .col(UserPlugins::UserId)
                    .unique()
                    .to_owned(),
            )
            .await?;

        // Index on user_id for fast lookups
        manager
            .create_index(
                Index::create()
                    .name("idx_user_plugins_user_id")
                    .table(UserPlugins::Table)
                    .col(UserPlugins::UserId)
                    .to_owned(),
            )
            .await?;

        // Index on plugin_id for broadcast operations
        manager
            .create_index(
                Index::create()
                    .name("idx_user_plugins_plugin_id")
                    .table(UserPlugins::Table)
                    .col(UserPlugins::PluginId)
                    .to_owned(),
            )
            .await?;

        // Index on enabled for filtering active instances
        manager
            .create_index(
                Index::create()
                    .name("idx_user_plugins_enabled")
                    .table(UserPlugins::Table)
                    .col(UserPlugins::Enabled)
                    .to_owned(),
            )
            .await?;

        // Create user_plugin_data table - key-value store per user-plugin instance
        manager
            .create_table(
                Table::create()
                    .table(UserPluginData::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(UserPluginData::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    // Reference to user's plugin instance
                    .col(
                        ColumnDef::new(UserPluginData::UserPluginId)
                            .uuid()
                            .not_null(),
                    )
                    // Key-value storage
                    .col(ColumnDef::new(UserPluginData::Key).text().not_null())
                    .col(ColumnDef::new(UserPluginData::Data).json().not_null())
                    // Optional TTL for cached data
                    .col(ColumnDef::new(UserPluginData::ExpiresAt).timestamp_with_time_zone())
                    // Timestamps
                    .col(
                        ColumnDef::new(UserPluginData::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(UserPluginData::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    // Foreign key
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_user_plugin_data_user_plugin")
                            .from(UserPluginData::Table, UserPluginData::UserPluginId)
                            .to(UserPlugins::Table, UserPlugins::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // Unique constraint: one value per key per user plugin instance
        manager
            .create_index(
                Index::create()
                    .name("idx_user_plugin_data_user_plugin_key")
                    .table(UserPluginData::Table)
                    .col(UserPluginData::UserPluginId)
                    .col(UserPluginData::Key)
                    .unique()
                    .to_owned(),
            )
            .await?;

        // Index on user_plugin_id for fast lookups
        manager
            .create_index(
                Index::create()
                    .name("idx_user_plugin_data_user_plugin_id")
                    .table(UserPluginData::Table)
                    .col(UserPluginData::UserPluginId)
                    .to_owned(),
            )
            .await?;

        // Partial index on expires_at for cleanup of expired data
        manager
            .create_index(
                Index::create()
                    .name("idx_user_plugin_data_expires_at")
                    .table(UserPluginData::Table)
                    .col(UserPluginData::ExpiresAt)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Drop user_plugin_data first (depends on user_plugins)
        manager
            .drop_table(Table::drop().table(UserPluginData::Table).to_owned())
            .await?;

        manager
            .drop_table(Table::drop().table(UserPlugins::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
pub enum UserPlugins {
    Table,
    Id,
    PluginId,
    UserId,
    Credentials,
    Config,
    OauthAccessToken,
    OauthRefreshToken,
    OauthExpiresAt,
    OauthScope,
    ExternalUserId,
    ExternalUsername,
    ExternalAvatarUrl,
    Enabled,
    HealthStatus,
    FailureCount,
    LastFailureAt,
    LastSuccessAt,
    LastSyncAt,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
pub enum UserPluginData {
    Table,
    Id,
    UserPluginId,
    Key,
    Data,
    ExpiresAt,
    CreatedAt,
    UpdatedAt,
}

// Local iden references for foreign keys
#[derive(DeriveIden)]
enum Plugins {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum Users {
    Table,
    Id,
}
