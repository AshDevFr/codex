use sea_orm_migration::prelude::*;

/// Partial unique index making at most one `pending`/`processing`
/// `user_plugin_sync` task exist per `(plugin_id, user_id)`.
///
/// `user_plugin_sync` tasks key their target by `plugin_id`/`user_id` inside the
/// `params` JSON (not the `library_id`/`series_id`/`book_id` columns that the
/// other dedup indexes cover), so this index is on the extracted JSON values.
/// `TaskRepository::enqueue` already retries on a unique violation and returns
/// the existing task, so this index turns its racy check-then-insert dedup into
/// an atomic one — closing the window where two schedulers (or a scheduled +
/// manual trigger) enqueue duplicate syncs for the same connection.
///
/// Safe to create on existing data: scheduled fan-out only ever fired on a
/// single replica before this, and the manual path already deduped, so no
/// pre-existing duplicate pending rows can exist to violate the index.
#[derive(DeriveMigrationName)]
pub struct Migration;

const INDEX_NAME: &str = "unique_pending_user_plugin_sync";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let sql = match manager.get_database_backend() {
            sea_orm::DatabaseBackend::Postgres => format!(
                r#"CREATE UNIQUE INDEX IF NOT EXISTS {INDEX_NAME}
                   ON tasks ((params->>'plugin_id'), (params->>'user_id'))
                   WHERE task_type = 'user_plugin_sync'
                     AND status IN ('pending', 'processing')"#
            ),
            _ => format!(
                r#"CREATE UNIQUE INDEX IF NOT EXISTS {INDEX_NAME}
                   ON tasks (json_extract(params, '$.plugin_id'), json_extract(params, '$.user_id'))
                   WHERE task_type = 'user_plugin_sync'
                     AND status IN ('pending', 'processing')"#
            ),
        };

        manager.get_connection().execute_unprepared(&sql).await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(&format!("DROP INDEX IF EXISTS {INDEX_NAME}"))
            .await?;
        Ok(())
    }
}
