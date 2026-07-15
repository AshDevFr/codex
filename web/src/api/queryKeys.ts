/**
 * Shared TanStack Query keys for book-scoped queries.
 *
 * Every query that renders a single book's data must nest under the
 * `["books", id]` prefix so that prefix-based invalidation (e.g. the
 * want-to-read toggle, metadata updates) reaches it. Never introduce a
 * sibling root like `"book-detail"`: queries keyed outside the prefix
 * silently escape those invalidations and render stale flags.
 */
export const bookKeys = {
  /** Prefix matching every query for this book. Use for broad invalidation. */
  all: (bookId: string | undefined) => ["books", bookId] as const,
  /** Basic detail shape: `{ book, metadata }` from `getDetail(id)`. */
  detail: (bookId: string | undefined) => ["books", bookId, "detail"] as const,
  /**
   * Full flat detail shape from `getDetail(id, { full: true })`. Kept as a
   * distinct key: the two shapes must never share a cache entry, or whichever
   * query runs first poisons the other's reads.
   */
  detailFull: (bookId: string | undefined) =>
    ["books", bookId, "detail", "full"] as const,
  adjacent: (bookId: string | undefined) =>
    ["books", bookId, "adjacent"] as const,
  externalIds: (bookId: string | undefined) =>
    ["books", bookId, "external-ids"] as const,
  externalLinks: (bookId: string | undefined) =>
    ["books", bookId, "external-links"] as const,
};
