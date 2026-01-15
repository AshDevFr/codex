use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(MetadataSources::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(MetadataSources::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(MetadataSources::SeriesId).uuid().not_null())
                    .col(
                        ColumnDef::new(MetadataSources::SourceName)
                            .text()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MetadataSources::ExternalId)
                            .text()
                            .not_null(),
                    )
                    .col(ColumnDef::new(MetadataSources::ExternalUrl).text())
                    .col(
                        ColumnDef::new(MetadataSources::Confidence)
                            .decimal()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MetadataSources::MetadataJson)
                            .text()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MetadataSources::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MetadataSources::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_metadata_sources_series_id")
                            .from(MetadataSources::Table, MetadataSources::SeriesId)
                            .to(Series::Table, Series::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::NoAction),
                    )
                    .to_owned(),
            )
            .await?;

        // Unique constraint on (series_id, source_name, external_id) - one record per source per series
        manager
            .create_index(
                Index::create()
                    .name("idx_metadata_sources_unique")
                    .table(MetadataSources::Table)
                    .col(MetadataSources::SeriesId)
                    .col(MetadataSources::SourceName)
                    .col(MetadataSources::ExternalId)
                    .unique()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(MetadataSources::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum MetadataSources {
    Table,
    Id,
    SeriesId,
    SourceName,
    ExternalId,
    ExternalUrl,
    Confidence,
    MetadataJson,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum Series {
    Table,
    Id,
}
