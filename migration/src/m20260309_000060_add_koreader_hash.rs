use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Add koreader_hash column to books table (nullable, computed on demand)
        manager
            .alter_table(
                Table::alter()
                    .table(Books::Table)
                    .add_column(ColumnDef::new(Alias::new("koreader_hash")).string().null())
                    .to_owned(),
            )
            .await?;

        // Add index for fast lookup by koreader_hash
        manager
            .create_index(
                Index::create()
                    .name("idx_books_koreader_hash")
                    .table(Books::Table)
                    .col(Alias::new("koreader_hash"))
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name("idx_books_koreader_hash")
                    .table(Books::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Books::Table)
                    .drop_column(Alias::new("koreader_hash"))
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
enum Books {
    Table,
}
