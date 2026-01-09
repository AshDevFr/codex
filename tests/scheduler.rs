mod common;

use codex::db::repositories::{SettingsRepository, TaskRepository, UserRepository};
use codex::scheduler::Scheduler;
use codex::utils::password;
use common::{create_test_user, setup_test_db};

/// Test that scheduler can be created and started without errors
#[tokio::test]
async fn test_scheduler_creation() {
    let (db, _temp_dir) = setup_test_db().await;

    let mut scheduler = Scheduler::new(db.clone())
        .await
        .expect("Failed to create scheduler");

    // Start should succeed even with no schedules configured
    scheduler.start().await.expect("Failed to start scheduler");

    // Shutdown
    scheduler.shutdown().await.expect("Failed to shutdown");
}

/// Test that scheduler loads when deduplication is disabled
#[tokio::test]
async fn test_scheduler_with_deduplication_disabled() {
    let (db, _temp_dir) = setup_test_db().await;

    // Create a test user for settings updates
    let password_hash = password::hash_password("test123").unwrap();
    let user = create_test_user("test", "test@example.com", &password_hash, true);
    let created_user = UserRepository::create(&db, &user).await.unwrap();

    // Set deduplication.enabled to false
    SettingsRepository::set(
        &db,
        "deduplication.enabled",
        "false".to_string(),
        created_user.id,
        None,
        None,
    )
    .await
    .expect("Failed to update setting");

    let mut scheduler = Scheduler::new(db.clone())
        .await
        .expect("Failed to create scheduler");

    // Should start successfully without loading deduplication job
    scheduler.start().await.expect("Failed to start scheduler");

    // Verify no FindDuplicates tasks were enqueued
    let tasks = TaskRepository::list(
        &db,
        Some("pending".to_string()),
        Some("find_duplicates".to_string()),
        Some(100),
    )
    .await
    .expect("Failed to list tasks");

    assert_eq!(
        tasks.len(),
        0,
        "No deduplication tasks should be enqueued when disabled"
    );

    scheduler.shutdown().await.expect("Failed to shutdown");
}

/// Test that scheduler loads when deduplication cron is empty
#[tokio::test]
async fn test_scheduler_with_empty_deduplication_cron() {
    let (db, _temp_dir) = setup_test_db().await;

    // Create a test user for settings updates
    let password_hash = password::hash_password("test123").unwrap();
    let user = create_test_user("test", "test@example.com", &password_hash, true);
    let created_user = UserRepository::create(&db, &user).await.unwrap();

    // Set deduplication.enabled to true but cron_schedule to empty
    SettingsRepository::set(
        &db,
        "deduplication.enabled",
        "true".to_string(),
        created_user.id,
        None,
        None,
    )
    .await
    .expect("Failed to update setting");
    SettingsRepository::set(
        &db,
        "deduplication.cron_schedule",
        "".to_string(),
        created_user.id,
        None,
        None,
    )
    .await
    .expect("Failed to update setting");

    let mut scheduler = Scheduler::new(db.clone())
        .await
        .expect("Failed to create scheduler");

    // Should start successfully without loading deduplication job
    scheduler.start().await.expect("Failed to start scheduler");

    // Verify no FindDuplicates tasks were enqueued
    let tasks = TaskRepository::list(
        &db,
        Some("pending".to_string()),
        Some("find_duplicates".to_string()),
        Some(100),
    )
    .await
    .expect("Failed to list tasks");

    assert_eq!(
        tasks.len(),
        0,
        "No deduplication tasks should be enqueued with empty cron"
    );

    scheduler.shutdown().await.expect("Failed to shutdown");
}

/// Test that scheduler loads when deduplication is enabled with a valid cron
/// Note: This doesn't test actual cron execution, just that the job is added
#[tokio::test]
async fn test_scheduler_with_deduplication_enabled() {
    let (db, _temp_dir) = setup_test_db().await;

    // Create a test user for settings updates
    let password_hash = password::hash_password("test123").unwrap();
    let user = create_test_user("test", "test@example.com", &password_hash, true);
    let created_user = UserRepository::create(&db, &user).await.unwrap();

    // Set deduplication.enabled to true and add a cron schedule
    // Use a cron that won't fire during the test (e.g., every day at 3am)
    // Format: "seconds minutes hours day month day_of_week"
    SettingsRepository::set(
        &db,
        "deduplication.enabled",
        "true".to_string(),
        created_user.id,
        None,
        None,
    )
    .await
    .expect("Failed to update setting");
    SettingsRepository::set(
        &db,
        "deduplication.cron_schedule",
        "0 0 3 * * *".to_string(), // Every day at 3:00:00 AM
        created_user.id,
        None,
        None,
    )
    .await
    .expect("Failed to update setting");

    let mut scheduler = Scheduler::new(db.clone())
        .await
        .expect("Failed to create scheduler");

    // Should start successfully and load the deduplication job
    scheduler.start().await.expect("Failed to start scheduler");

    // Note: We can't easily verify the cron job was added without triggering it
    // This test mainly ensures the scheduler doesn't error when loading the schedule

    scheduler.shutdown().await.expect("Failed to shutdown");
}

/// Test scheduler reload_schedules method
#[tokio::test]
async fn test_scheduler_reload_schedules() {
    let (db, _temp_dir) = setup_test_db().await;

    // Create a test user for settings updates
    let password_hash = password::hash_password("test123").unwrap();
    let user = create_test_user("test", "test@example.com", &password_hash, true);
    let created_user = UserRepository::create(&db, &user).await.unwrap();

    let mut scheduler = Scheduler::new(db.clone())
        .await
        .expect("Failed to create scheduler");

    scheduler.start().await.expect("Failed to start scheduler");

    // Update deduplication settings
    // Format: "seconds minutes hours day month day_of_week"
    SettingsRepository::set(
        &db,
        "deduplication.enabled",
        "true".to_string(),
        created_user.id,
        None,
        None,
    )
    .await
    .expect("Failed to update setting");
    SettingsRepository::set(
        &db,
        "deduplication.cron_schedule",
        "0 0 4 * * *".to_string(), // Every day at 4:00:00 AM
        created_user.id,
        None,
        None,
    )
    .await
    .expect("Failed to update setting");

    // Reload schedules
    scheduler
        .reload_schedules()
        .await
        .expect("Failed to reload schedules");

    scheduler.shutdown().await.expect("Failed to shutdown");
}
