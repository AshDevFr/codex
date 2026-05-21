//! Tests for the SeriesDuplicatesRepository
//!
//! Covers both detection passes:
//! - external-id (cross-library, high confidence)
//! - title       (per-library, medium confidence)
//!
//! Plus cleanup-on-delete and idempotency.

#[path = "../common/mod.rs"]
mod common;

use chrono::Utc;
use codex::db::entities::series_duplicates::{MATCH_TYPE_EXTERNAL_ID, MATCH_TYPE_TITLE};
use codex::db::entities::{libraries, series, series_metadata};
use codex::db::repositories::{
    SeriesDuplicatesRepository, SeriesExternalIdRepository, SeriesMetadataRepository,
};
use codex::utils::normalize_for_search;
use common::{create_test_library, setup_test_db};
use sea_orm::{ActiveModelTrait, DatabaseConnection, Set};
use uuid::Uuid;

/// Whitelist used by tests that exercise the external-ID pass. The pass is
/// disabled when this slice is empty, so every existing test passes the
/// sources it relies on through here.
fn trusted_test_sources() -> Vec<String> {
    vec!["plugin:mangabaka".to_string(), "plugin:anilist".to_string()]
}

/// Create a series with a unique path (UUID-based) and an explicit name.
/// Unlike the shared `create_test_series` helper this never collides on the
/// `(library_id, path)` unique constraint when multiple series share a name.
async fn make_series(
    db: &DatabaseConnection,
    library: &libraries::Model,
    name: &str,
) -> series::Model {
    let id = Uuid::new_v4();
    let now = Utc::now();
    let model = series::ActiveModel {
        id: Set(id),
        library_id: Set(library.id),
        fingerprint: Set(Some(format!("fp-{}", id))),
        path: Set(format!("/series/{}", id)),
        name: Set(name.to_string()),
        normalized_name: Set(name.to_lowercase()),
        created_at: Set(now),
        updated_at: Set(now),
    };
    model.insert(db).await.unwrap()
}

/// Helper: ensure a series has metadata with the given title and a normalized
/// search_title. Returns the metadata model.
async fn upsert_series_title(
    db: &DatabaseConnection,
    series_id: Uuid,
    title: &str,
) -> series_metadata::Model {
    // Create initial metadata via the repository (which normalizes search_title),
    // then overwrite if it already exists.
    match SeriesMetadataRepository::get_by_series_id(db, series_id)
        .await
        .unwrap()
    {
        Some(existing) => {
            let mut active: series_metadata::ActiveModel = existing.into();
            active.title = Set(title.to_string());
            active.search_title = Set(normalize_for_search(title));
            active.updated_at = Set(Utc::now());
            active.update(db).await.unwrap()
        }
        None => SeriesMetadataRepository::create(db, series_id, title)
            .await
            .unwrap(),
    }
}

// ---------------------------------------------------------------------------
// External-ID detection
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_rebuild_detects_shared_external_id_within_library() {
    let (db, _tmp) = setup_test_db().await;
    let library = create_test_library(&db, "Lib", "/lib").await;

    let s1 = make_series(&db, &library, "Naruto").await;
    let s2 = make_series(&db, &library, "ナルト").await;

    // Both end up matched to the same MangaBaka entry after metadata fetch.
    SeriesExternalIdRepository::create_for_plugin(&db, s1.id, "mangabaka", "12345", None, None)
        .await
        .unwrap();
    SeriesExternalIdRepository::create_for_plugin(&db, s2.id, "mangabaka", "12345", None, None)
        .await
        .unwrap();

    let count = SeriesDuplicatesRepository::rebuild_from_series(&db, &trusted_test_sources())
        .await
        .unwrap();
    assert_eq!(count, 1);

    let groups = SeriesDuplicatesRepository::find_all(&db).await.unwrap();
    assert_eq!(groups.len(), 1);
    let g = &groups[0];
    assert_eq!(g.match_type, MATCH_TYPE_EXTERNAL_ID);
    assert_eq!(g.match_key, "plugin:mangabaka:12345");
    assert!(
        g.library_id.is_none(),
        "external_id matches are cross-library"
    );
    let ids: Vec<Uuid> = serde_json::from_str(&g.series_ids).unwrap();
    assert_eq!(g.duplicate_count, 2);
    assert!(ids.contains(&s1.id));
    assert!(ids.contains(&s2.id));
}

#[tokio::test]
async fn test_rebuild_detects_external_id_across_libraries() {
    let (db, _tmp) = setup_test_db().await;
    let lib1 = create_test_library(&db, "Comics", "/comics").await;
    let lib2 = create_test_library(&db, "Manga", "/manga").await;

    let s1 = make_series(&db, &lib1, "Naruto").await;
    let s2 = make_series(&db, &lib2, "Naruto").await;

    SeriesExternalIdRepository::create_for_plugin(&db, s1.id, "mangabaka", "999", None, None)
        .await
        .unwrap();
    SeriesExternalIdRepository::create_for_plugin(&db, s2.id, "mangabaka", "999", None, None)
        .await
        .unwrap();

    let count = SeriesDuplicatesRepository::rebuild_from_series(&db, &trusted_test_sources())
        .await
        .unwrap();
    assert_eq!(count, 1);

    let groups = SeriesDuplicatesRepository::find_all(&db).await.unwrap();
    assert_eq!(groups[0].match_type, MATCH_TYPE_EXTERNAL_ID);
    assert!(groups[0].library_id.is_none());
}

#[tokio::test]
async fn test_rebuild_distinguishes_sources_for_same_external_id() {
    let (db, _tmp) = setup_test_db().await;
    let library = create_test_library(&db, "Lib", "/lib").await;

    let s1 = make_series(&db, &library, "A").await;
    let s2 = make_series(&db, &library, "B").await;

    // Same external_id "42" but different sources should NOT collide.
    SeriesExternalIdRepository::create_for_plugin(&db, s1.id, "mangabaka", "42", None, None)
        .await
        .unwrap();
    SeriesExternalIdRepository::create_for_plugin(&db, s2.id, "anilist", "42", None, None)
        .await
        .unwrap();

    let count = SeriesDuplicatesRepository::rebuild_from_series(&db, &trusted_test_sources())
        .await
        .unwrap();
    assert_eq!(count, 0);
}

// ---------------------------------------------------------------------------
// Title detection
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_rebuild_detects_title_duplicates_within_library() {
    let (db, _tmp) = setup_test_db().await;
    let library = create_test_library(&db, "Lib", "/lib").await;

    let s1 = make_series(&db, &library, "Naruto").await;
    let s2 = make_series(&db, &library, "naruto").await;
    let s3 = make_series(&db, &library, "Bleach").await;

    upsert_series_title(&db, s1.id, "Naruto").await;
    upsert_series_title(&db, s2.id, "Naruto").await;
    upsert_series_title(&db, s3.id, "Bleach").await;

    let count = SeriesDuplicatesRepository::rebuild_from_series(&db, &trusted_test_sources())
        .await
        .unwrap();
    assert_eq!(count, 1);

    let groups = SeriesDuplicatesRepository::find_all(&db).await.unwrap();
    assert_eq!(groups.len(), 1);
    let g = &groups[0];
    assert_eq!(g.match_type, MATCH_TYPE_TITLE);
    assert_eq!(g.match_key, "naruto");
    assert_eq!(g.library_id, Some(library.id));
    assert_eq!(g.duplicate_count, 2);
    let ids: Vec<Uuid> = serde_json::from_str(&g.series_ids).unwrap();
    assert!(ids.contains(&s1.id));
    assert!(ids.contains(&s2.id));
    assert!(!ids.contains(&s3.id));
}

#[tokio::test]
async fn test_title_duplicates_scoped_to_library() {
    let (db, _tmp) = setup_test_db().await;
    let comics = create_test_library(&db, "Comics", "/comics").await;
    let manga = create_test_library(&db, "Manga", "/manga").await;

    let s1 = make_series(&db, &comics, "Naruto").await;
    let s2 = make_series(&db, &manga, "Naruto").await;

    upsert_series_title(&db, s1.id, "Naruto").await;
    upsert_series_title(&db, s2.id, "Naruto").await;

    // Title-only matches across libraries are intentionally not flagged.
    let count = SeriesDuplicatesRepository::rebuild_from_series(&db, &trusted_test_sources())
        .await
        .unwrap();
    assert_eq!(count, 0);
}

#[tokio::test]
async fn test_rebuild_ignores_empty_search_title() {
    let (db, _tmp) = setup_test_db().await;
    let library = create_test_library(&db, "Lib", "/lib").await;

    let s1 = make_series(&db, &library, "").await;
    let s2 = make_series(&db, &library, "").await;
    // Create metadata rows but with an empty title -> empty search_title.
    upsert_series_title(&db, s1.id, "").await;
    upsert_series_title(&db, s2.id, "").await;

    let count = SeriesDuplicatesRepository::rebuild_from_series(&db, &trusted_test_sources())
        .await
        .unwrap();
    assert_eq!(count, 0);
}

// ---------------------------------------------------------------------------
// Mixed signals
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_rebuild_emits_both_match_types() {
    let (db, _tmp) = setup_test_db().await;
    let library = create_test_library(&db, "Lib", "/lib").await;

    // Pair 1: shared external id only.
    let a1 = make_series(&db, &library, "One Piece").await;
    let a2 = make_series(&db, &library, "ワンピース").await;
    SeriesExternalIdRepository::create_for_plugin(&db, a1.id, "mangabaka", "1", None, None)
        .await
        .unwrap();
    SeriesExternalIdRepository::create_for_plugin(&db, a2.id, "mangabaka", "1", None, None)
        .await
        .unwrap();

    // Pair 2: shared title only.
    let b1 = make_series(&db, &library, "Bleach").await;
    let b2 = make_series(&db, &library, "Bleach").await;
    upsert_series_title(&db, b1.id, "Bleach").await;
    upsert_series_title(&db, b2.id, "Bleach").await;

    let count = SeriesDuplicatesRepository::rebuild_from_series(&db, &trusted_test_sources())
        .await
        .unwrap();
    assert_eq!(count, 2);

    let by_type = SeriesDuplicatesRepository::find_by_match_type(&db, MATCH_TYPE_EXTERNAL_ID)
        .await
        .unwrap();
    assert_eq!(by_type.len(), 1);
    let by_type = SeriesDuplicatesRepository::find_by_match_type(&db, MATCH_TYPE_TITLE)
        .await
        .unwrap();
    assert_eq!(by_type.len(), 1);
}

// ---------------------------------------------------------------------------
// Idempotency, cleanup, delete
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_rebuild_is_idempotent() {
    let (db, _tmp) = setup_test_db().await;
    let library = create_test_library(&db, "Lib", "/lib").await;

    let s1 = make_series(&db, &library, "A").await;
    let s2 = make_series(&db, &library, "A").await;
    upsert_series_title(&db, s1.id, "A").await;
    upsert_series_title(&db, s2.id, "A").await;

    let c1 = SeriesDuplicatesRepository::rebuild_from_series(&db, &trusted_test_sources())
        .await
        .unwrap();
    let c2 = SeriesDuplicatesRepository::rebuild_from_series(&db, &trusted_test_sources())
        .await
        .unwrap();
    let c3 = SeriesDuplicatesRepository::rebuild_from_series(&db, &trusted_test_sources())
        .await
        .unwrap();
    assert_eq!(c1, c2);
    assert_eq!(c2, c3);
    assert_eq!(SeriesDuplicatesRepository::count(&db).await.unwrap(), 1);
}

#[tokio::test]
async fn test_cleanup_removes_series_from_group() {
    let (db, _tmp) = setup_test_db().await;
    let library = create_test_library(&db, "Lib", "/lib").await;

    let s1 = make_series(&db, &library, "X").await;
    let s2 = make_series(&db, &library, "X").await;
    let s3 = make_series(&db, &library, "X").await;
    upsert_series_title(&db, s1.id, "X").await;
    upsert_series_title(&db, s2.id, "X").await;
    upsert_series_title(&db, s3.id, "X").await;

    SeriesDuplicatesRepository::rebuild_from_series(&db, &trusted_test_sources())
        .await
        .unwrap();
    let groups = SeriesDuplicatesRepository::find_all(&db).await.unwrap();
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].duplicate_count, 3);

    SeriesDuplicatesRepository::cleanup_for_series(&db, s2.id)
        .await
        .unwrap();

    let groups = SeriesDuplicatesRepository::find_all(&db).await.unwrap();
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].duplicate_count, 2);
    let ids: Vec<Uuid> = serde_json::from_str(&groups[0].series_ids).unwrap();
    assert!(!ids.contains(&s2.id));
    assert!(ids.contains(&s1.id));
    assert!(ids.contains(&s3.id));
}

#[tokio::test]
async fn test_cleanup_deletes_group_when_one_remains() {
    let (db, _tmp) = setup_test_db().await;
    let library = create_test_library(&db, "Lib", "/lib").await;

    let s1 = make_series(&db, &library, "Y").await;
    let s2 = make_series(&db, &library, "Y").await;
    upsert_series_title(&db, s1.id, "Y").await;
    upsert_series_title(&db, s2.id, "Y").await;

    SeriesDuplicatesRepository::rebuild_from_series(&db, &trusted_test_sources())
        .await
        .unwrap();
    assert_eq!(
        SeriesDuplicatesRepository::find_all(&db)
            .await
            .unwrap()
            .len(),
        1
    );

    SeriesDuplicatesRepository::cleanup_for_series(&db, s2.id)
        .await
        .unwrap();

    assert!(
        SeriesDuplicatesRepository::find_all(&db)
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn test_delete_group() {
    let (db, _tmp) = setup_test_db().await;
    let library = create_test_library(&db, "Lib", "/lib").await;

    let s1 = make_series(&db, &library, "Z").await;
    let s2 = make_series(&db, &library, "Z").await;
    upsert_series_title(&db, s1.id, "Z").await;
    upsert_series_title(&db, s2.id, "Z").await;

    SeriesDuplicatesRepository::rebuild_from_series(&db, &trusted_test_sources())
        .await
        .unwrap();
    let groups = SeriesDuplicatesRepository::find_all(&db).await.unwrap();
    assert_eq!(groups.len(), 1);

    SeriesDuplicatesRepository::delete_group(&db, groups[0].id)
        .await
        .unwrap();
    assert!(
        SeriesDuplicatesRepository::find_all(&db)
            .await
            .unwrap()
            .is_empty()
    );
}

// ---------------------------------------------------------------------------
// Trusted-source whitelist
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_external_id_pass_skipped_when_whitelist_empty() {
    // Two series share `plugin:mangabaka:12345` and would otherwise be a
    // high-confidence external-ID match. With an empty whitelist the
    // external-ID pass should not run at all.
    let (db, _tmp) = setup_test_db().await;
    let library = create_test_library(&db, "Lib", "/lib").await;

    let s1 = make_series(&db, &library, "Naruto").await;
    let s2 = make_series(&db, &library, "ナルト").await;
    SeriesExternalIdRepository::create_for_plugin(&db, s1.id, "mangabaka", "12345", None, None)
        .await
        .unwrap();
    SeriesExternalIdRepository::create_for_plugin(&db, s2.id, "mangabaka", "12345", None, None)
        .await
        .unwrap();

    let count = SeriesDuplicatesRepository::rebuild_from_series(&db, &[])
        .await
        .unwrap();
    assert_eq!(count, 0, "external-ID pass must be disabled by default");
    let by_type = SeriesDuplicatesRepository::find_by_match_type(&db, MATCH_TYPE_EXTERNAL_ID)
        .await
        .unwrap();
    assert!(by_type.is_empty());
}

#[tokio::test]
async fn test_external_id_pass_ignores_untrusted_sources() {
    // `api:animenewsnetwork` is known-noisy: two unrelated series share the
    // same ANN ID. Trusting only `plugin:mangabaka` must skip the ANN match.
    let (db, _tmp) = setup_test_db().await;
    let library = create_test_library(&db, "Lib", "/lib").await;

    let s1 = make_series(&db, &library, "Fairy Tail").await;
    let s2 = make_series(&db, &library, "Fairy Tail: Blue Mistral").await;

    // Bad ANN data (untrusted): same external_id on two distinct series.
    SeriesExternalIdRepository::create(&db, s1.id, "api:animenewsnetwork", "6872", None, None)
        .await
        .unwrap();
    SeriesExternalIdRepository::create(&db, s2.id, "api:animenewsnetwork", "6872", None, None)
        .await
        .unwrap();

    // Good MangaBaka data (trusted): different external_id per series — no match.
    SeriesExternalIdRepository::create_for_plugin(&db, s1.id, "mangabaka", "a", None, None)
        .await
        .unwrap();
    SeriesExternalIdRepository::create_for_plugin(&db, s2.id, "mangabaka", "b", None, None)
        .await
        .unwrap();

    let count =
        SeriesDuplicatesRepository::rebuild_from_series(&db, &["plugin:mangabaka".to_string()])
            .await
            .unwrap();
    assert_eq!(
        count, 0,
        "ANN false positive must not be grouped when ANN is not whitelisted"
    );
}

#[tokio::test]
async fn test_external_id_pass_groups_only_whitelisted_sources() {
    // Same external_id "42" on two sources; whitelist allows only mangabaka.
    let (db, _tmp) = setup_test_db().await;
    let library = create_test_library(&db, "Lib", "/lib").await;

    let s1 = make_series(&db, &library, "A").await;
    let s2 = make_series(&db, &library, "B").await;
    let s3 = make_series(&db, &library, "C").await;
    let s4 = make_series(&db, &library, "D").await;

    SeriesExternalIdRepository::create_for_plugin(&db, s1.id, "mangabaka", "42", None, None)
        .await
        .unwrap();
    SeriesExternalIdRepository::create_for_plugin(&db, s2.id, "mangabaka", "42", None, None)
        .await
        .unwrap();
    SeriesExternalIdRepository::create(&db, s3.id, "api:animenewsnetwork", "42", None, None)
        .await
        .unwrap();
    SeriesExternalIdRepository::create(&db, s4.id, "api:animenewsnetwork", "42", None, None)
        .await
        .unwrap();

    let count =
        SeriesDuplicatesRepository::rebuild_from_series(&db, &["plugin:mangabaka".to_string()])
            .await
            .unwrap();
    assert_eq!(count, 1);

    let groups = SeriesDuplicatesRepository::find_by_match_type(&db, MATCH_TYPE_EXTERNAL_ID)
        .await
        .unwrap();
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].match_key, "plugin:mangabaka:42");
}
