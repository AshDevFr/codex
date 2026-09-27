//! Let reading history outlive the books it was recorded against.
//!
//! `reading_sessions` is the only source of every reading statistic and
//! `read_completions` the only record that a book was ever finished. Both
//! cascaded on `books.id`, so purging deleted books or removing a library
//! erased hours the user really spent reading, with nothing left to rebuild
//! them from. A session records something that happened; it does not stop
//! having happened because the file was later removed.
//!
//! After this migration `book_id` is nullable on both tables and its foreign
//! key is `ON DELETE SET NULL`: the row survives and loses only its
//! attribution. `user_id` keeps `ON DELETE CASCADE`, because deleting a user
//! must still remove their history.
//!
//! # How each backend gets there
//!
//! PostgreSQL alters the column and swaps the constraint in place.
//!
//! SQLite cannot alter a column's nullability or a foreign key's action, so
//! each table is rebuilt: create a replacement, copy every row, drop the
//! original, rename the replacement, recreate the indexes. SQLite's documented
//! procedure also switches `foreign_keys` off around the swap, but that step
//! exists to stop dropping a *parent* table from cascading into its children.
//! Nothing references a session or a completion, so these tables have no
//! children and the rebuild is safe with enforcement left on. That matters: the
//! pragma is a no-op inside a transaction and is per connection in a pool,
//! which is what made the swap unreliable in
//! `m20260508_000081_add_release_sources_plugin_uuid_fk`. Here each rebuild
//! runs in one transaction and never touches the pragma.
//!
//! The copy is checked by row count before the original is dropped, and every
//! index from the create migrations is recreated with its original columns and
//! ordering; a rebuild that silently lost an index would slow the statistics
//! queries without failing anything.
//!
//! # A `book_id` index
//!
//! Neither table had an index leading with `book_id`, so every deleted book
//! made the foreign key action scan both tables in full. Deleting a library
//! deletes every book in it, one scan each. That was already true under
//! `CASCADE`; `SET NULL` additionally rewrites the rows it finds, and the
//! rebuild is the cheapest moment to add the index.
//!
//! # Rollback is lossy
//!
//! `down` restores `NOT NULL` and `CASCADE`, which orphaned rows cannot
//! satisfy, so it deletes every row whose `book_id` is null first. Those are
//! exactly the rows this migration exists to keep.

use sea_orm::{
    ConnectionTrait, DatabaseTransaction, DbBackend, DbErr, Statement, TransactionTrait,
};
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        migrate(manager, Direction::Up).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        migrate(manager, Direction::Down).await
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Direction {
    /// `book_id` nullable, `ON DELETE SET NULL`.
    Up,
    /// `book_id` required, `ON DELETE CASCADE`, as originally created.
    Down,
}

impl Direction {
    fn on_delete(self) -> ForeignKeyAction {
        match self {
            Self::Up => ForeignKeyAction::SetNull,
            Self::Down => ForeignKeyAction::Cascade,
        }
    }
}

const HISTORY_TABLES: [&str; 2] = ["reading_sessions", "read_completions"];

fn book_index_name(table: &str) -> String {
    format!("idx_{table}_book_id")
}

/// `down` restores `NOT NULL`, which orphaned rows cannot satisfy.
async fn delete_orphans<C: ConnectionTrait>(db: &C) -> Result<(), DbErr> {
    for table in HISTORY_TABLES {
        db.execute_unprepared(&format!("DELETE FROM {table} WHERE book_id IS NULL"))
            .await?;
    }
    Ok(())
}

async fn migrate(manager: &SchemaManager<'_>, direction: Direction) -> Result<(), DbErr> {
    let db = manager.get_connection();

    match db.get_database_backend() {
        // Already inside the migrator's transaction.
        DbBackend::Postgres => {
            if direction == Direction::Down {
                delete_orphans(db).await?;
                for table in HISTORY_TABLES {
                    db.execute_unprepared(&format!(
                        "DROP INDEX IF EXISTS {}",
                        book_index_name(table)
                    ))
                    .await?;
                }
            }
            alter_postgres(
                db,
                "reading_sessions",
                "fk_reading_sessions_book_id",
                direction,
            )
            .await?;
            alter_postgres(
                db,
                "read_completions",
                "fk_read_completions_book_id",
                direction,
            )
            .await?;
            if direction == Direction::Up {
                for table in HISTORY_TABLES {
                    db.execute_unprepared(&format!(
                        "CREATE INDEX {} ON {table} (book_id)",
                        book_index_name(table)
                    ))
                    .await?;
                }
            }
        }
        // The migrator does not wrap SQLite migrations in a transaction, so
        // this one opens its own: a rollback that deleted the orphans and then
        // failed the rebuild would otherwise lose them for nothing.
        DbBackend::Sqlite => {
            let txn = db.begin().await?;
            if direction == Direction::Down {
                delete_orphans(&txn).await?;
            }
            rebuild_sqlite(&txn, &ReadingSessionsTable, direction).await?;
            rebuild_sqlite(&txn, &ReadCompletionsTable, direction).await?;
            txn.commit().await?;
        }
        DbBackend::MySql => {
            return Err(DbErr::Migration(
                "MySQL is not a supported backend".to_string(),
            ));
        }
    }

    Ok(())
}

async fn alter_postgres<C: ConnectionTrait>(
    db: &C,
    table: &str,
    fk_name: &str,
    direction: Direction,
) -> Result<(), DbErr> {
    let (nullability, action) = match direction {
        Direction::Up => ("DROP NOT NULL", "SET NULL"),
        Direction::Down => ("SET NOT NULL", "CASCADE"),
    };
    db.execute_unprepared(&format!(
        "ALTER TABLE {table} ALTER COLUMN book_id {nullability}"
    ))
    .await?;
    db.execute_unprepared(&format!("ALTER TABLE {table} DROP CONSTRAINT {fk_name}"))
        .await?;
    db.execute_unprepared(&format!(
        "ALTER TABLE {table} ADD CONSTRAINT {fk_name} FOREIGN KEY (book_id) \
         REFERENCES books (id) ON DELETE {action} ON UPDATE NO ACTION"
    ))
    .await?;
    Ok(())
}

/// One table's shape, as SQLite needs it to rebuild the table from scratch.
trait RebuildableTable: Sync {
    fn name(&self) -> &'static str;
    /// Every column, in the order the copy lists them.
    fn columns(&self) -> &'static [&'static str];
    /// The full table definition under `table_name`, with `book_id` shaped by
    /// `direction`.
    fn create(&self, table_name: &str, direction: Direction) -> TableCreateStatement;
    /// Every index the create migration made, against the final table name.
    fn indexes(&self) -> Vec<IndexCreateStatement>;
}

async fn rebuild_sqlite(
    txn: &DatabaseTransaction,
    table: &dyn RebuildableTable,
    direction: Direction,
) -> Result<(), DbErr> {
    let backend = DbBackend::Sqlite;
    let name = table.name();
    let staging = format!("{name}_rebuild");
    let columns = table.columns().join(", ");

    // A row that already references a missing user or book would fail the
    // copy below with SQLite's bare "FOREIGN KEY constraint failed", and since
    // migrations run at startup the server would not start. Say which table.
    let existing = foreign_key_violations(txn, name).await?;
    if existing > 0 {
        return Err(DbErr::Migration(format!(
            "{name} has {existing} rows referencing a missing user or book; \
             `PRAGMA foreign_key_check({name})` lists them. Delete them and restart."
        )));
    }

    let before = count_rows(txn, name).await?;

    txn.execute(backend.build(&table.create(&staging, direction)))
        .await?;
    txn.execute_unprepared(&format!(
        "INSERT INTO {staging} ({columns}) SELECT {columns} FROM {name}"
    ))
    .await?;

    let copied = count_rows(txn, &staging).await?;
    if copied != before {
        return Err(DbErr::Migration(format!(
            "rebuilding {name} copied {copied} of {before} rows; aborting before the original is dropped"
        )));
    }

    // Dropping the original drops its indexes with it, which frees their
    // names for the recreation below.
    txn.execute_unprepared(&format!("DROP TABLE {name}"))
        .await?;
    txn.execute_unprepared(&format!("ALTER TABLE {staging} RENAME TO {name}"))
        .await?;
    for index in table.indexes() {
        txn.execute(backend.build(&index)).await?;
    }
    if direction == Direction::Up {
        txn.execute_unprepared(&format!(
            "CREATE INDEX {} ON {name} (book_id)",
            book_index_name(name)
        ))
        .await?;
    }

    let violations = foreign_key_violations(txn, name).await?;
    if violations > 0 {
        return Err(DbErr::Migration(format!(
            "rebuilding {name} left {violations} foreign key violations"
        )));
    }

    Ok(())
}

async fn foreign_key_violations(txn: &DatabaseTransaction, table: &str) -> Result<usize, DbErr> {
    Ok(txn
        .query_all(Statement::from_string(
            DbBackend::Sqlite,
            format!("PRAGMA foreign_key_check({table})"),
        ))
        .await?
        .len())
}

async fn count_rows<C: ConnectionTrait>(db: &C, table: &str) -> Result<i64, DbErr> {
    let row = db
        .query_one(Statement::from_string(
            db.get_database_backend(),
            format!("SELECT COUNT(*) AS n FROM {table}"),
        ))
        .await?
        .ok_or_else(|| DbErr::Migration(format!("COUNT(*) on {table} returned no row")))?;
    row.try_get("", "n")
}

fn book_id_column(direction: Direction) -> ColumnDef {
    let mut column = ColumnDef::new(Alias::new("book_id"));
    column.uuid();
    match direction {
        Direction::Up => column.null(),
        Direction::Down => column.not_null(),
    };
    column
}

struct ReadingSessionsTable;

impl RebuildableTable for ReadingSessionsTable {
    fn name(&self) -> &'static str {
        "reading_sessions"
    }

    fn columns(&self) -> &'static [&'static str] {
        &[
            "id",
            "user_id",
            "book_id",
            "device_id",
            "device_name",
            "pass",
            "kind",
            "to_page",
            "to_percentage",
            "r2_progression",
            "active_duration_ms",
            "duration_source",
            "pages_read",
            "client_started_at",
            "client_ended_at",
            "server_recorded_at",
        ]
    }

    // Mirrors `m20260814_000105_create_reading_sessions` column for column.
    fn create(&self, table_name: &str, direction: Direction) -> TableCreateStatement {
        let table = Alias::new(table_name);
        Table::create()
            .table(table.clone())
            .col(
                ColumnDef::new(ReadingSessions::Id)
                    .uuid()
                    .not_null()
                    .primary_key(),
            )
            .col(ColumnDef::new(ReadingSessions::UserId).uuid().not_null())
            .col(book_id_column(direction))
            .col(
                ColumnDef::new(ReadingSessions::DeviceId)
                    .string()
                    .not_null(),
            )
            .col(ColumnDef::new(ReadingSessions::DeviceName).string().null())
            .col(
                ColumnDef::new(ReadingSessions::Pass)
                    .integer()
                    .not_null()
                    .default(1),
            )
            .col(ColumnDef::new(ReadingSessions::Kind).string().not_null())
            .col(ColumnDef::new(ReadingSessions::ToPage).integer().null())
            .col(
                ColumnDef::new(ReadingSessions::ToPercentage)
                    .double()
                    .null(),
            )
            .col(ColumnDef::new(ReadingSessions::R2Progression).text().null())
            .col(
                ColumnDef::new(ReadingSessions::ActiveDurationMs)
                    .big_integer()
                    .null(),
            )
            .col(
                ColumnDef::new(ReadingSessions::DurationSource)
                    .string()
                    .not_null()
                    .default("unknown"),
            )
            .col(ColumnDef::new(ReadingSessions::PagesRead).integer().null())
            .col(
                ColumnDef::new(ReadingSessions::ClientStartedAt)
                    .timestamp_with_time_zone()
                    .not_null(),
            )
            .col(
                ColumnDef::new(ReadingSessions::ClientEndedAt)
                    .timestamp_with_time_zone()
                    .not_null(),
            )
            .col(
                ColumnDef::new(ReadingSessions::ServerRecordedAt)
                    .timestamp_with_time_zone()
                    .not_null(),
            )
            .foreign_key(
                ForeignKey::create()
                    .name("fk_reading_sessions_user_id")
                    .from(table.clone(), ReadingSessions::UserId)
                    .to(Users::Table, Users::Id)
                    .on_delete(ForeignKeyAction::Cascade)
                    .on_update(ForeignKeyAction::NoAction),
            )
            .foreign_key(
                ForeignKey::create()
                    .name("fk_reading_sessions_book_id")
                    .from(table, ReadingSessions::BookId)
                    .to(Books::Table, Books::Id)
                    .on_delete(direction.on_delete())
                    .on_update(ForeignKeyAction::NoAction),
            )
            .to_owned()
    }

    fn indexes(&self) -> Vec<IndexCreateStatement> {
        vec![
            Index::create()
                .name("idx_reading_sessions_fold")
                .table(ReadingSessions::Table)
                .col(ReadingSessions::UserId)
                .col(ReadingSessions::BookId)
                .col(ReadingSessions::Pass)
                .col(ReadingSessions::ClientEndedAt)
                .to_owned(),
            Index::create()
                .name("idx_reading_sessions_stats")
                .table(ReadingSessions::Table)
                .col(ReadingSessions::UserId)
                .col(ReadingSessions::ClientStartedAt)
                .to_owned(),
            Index::create()
                .name("idx_reading_sessions_coalesce")
                .table(ReadingSessions::Table)
                .col(ReadingSessions::UserId)
                .col(ReadingSessions::BookId)
                .col(ReadingSessions::DeviceId)
                .col(ReadingSessions::Pass)
                .col((ReadingSessions::ClientEndedAt, IndexOrder::Desc))
                .to_owned(),
        ]
    }
}

struct ReadCompletionsTable;

impl RebuildableTable for ReadCompletionsTable {
    fn name(&self) -> &'static str {
        "read_completions"
    }

    fn columns(&self) -> &'static [&'static str] {
        &["id", "user_id", "book_id", "started_at", "completed_at"]
    }

    // Mirrors `m20260729_000103_create_read_completions` column for column.
    fn create(&self, table_name: &str, direction: Direction) -> TableCreateStatement {
        let table = Alias::new(table_name);
        Table::create()
            .table(table.clone())
            .col(
                ColumnDef::new(ReadCompletions::Id)
                    .uuid()
                    .not_null()
                    .primary_key(),
            )
            .col(ColumnDef::new(ReadCompletions::UserId).uuid().not_null())
            .col(book_id_column(direction))
            .col(
                ColumnDef::new(ReadCompletions::StartedAt)
                    .timestamp_with_time_zone()
                    .not_null(),
            )
            .col(
                ColumnDef::new(ReadCompletions::CompletedAt)
                    .timestamp_with_time_zone()
                    .not_null(),
            )
            .foreign_key(
                ForeignKey::create()
                    .name("fk_read_completions_user_id")
                    .from(table.clone(), ReadCompletions::UserId)
                    .to(Users::Table, Users::Id)
                    .on_delete(ForeignKeyAction::Cascade)
                    .on_update(ForeignKeyAction::NoAction),
            )
            .foreign_key(
                ForeignKey::create()
                    .name("fk_read_completions_book_id")
                    .from(table, ReadCompletions::BookId)
                    .to(Books::Table, Books::Id)
                    .on_delete(direction.on_delete())
                    .on_update(ForeignKeyAction::NoAction),
            )
            .to_owned()
    }

    fn indexes(&self) -> Vec<IndexCreateStatement> {
        vec![
            Index::create()
                .name("idx_read_completions_user_book")
                .table(ReadCompletions::Table)
                .col(ReadCompletions::UserId)
                .col(ReadCompletions::BookId)
                .to_owned(),
            Index::create()
                .name("idx_read_completions_user_date")
                .table(ReadCompletions::Table)
                .col(ReadCompletions::UserId)
                .col((ReadCompletions::CompletedAt, IndexOrder::Desc))
                .to_owned(),
        ]
    }
}

#[derive(DeriveIden)]
enum ReadingSessions {
    Table,
    Id,
    UserId,
    BookId,
    DeviceId,
    DeviceName,
    Pass,
    Kind,
    ToPage,
    ToPercentage,
    R2Progression,
    ActiveDurationMs,
    DurationSource,
    PagesRead,
    ClientStartedAt,
    ClientEndedAt,
    ServerRecordedAt,
}

#[derive(DeriveIden)]
enum ReadCompletions {
    Table,
    Id,
    UserId,
    BookId,
    StartedAt,
    CompletedAt,
}

#[derive(DeriveIden)]
enum Users {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum Books {
    Table,
    Id,
}
