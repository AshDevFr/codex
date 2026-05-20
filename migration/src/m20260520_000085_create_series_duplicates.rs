use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(SeriesDuplicates::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(SeriesDuplicates::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    // 'external_id' (cross-library) or 'title' (scoped to library_id)
                    .col(
                        ColumnDef::new(SeriesDuplicates::MatchType)
                            .string()
                            .not_null(),
                    )
                    // For external_id: "<source>:<external_id>" e.g. "plugin:mangabaka:12345"
                    // For title:       normalized search_title value
                    .col(
                        ColumnDef::new(SeriesDuplicates::MatchKey)
                            .string()
                            .not_null(),
                    )
                    // Null for external_id matches (cross-library); set for title matches.
                    .col(ColumnDef::new(SeriesDuplicates::LibraryId).uuid().null())
                    .col(
                        ColumnDef::new(SeriesDuplicates::SeriesIds)
                            .text()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SeriesDuplicates::DuplicateCount)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SeriesDuplicates::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SeriesDuplicates::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_series_duplicates_match_type")
                    .table(SeriesDuplicates::Table)
                    .col(SeriesDuplicates::MatchType)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_series_duplicates_match_key")
                    .table(SeriesDuplicates::Table)
                    .col(SeriesDuplicates::MatchKey)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_series_duplicates_count")
                    .table(SeriesDuplicates::Table)
                    .col(SeriesDuplicates::DuplicateCount)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(SeriesDuplicates::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum SeriesDuplicates {
    Table,
    Id,
    MatchType,
    MatchKey,
    LibraryId,
    SeriesIds,
    DuplicateCount,
    CreatedAt,
    UpdatedAt,
}
