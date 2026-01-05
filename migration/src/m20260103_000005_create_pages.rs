use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Pages::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Pages::Id).uuid().not_null().primary_key())
                    .col(ColumnDef::new(Pages::BookId).uuid().not_null())
                    .col(ColumnDef::new(Pages::PageNumber).integer().not_null())
                    .col(ColumnDef::new(Pages::FileName).string().not_null())
                    .col(ColumnDef::new(Pages::Format).string().not_null())
                    .col(ColumnDef::new(Pages::Width).integer().not_null())
                    .col(ColumnDef::new(Pages::Height).integer().not_null())
                    .col(ColumnDef::new(Pages::FileSize).big_integer().not_null())
                    .col(
                        ColumnDef::new(Pages::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_pages_book_id")
                            .from(Pages::Table, Pages::BookId)
                            .to(Books::Table, Books::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::NoAction),
                    )
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Pages::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum Pages {
    Table,
    Id,
    BookId,
    PageNumber,
    FileName,
    Format,
    Width,
    Height,
    FileSize,
    CreatedAt,
}

#[derive(DeriveIden)]
enum Books {
    Table,
    Id,
}
