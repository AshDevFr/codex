use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Collections: shared, named groupings of series (Komga-style).
        manager
            .create_table(
                Table::create()
                    .table(Collections::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Collections::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(Collections::Name)
                            .string_len(255)
                            .not_null()
                            .unique_key(),
                    )
                    .col(
                        ColumnDef::new(Collections::NormalizedName)
                            .string_len(255)
                            .not_null()
                            .unique_key(),
                    )
                    // false => members sorted by series title; true => use position
                    .col(
                        ColumnDef::new(Collections::Ordered)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .col(
                        ColumnDef::new(Collections::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Collections::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_collections_normalized_name")
                    .table(Collections::Table)
                    .col(Collections::NormalizedName)
                    .to_owned(),
            )
            .await?;

        // collection_series: ordered membership (collection has many series,
        // series may belong to many collections).
        manager
            .create_table(
                Table::create()
                    .table(CollectionSeries::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(CollectionSeries::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(CollectionSeries::CollectionId)
                            .uuid()
                            .not_null(),
                    )
                    .col(ColumnDef::new(CollectionSeries::SeriesId).uuid().not_null())
                    // Honored only when collections.ordered = true.
                    .col(
                        ColumnDef::new(CollectionSeries::Position)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(CollectionSeries::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_collection_series_collection_id")
                            .from(CollectionSeries::Table, CollectionSeries::CollectionId)
                            .to(Collections::Table, Collections::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::NoAction),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_collection_series_series_id")
                            .from(CollectionSeries::Table, CollectionSeries::SeriesId)
                            .to(Series::Table, Series::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::NoAction),
                    )
                    .to_owned(),
            )
            .await?;

        // A series appears at most once per collection.
        manager
            .create_index(
                Index::create()
                    .name("idx_collection_series_unique")
                    .table(CollectionSeries::Table)
                    .col(CollectionSeries::CollectionId)
                    .col(CollectionSeries::SeriesId)
                    .unique()
                    .to_owned(),
            )
            .await?;

        // Reverse lookup: which collections contain a given series.
        manager
            .create_index(
                Index::create()
                    .name("idx_collection_series_series_id")
                    .table(CollectionSeries::Table)
                    .col(CollectionSeries::SeriesId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(CollectionSeries::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Collections::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
pub enum Collections {
    Table,
    Id,
    Name,
    NormalizedName,
    Ordered,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
pub enum CollectionSeries {
    Table,
    Id,
    CollectionId,
    SeriesId,
    Position,
    CreatedAt,
}

#[derive(DeriveIden)]
enum Series {
    Table,
    Id,
}
