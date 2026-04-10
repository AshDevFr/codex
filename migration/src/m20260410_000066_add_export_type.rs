use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Add export_type column: "series" (default), "books", or "both"
        manager
            .alter_table(
                Table::alter()
                    .table(SeriesExports::Table)
                    .add_column(
                        ColumnDef::new(SeriesExports::ExportType)
                            .string_len(10)
                            .not_null()
                            .default("series"),
                    )
                    .to_owned(),
            )
            .await?;

        // Add book_fields column (JSON array, nullable - only used for "books" or "both")
        manager
            .alter_table(
                Table::alter()
                    .table(SeriesExports::Table)
                    .add_column(
                        ColumnDef::new(SeriesExports::BookFields)
                            .json_binary()
                            .null(),
                    )
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(SeriesExports::Table)
                    .drop_column(SeriesExports::ExportType)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(SeriesExports::Table)
                    .drop_column(SeriesExports::BookFields)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
enum SeriesExports {
    Table,
    ExportType,
    BookFields,
}
