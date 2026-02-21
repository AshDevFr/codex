mod common;

use codex::db::repositories::{SettingsRepository, TaskRepository, UserRepository};
use codex::scheduler::Scheduler;
use codex::utils::password;
use common::{create_test_user, setup_test_db};

/// Test that scheduler can be created and started without errors
#[tokio::test]
async fn test_scheduler_creation() {
    let (db, _temp_dir) = setup_test_db().await;

    let mut scheduler = Scheduler::new(db.clone(), "UTC")
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

    let mut scheduler = Scheduler::new(db.clone(), "UTC")
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

    let mut scheduler = Scheduler::new(db.clone(), "UTC")
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

    let mut scheduler = Scheduler::new(db.clone(), "UTC")
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

    let mut scheduler = Scheduler::new(db.clone(), "UTC")
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

// =============================================================================
// Thumbnail Cron Job Tests
// =============================================================================

/// Test that scheduler loads with empty book thumbnail cron schedule (disabled)
#[tokio::test]
async fn test_scheduler_with_empty_book_thumbnail_cron() {
    let (db, _temp_dir) = setup_test_db().await;

    let mut scheduler = Scheduler::new(db.clone(), "UTC")
        .await
        .expect("Failed to create scheduler");

    // Should start successfully without loading book thumbnail job (empty by default)
    scheduler.start().await.expect("Failed to start scheduler");

    // Verify no GenerateThumbnails tasks were enqueued
    let tasks = TaskRepository::list(
        &db,
        Some("pending".to_string()),
        Some("generate_thumbnails".to_string()),
        Some(100),
    )
    .await
    .expect("Failed to list tasks");

    assert_eq!(
        tasks.len(),
        0,
        "No thumbnail generation tasks should be enqueued with empty cron"
    );

    scheduler.shutdown().await.expect("Failed to shutdown");
}

/// Test that scheduler loads book thumbnail cron when enabled with a valid schedule
#[tokio::test]
async fn test_scheduler_with_book_thumbnail_cron_enabled() {
    let (db, _temp_dir) = setup_test_db().await;

    // Create a test user for settings updates
    let password_hash = password::hash_password("test123").unwrap();
    let user = create_test_user("test", "test@example.com", &password_hash, true);
    let created_user = UserRepository::create(&db, &user).await.unwrap();

    // Set book thumbnail cron schedule
    SettingsRepository::set(
        &db,
        "thumbnail.book_cron_schedule",
        "0 0 3 * * *".to_string(), // Every day at 3:00:00 AM
        created_user.id,
        None,
        None,
    )
    .await
    .expect("Failed to update setting");

    let mut scheduler = Scheduler::new(db.clone(), "UTC")
        .await
        .expect("Failed to create scheduler");

    // Should start successfully and load the book thumbnail job
    scheduler.start().await.expect("Failed to start scheduler");

    // Note: We can't easily verify the cron job was added without triggering it
    // This test mainly ensures the scheduler doesn't error when loading the schedule

    scheduler.shutdown().await.expect("Failed to shutdown");
}

/// Test that scheduler loads with empty series thumbnail cron schedule (disabled)
#[tokio::test]
async fn test_scheduler_with_empty_series_thumbnail_cron() {
    let (db, _temp_dir) = setup_test_db().await;

    let mut scheduler = Scheduler::new(db.clone(), "UTC")
        .await
        .expect("Failed to create scheduler");

    // Should start successfully without loading series thumbnail job (empty by default)
    scheduler.start().await.expect("Failed to start scheduler");

    // Verify no GenerateSeriesThumbnail tasks were enqueued
    let tasks = TaskRepository::list(
        &db,
        Some("pending".to_string()),
        Some("generate_series_thumbnail".to_string()),
        Some(100),
    )
    .await
    .expect("Failed to list tasks");

    assert_eq!(
        tasks.len(),
        0,
        "No series thumbnail generation tasks should be enqueued with empty cron"
    );

    scheduler.shutdown().await.expect("Failed to shutdown");
}

/// Test that scheduler loads series thumbnail cron when enabled with a valid schedule
#[tokio::test]
async fn test_scheduler_with_series_thumbnail_cron_enabled() {
    let (db, _temp_dir) = setup_test_db().await;

    // Create a test user for settings updates
    let password_hash = password::hash_password("test123").unwrap();
    let user = create_test_user("test", "test@example.com", &password_hash, true);
    let created_user = UserRepository::create(&db, &user).await.unwrap();

    // Set series thumbnail cron schedule
    SettingsRepository::set(
        &db,
        "thumbnail.series_cron_schedule",
        "0 0 4 * * *".to_string(), // Every day at 4:00:00 AM
        created_user.id,
        None,
        None,
    )
    .await
    .expect("Failed to update setting");

    let mut scheduler = Scheduler::new(db.clone(), "UTC")
        .await
        .expect("Failed to create scheduler");

    // Should start successfully and load the series thumbnail job
    scheduler.start().await.expect("Failed to start scheduler");

    // Note: We can't easily verify the cron job was added without triggering it
    // This test mainly ensures the scheduler doesn't error when loading the schedule

    scheduler.shutdown().await.expect("Failed to shutdown");
}

/// Test that both thumbnail cron schedules can be enabled simultaneously
#[tokio::test]
async fn test_scheduler_with_both_thumbnail_crons_enabled() {
    let (db, _temp_dir) = setup_test_db().await;

    // Create a test user for settings updates
    let password_hash = password::hash_password("test123").unwrap();
    let user = create_test_user("test", "test@example.com", &password_hash, true);
    let created_user = UserRepository::create(&db, &user).await.unwrap();

    // Set both thumbnail cron schedules
    SettingsRepository::set(
        &db,
        "thumbnail.book_cron_schedule",
        "0 0 3 * * *".to_string(), // Every day at 3:00:00 AM
        created_user.id,
        None,
        None,
    )
    .await
    .expect("Failed to update setting");

    SettingsRepository::set(
        &db,
        "thumbnail.series_cron_schedule",
        "0 0 4 * * *".to_string(), // Every day at 4:00:00 AM
        created_user.id,
        None,
        None,
    )
    .await
    .expect("Failed to update setting");

    let mut scheduler = Scheduler::new(db.clone(), "UTC")
        .await
        .expect("Failed to create scheduler");

    // Should start successfully and load both thumbnail jobs
    scheduler.start().await.expect("Failed to start scheduler");

    // This test ensures both cron jobs can be loaded without conflicts

    scheduler.shutdown().await.expect("Failed to shutdown");
}

// =============================================================================
// Timezone Tests
// =============================================================================

/// Test that scheduler can be created with a valid IANA timezone
#[tokio::test]
async fn test_scheduler_with_valid_timezone() {
    let (db, _temp_dir) = setup_test_db().await;

    let mut scheduler = Scheduler::new(db.clone(), "America/Los_Angeles")
        .await
        .expect("Failed to create scheduler with LA timezone");

    scheduler.start().await.expect("Failed to start scheduler");
    scheduler.shutdown().await.expect("Failed to shutdown");
}

/// Test that scheduler falls back to UTC for an invalid timezone string
#[tokio::test]
async fn test_scheduler_with_invalid_timezone_falls_back_to_utc() {
    let (db, _temp_dir) = setup_test_db().await;

    // Should not error; should warn and fall back to UTC
    let mut scheduler = Scheduler::new(db.clone(), "Invalid/Timezone")
        .await
        .expect("Scheduler should fall back to UTC for invalid timezone");

    scheduler.start().await.expect("Failed to start scheduler");
    scheduler.shutdown().await.expect("Failed to shutdown");
}

/// Test that scheduler works with various valid IANA timezones
#[tokio::test]
async fn test_scheduler_with_various_timezones() {
    let timezones = [
        "UTC",
        "America/New_York",
        "Europe/London",
        "Asia/Tokyo",
        "Australia/Sydney",
    ];

    for tz in &timezones {
        let (db, _temp_dir) = setup_test_db().await;

        let mut scheduler = Scheduler::new(db.clone(), tz)
            .await
            .unwrap_or_else(|_| panic!("Failed to create scheduler with timezone {}", tz));

        scheduler
            .start()
            .await
            .unwrap_or_else(|_| panic!("Failed to start scheduler with timezone {}", tz));

        scheduler.shutdown().await.expect("Failed to shutdown");
    }
}

/// Test that scheduler with non-UTC timezone loads deduplication cron correctly
#[tokio::test]
async fn test_scheduler_timezone_with_deduplication_cron() {
    let (db, _temp_dir) = setup_test_db().await;

    let password_hash = password::hash_password("test123").unwrap();
    let user = create_test_user("test", "test@example.com", &password_hash, true);
    let created_user = UserRepository::create(&db, &user).await.unwrap();

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
        "0 0 3 * * *".to_string(),
        created_user.id,
        None,
        None,
    )
    .await
    .expect("Failed to update setting");

    // Create scheduler with non-UTC timezone
    let mut scheduler = Scheduler::new(db.clone(), "America/Chicago")
        .await
        .expect("Failed to create scheduler");

    // Should start and load the deduplication job with Chicago timezone
    scheduler.start().await.expect("Failed to start scheduler");
    scheduler.shutdown().await.expect("Failed to shutdown");
}
