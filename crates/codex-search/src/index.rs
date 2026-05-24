//! Index storage and query API for fuzzy search.
//!
//! The `sources` fields on `SeriesEntry` / `BookEntry` are read by the Phase 2
//! event listener (incremental updates) but not yet by Phase 1. Suppress the
//! dead-code warning until then.

#![allow(dead_code)]

use std::cmp::Reverse;

use nucleo_matcher::pattern::{AtomKind, CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher, Utf32String};
use parking_lot::{Mutex, RwLock};
use uuid::Uuid;

use codex_utils::normalize_for_search;

/// Source strings for a series entry, retained so we can rebuild the
/// haystack on an incremental update (Phase 2) without round-tripping
/// through the DB more than necessary.
#[derive(Debug, Clone, Default)]
pub struct SeriesSources {
    pub title: String,
    pub title_sort: Option<String>,
    /// Directory-derived fallback (`series.name`). Kept because some files
    /// will only match against the on-disk name.
    pub name: String,
    pub alt_titles: Vec<String>,
    pub authors: Vec<String>,
}

impl SeriesSources {
    /// Concatenate every searchable string into one normalized haystack.
    pub fn haystack(&self) -> String {
        let mut parts: Vec<&str> =
            Vec::with_capacity(3 + self.alt_titles.len() + self.authors.len());
        parts.push(&self.title);
        if let Some(s) = self.title_sort.as_deref() {
            parts.push(s);
        }
        parts.push(&self.name);
        for t in &self.alt_titles {
            parts.push(t);
        }
        for a in &self.authors {
            parts.push(a);
        }
        normalize_for_search(&parts.join(" "))
    }
}

/// Source strings for a book entry.
#[derive(Debug, Clone, Default)]
pub struct BookSources {
    pub title: Option<String>,
    pub file_name: String,
}

impl BookSources {
    pub fn haystack(&self) -> String {
        let joined = match self.title.as_deref() {
            Some(t) => format!("{} {}", t, self.file_name),
            None => self.file_name.clone(),
        };
        normalize_for_search(&joined)
    }
}

/// A single series row in the index.
#[derive(Debug, Clone)]
pub struct SeriesEntry {
    pub id: Uuid,
    pub library_id: Uuid,
    /// Pre-normalized concatenation of every searchable field.
    pub haystack: Utf32String,
    pub sources: SeriesSources,
}

impl SeriesEntry {
    pub fn new(id: Uuid, library_id: Uuid, sources: SeriesSources) -> Self {
        let haystack = Utf32String::from(sources.haystack());
        Self {
            id,
            library_id,
            haystack,
            sources,
        }
    }
}

/// A single book row in the index.
#[derive(Debug, Clone)]
pub struct BookEntry {
    pub id: Uuid,
    pub series_id: Uuid,
    pub library_id: Uuid,
    pub haystack: Utf32String,
    pub sources: BookSources,
}

impl BookEntry {
    pub fn new(id: Uuid, series_id: Uuid, library_id: Uuid, sources: BookSources) -> Self {
        let haystack = Utf32String::from(sources.haystack());
        Self {
            id,
            series_id,
            library_id,
            haystack,
            sources,
        }
    }
}

/// In-memory fuzzy search index over the entire library.
///
/// The index is read-mostly: queries take a read lock on the entry vecs and
/// only the matcher's scratch buffer is mutably borrowed (under a `Mutex`).
/// Writes (Phase 2) take a write lock briefly to swap in updated entries.
pub struct FuzzyIndex {
    series: RwLock<Vec<SeriesEntry>>,
    books: RwLock<Vec<BookEntry>>,
    /// `nucleo_matcher::Matcher` is `!Sync`; it owns a ~135KB scratch buffer
    /// that is reused across queries. A single global mutex is fine: scoring
    /// is microseconds and we expect very few concurrent search requests.
    matcher: Mutex<Matcher>,
}

impl FuzzyIndex {
    /// Create an empty index. Use `builder::build_from_db` to populate it.
    pub fn empty() -> Self {
        Self {
            series: RwLock::new(Vec::new()),
            books: RwLock::new(Vec::new()),
            matcher: Mutex::new(Matcher::new(Config::DEFAULT)),
        }
    }

    /// Total number of indexed series entries.
    pub fn series_count(&self) -> usize {
        self.series.read().len()
    }

    /// Total number of indexed book entries.
    pub fn book_count(&self) -> usize {
        self.books.read().len()
    }

    /// Replace the entire series vec. Used by the builder on initial load
    /// and by Phase 2's full-rebuild path.
    pub(crate) fn replace_series(&self, entries: Vec<SeriesEntry>) {
        *self.series.write() = entries;
    }

    /// Replace the entire books vec.
    pub(crate) fn replace_books(&self, entries: Vec<BookEntry>) {
        *self.books.write() = entries;
    }

    /// Insert or replace a series entry by id.
    ///
    /// Used by the event listener to apply incremental updates. Returns
    /// `true` if an existing entry was replaced, `false` if appended.
    pub fn upsert_series(&self, entry: SeriesEntry) -> bool {
        let mut entries = self.series.write();
        if let Some(slot) = entries.iter_mut().find(|e| e.id == entry.id) {
            *slot = entry;
            true
        } else {
            entries.push(entry);
            false
        }
    }

    /// Remove a series entry by id.
    ///
    /// Returns `true` if a row was removed. Cascade-removes any book entries
    /// whose `series_id` matches so the index doesn't keep orphaned books
    /// pointing at a deleted parent.
    pub fn remove_series(&self, id: Uuid) -> bool {
        let removed = {
            let mut entries = self.series.write();
            let before = entries.len();
            entries.retain(|e| e.id != id);
            entries.len() != before
        };
        if removed {
            let mut books = self.books.write();
            books.retain(|b| b.series_id != id);
        }
        removed
    }

    /// Insert or replace a book entry by id.
    pub fn upsert_book(&self, entry: BookEntry) -> bool {
        let mut entries = self.books.write();
        if let Some(slot) = entries.iter_mut().find(|e| e.id == entry.id) {
            *slot = entry;
            true
        } else {
            entries.push(entry);
            false
        }
    }

    /// Remove a book entry by id.
    pub fn remove_book(&self, id: Uuid) -> bool {
        let mut entries = self.books.write();
        let before = entries.len();
        entries.retain(|e| e.id != id);
        entries.len() != before
    }

    /// Remove all books for a given series. Used by `SeriesBulkPurged` to
    /// drop the deleted-and-purged books from the index without touching the
    /// surviving series row.
    pub fn remove_books_for_series(&self, series_id: Uuid) -> usize {
        let mut entries = self.books.write();
        let before = entries.len();
        entries.retain(|e| e.series_id != series_id);
        before - entries.len()
    }

    /// Score every series against `query` and return up to `limit` results,
    /// ranked by descending score. Empty/whitespace-only queries return
    /// nothing (explicit policy — see Phase 3 of the plan).
    ///
    /// Optionally restrict to a single library.
    pub fn search_series(
        &self,
        query: &str,
        limit: usize,
        library_id: Option<Uuid>,
    ) -> Vec<(Uuid, u32)> {
        if query.trim().is_empty() || limit == 0 {
            return Vec::new();
        }
        let Some(pattern) = build_pattern(query) else {
            return Vec::new();
        };

        let entries = self.series.read();
        let mut matcher = self.matcher.lock();

        let mut hits: Vec<(Uuid, u32)> = entries
            .iter()
            .filter(|e| library_id.is_none_or(|lib| e.library_id == lib))
            .filter_map(|e| {
                pattern
                    .score(e.haystack.slice(..), &mut matcher)
                    .map(|score| (e.id, score))
            })
            .collect();
        // Highest score first; ties broken by id for deterministic ordering.
        hits.sort_unstable_by_key(|(id, score)| (Reverse(*score), *id));
        hits.truncate(limit);
        hits
    }

    /// Score every book against `query`. Returns `(book_id, series_id, score)`
    /// so callers can run `ContentFilter::is_book_visible` without re-fetching
    /// the parent series.
    pub fn search_books(
        &self,
        query: &str,
        limit: usize,
        library_id: Option<Uuid>,
    ) -> Vec<(Uuid, Uuid, u32)> {
        if query.trim().is_empty() || limit == 0 {
            return Vec::new();
        }
        let Some(pattern) = build_pattern(query) else {
            return Vec::new();
        };

        let entries = self.books.read();
        let mut matcher = self.matcher.lock();

        let mut hits: Vec<(Uuid, Uuid, u32)> = entries
            .iter()
            .filter(|e| library_id.is_none_or(|lib| e.library_id == lib))
            .filter_map(|e| {
                pattern
                    .score(e.haystack.slice(..), &mut matcher)
                    .map(|score| (e.id, e.series_id, score))
            })
            .collect();
        hits.sort_unstable_by_key(|(id, _, score)| (Reverse(*score), *id));
        hits.truncate(limit);
        hits
    }

    /// Approximate memory footprint of the entry vecs (excluding the small
    /// matcher scratch space). Useful for the build-time log line.
    pub fn approx_memory_bytes(&self) -> usize {
        let series = self.series.read();
        let books = self.books.read();
        let series_haystacks: usize = series.iter().map(|e| utf32_string_bytes(&e.haystack)).sum();
        let book_haystacks: usize = books.iter().map(|e| utf32_string_bytes(&e.haystack)).sum();
        series_haystacks
            + book_haystacks
            + series.len() * std::mem::size_of::<SeriesEntry>()
            + books.len() * std::mem::size_of::<BookEntry>()
    }
}

impl Default for FuzzyIndex {
    fn default() -> Self {
        Self::empty()
    }
}

/// Build a `Pattern` for the query.
///
/// The query is pre-normalized via `normalize_for_search` (NFD, strip Latin
/// combining marks, lowercase) so the matcher sees the same shape we used when
/// building the haystack. `Pattern::new` then splits the result on whitespace
/// and produces one fuzzy atom per word — that is what makes `"berserk chapter"`
/// match `"berserk-chapter-12.cbz"`: each word matches independently with the
/// punctuation between them counted as a gap.
///
/// Returns `None` when the normalized query is empty (the caller should treat
/// that as "no results"; see the explicit policy in Phase 3 of the plan).
fn build_pattern(query: &str) -> Option<Pattern> {
    let needle = normalize_for_search(query);
    let trimmed = needle.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(Pattern::new(
        trimmed,
        CaseMatching::Ignore,
        Normalization::Smart,
        AtomKind::Fuzzy,
    ))
}

fn utf32_string_bytes(s: &Utf32String) -> usize {
    match s {
        Utf32String::Ascii(b) => b.len(),
        Utf32String::Unicode(c) => c.len() * std::mem::size_of::<char>(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn series_entry(title: &str, alt_titles: &[&str]) -> SeriesEntry {
        SeriesEntry::new(
            Uuid::new_v4(),
            Uuid::nil(),
            SeriesSources {
                title: title.to_string(),
                title_sort: None,
                name: title.to_string(),
                alt_titles: alt_titles.iter().map(|s| s.to_string()).collect(),
                authors: Vec::new(),
            },
        )
    }

    fn book_entry(title: Option<&str>, file_name: &str, series_id: Uuid) -> BookEntry {
        BookEntry::new(
            Uuid::new_v4(),
            series_id,
            Uuid::nil(),
            BookSources {
                title: title.map(|s| s.to_string()),
                file_name: file_name.to_string(),
            },
        )
    }

    fn populated_series(entries: Vec<SeriesEntry>) -> FuzzyIndex {
        let idx = FuzzyIndex::empty();
        idx.replace_series(entries);
        idx
    }

    fn populated_books(entries: Vec<BookEntry>) -> FuzzyIndex {
        let idx = FuzzyIndex::empty();
        idx.replace_books(entries);
        idx
    }

    #[test]
    fn finds_exact_match() {
        let one_punch = series_entry("One-Punch Man", &[]);
        let target_id = one_punch.id;
        let idx = populated_series(vec![series_entry("Berserk", &[]), one_punch]);

        let hits = idx.search_series("One-Punch Man", 10, None);
        assert!(
            hits.first().map(|h| h.0) == Some(target_id),
            "expected One-Punch Man at rank 1, got {:?}",
            hits
        );
    }

    #[test]
    fn matches_gapped_subsequence() {
        // The motivating example: "on ch" should match "one-punch".
        let one_punch = series_entry("One-Punch Man", &[]);
        let target_id = one_punch.id;
        let idx = populated_series(vec![
            series_entry("Berserk", &[]),
            series_entry("Vinland Saga", &[]),
            one_punch,
            series_entry("Naruto", &[]),
        ]);

        let hits = idx.search_series("on ch", 10, None);
        assert!(!hits.is_empty(), "expected at least one hit for 'on ch'");
        let ids: Vec<Uuid> = hits.iter().map(|h| h.0).collect();
        assert!(
            ids.contains(&target_id),
            "expected One-Punch Man in results for 'on ch', got {:?}",
            ids
        );
    }

    #[test]
    fn matches_across_punctuation() {
        // "one punch" should match "One-Punch Man" despite the hyphen.
        let one_punch = series_entry("One-Punch Man", &[]);
        let target_id = one_punch.id;
        let idx = populated_series(vec![series_entry("Berserk", &[]), one_punch]);

        let hits = idx.search_series("one punch", 10, None);
        assert!(
            hits.first().map(|h| h.0) == Some(target_id),
            "expected One-Punch Man at rank 1 for 'one punch', got {:?}",
            hits
        );
    }

    #[test]
    fn matches_are_case_insensitive() {
        let target = series_entry("Berserk", &[]);
        let target_id = target.id;
        let idx = populated_series(vec![target]);
        let hits = idx.search_series("BERSERK", 10, None);
        assert_eq!(hits.first().map(|h| h.0), Some(target_id));
    }

    #[test]
    fn matches_are_accent_insensitive() {
        let target = series_entry("MÄR", &[]);
        let target_id = target.id;
        let idx = populated_series(vec![target]);
        let hits = idx.search_series("mar", 10, None);
        assert_eq!(hits.first().map(|h| h.0), Some(target_id));
    }

    #[test]
    fn empty_query_returns_nothing() {
        let idx = populated_series(vec![series_entry("Berserk", &[])]);
        assert!(idx.search_series("", 10, None).is_empty());
        assert!(idx.search_series("   ", 10, None).is_empty());
    }

    #[test]
    fn no_match_returns_empty() {
        let idx = populated_series(vec![series_entry("Berserk", &[])]);
        let hits = idx.search_series("xyzabc123", 10, None);
        assert!(hits.is_empty(), "expected no hits, got {:?}", hits);
    }

    #[test]
    fn matches_alt_title() {
        // Japanese alt title should match a romaji query (and vice versa) once
        // both strings are in the haystack.
        let target = series_entry("Shingeki no Kyojin", &["進撃の巨人"]);
        let target_id = target.id;
        let idx = populated_series(vec![target]);

        let romaji = idx.search_series("kyojin", 10, None);
        assert_eq!(romaji.first().map(|h| h.0), Some(target_id));

        let japanese = idx.search_series("進撃", 10, None);
        assert_eq!(japanese.first().map(|h| h.0), Some(target_id));
    }

    #[test]
    fn library_filter_narrows_results() {
        let lib_a = Uuid::new_v4();
        let lib_b = Uuid::new_v4();
        let mut entry_a = series_entry("One-Punch Man", &[]);
        entry_a.library_id = lib_a;
        let target_a = entry_a.id;
        let mut entry_b = series_entry("One-Punch Man", &[]);
        entry_b.library_id = lib_b;
        let target_b = entry_b.id;

        let idx = populated_series(vec![entry_a, entry_b]);

        let hits = idx.search_series("one punch", 10, Some(lib_a));
        let ids: Vec<Uuid> = hits.iter().map(|h| h.0).collect();
        assert_eq!(ids, vec![target_a]);

        let hits_b = idx.search_series("one punch", 10, Some(lib_b));
        let ids_b: Vec<Uuid> = hits_b.iter().map(|h| h.0).collect();
        assert_eq!(ids_b, vec![target_b]);
    }

    #[test]
    fn limit_truncates_results() {
        let entries: Vec<_> = (0..5)
            .map(|i| series_entry(&format!("Berserk {i}"), &[]))
            .collect();
        let idx = populated_series(entries);
        let hits = idx.search_series("berserk", 2, None);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn upsert_series_appends_when_new_and_replaces_when_existing() {
        let idx = FuzzyIndex::empty();
        let initial = series_entry("Berserk", &[]);
        let initial_id = initial.id;
        assert!(!idx.upsert_series(initial));
        assert_eq!(idx.series_count(), 1);

        // Re-upserting the same id with a new title should replace, not append.
        let replacement = SeriesEntry::new(
            initial_id,
            Uuid::nil(),
            SeriesSources {
                title: "Berserk Deluxe".to_string(),
                title_sort: None,
                name: "Berserk Deluxe".to_string(),
                alt_titles: Vec::new(),
                authors: Vec::new(),
            },
        );
        assert!(idx.upsert_series(replacement));
        assert_eq!(idx.series_count(), 1);

        let hits = idx.search_series("deluxe", 10, None);
        assert_eq!(hits.first().map(|h| h.0), Some(initial_id));
    }

    #[test]
    fn remove_series_cascades_to_books() {
        let series = series_entry("Berserk", &[]);
        let series_id = series.id;
        let idx = populated_series(vec![series]);
        let book = book_entry(Some("Vol 1"), "berserk-01.cbz", series_id);
        idx.upsert_book(book);
        assert_eq!(idx.book_count(), 1);

        assert!(idx.remove_series(series_id));
        assert_eq!(idx.series_count(), 0);
        assert_eq!(idx.book_count(), 0, "books should cascade with parent");

        // Removing a missing id is a no-op.
        assert!(!idx.remove_series(Uuid::new_v4()));
    }

    #[test]
    fn upsert_book_appends_or_replaces() {
        let series_id = Uuid::new_v4();
        let book = book_entry(Some("Old Title"), "old.cbz", series_id);
        let book_id = book.id;
        let idx = FuzzyIndex::empty();
        assert!(!idx.upsert_book(book));

        let replacement = BookEntry::new(
            book_id,
            series_id,
            Uuid::nil(),
            BookSources {
                title: Some("New Title".to_string()),
                file_name: "new.cbz".to_string(),
            },
        );
        assert!(idx.upsert_book(replacement));
        assert_eq!(idx.book_count(), 1);

        let hits = idx.search_books("new title", 10, None);
        assert_eq!(hits.first().map(|h| h.0), Some(book_id));
    }

    #[test]
    fn remove_book_is_idempotent() {
        let series_id = Uuid::new_v4();
        let book = book_entry(Some("Vol 1"), "vol01.cbz", series_id);
        let book_id = book.id;
        let idx = populated_books(vec![book]);
        assert!(idx.remove_book(book_id));
        assert_eq!(idx.book_count(), 0);
        assert!(!idx.remove_book(book_id));
    }

    #[test]
    fn remove_books_for_series_only_touches_matching_parent() {
        let s1 = Uuid::new_v4();
        let s2 = Uuid::new_v4();
        let idx = populated_books(vec![
            book_entry(None, "a.cbz", s1),
            book_entry(None, "b.cbz", s1),
            book_entry(None, "c.cbz", s2),
        ]);
        let removed = idx.remove_books_for_series(s1);
        assert_eq!(removed, 2);
        assert_eq!(idx.book_count(), 1);
        // Bulk-purging for a series with no books still works.
        assert_eq!(idx.remove_books_for_series(Uuid::new_v4()), 0);
    }

    #[test]
    fn search_books_uses_title_and_filename() {
        let series_id = Uuid::new_v4();
        let by_title = book_entry(Some("Volume One"), "vol01.cbz", series_id);
        let title_id = by_title.id;
        let by_filename = book_entry(None, "berserk-chapter-12.cbz", series_id);
        let filename_id = by_filename.id;
        let idx = populated_books(vec![by_title, by_filename]);

        let title_hits = idx.search_books("Volume One", 10, None);
        assert_eq!(title_hits.first().map(|h| h.0), Some(title_id));

        let filename_hits = idx.search_books("berserk chapter", 10, None);
        assert_eq!(filename_hits.first().map(|h| h.0), Some(filename_id));
        // The series_id is preserved alongside the book id.
        assert_eq!(filename_hits.first().map(|h| h.1), Some(series_id));
    }
}
