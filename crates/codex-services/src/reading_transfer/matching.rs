//! Resolving an export document's series and books against the current
//! library.
//!
//! Every step here follows the same rule: gather every candidate, keep only
//! the ones visible to the importing user, and accept the step's result only
//! when exactly one visible candidate remains. Multiple candidates are
//! reported as ambiguous rather than guessed, and a candidate the user cannot
//! see is simply not a candidate at all, so a series or book hidden behind a
//! sharing tag resolves as unmatched rather than as a permission error that
//! would confirm it exists.

use anyhow::Result;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use std::collections::HashSet;
use uuid::Uuid;

use codex_db::entities::{books, series, series_external_ids};
use codex_db::repositories::SeriesRepository;

use crate::content_filter::ContentFilter;

use super::model::{ExportBookDto, ExportSeriesDto, HashMode};
use super::{file_stem, series_relative_book_path};

/// The outcome of resolving one exported series against the current library.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeriesMatch {
    Matched(Uuid),
    Ambiguous,
    Unmatched,
}

/// The outcome of resolving one exported book against its matched series.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BookMatch {
    Matched(Uuid),
    StemMatch(Uuid),
    Ambiguous,
    Unmatched,
    HashMismatch,
}

/// A book in the current library, scoped to one series, as seen by the
/// matcher. Deliberately narrower than `books::Model`: the matcher only needs
/// enough to compare against the export.
#[derive(Debug, Clone)]
pub struct BookCandidate {
    pub id: Uuid,
    pub relative_path: String,
    pub file_name: String,
    pub file_hash: String,
    pub partial_hash: String,
}

impl BookCandidate {
    pub fn from_model(model: &books::Model, library_path: &str, series_path: &str) -> Self {
        Self {
            id: model.id,
            relative_path: series_relative_book_path(library_path, series_path, &model.path),
            file_name: model.file_name.clone(),
            file_hash: model.file_hash.clone(),
            partial_hash: model.partial_hash.clone(),
        }
    }
}

/// Keep only the ids visible to the user, deduplicated. Order does not
/// matter: callers only ever ask how many are left.
fn visible_ids(content_filter: &ContentFilter, ids: Vec<Uuid>) -> Vec<Uuid> {
    let mut seen = HashSet::new();
    ids.into_iter()
        .filter(|id| content_filter.is_series_visible(*id) && seen.insert(*id))
        .collect()
}

/// Visible candidates that still have at least one book on disk.
///
/// Splitting a library on the same instance leaves the old library's series
/// in place until someone deletes it, with every book soft-deleted by the
/// rescan that noticed the files had gone. That leftover series matches the
/// export exactly by path, and matching it would stop the search there with
/// no book to write onto, never reaching the new series. A series with nothing
/// on disk is not somewhere reading state can land, so it is not a candidate.
/// Every matching step funnels through here, so scoping to a library is done
/// once rather than in each lookup. That scope is what lets an import run
/// while the old copy of a series is still on disk: without it the old and
/// new series both match the same name and the step reports `Ambiguous`.
async fn live_candidates(
    db: &DatabaseConnection,
    content_filter: &ContentFilter,
    ids: Vec<Uuid>,
    library_ids: Option<&[Uuid]>,
) -> Result<Vec<Uuid>> {
    let visible = visible_ids(content_filter, ids);
    if visible.is_empty() {
        return Ok(visible);
    }
    let mut query = books::Entity::find()
        .filter(books::Column::SeriesId.is_in(visible.clone()))
        .filter(books::Column::Deleted.eq(false));
    if let Some(wanted) = library_ids {
        query = query.filter(books::Column::LibraryId.is_in(wanted.to_vec()));
    }
    let live: HashSet<Uuid> = query
        .all(db)
        .await?
        .into_iter()
        .map(|b| b.series_id)
        .collect();
    Ok(visible.into_iter().filter(|id| live.contains(id)).collect())
}

async fn series_ids_by_external_id(
    db: &DatabaseConnection,
    source: &str,
    external_id: &str,
) -> Result<Vec<Uuid>> {
    let rows = series_external_ids::Entity::find()
        .filter(series_external_ids::Column::Source.eq(source))
        .filter(series_external_ids::Column::ExternalId.eq(external_id))
        .all(db)
        .await?;
    Ok(rows.into_iter().map(|r| r.series_id).collect())
}

/// Match by `series.path` across every library, not just the one the export
/// came from: a library split re-roots series under a different library id,
/// so scoping this to one library would defeat the entire feature.
///
/// Stored series paths use the server's own separator, while exports always
/// use `/`, so a Windows-stored path is compared in both spellings.
async fn series_ids_by_path(db: &DatabaseConnection, path: &str) -> Result<Vec<Uuid>> {
    let spellings = vec![path.to_string(), path.replace('/', "\\")];
    let rows = series::Entity::find()
        .filter(series::Column::Path.is_in(spellings))
        .all(db)
        .await?;
    Ok(rows.into_iter().map(|r| r.id).collect())
}

/// Match by `series.normalized_name` across every library, for the same
/// reason as [`series_ids_by_path`].
async fn series_ids_by_normalized_name(
    db: &DatabaseConnection,
    normalized_name: &str,
) -> Result<Vec<Uuid>> {
    let rows = series::Entity::find()
        .filter(series::Column::NormalizedName.eq(normalized_name))
        .all(db)
        .await?;
    Ok(rows.into_iter().map(|r| r.id).collect())
}

/// Resolve an exported series against the current library.
///
/// Tries, in order: each source in `source_preference` for which the export
/// carries an external id (skipping a source the export does not have, and
/// moving to the next preferred source when a tried one yields no visible
/// candidate), then `series.path`, then `series.normalized_name`. Stops at
/// the first step that produces any visible candidate.
///
/// `source_preference` of `None` means every source the exported series
/// carries, in document order. That is the useful default: an external id is
/// the only key that survives both a rename and a move, so a caller who names
/// no sources should still get it rather than falling through to the two
/// weakest steps. `Some(&[])` skips the id steps outright.
pub async fn resolve_series(
    db: &DatabaseConnection,
    content_filter: &ContentFilter,
    exported: &ExportSeriesDto,
    source_preference: Option<&[String]>,
    library_ids: Option<&[Uuid]>,
) -> Result<SeriesMatch> {
    let sources: Vec<&str> = match source_preference {
        Some(preferred) => preferred.iter().map(String::as_str).collect(),
        None => exported
            .external_ids
            .iter()
            .map(|e| e.source.as_str())
            .collect(),
    };

    for source in sources {
        let Some(external) = exported.external_ids.iter().find(|e| e.source == source) else {
            continue;
        };

        let candidates = series_ids_by_external_id(db, source, &external.id).await?;
        match live_candidates(db, content_filter, candidates, library_ids)
            .await?
            .as_slice()
        {
            [] => continue,
            [only] => return Ok(SeriesMatch::Matched(*only)),
            _ => return Ok(SeriesMatch::Ambiguous),
        }
    }

    let by_path = series_ids_by_path(db, &exported.library_relative_path).await?;
    match live_candidates(db, content_filter, by_path, library_ids)
        .await?
        .as_slice()
    {
        [] => {}
        [only] => return Ok(SeriesMatch::Matched(*only)),
        _ => return Ok(SeriesMatch::Ambiguous),
    }

    let normalized = SeriesRepository::normalize_name(&exported.name);
    let by_name = series_ids_by_normalized_name(db, &normalized).await?;
    match live_candidates(db, content_filter, by_name, library_ids)
        .await?
        .as_slice()
    {
        [] => Ok(SeriesMatch::Unmatched),
        [only] => Ok(SeriesMatch::Matched(*only)),
        _ => Ok(SeriesMatch::Ambiguous),
    }
}

/// One matching step's outcome: `None` means "no usable candidate, try the
/// next step"; `Some` means the search is over, one way or another.
fn decide_step(
    matches: Vec<&BookCandidate>,
    exported: &ExportBookDto,
    hash_mode: HashMode,
    is_stem_step: bool,
) -> Option<BookMatch> {
    match matches.len() {
        0 => None,
        1 => {
            let candidate = matches[0];
            // Never on the stem step: it exists for a `.cbr` repacked to
            // `.cbz`, and a repack always changes the hash, so checking there
            // would turn every real repack into a mismatch.
            let hashes_disagree = !is_stem_step
                && hash_mode != HashMode::Off
                && !exported.file_hash.is_empty()
                && !candidate.file_hash.is_empty()
                && exported.file_hash != candidate.file_hash;
            if hashes_disagree {
                Some(BookMatch::HashMismatch)
            } else if is_stem_step {
                Some(BookMatch::StemMatch(candidate.id))
            } else {
                Some(BookMatch::Matched(candidate.id))
            }
        }
        _ => Some(BookMatch::Ambiguous),
    }
}

/// Resolve an exported book against the books already known to belong to its
/// matched series.
///
/// Tries, in order: series-relative path, file name, filename stem (survives
/// a `.cbr` repacked to `.cbz`), and finally, only under `hash_mode = match`,
/// `file_hash` / `partial_hash` (rescues a bulk rename). Empty hash values
/// are never used as a matching key or a mismatch signal: the column is only
/// populated during analysis, so an unanalyzed book legitimately has `""`.
pub fn resolve_book(
    candidates: &[BookCandidate],
    exported: &ExportBookDto,
    hash_mode: HashMode,
) -> BookMatch {
    let by_path: Vec<&BookCandidate> = candidates
        .iter()
        .filter(|c| c.relative_path == exported.path)
        .collect();
    if let Some(result) = decide_step(by_path, exported, hash_mode, false) {
        return result;
    }

    let by_name: Vec<&BookCandidate> = candidates
        .iter()
        .filter(|c| c.file_name == exported.file_name)
        .collect();
    if let Some(result) = decide_step(by_name, exported, hash_mode, false) {
        return result;
    }

    let stem = file_stem(&exported.file_name);
    let by_stem: Vec<&BookCandidate> = candidates
        .iter()
        .filter(|c| file_stem(&c.file_name) == stem)
        .collect();
    if let Some(result) = decide_step(by_stem, exported, hash_mode, true) {
        return result;
    }

    if hash_mode == HashMode::Match {
        let hash_usable = !exported.file_hash.is_empty();
        let partial_usable = !exported.partial_hash.is_empty();
        let by_hash: Vec<&BookCandidate> = candidates
            .iter()
            .filter(|c| {
                (hash_usable && !c.file_hash.is_empty() && c.file_hash == exported.file_hash)
                    || (partial_usable
                        && !c.partial_hash.is_empty()
                        && c.partial_hash == exported.partial_hash)
            })
            .collect();
        match by_hash.len() {
            0 => {}
            1 => return BookMatch::Matched(by_hash[0].id),
            _ => return BookMatch::Ambiguous,
        }
    }

    BookMatch::Unmatched
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content_filter::ContentFilter;

    fn candidate(
        id: Uuid,
        relative_path: &str,
        file_name: &str,
        file_hash: &str,
        partial_hash: &str,
    ) -> BookCandidate {
        BookCandidate {
            id,
            relative_path: relative_path.to_string(),
            file_name: file_name.to_string(),
            file_hash: file_hash.to_string(),
            partial_hash: partial_hash.to_string(),
        }
    }

    fn exported_book(
        path: &str,
        file_name: &str,
        file_hash: &str,
        partial_hash: &str,
    ) -> ExportBookDto {
        ExportBookDto {
            path: path.to_string(),
            file_name: file_name.to_string(),
            file_hash: file_hash.to_string(),
            partial_hash: partial_hash.to_string(),
            progress: None,
            completions: vec![],
            sessions: None,
        }
    }

    #[test]
    fn matches_by_exact_relative_path() {
        let id = Uuid::new_v4();
        let candidates = vec![candidate(id, "Vol 01/v01.cbz", "v01.cbz", "", "")];
        let exported = exported_book("Vol 01/v01.cbz", "v01.cbz", "", "");
        assert_eq!(
            resolve_book(&candidates, &exported, HashMode::Verify),
            BookMatch::Matched(id)
        );
    }

    #[test]
    fn falls_back_to_file_name_when_path_moved() {
        let id = Uuid::new_v4();
        // The volume folder was flattened away, but the file kept its name.
        let candidates = vec![candidate(id, "v01.cbz", "v01.cbz", "", "")];
        let exported = exported_book("Vol 01/v01.cbz", "v01.cbz", "", "");
        assert_eq!(
            resolve_book(&candidates, &exported, HashMode::Verify),
            BookMatch::Matched(id)
        );
    }

    #[test]
    fn falls_back_to_stem_for_a_repack() {
        let id = Uuid::new_v4();
        let candidates = vec![candidate(id, "v01.cbz", "v01.cbz", "", "")];
        let exported = exported_book("v01.cbr", "v01.cbr", "", "");
        assert_eq!(
            resolve_book(&candidates, &exported, HashMode::Verify),
            BookMatch::StemMatch(id)
        );
    }

    #[test]
    fn two_files_sharing_a_stem_are_ambiguous() {
        let candidates = vec![
            candidate(Uuid::new_v4(), "v01.cbz", "v01.cbz", "", ""),
            candidate(Uuid::new_v4(), "v01.cbr", "v01.cbr", "", ""),
        ];
        let exported = exported_book("elsewhere/v01.epub", "v01.epub", "", "");
        assert_eq!(
            resolve_book(&candidates, &exported, HashMode::Verify),
            BookMatch::Ambiguous
        );
    }

    #[test]
    fn verify_mode_rejects_a_hash_disagreement() {
        let id = Uuid::new_v4();
        let candidates = vec![candidate(id, "v01.cbz", "v01.cbz", "hash-b", "")];
        let exported = exported_book("v01.cbz", "v01.cbz", "hash-a", "");
        assert_eq!(
            resolve_book(&candidates, &exported, HashMode::Verify),
            BookMatch::HashMismatch
        );
    }

    #[test]
    fn off_mode_ignores_a_hash_disagreement() {
        let id = Uuid::new_v4();
        let candidates = vec![candidate(id, "v01.cbz", "v01.cbz", "hash-b", "")];
        let exported = exported_book("v01.cbz", "v01.cbz", "hash-a", "");
        assert_eq!(
            resolve_book(&candidates, &exported, HashMode::Off),
            BookMatch::Matched(id)
        );
    }

    #[test]
    fn empty_hashes_never_trigger_a_mismatch() {
        let id = Uuid::new_v4();
        // Neither side has been analyzed.
        let candidates = vec![candidate(id, "v01.cbz", "v01.cbz", "", "")];
        let exported = exported_book("v01.cbz", "v01.cbz", "", "");
        assert_eq!(
            resolve_book(&candidates, &exported, HashMode::Verify),
            BookMatch::Matched(id)
        );
    }

    #[test]
    fn match_mode_rescues_a_bulk_rename_by_hash() {
        let id = Uuid::new_v4();
        let candidates = vec![candidate(
            id,
            "renamed/totally-different.cbz",
            "totally-different.cbz",
            "hash-a",
            "",
        )];
        let exported = exported_book("v01.cbz", "v01.cbz", "hash-a", "");
        assert_eq!(
            resolve_book(&candidates, &exported, HashMode::Match),
            BookMatch::Matched(id)
        );
    }

    #[test]
    fn verify_mode_does_not_use_hash_as_a_matching_key() {
        // Same scenario as the rescue above, but under `verify`: path and
        // name both fail, and hash matching is a `match`-mode-only step.
        let id = Uuid::new_v4();
        let candidates = vec![candidate(
            id,
            "renamed/totally-different.cbz",
            "totally-different.cbz",
            "hash-a",
            "",
        )];
        let exported = exported_book("v01.cbz", "v01.cbz", "hash-a", "");
        assert_eq!(
            resolve_book(&candidates, &exported, HashMode::Verify),
            BookMatch::Unmatched
        );
    }

    #[test]
    fn no_candidates_is_unmatched() {
        let exported = exported_book("v01.cbz", "v01.cbz", "", "");
        assert_eq!(
            resolve_book(&[], &exported, HashMode::Verify),
            BookMatch::Unmatched
        );
    }

    #[test]
    fn ambiguous_by_path_does_not_fall_through_to_name() {
        // Two candidates share the exact relative path (should not happen,
        // but must not be resolved by silently trying the next step).
        let candidates = vec![
            candidate(Uuid::new_v4(), "v01.cbz", "a.cbz", "", ""),
            candidate(Uuid::new_v4(), "v01.cbz", "b.cbz", "", ""),
        ];
        let exported = exported_book("v01.cbz", "a.cbz", "", "");
        assert_eq!(
            resolve_book(&candidates, &exported, HashMode::Verify),
            BookMatch::Ambiguous
        );
    }

    // ------------------------------------------------------------------
    // Series resolution: exercises the DB queries and the visibility gate.
    // ------------------------------------------------------------------

    use codex_db::ScanningStrategy;
    use codex_db::entities::user_sharing_tags::AccessMode;
    use codex_db::repositories::{
        LibraryRepository, SeriesExternalIdRepository, SeriesRepository, SharingTagRepository,
        UserRepository,
    };
    use codex_db::test_helpers::create_test_db;

    async fn make_user(db: &DatabaseConnection) -> Uuid {
        use chrono::Utc;
        use codex_db::entities::users;
        let now = Utc::now();
        let model = users::Model {
            id: Uuid::new_v4(),
            username: format!("u-{}", Uuid::new_v4()),
            email: format!("{}@test.com", Uuid::new_v4()),
            password_hash: "hash".to_string(),
            role: "reader".to_string(),
            is_active: true,
            email_verified: true,
            permissions: serde_json::json!([]),
            created_at: now,
            updated_at: now,
            last_login_at: None,
        };
        UserRepository::create(db, &model).await.unwrap().id
    }

    fn doc_series(name: &str, path: &str, external: Option<(&str, &str)>) -> ExportSeriesDto {
        ExportSeriesDto {
            external_ids: external
                .map(|(source, id)| {
                    vec![super::super::model::ExportExternalIdDto {
                        source: source.to_string(),
                        id: id.to_string(),
                    }]
                })
                .unwrap_or_default(),
            library_relative_path: path.to_string(),
            name: name.to_string(),
            rating: None,
            notes: None,
            rating_updated_at: None,
            books: vec![],
        }
    }

    /// A series with one book on disk: a series with nothing on disk is not a
    /// place reading state can land, so the matcher ignores it.
    async fn live_series(
        conn: &DatabaseConnection,
        library_id: Uuid,
        name: &str,
    ) -> codex_db::entities::series::Model {
        let series = SeriesRepository::create(conn, library_id, name, None)
            .await
            .unwrap();
        add_book(conn, &series, false).await;
        series
    }

    async fn add_book(
        conn: &DatabaseConnection,
        series: &codex_db::entities::series::Model,
        deleted: bool,
    ) {
        use codex_db::repositories::BookRepository;
        use sea_orm::{ActiveModelTrait, Set};
        let now = chrono::Utc::now();
        let book = books::Model {
            id: Uuid::new_v4(),
            series_id: series.id,
            library_id: series.library_id,
            path: format!("/lib/{}.cbz", Uuid::new_v4()),
            file_name: "b.cbz".to_string(),
            file_size: 1,
            file_hash: String::new(),
            partial_hash: String::new(),
            format: "cbz".to_string(),
            page_count: 1,
            deleted: false,
            analyzed: false,
            analysis_error: None,
            analysis_errors: None,
            modified_at: now,
            created_at: now,
            updated_at: now,
            thumbnail_path: None,
            thumbnail_generated_at: None,
            koreader_hash: None,
            epub_positions: None,
            epub_spine_items: None,
        };
        let book = BookRepository::create(conn, &book, None).await.unwrap();
        if deleted {
            let mut gone: books::ActiveModel = book.into();
            gone.deleted = Set(true);
            gone.update(conn).await.unwrap();
        }
    }

    /// The same-instance split: the old library's series is still there with
    /// every book soft-deleted, and matches the export exactly by path. It
    /// must not stop the search, or the new series is never reached.
    #[tokio::test]
    async fn a_series_with_nothing_on_disk_is_not_a_candidate() {
        let (db, _tmp) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let user = make_user(conn).await;
        let old_library =
            LibraryRepository::create(conn, "Old", "/manga", ScanningStrategy::Default)
                .await
                .unwrap();
        let new_library =
            LibraryRepository::create(conn, "New", "/shonen", ScanningStrategy::Default)
                .await
                .unwrap();
        let leftover = SeriesRepository::create(conn, old_library.id, "Naruto", None)
            .await
            .unwrap();
        SeriesRepository::update_path(conn, leftover.id, "shonen/Naruto".to_string())
            .await
            .unwrap();
        add_book(conn, &leftover, true).await;
        let moved = live_series(conn, new_library.id, "Naruto").await;

        let filter = ContentFilter::for_user(conn, user).await.unwrap();
        let exported = doc_series("Naruto", "shonen/Naruto", None);
        let result = resolve_series(conn, &filter, &exported, None, None)
            .await
            .unwrap();
        assert_eq!(result, SeriesMatch::Matched(moved.id));
    }

    #[tokio::test]
    async fn resolves_by_path_when_no_external_id_matches() {
        let (db, _tmp) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let user = make_user(conn).await;
        let library = LibraryRepository::create(conn, "Lib", "/lib", ScanningStrategy::Default)
            .await
            .unwrap();
        let series = live_series(conn, library.id, "Naruto").await;
        SeriesRepository::update_path(conn, series.id, "shonen/Naruto".to_string())
            .await
            .unwrap();

        let filter = ContentFilter::for_user(conn, user).await.unwrap();
        let doc = doc_series("Naruto", "shonen/Naruto", None);
        let result = resolve_series(conn, &filter, &doc, None, None)
            .await
            .unwrap();
        assert_eq!(result, SeriesMatch::Matched(series.id));
    }

    #[tokio::test]
    async fn falls_back_to_normalized_name() {
        let (db, _tmp) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let user = make_user(conn).await;
        let library = LibraryRepository::create(conn, "Lib", "/lib", ScanningStrategy::Default)
            .await
            .unwrap();
        let series = live_series(conn, library.id, "One Piece").await;

        let filter = ContentFilter::for_user(conn, user).await.unwrap();
        // A path that does not exist anywhere; only the name matches.
        let doc = doc_series("One Piece", "moved/somewhere/else", None);
        let result = resolve_series(conn, &filter, &doc, None, None)
            .await
            .unwrap();
        assert_eq!(result, SeriesMatch::Matched(series.id));
    }

    #[tokio::test]
    async fn preference_order_decides_the_winner() {
        let (db, _tmp) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let user = make_user(conn).await;
        let library = LibraryRepository::create(conn, "Lib", "/lib", ScanningStrategy::Default)
            .await
            .unwrap();
        let series_a = live_series(conn, library.id, "Series A").await;
        let series_b = live_series(conn, library.id, "Series B").await;
        SeriesExternalIdRepository::create(
            conn,
            series_a.id,
            "plugin:mangabaka",
            "111",
            None,
            None,
        )
        .await
        .unwrap();
        SeriesExternalIdRepository::create(conn, series_b.id, "plugin:anilist", "222", None, None)
            .await
            .unwrap();

        let filter = ContentFilter::for_user(conn, user).await.unwrap();
        let mut doc = doc_series("Whatever", "does/not/exist", None);
        doc.external_ids = vec![
            super::super::model::ExportExternalIdDto {
                source: "plugin:mangabaka".to_string(),
                id: "111".to_string(),
            },
            super::super::model::ExportExternalIdDto {
                source: "plugin:anilist".to_string(),
                id: "222".to_string(),
            },
        ];

        let winner_a = resolve_series(
            conn,
            &filter,
            &doc,
            Some(&["plugin:mangabaka".to_string(), "plugin:anilist".to_string()]),
            None,
        )
        .await
        .unwrap();
        assert_eq!(winner_a, SeriesMatch::Matched(series_a.id));

        let winner_b = resolve_series(
            conn,
            &filter,
            &doc,
            Some(&["plugin:anilist".to_string(), "plugin:mangabaka".to_string()]),
            None,
        )
        .await
        .unwrap();
        assert_eq!(winner_b, SeriesMatch::Matched(series_b.id));
    }

    #[tokio::test]
    async fn multiple_candidates_are_ambiguous() {
        let (db, _tmp) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let user = make_user(conn).await;
        let library = LibraryRepository::create(conn, "Lib", "/lib", ScanningStrategy::Default)
            .await
            .unwrap();
        // Two libraries can each contain a series with the same normalized
        // name after a split; nothing may guess between them.
        let library2 = LibraryRepository::create(conn, "Lib2", "/lib2", ScanningStrategy::Default)
            .await
            .unwrap();
        live_series(conn, library.id, "Duplicate").await;
        live_series(conn, library2.id, "Duplicate").await;

        let filter = ContentFilter::for_user(conn, user).await.unwrap();
        let doc = doc_series("Duplicate", "nowhere/matching", None);
        let result = resolve_series(conn, &filter, &doc, None, None)
            .await
            .unwrap();
        assert_eq!(result, SeriesMatch::Ambiguous);
    }

    #[tokio::test]
    async fn a_series_the_user_cannot_see_resolves_as_unmatched() {
        let (db, _tmp) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let user = make_user(conn).await;
        let library = LibraryRepository::create(conn, "Lib", "/lib", ScanningStrategy::Default)
            .await
            .unwrap();
        let series = live_series(conn, library.id, "Hidden").await;

        let tag = SharingTagRepository::create(conn, "restricted", None)
            .await
            .unwrap();
        SharingTagRepository::add_tag_to_series(conn, series.id, tag.id)
            .await
            .unwrap();
        // A personal deny grant is enough on its own: deny always wins,
        // regardless of whitelist mode.
        SharingTagRepository::set_user_grant(conn, user, tag.id, AccessMode::Deny)
            .await
            .unwrap();

        let filter = ContentFilter::for_user(conn, user).await.unwrap();
        let doc = doc_series("Hidden", &series.path, None);
        let result = resolve_series(conn, &filter, &doc, None, None)
            .await
            .unwrap();
        assert_eq!(result, SeriesMatch::Unmatched);
    }
}
