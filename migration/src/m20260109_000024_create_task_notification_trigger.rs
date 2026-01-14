use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Only create trigger for PostgreSQL
        if manager.get_database_backend() == sea_orm::DatabaseBackend::Postgres {
            // Create trigger function that sends NOTIFY when task is completed or failed
            manager
                .get_connection()
                .execute_unprepared(
                    r#"
                    CREATE OR REPLACE FUNCTION notify_task_completion()
                    RETURNS TRIGGER AS $$
                    BEGIN
                        -- Only notify if status changed to 'completed' or 'failed'
                        IF (NEW.status IN ('completed', 'failed')) AND
                           (OLD.status IS NULL OR OLD.status != NEW.status) THEN
                            PERFORM pg_notify(
                                'task_completion',
                                json_build_object(
                                    'task_id', NEW.id::text,
                                    'task_type', NEW.task_type,
                                    'status', NEW.status,
                                    'library_id', NEW.library_id::text,
                                    'series_id', NEW.series_id::text,
                                    'book_id', NEW.book_id::text,
                                    'started_at', EXTRACT(EPOCH FROM NEW.started_at),
                                    'completed_at', EXTRACT(EPOCH FROM NEW.completed_at)
                                )::text
                            );
                        END IF;
                        RETURN NEW;
                    END;
                    $$ LANGUAGE plpgsql;
                    "#,
                )
                .await?;

            // Create trigger on tasks table
            manager
                .get_connection()
                .execute_unprepared(
                    r#"
                    CREATE TRIGGER task_completion_trigger
                    AFTER UPDATE ON tasks
                    FOR EACH ROW
                    EXECUTE FUNCTION notify_task_completion();
                    "#,
                )
                .await?;
        }

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Only drop trigger for PostgreSQL
        if manager.get_database_backend() == sea_orm::DatabaseBackend::Postgres {
            manager
                .get_connection()
                .execute_unprepared("DROP TRIGGER IF EXISTS task_completion_trigger ON tasks;")
                .await?;

            manager
                .get_connection()
                .execute_unprepared("DROP FUNCTION IF EXISTS notify_task_completion();")
                .await?;
        }

        Ok(())
    }
}
