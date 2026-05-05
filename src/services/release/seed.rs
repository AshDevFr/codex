//! Seed defaults for `series_tracking` rows.
//!
//! Called whenever a series transitions to `tracked = true`, and from the
//! retired-but-still-routed `BackfillTrackingFromMetadata` task. The goal is
//! to remove the empty-form UX where a user toggles tracking on and is then
//! presented with a panel full of inputs they have to manually populate.
//!
//! What gets seeded:
//!
//! - **Aliases** (`series_aliases`): inserted from `series.name`,
//!   `series_metadata.title`, `series_metadata.title_sort`, and English
//!   alternate titles. Non-Latin (CJK, Korean, Cyrillic, …) aliases are
//!   skipped today because the alias matcher in the Nyaa / MangaUpdates
//!   plugins normalizes Latin text only — non-Latin entries would never
//!   match against typical uploader filenames and would just clutter the
//!   alias list. Append-only: existing aliases (including user-added) are
//!   never deleted by re-seeding.
//!
//! - **`latest_known_chapter` / `latest_known_volume`**: set to the local
//!   max chapter / volume across the series's books. The first poll after
//!   seeding then announces only releases strictly above the high-water
//!   mark, so a user with v01..v15 on disk doesn't get spammed with
//!   announcements for chapters they already own. Overwritten on every
//!   re-seed (per the "reset all to derived defaults on re-track" rule).
//!
//! - **`track_chapters` / `track_volumes`**: inferred from the series's
//!   book classification. If any book in the series has
//!   `book_metadata.chapter` populated, `track_chapters = true`; same for
//!   volumes. A series organized purely by volume gets `track_chapters =
//!   false`, suppressing chapter-axis announcements. If neither axis has
//!   any classified data (fresh import), both default to `true` so
//!   announcements aren't silently dropped.
//!
//! `tracked` itself is **not** flipped here — that's the caller's
//! responsibility, since this function is called from both the per-series
//! PATCH handler (which interprets the user's intent) and the bulk
//! track-all endpoint.
//!
//! Re-running the seed on an already-tracked series is safe and is the
//! intended idempotent behavior. The retired backfill task uses this
//! property to refresh derived state across all series after a metadata
//! refresh.

use anyhow::{Context, Result};
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use crate::db::entities::series_aliases::alias_source;
use crate::db::repositories::{
    AlternateTitleRepository, SeriesAliasRepository, SeriesMetadataRepository, SeriesRepository,
    SeriesTrackingRepository, TrackingUpdate,
};

/// Outcome of a seed run, suitable for logging and surfacing in API responses.
///
/// `PartialEq` (not `Eq`) because `f32` doesn't have total equality. Tests
/// compare individual fields rather than whole reports anyway.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct SeedReport {
    /// Aliases newly inserted (does not count duplicates skipped).
    pub aliases_inserted: usize,
    /// Aliases skipped because they were not Latin-script.
    pub aliases_skipped_non_latin: usize,
    /// Aliases skipped because an equivalent already existed for the series.
    pub aliases_skipped_duplicate: usize,
    /// Final `track_chapters` value after seeding.
    pub track_chapters: bool,
    /// Final `track_volumes` value after seeding.
    pub track_volumes: bool,
    /// Final `latest_known_chapter` after seeding (`None` when no books
    /// have a classified chapter). f32 to match the aggregate column.
    pub latest_known_chapter: Option<f32>,
    /// Final `latest_known_volume` after seeding (`None` when no books
    /// have a classified volume).
    pub latest_known_volume: Option<i32>,
}

/// Seed (or re-seed) tracking defaults for a single series.
///
/// Updates / inserts a `series_tracking` row with the auto-derived
/// `track_chapters`, `track_volumes`, `latest_known_chapter`,
/// `latest_known_volume` fields. Does **not** modify `tracked` — the caller
/// owns that flip.
///
/// Idempotent: safe to call repeatedly. Aliases are append-only; tracking
/// flags overwrite on every call.
pub async fn seed_tracking_for_series(
    db: &DatabaseConnection,
    series_id: Uuid,
) -> Result<SeedReport> {
    let series = SeriesRepository::get_by_id(db, series_id)
        .await
        .with_context(|| format!("Failed to load series {} for seeding", series_id))?
        .ok_or_else(|| anyhow::anyhow!("series {} not found", series_id))?;

    let metadata = SeriesMetadataRepository::get_by_series_id(db, series_id)
        .await
        .context("Failed to load series metadata for seeding")?;

    let mut report = SeedReport::default();

    // -------------------------------------------------------------------
    // 1. Aliases — collect Latin-script candidates from name + metadata,
    //    bulk-insert (idempotent on duplicates).
    // -------------------------------------------------------------------
    let mut candidates: Vec<String> = Vec::new();
    candidates.push(series.name.clone());
    if let Some(meta) = metadata.as_ref() {
        candidates.push(meta.title.clone());
        if let Some(sort) = meta.title_sort.as_ref()
            && !sort.trim().is_empty()
        {
            candidates.push(sort.clone());
        }
    }
    let alt_titles = AlternateTitleRepository::get_for_series(db, series_id)
        .await
        .context("Failed to load alternate titles")?;
    for alt in alt_titles {
        if !alt.title.trim().is_empty() {
            candidates.push(alt.title);
        }
    }

    // Filter and dedupe (case-insensitive trimmed) so the bulk-insert call
    // doesn't churn on identical inputs from different sources.
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut accepted: Vec<String> = Vec::new();
    for raw in candidates {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            continue;
        }
        if !is_latin_alias(trimmed) {
            report.aliases_skipped_non_latin += 1;
            continue;
        }
        let key = trimmed.to_lowercase();
        if !seen.insert(key) {
            continue;
        }
        accepted.push(trimmed.to_string());
    }

    if !accepted.is_empty() {
        let refs: Vec<&str> = accepted.iter().map(|s| s.as_str()).collect();
        let inserted =
            SeriesAliasRepository::bulk_create(db, series_id, &refs, alias_source::METADATA)
                .await
                .context("Failed to bulk-insert seeded aliases")?;
        report.aliases_inserted = inserted;
        report.aliases_skipped_duplicate = accepted.len().saturating_sub(inserted);
    }

    // -------------------------------------------------------------------
    // 2. Per-axis tracking flags + latest_known_* from book classification.
    // -------------------------------------------------------------------
    let aggregates = SeriesRepository::get_book_classification_aggregates(db, series_id)
        .await
        .context("Failed to load book classification aggregates for seeding")?;

    // Default both axes to true when nothing is classified — losing
    // announcements silently on a fresh series is worse than getting one
    // false-positive on an axis the series doesn't actually use.
    let any_classified =
        aggregates.local_max_chapter.is_some() || aggregates.local_max_volume.is_some();
    let track_chapters = if any_classified {
        aggregates.local_max_chapter.is_some()
    } else {
        true
    };
    let track_volumes = if any_classified {
        aggregates.local_max_volume.is_some()
    } else {
        true
    };

    let update = TrackingUpdate {
        track_chapters: Some(track_chapters),
        track_volumes: Some(track_volumes),
        // The persisted column is f64; widen from the aggregate's f32.
        latest_known_chapter: Some(aggregates.local_max_chapter.map(f64::from)),
        latest_known_volume: Some(aggregates.local_max_volume),
        ..Default::default()
    };
    SeriesTrackingRepository::upsert(db, series_id, update)
        .await
        .context("Failed to upsert series tracking row during seeding")?;

    report.track_chapters = track_chapters;
    report.track_volumes = track_volumes;
    report.latest_known_chapter = aggregates.local_max_chapter;
    report.latest_known_volume = aggregates.local_max_volume;

    Ok(report)
}

/// Whether an alias string is composed entirely of Latin-script characters
/// plus common typography (digits, whitespace, punctuation). Non-Latin
/// scripts (CJK, Korean, Cyrillic, etc.) are rejected today because the
/// alias matcher in the Nyaa / MangaUpdates plugins normalizes Latin text
/// only; a non-Latin alias would never match against typical uploader
/// filenames and would just clutter the alias list.
///
/// Conservative implementation: accept if every alphabetic character is
/// ASCII. This passes "Solo Leveling", "Don't Toy with Me", "Re:Zero",
/// "Bocchi the Rock!", and rejects anything containing CJK ideographs,
/// Hangul, Hiragana/Katakana, Cyrillic, etc. Diacritics (é, ñ, ü, …) are
/// non-ASCII alphabetic and are also rejected — users with such titles can
/// add them as manual aliases. We can widen this later if it bites.
fn is_latin_alias(s: &str) -> bool {
    s.chars()
        .filter(|c| c.is_alphabetic())
        .all(|c| c.is_ascii())
        // Reject empty / pure-punctuation strings as well; downstream
        // create() would error on them anyway.
        && s.chars().any(|c| c.is_ascii_alphanumeric())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use sea_orm::{ActiveModelTrait, Set};

    use crate::db::ScanningStrategy;
    use crate::db::entities::{book_metadata, books};
    use crate::db::repositories::{
        AlternateTitleRepository, BookMetadataRepository, BookRepository, LibraryRepository,
        SeriesAliasRepository, SeriesRepository, SeriesTrackingRepository,
    };
    use crate::db::test_helpers::create_test_db;

    #[test]
    fn is_latin_alias_accepts_latin_strings() {
        assert!(is_latin_alias("Solo Leveling"));
        assert!(is_latin_alias("Don't Toy with Me"));
        assert!(is_latin_alias("Re:Zero - Starting Life in Another World"));
        assert!(is_latin_alias("Bocchi the Rock!"));
        assert!(is_latin_alias("JoJo's Bizarre Adventure Part 7"));
        assert!(is_latin_alias("Boruto: Two Blue Vortex"));
    }

    #[test]
    fn is_latin_alias_rejects_non_latin_strings() {
        assert!(!is_latin_alias("나 혼자만 레벨업")); // Korean Hangul
        assert!(!is_latin_alias("僕のヒーローアカデミア")); // Japanese
        assert!(!is_latin_alias("ダンダダン")); // Katakana
        assert!(!is_latin_alias("Война и мир")); // Cyrillic
    }

    #[test]
    fn is_latin_alias_rejects_diacritics_and_empty_inputs() {
        // Conservative: diacritics are non-ASCII, rejected for now.
        assert!(!is_latin_alias("Pokémon"));
        assert!(!is_latin_alias("Crónica"));
        // Pure punctuation / whitespace.
        assert!(!is_latin_alias(""));
        assert!(!is_latin_alias("   "));
        assert!(!is_latin_alias("!!!---!!!"));
    }

    async fn make_series(db: &DatabaseConnection, library_id: Uuid, name: &str) -> Uuid {
        let series = SeriesRepository::create(db, library_id, name, None)
            .await
            .unwrap();
        // SeriesRepository::create already creates a metadata row with title =
        // name, so we don't need to insert another one.
        let _ = library_id;
        series.id
    }

    async fn add_classified_book(
        db: &DatabaseConnection,
        series_id: Uuid,
        library_id: Uuid,
        path: &str,
        volume: Option<i32>,
        chapter: Option<f32>,
    ) {
        let book = books::Model {
            id: Uuid::new_v4(),
            series_id,
            library_id,
            file_path: path.to_string(),
            file_name: path.rsplit('/').next().unwrap_or(path).to_string(),
            file_size: 1024,
            file_hash: format!("hash_{}", Uuid::new_v4()),
            partial_hash: String::new(),
            format: "cbz".to_string(),
            page_count: 10,
            deleted: false,
            analyzed: false,
            analysis_error: None,
            analysis_errors: None,
            modified_at: Utc::now(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            thumbnail_path: None,
            thumbnail_generated_at: None,
            koreader_hash: None,
            epub_positions: None,
            epub_spine_items: None,
        };
        let created = BookRepository::create(db, &book, None).await.unwrap();
        let meta = BookMetadataRepository::create_with_title_and_number(db, created.id, None, None)
            .await
            .unwrap();
        let mut active: book_metadata::ActiveModel = meta.into();
        active.volume = Set(volume);
        active.chapter = Set(chapter);
        active.update(db).await.unwrap();
    }

    #[tokio::test]
    async fn seed_inserts_latin_aliases_and_skips_non_latin() {
        let (db, _temp) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let lib = LibraryRepository::create(conn, "L", "/p", ScanningStrategy::Default)
            .await
            .unwrap();
        let s = make_series(conn, lib.id, "Solo Leveling").await;
        AlternateTitleRepository::create(conn, s, "Korean", "나 혼자만 레벨업")
            .await
            .unwrap();
        AlternateTitleRepository::create(conn, s, "Romaji", "Na Honjaman Lebel-eob")
            .await
            .unwrap();

        let report = seed_tracking_for_series(conn, s).await.unwrap();
        // "Solo Leveling" is in both `series.name` and `series_metadata.title`,
        // so dedup folds them; "Na Honjaman Lebel-eob" adds one. Korean alt
        // is rejected as non-Latin.
        assert_eq!(report.aliases_inserted, 2);
        assert_eq!(report.aliases_skipped_non_latin, 1);

        let aliases = SeriesAliasRepository::get_for_series(conn, s)
            .await
            .unwrap();
        let texts: Vec<&str> = aliases.iter().map(|a| a.alias.as_str()).collect();
        assert!(texts.contains(&"Solo Leveling"));
        assert!(texts.contains(&"Na Honjaman Lebel-eob"));
        assert!(!texts.iter().any(|a| a.contains('나')));
    }

    #[tokio::test]
    async fn seed_is_idempotent_for_aliases() {
        let (db, _temp) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let lib = LibraryRepository::create(conn, "L", "/p", ScanningStrategy::Default)
            .await
            .unwrap();
        let s = make_series(conn, lib.id, "Berserk").await;

        let first = seed_tracking_for_series(conn, s).await.unwrap();
        assert_eq!(first.aliases_inserted, 1);

        let second = seed_tracking_for_series(conn, s).await.unwrap();
        assert_eq!(second.aliases_inserted, 0);
        assert_eq!(second.aliases_skipped_duplicate, 1);

        let aliases = SeriesAliasRepository::get_for_series(conn, s)
            .await
            .unwrap();
        assert_eq!(aliases.len(), 1);
    }

    #[tokio::test]
    async fn seed_preserves_user_added_aliases_on_re_seed() {
        let (db, _temp) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let lib = LibraryRepository::create(conn, "L", "/p", ScanningStrategy::Default)
            .await
            .unwrap();
        let s = make_series(conn, lib.id, "Boruto").await;

        seed_tracking_for_series(conn, s).await.unwrap();
        // User adds a custom alias their uploader uses.
        SeriesAliasRepository::create(conn, s, "Boruto: Two Blue Vortex", alias_source::MANUAL)
            .await
            .unwrap();

        // Re-seed should not remove the manual alias.
        let _ = seed_tracking_for_series(conn, s).await.unwrap();
        let aliases = SeriesAliasRepository::get_for_series(conn, s)
            .await
            .unwrap();
        let texts: Vec<&str> = aliases.iter().map(|a| a.alias.as_str()).collect();
        assert!(texts.contains(&"Boruto"));
        assert!(texts.contains(&"Boruto: Two Blue Vortex"));
    }

    #[tokio::test]
    async fn seed_writes_track_flags_and_latest_known_with_no_books() {
        let (db, _temp) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let lib = LibraryRepository::create(conn, "L", "/p", ScanningStrategy::Default)
            .await
            .unwrap();
        let s = make_series(conn, lib.id, "Empty Series").await;

        let report = seed_tracking_for_series(conn, s).await.unwrap();
        // Nothing classified — both axes default to true.
        assert!(report.track_chapters);
        assert!(report.track_volumes);
        assert_eq!(report.latest_known_chapter, None);
        assert_eq!(report.latest_known_volume, None);

        let row = SeriesTrackingRepository::get(conn, s)
            .await
            .unwrap()
            .unwrap();
        assert!(row.track_chapters);
        assert!(row.track_volumes);
        assert!(!row.tracked, "seeding must not flip `tracked` on");
        assert_eq!(row.latest_known_chapter, None);
        assert_eq!(row.latest_known_volume, None);
    }

    #[tokio::test]
    async fn seed_infers_track_volumes_only_for_volume_organized_series() {
        let (db, _temp) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let lib = LibraryRepository::create(conn, "L", "/p", ScanningStrategy::Default)
            .await
            .unwrap();
        let s = make_series(conn, lib.id, "Volume Series").await;
        add_classified_book(conn, s, lib.id, "/v1.cbz", Some(1), None).await;
        add_classified_book(conn, s, lib.id, "/v2.cbz", Some(2), None).await;

        let report = seed_tracking_for_series(conn, s).await.unwrap();
        assert!(!report.track_chapters);
        assert!(report.track_volumes);
        assert_eq!(report.latest_known_chapter, None);
        assert_eq!(report.latest_known_volume, Some(2));
    }

    #[tokio::test]
    async fn seed_infers_track_chapters_only_for_chapter_organized_series() {
        let (db, _temp) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let lib = LibraryRepository::create(conn, "L", "/p", ScanningStrategy::Default)
            .await
            .unwrap();
        let s = make_series(conn, lib.id, "Chapter Series").await;
        add_classified_book(conn, s, lib.id, "/c1.cbz", None, Some(1.0)).await;
        add_classified_book(conn, s, lib.id, "/c2.cbz", None, Some(142.5)).await;

        let report = seed_tracking_for_series(conn, s).await.unwrap();
        assert!(report.track_chapters);
        assert!(!report.track_volumes);
        assert_eq!(report.latest_known_chapter, Some(142.5));
        assert_eq!(report.latest_known_volume, None);
    }

    #[tokio::test]
    async fn seed_keeps_both_axes_when_books_have_both_classifications() {
        let (db, _temp) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let lib = LibraryRepository::create(conn, "L", "/p", ScanningStrategy::Default)
            .await
            .unwrap();
        let s = make_series(conn, lib.id, "Mixed Series").await;
        add_classified_book(conn, s, lib.id, "/v1.cbz", Some(1), None).await;
        add_classified_book(conn, s, lib.id, "/v2c10.cbz", Some(2), Some(10.0)).await;

        let report = seed_tracking_for_series(conn, s).await.unwrap();
        assert!(report.track_chapters);
        assert!(report.track_volumes);
        assert_eq!(report.latest_known_chapter, Some(10.0));
        assert_eq!(report.latest_known_volume, Some(2));
    }

    #[tokio::test]
    async fn seed_overwrites_track_flags_and_latest_known_on_re_seed() {
        let (db, _temp) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let lib = LibraryRepository::create(conn, "L", "/p", ScanningStrategy::Default)
            .await
            .unwrap();
        let s = make_series(conn, lib.id, "Repolled").await;
        add_classified_book(conn, s, lib.id, "/v1.cbz", Some(1), None).await;

        // First seed: only volume axis on disk, latest_known_volume = 1.
        let first = seed_tracking_for_series(conn, s).await.unwrap();
        assert_eq!(first.latest_known_volume, Some(1));

        // User adds a new book with vol 5; re-seed bumps latest_known_volume.
        add_classified_book(conn, s, lib.id, "/v5.cbz", Some(5), None).await;
        let second = seed_tracking_for_series(conn, s).await.unwrap();
        assert_eq!(second.latest_known_volume, Some(5));

        let row = SeriesTrackingRepository::get(conn, s)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(row.latest_known_volume, Some(5));
    }

    #[tokio::test]
    async fn seed_does_not_flip_tracked() {
        let (db, _temp) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let lib = LibraryRepository::create(conn, "L", "/p", ScanningStrategy::Default)
            .await
            .unwrap();
        let s = make_series(conn, lib.id, "Untracked").await;

        seed_tracking_for_series(conn, s).await.unwrap();
        let row = SeriesTrackingRepository::get(conn, s)
            .await
            .unwrap()
            .unwrap();
        assert!(!row.tracked);
    }

    #[tokio::test]
    async fn seed_reports_missing_series_as_error() {
        let (db, _temp) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let bogus = Uuid::new_v4();
        let err = seed_tracking_for_series(conn, bogus).await.unwrap_err();
        assert!(err.to_string().contains("not found"));
    }
}
