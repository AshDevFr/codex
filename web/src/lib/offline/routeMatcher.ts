/**
 * Pure URL matcher used by the service worker to decide whether a request
 * for a book resource should be served from the per-book offline cache.
 *
 * Extracted from sw.ts so it can be unit-tested without a SW environment.
 */

export interface PageResource {
  kind: "page";
  number: number;
}

export interface FileResource {
  kind: "file";
}

export type BookResource = PageResource | FileResource;

export interface DownloadedBookMatch {
  bookId: string;
  resource: BookResource;
}

// Matches /api/v1/books/{id}/pages/{n} OR /api/v1/books/{id}/file.
// Captures the id and (optionally) the page number.
const BOOK_RESOURCE_PATTERN =
  /^\/api\/v1\/books\/([^/]+)\/(?:pages\/(\d+)|file)$/;

/**
 * Return a match descriptor if `url` is a book-resource request for a book
 * that is currently downloaded; otherwise null.
 *
 * Only GET requests are matched. Other methods (PUT for progress, DELETE)
 * always go through to the network so server state stays canonical.
 */
export function matchDownloadedBookRequest(
  url: URL,
  method: string,
  downloadedIds: ReadonlySet<string>,
): DownloadedBookMatch | null {
  if (method !== "GET") return null;
  const match = BOOK_RESOURCE_PATTERN.exec(url.pathname);
  if (!match) return null;
  const bookId = match[1];
  const pageStr = match[2];
  if (!bookId || !downloadedIds.has(bookId)) return null;
  return {
    bookId,
    resource: pageStr
      ? { kind: "page", number: Number(pageStr) }
      : { kind: "file" },
  };
}

/**
 * Cache name for a single book's resources. Per-book naming makes eviction
 * a single `caches.delete()` call.
 */
export function cacheNameForBook(bookId: string): string {
  return `codex-book-${bookId}`;
}
