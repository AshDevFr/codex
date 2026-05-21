use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::Statement;

/// Rename `books.file_path` to `books.path` for symmetry with `series.path`.
///
/// `ALTER TABLE ... RENAME COLUMN` is supported on SQLite (>= 3.25) and Postgres
/// and is a metadata-only operation on both — no table rewrite, no long lock.
///
/// Also renames the unique index that backed `(library_id, file_path)` so its
/// name matches the new column.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        let backend = db.get_database_backend();

        // Drop the old unique index first so the rename doesn't have to worry
        // about it. We recreate it on the new column name below.
        manager
            .drop_index(
                Index::drop()
                    .name("idx_books_library_file_path_unique")
                    .table(Books::Table)
                    .to_owned(),
            )
            .await?;

        // SeaORM's SchemaManager doesn't expose a portable `RENAME COLUMN`, so
        // emit dialect-appropriate SQL. Both branches are metadata-only.
        let sql = match backend {
            sea_orm_migration::sea_orm::DatabaseBackend::Postgres => {
                "ALTER TABLE books RENAME COLUMN file_path TO path"
            }
            sea_orm_migration::sea_orm::DatabaseBackend::Sqlite => {
                "ALTER TABLE books RENAME COLUMN file_path TO path"
            }
            sea_orm_migration::sea_orm::DatabaseBackend::MySql => {
                // Codex doesn't target MySQL but spell it out so the migration
                // doesn't silently no-op if a contributor wires it up.
                "ALTER TABLE books RENAME COLUMN file_path TO path"
            }
        };
        db.execute(Statement::from_string(backend, sql.to_owned()))
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_books_library_path_unique")
                    .table(Books::Table)
                    .col(Books::LibraryId)
                    .col(Books::Path)
                    .unique()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        let backend = db.get_database_backend();

        manager
            .drop_index(
                Index::drop()
                    .name("idx_books_library_path_unique")
                    .table(Books::Table)
                    .to_owned(),
            )
            .await?;

        let sql = "ALTER TABLE books RENAME COLUMN path TO file_path";
        db.execute(Statement::from_string(backend, sql.to_owned()))
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_books_library_file_path_unique")
                    .table(Books::Table)
                    .col(Books::LibraryId)
                    .col(Books::FilePath)
                    .unique()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
enum Books {
    Table,
    LibraryId,
    Path,
    FilePath,
}
