/**
 * Second key segment of every LIST/grid/home-section query for series and
 * books. Detail queries are keyed `["series", <id>, ...]` / `["books", <id>,
 * ...]` instead, so invalidating these section prefixes refreshes the lists
 * (counts, membership, `wantToRead` flags on cards) without touching open
 * detail tabs.
 *
 * React Query matches by prefix, and a list key's slot 2 is a section string
 * (`"search"`, `"recently-added"`, …) — never an entity id — so a bare
 * `["series", <id>]` invalidation does NOT reach these lists. Anything that
 * mutates a flag rendered on a card (e.g. want-to-read membership) must
 * invalidate the section prefixes here in addition to the entity's detail key.
 *
 * Keep in sync with the query keys in the components that own these lists
 * (SeriesSection, RecommendedSection, ReadingFeedSection, the books/series
 * grids).
 */
export const SERIES_LIST_SECTIONS = [
  "search",
  "alphabetical-groups",
  "recently-added",
  "recently-updated",
] as const;

export const BOOKS_LIST_SECTIONS = [
  "search",
  "in-progress",
  "on-deck",
  "recently-added",
  "recently-read",
] as const;
