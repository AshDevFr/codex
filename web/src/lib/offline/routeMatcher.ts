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

/**
 * Backend paths that must bypass the SPA navigation fallback so the active
 * service worker lets them hit the network instead of serving the cached
 * index.html shell.
 *
 * `/docs` is the Scalar API reference: a *leaf* route with no sub-path, so its
 * pattern matches the bare `/docs` as well as `/docs/...`. The other prefixes
 * always carry a sub-path, so a trailing-slash anchor is sufficient. Getting
 * `/docs` wrong makes the SW serve the SPA for the bare path, and React Router
 * (which has no `/docs` route) renders a 404.
 */
export const NAVIGATION_DENYLIST: RegExp[] = [
  /^\/api\//,
  /^\/opds\//,
  /^\/komga\//,
  /^\/docs(\/|$)/,
  /^\/health$/,
];

/**
 * True when a navigation to `pathname` should be served by the SPA shell
 * (client-side routing). Backend paths in {@link NAVIGATION_DENYLIST} return
 * false so they reach the network.
 */
export function shouldServeSpaShell(pathname: string): boolean {
  return !NAVIGATION_DENYLIST.some((pattern) => pattern.test(pathname));
}
