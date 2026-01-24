use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Books::Table)
                    .add_column(ColumnDef::new(Books::AnalysisErrors).text())
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Books::Table)
                    .drop_column(Books::AnalysisErrors)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum Books {
    Table,
    AnalysisErrors,
}
