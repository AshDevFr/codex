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
  detail: (bookId: string | undefined) => ["books", bookId, "detail"] as const,
  adjacent: (bookId: string | undefined) =>
    ["books", bookId, "adjacent"] as const,
  externalIds: (bookId: string | undefined) =>
    ["books", bookId, "external-ids"] as const,
  externalLinks: (bookId: string | undefined) =>
    ["books", bookId, "external-links"] as const,
};
