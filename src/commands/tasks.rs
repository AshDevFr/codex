use anyhow::Result;
use chrono::Utc;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter};
use std::time::Duration;
use uuid::Uuid;

use crate::db::entities::prelude::Tasks;
use crate::db::entities::tasks;
use crate::db::repositories::TaskRepository;
use crate::tasks::TaskWorker;

/// Start a task worker
pub async fn worker_command(db: DatabaseConnection, poll_interval: u64) -> Result<()> {
    println!("Starting task worker (press Ctrl+C to stop)...");

    let worker = TaskWorker::new(db).with_poll_interval(Duration::from_secs(poll_interval));
    let (mut worker, shutdown_tx) = worker.with_shutdown();

    // Spawn worker in background
    let worker_handle = tokio::spawn(async move { worker.run().await });

    // Wait for Ctrl+C
    tokio::signal::ctrl_c().await?;
    println!("\nShutting down worker...");

    // Signal shutdown
    let _ = shutdown_tx.send(());

    // Wait for worker to finish (with timeout)
    match tokio::time::timeout(Duration::from_secs(30), worker_handle).await {
        Ok(Ok(Ok(()))) => println!("Worker stopped gracefully"),
        Ok(Ok(Err(e))) => eprintln!("Worker error: {}", e),
        Ok(Err(e)) => eprintln!("Worker task error: {}", e),
        Err(_) => eprintln!("Worker shutdown timeout"),
    }

    Ok(())
}

/// List tasks
pub async fn list_tasks(
    db: &DatabaseConnection,
    status: Option<String>,
    task_type: Option<String>,
    limit: u64,
) -> Result<()> {
    let tasks = TaskRepository::list(db, status, task_type, Some(limit)).await?;

    if tasks.is_empty() {
        println!("No tasks found");
        return Ok(());
    }

    // Print as table
    println!(
        "{:<36} {:<20} {:<12} {:<8} {:<20} {:<3} {}",
        "ID", "Type", "Status", "Priority", "Scheduled", "Att", "Locked By"
    );
    println!("{}", "-".repeat(120));

    for task in tasks {
        println!(
            "{:<36} {:<20} {:<12} {:<8} {:<20} {:<3} {}",
            task.id,
            task.task_type,
            task.status,
            task.priority,
            task.scheduled_for.format("%Y-%m-%d %H:%M"),
            task.attempts,
            task.locked_by.unwrap_or_else(|| "-".to_string()),
        );
    }

    Ok(())
}

/// Get task details
pub async fn get_task(db: &DatabaseConnection, task_id: Uuid) -> Result<()> {
    let task = TaskRepository::get_by_id(db, task_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Task not found"))?;

    println!("Task Details:");
    println!("  ID:           {}", task.id);
    println!("  Type:         {}", task.task_type);
    println!("  Status:       {}", task.status);
    println!("  Priority:     {}", task.priority);
    println!("  Attempts:     {}/{}", task.attempts, task.max_attempts);
    println!("  Scheduled:    {}", task.scheduled_for);
    println!("  Created:      {}", task.created_at);

    if let Some(started) = task.started_at {
        println!("  Started:      {}", started);
    }
    if let Some(completed) = task.completed_at {
        println!("  Completed:    {}", completed);
    }
    if let Some(locked_by) = task.locked_by {
        println!("  Locked By:    {}", locked_by);
    }
    if let Some(locked_until) = task.locked_until {
        println!("  Locked Until: {}", locked_until);
    }
    if let Some(error) = task.last_error {
        println!("  Last Error:   {}", error);
    }

    if let Some(params) = task.params {
        println!("\nParameters:");
        println!("{}", serde_json::to_string_pretty(&params)?);
    }

    if let Some(result) = task.result {
        println!("\nResult:");
        println!("{}", serde_json::to_string_pretty(&result)?);
    }

    Ok(())
}

/// Show queue statistics
pub async fn stats_command(db: &DatabaseConnection) -> Result<()> {
    let stats = TaskRepository::get_stats(db).await?;

    println!("Queue Statistics:");
    println!("  Pending:      {}", stats.pending);
    println!("  Processing:   {}", stats.processing);
    println!("  Completed:    {}", stats.completed);
    println!("  Failed:       {}", stats.failed);
    println!("  Stale:        {} (locked > 10 min)", stats.stale);
    println!("  ─────────────");
    println!("  Total:        {}", stats.total);

    if stats.stale > 0 {
        println!(
            "\n⚠️  Warning: {} stale tasks detected (locked > 10 min)",
            stats.stale
        );
        println!("   These may be from crashed workers. Use 'unlock' to release them.");
    }

    Ok(())
}

/// Cancel a task
pub async fn cancel_task(db: &DatabaseConnection, task_id: Uuid) -> Result<()> {
    TaskRepository::cancel(db, task_id).await?;
    println!("✓ Task {} cancelled", task_id);
    Ok(())
}

/// Unlock a task
pub async fn unlock_task(db: &DatabaseConnection, task_id: Uuid) -> Result<()> {
    TaskRepository::unlock(db, task_id).await?;
    println!("✓ Task {} unlocked and reset to pending", task_id);
    Ok(())
}

/// Retry a failed task
pub async fn retry_task(db: &DatabaseConnection, task_id: Uuid) -> Result<()> {
    let task = TaskRepository::get_by_id(db, task_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Task not found"))?;

    if task.status != "failed" {
        anyhow::bail!(
            "Can only retry failed tasks (current status: {})",
            task.status
        );
    }

    TaskRepository::unlock(db, task_id).await?;
    println!("✓ Task {} queued for retry", task_id);
    Ok(())
}

/// Purge old tasks
pub async fn purge_tasks(db: &DatabaseConnection, days: i64, confirm: bool) -> Result<()> {
    if !confirm {
        // Dry run - show what would be deleted
        let cutoff = Utc::now() - chrono::Duration::days(days);
        let count = Tasks::find()
            .filter(tasks::Column::Status.is_in(["completed", "failed", "cancelled"]))
            .filter(tasks::Column::CompletedAt.lt(cutoff))
            .count(db)
            .await?;

        println!("Would delete {} tasks older than {} days", count, days);
        println!("Run with --confirm to actually delete");
        return Ok(());
    }

    let deleted = TaskRepository::purge_old_tasks(db, days).await?;
    println!("✓ Purged {} old tasks", deleted);
    Ok(())
}

/// Nuke all tasks
pub async fn nuke_tasks(db: &DatabaseConnection, confirm: &str) -> Result<()> {
    if confirm != "yes" {
        println!("⚠️  This will DELETE ALL TASKS from the queue!");
        println!("To confirm, run: codex tasks nuke --confirm yes");
        return Ok(());
    }

    let deleted = TaskRepository::nuke_all_tasks(db).await?;
    println!("💥 Nuked {} tasks from the queue", deleted);
    Ok(())
}
