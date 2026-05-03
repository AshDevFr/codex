//! Tests for `MetadataApplier::apply` with `dry_run = true`.
//!
//! Verifies that:
//! - DB writes are gated (row state unchanged after a dry-run apply).
//! - The returned `dry_run_report` enumerates the would-be changes.
//! - Lock and permission skips still surface in `skipped_fields`, same code
//!   path as a real apply.
//! - The report matches what a real apply *would* have written for the
//!   selected fields.

#[path = "../common/mod.rs"]
mod common;

use chrono::Utc;
use codex::db::ScanningStrategy;
use codex::db::entities::plugins;
use codex::db::entities::series_metadata;
use codex::db::repositories::{LibraryRepository, SeriesMetadataRepository, SeriesRepository};
use codex::services::metadata::{ApplyOptions, MetadataApplier};
use codex::services::plugin::PluginSeriesMetadata;
use common::db::setup_test_db;
use sea_orm::{ActiveModelTrait, Set};
use serde_json::json;
use std::collections::HashSet;
use uuid::Uuid;

fn create_test_plugin() -> plugins::Model {
    plugins::Model {
        id: Uuid::new_v4(),
        name: "test-plugin".to_string(),
        display_name: "Test Plugin".to_string(),
        description: None,
        plugin_type: "system".to_string(),
        command: "node".to_string(),
        args: json!([]),
        env: json!({}),
        working_directory: None,
        // Broad permission set so individual tests can pick & choose fields
        // without per-field permission noise. Field locks are still tested
        // explicitly.
        permissions: json!([
            "metadata:write:title",
            "metadata:write:summary",
            "metadata:write:status",
            "metadata:write:total_volume_count",
            "metadata:write:total_chapter_count",
            "metadata:write:ratings",
            "metadata:write:year",
        ]),
        scopes: json!(["series:detail"]),
        library_ids: json!([]),
        credentials: None,
        credential_delivery: "env".to_string(),
        config: json!({}),
        manifest: None,
        enabled: true,
        health_status: "healthy".to_string(),
        failure_count: 0,
        last_failure_at: None,
        last_success_at: None,
        disabled_reason: None,
        rate_limit_requests_per_minute: None,
        search_query_template: None,
        search_preprocessing_rules: None,
        auto_match_conditions: None,
        use_existing_external_id: true,
        metadata_targets: None,
        internal_config: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        created_by: None,
        updated_by: None,
    }
}

fn empty_metadata() -> PluginSeriesMetadata {
    PluginSeriesMetadata {
        external_id: "test-123".to_string(),
        external_url: "https://example.com/test-123".to_string(),
        title: None,
        alternate_titles: vec![],
        summary: None,
        status: None,
        year: None,
        total_volume_count: None,
        total_chapter_count: None,
        language: None,
        age_rating: None,
        reading_direction: None,
        genres: vec![],
        tags: vec![],
        authors: vec![],
        artists: vec![],
        publisher: None,
        cover_url: None,
        banner_url: None,
        rating: None,
        external_ratings: vec![],
        external_links: vec![],
        external_ids: vec![],
    }
}

#[tokio::test]
async fn dry_run_does_not_write_to_database() {
    let (db, _temp_dir) = setup_test_db().await;

    let library =
        LibraryRepository::create(&db, "Test Library", "/test/path", ScanningStrategy::Default)
            .await
            .unwrap();
    let series = SeriesRepository::create(&db, library.id, "Original Title", None)
        .await
        .unwrap();

    let original = SeriesMetadataRepository::get_by_series_id(&db, series.id)
        .await
        .unwrap()
        .unwrap();

    // Plugin payload changes title, summary, year, and status.
    let plugin = create_test_plugin();
    let mut plugin_metadata = empty_metadata();
    plugin_metadata.title = Some("Plugin Title".to_string());
    plugin_metadata.summary = Some("Plugin summary".to_string());
    plugin_metadata.year = Some(2024);
    plugin_metadata.status = Some(codex::db::entities::SeriesStatus::Ongoing);

    let options = ApplyOptions {
        dry_run: true,
        ..Default::default()
    };

    let result = MetadataApplier::apply(
        &db,
        series.id,
        library.id,
        &plugin,
        &plugin_metadata,
        Some(&original),
        &options,
    )
    .await
    .unwrap();

    // applied_fields tracks what *would* have been applied even in dry-run,
    // because the dry-run still walks the same branches.
    assert!(result.applied_fields.contains(&"title".to_string()));
    assert!(result.applied_fields.contains(&"summary".to_string()));
    assert!(result.applied_fields.contains(&"year".to_string()));
    assert!(result.applied_fields.contains(&"status".to_string()));

    // Report contents.
    let report = result.dry_run_report.expect("dry_run set ⇒ report present");
    let fields: HashSet<&str> = report.changes.iter().map(|c| c.field.as_str()).collect();
    assert!(fields.contains("title"));
    assert!(fields.contains("summary"));
    assert!(fields.contains("year"));
    assert!(fields.contains("status"));

    // No DB write happened: the row matches the original.
    let after = SeriesMetadataRepository::get_by_series_id(&db, series.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(after.title, original.title);
    assert_eq!(after.summary, original.summary);
    assert_eq!(after.year, original.year);
    assert_eq!(after.status, original.status);
}

#[tokio::test]
async fn dry_run_real_apply_returns_no_report() {
    let (db, _temp_dir) = setup_test_db().await;

    let library =
        LibraryRepository::create(&db, "Test Library", "/test/path", ScanningStrategy::Default)
            .await
            .unwrap();
    let series = SeriesRepository::create(&db, library.id, "Original Title", None)
        .await
        .unwrap();

    let original = SeriesMetadataRepository::get_by_series_id(&db, series.id)
        .await
        .unwrap()
        .unwrap();

    let plugin = create_test_plugin();
    let mut plugin_metadata = empty_metadata();
    plugin_metadata.summary = Some("Plugin summary".to_string());

    let options = ApplyOptions::default(); // dry_run defaults to false

    let result = MetadataApplier::apply(
        &db,
        series.id,
        library.id,
        &plugin,
        &plugin_metadata,
        Some(&original),
        &options,
    )
    .await
    .unwrap();

    assert!(result.dry_run_report.is_none(), "real apply ⇒ no report");

    // And the write actually happened.
    let after = SeriesMetadataRepository::get_by_series_id(&db, series.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(after.summary, Some("Plugin summary".to_string()));
}

#[tokio::test]
async fn dry_run_records_locked_fields_in_skipped_not_changes() {
    let (db, _temp_dir) = setup_test_db().await;

    let library =
        LibraryRepository::create(&db, "Test Library", "/test/path", ScanningStrategy::Default)
            .await
            .unwrap();
    let series = SeriesRepository::create(&db, library.id, "Original Title", None)
        .await
        .unwrap();

    // Lock summary so the dry-run should skip-with-reason rather than record a change.
    let metadata = SeriesMetadataRepository::get_by_series_id(&db, series.id)
        .await
        .unwrap()
        .unwrap();
    let mut active: series_metadata::ActiveModel = metadata.into();
    active.summary = Set(Some("Original summary".to_string()));
    active.summary_lock = Set(true);
    active.update(&db).await.unwrap();

    let original = SeriesMetadataRepository::get_by_series_id(&db, series.id)
        .await
        .unwrap()
        .unwrap();
    assert!(original.summary_lock);

    let plugin = create_test_plugin();
    let mut plugin_metadata = empty_metadata();
    plugin_metadata.summary = Some("Plugin summary".to_string());
    plugin_metadata.year = Some(2024);

    let options = ApplyOptions {
        dry_run: true,
        ..Default::default()
    };

    let result = MetadataApplier::apply(
        &db,
        series.id,
        library.id,
        &plugin,
        &plugin_metadata,
        Some(&original),
        &options,
    )
    .await
    .unwrap();

    // summary should be in skipped_fields (locked), NOT in dry_run_report.changes.
    let summary_skipped = result.skipped_fields.iter().any(|s| s.field == "summary");
    assert!(
        summary_skipped,
        "locked field should appear in skipped_fields"
    );

    let report = result.dry_run_report.expect("dry_run set ⇒ report present");
    let summary_in_report = report.changes.iter().any(|c| c.field == "summary");
    assert!(
        !summary_in_report,
        "locked field should NOT appear in dry_run_report.changes"
    );

    // year (not locked) should appear in the report.
    let year_in_report = report.changes.iter().any(|c| c.field == "year");
    assert!(year_in_report, "unlocked field should appear in report");

    // DB unchanged for summary either way (locked AND dry-run).
    let after = SeriesMetadataRepository::get_by_series_id(&db, series.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(after.summary, Some("Original summary".to_string()));
}

#[tokio::test]
async fn dry_run_filtered_by_fields_filter() {
    let (db, _temp_dir) = setup_test_db().await;

    let library =
        LibraryRepository::create(&db, "Test Library", "/test/path", ScanningStrategy::Default)
            .await
            .unwrap();
    let series = SeriesRepository::create(&db, library.id, "Original Title", None)
        .await
        .unwrap();

    let original = SeriesMetadataRepository::get_by_series_id(&db, series.id)
        .await
        .unwrap()
        .unwrap();

    // Plugin returns multiple fields, but the filter only allows summary + year.
    let plugin = create_test_plugin();
    let mut plugin_metadata = empty_metadata();
    plugin_metadata.title = Some("Plugin Title".to_string());
    plugin_metadata.summary = Some("Plugin summary".to_string());
    plugin_metadata.year = Some(2024);
    plugin_metadata.status = Some(codex::db::entities::SeriesStatus::Ongoing);

    let mut filter = HashSet::new();
    filter.insert("summary".to_string());
    filter.insert("year".to_string());

    let options = ApplyOptions {
        dry_run: true,
        fields_filter: Some(filter),
        ..Default::default()
    };

    let result = MetadataApplier::apply(
        &db,
        series.id,
        library.id,
        &plugin,
        &plugin_metadata,
        Some(&original),
        &options,
    )
    .await
    .unwrap();

    let report = result.dry_run_report.expect("dry_run set ⇒ report present");
    let fields: HashSet<&str> = report.changes.iter().map(|c| c.field.as_str()).collect();

    assert!(fields.contains("summary"), "summary must be in report");
    assert!(fields.contains("year"), "year must be in report");
    assert!(
        !fields.contains("title"),
        "title is filtered out, must not be in report"
    );
    assert!(
        !fields.contains("status"),
        "status is filtered out, must not be in report"
    );

    // No DB write happened.
    let after = SeriesMetadataRepository::get_by_series_id(&db, series.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(after.title, original.title);
    assert_eq!(after.summary, original.summary);
    assert_eq!(after.status, original.status);
}

#[tokio::test]
async fn dry_run_records_before_and_after_for_simple_field() {
    let (db, _temp_dir) = setup_test_db().await;

    let library =
        LibraryRepository::create(&db, "Test Library", "/test/path", ScanningStrategy::Default)
            .await
            .unwrap();
    let series = SeriesRepository::create(&db, library.id, "Original Title", None)
        .await
        .unwrap();

    // Set a known summary on the row so we can assert `before`.
    let metadata = SeriesMetadataRepository::get_by_series_id(&db, series.id)
        .await
        .unwrap()
        .unwrap();
    let mut active: series_metadata::ActiveModel = metadata.into();
    active.summary = Set(Some("Original summary".to_string()));
    active.update(&db).await.unwrap();

    let original = SeriesMetadataRepository::get_by_series_id(&db, series.id)
        .await
        .unwrap()
        .unwrap();

    let plugin = create_test_plugin();
    let mut plugin_metadata = empty_metadata();
    plugin_metadata.summary = Some("New summary".to_string());

    let options = ApplyOptions {
        dry_run: true,
        ..Default::default()
    };

    let result = MetadataApplier::apply(
        &db,
        series.id,
        library.id,
        &plugin,
        &plugin_metadata,
        Some(&original),
        &options,
    )
    .await
    .unwrap();

    let report = result.dry_run_report.expect("dry_run ⇒ report present");
    let summary_change = report
        .changes
        .iter()
        .find(|c| c.field == "summary")
        .expect("summary in report");

    // before is wrapped because the column is `Option<String>`.
    let before_json = summary_change.before.as_ref().expect("before set");
    assert_eq!(before_json, &json!("Original summary"));
    assert_eq!(summary_change.after, json!("New summary"));
}
