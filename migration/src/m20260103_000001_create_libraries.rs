use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Libraries::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Libraries::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Libraries::Name).string().not_null())
                    .col(ColumnDef::new(Libraries::Path).string().not_null())
                    .col(
                        ColumnDef::new(Libraries::ScanningStrategy)
                            .string()
                            .not_null(),
                    )
                    .col(ColumnDef::new(Libraries::ScanningConfig).string())
                    .col(
                        ColumnDef::new(Libraries::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Libraries::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(ColumnDef::new(Libraries::LastScannedAt).timestamp_with_time_zone())
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Libraries::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum Libraries {
    Table,
    Id,
    Name,
    Path,
    ScanningStrategy,
    ScanningConfig,
    CreatedAt,
    UpdatedAt,
    LastScannedAt,
}
