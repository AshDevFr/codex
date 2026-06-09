use sea_orm_migration::prelude::*;

/// System-scoped plugin key-value store.
///
/// Mirrors `user_plugin_data` but is keyed by `plugin_id` (the `plugins` row)
/// rather than `user_plugin_id`. System plugins (e.g. release sources) run
/// with no user context, so they can't use the per-user store; this table is
/// their durable KV bucket (used for things like a release feed cursor).
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(PluginData::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(PluginData::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    // Reference to the system plugin (no user scoping)
                    .col(ColumnDef::new(PluginData::PluginId).uuid().not_null())
                    // Key-value storage
                    .col(ColumnDef::new(PluginData::Key).text().not_null())
                    .col(ColumnDef::new(PluginData::Data).json().not_null())
                    // Optional TTL for cached data
                    .col(ColumnDef::new(PluginData::ExpiresAt).timestamp_with_time_zone())
                    // Timestamps
                    .col(
                        ColumnDef::new(PluginData::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PluginData::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    // Foreign key — data is removed when the plugin is deleted.
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_plugin_data_plugin")
                            .from(PluginData::Table, PluginData::PluginId)
                            .to(Plugins::Table, Plugins::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // Unique constraint: one value per key per plugin.
        manager
            .create_index(
                Index::create()
                    .name("idx_plugin_data_plugin_key")
                    .table(PluginData::Table)
                    .col(PluginData::PluginId)
                    .col(PluginData::Key)
                    .unique()
                    .to_owned(),
            )
            .await?;

        // Index on plugin_id for fast lookups.
        manager
            .create_index(
                Index::create()
                    .name("idx_plugin_data_plugin_id")
                    .table(PluginData::Table)
                    .col(PluginData::PluginId)
                    .to_owned(),
            )
            .await?;

        // Index on expires_at for cleanup of expired data.
        manager
            .create_index(
                Index::create()
                    .name("idx_plugin_data_expires_at")
                    .table(PluginData::Table)
                    .col(PluginData::ExpiresAt)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(PluginData::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
pub enum PluginData {
    Table,
    Id,
    PluginId,
    Key,
    Data,
    ExpiresAt,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum Plugins {
    Table,
    Id,
}
