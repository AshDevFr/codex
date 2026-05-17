/**
 * Page-side download manager for the offline-reading feature (Phase 12).
 *
 * Two entry points cover every book format Codex supports:
 *
 * - `downloadSingleFileBook` (T3) for EPUB and PDF, which the backend serves
 *   as one response from `/api/v1/books/{id}/file`. The body is streamed via
 *   `ReadableStream.getReader()` so progress reports against `Content-Length`
 *   when present; the assembled response is stored in a per-book Cache.
 *
 * - `downloadComicBook` (T4) for CBZ/CBR, which the backend serves one page
 *   at a time from `/api/v1/books/{id}/pages/{n}`. Pages are fetched with
 *   bounded concurrency; progress is reported as pages-done/pages-total.
 *
 * Both flows write the IDB row as `downloading` immediately, then flip it to
 * `complete` once everything lands. Abort cleans up the IDB row and the
 * per-book cache so a retry starts from a clean slate. Any other failure
 * (network throw, non-2xx response, mid-stream error, per-page 404) sets
 * the IDB row to `error` with the message preserved for the T7 Downloads
 * page to surface, and removes the partial cache so the reader never sees
 * a half-downloaded book.
 *
 * Series batch (T5) is a queue around these functions; it is not in this
 * module.
 */

import {
  broadcastDownloadsChange,
  type DownloadRecord,
  deleteDownload,
  putDownload,
} from "./db";
import { cacheNameForBook } from "./routeMatcher";

export type SingleFileFormat = "epub" | "pdf";
export type ComicFormat = "cbz" | "cbr";
export type DownloadableFormat = SingleFileFormat | ComicFormat;

export interface ProgressUpdate {
  /**
   * Units depend on the download flow:
   * - Single-file: bytes received so far.
   * - Comic: pages fetched so far.
   */
  loaded: number;
  /**
   * Single-file: total bytes from `Content-Length`; `null` if the header
   * is missing. Comic: total page count.
   */
  total: number | null;
}

export interface SingleFileDownloadOptions {
  bookId: string;
  format: SingleFileFormat;
  /** Cancels the download. Cleans up IDB and cache on cancellation. */
  signal?: AbortSignal;
  /** Invoked after every chunk arrives. */
  onProgress?: (progress: ProgressUpdate) => void;
  /** Injection points for testing. Default to global `fetch` / `caches`. */
  fetch?: typeof globalThis.fetch;
  caches?: CacheStorage;
}

export interface DownloadResult {
  bookId: string;
  bytes: number;
}

/** @deprecated Use `DownloadResult`. Kept for back-compat. */
export type SingleFileDownloadResult = DownloadResult;

function bookFileUrl(bookId: string): string {
  return `/api/v1/books/${bookId}/file`;
}

function bookPageUrl(bookId: string, pageNumber: number): string {
  return `/api/v1/books/${bookId}/pages/${pageNumber}`;
}

// -- Storage persistence (T9) --------------------------------------------

/**
 * Result of `navigator.storage.persist()` for the current session.
 *
 * - `null`: not yet attempted (no successful download in this session, or
 *   the StorageManager API is not available).
 * - `true`: persist was granted; the browser will not evict our data under
 *   ordinary storage pressure.
 * - `false`: persist was denied (typical for non-installed PWAs on Safari
 *   and for tabs that have not built up enough engagement on Chromium).
 */
export type StoragePersistence = boolean | null;

let cachedPersistResult: StoragePersistence = null;
let persistInFlight: Promise<StoragePersistence> | null = null;

/**
 * Returns the cached `navigator.storage.persist()` result without making a
 * new request. Used by the Downloads page (T7) to render the durability
 * indicator without forcing a re-prompt.
 */
export function getStoragePersistence(): StoragePersistence {
  return cachedPersistResult;
}

/**
 * Requests persistent storage if it has not been requested this session.
 * Idempotent: subsequent calls return the cached result without re-asking
 * the browser. Falls through silently in environments without the
 * StorageManager API (older Safari, jsdom without injection).
 *
 * Exposed primarily so the Downloads page (T7) can opportunistically
 * trigger the prompt when a user lands there, even if they haven't
 * downloaded anything yet. The download flows below also call this after
 * each successful completion.
 */
export async function requestStoragePersistence(
  storage?: StorageManager,
): Promise<StoragePersistence> {
  if (cachedPersistResult !== null) return cachedPersistResult;
  if (persistInFlight) return persistInFlight;

  const storageManager = storage ?? globalThis.navigator?.storage;
  if (!storageManager || typeof storageManager.persist !== "function") {
    return null;
  }

  persistInFlight = (async () => {
    try {
      const granted = await storageManager.persist();
      cachedPersistResult = granted;
      return granted;
    } catch {
      // Some browsers reject persist() under restricted contexts; treat
      // that as "not granted" rather than letting it propagate.
      cachedPersistResult = false;
      return false;
    } finally {
      persistInFlight = null;
    }
  })();

  return persistInFlight;
}

/**
 * Reset the cached persist result. Test-only.
 */
export function _resetPersistenceForTests(): void {
  cachedPersistResult = null;
  persistInFlight = null;
}

export async function downloadSingleFileBook(
  options: SingleFileDownloadOptions,
): Promise<DownloadResult> {
  const { bookId, format, signal, onProgress } = options;
  const fetchImpl = options.fetch ?? globalThis.fetch.bind(globalThis);
  const cachesImpl = options.caches ?? globalThis.caches;
  if (!cachesImpl) {
    throw new Error("Cache Storage is not available in this environment");
  }

  const url = bookFileUrl(bookId);

  const startRecord: DownloadRecord = {
    id: bookId,
    format,
    status: "downloading",
    bytes: 0,
    pageCount: 1,
  };
  await putDownload(startRecord);
  broadcastDownloadsChange({ kind: "put", record: startRecord });

  let response: Response;
  try {
    response = await fetchImpl(url, { signal });
  } catch (err) {
    if (signal?.aborted) {
      await cleanupAfterAbort(bookId, cachesImpl);
      throw abortError(err);
    }
    await recordError(startRecord, err);
    throw normalizeError(err);
  }

  if (!response.ok) {
    const err = new Error(`HTTP ${response.status} fetching ${url}`);
    await recordError(startRecord, err);
    throw err;
  }

  const body = response.body;
  if (!body) {
    const err = new Error(`No response body for ${url}`);
    await recordError(startRecord, err);
    throw err;
  }

  const totalHeader = response.headers.get("content-length");
  const total = totalHeader ? Number(totalHeader) : null;

  const reader = body.getReader();
  const chunks: Uint8Array[] = [];
  let loaded = 0;
  try {
    while (true) {
      const { done, value } = await reader.read();
      if (done) break;
      if (value) {
        chunks.push(value);
        loaded += value.length;
        onProgress?.({ loaded, total: total ?? null });
      }
    }
  } catch (err) {
    if (signal?.aborted) {
      await cleanupAfterAbort(bookId, cachesImpl);
      throw abortError(err);
    }
    await recordError(startRecord, err);
    throw normalizeError(err);
  }

  // Build a fresh Response so the cache key is stable and the headers
  // (especially Content-Type) match what the server sent. Concatenate the
  // chunks into a single Uint8Array rather than wrapping them in a Blob:
  // jsdom's Response constructor stringifies Blob inputs ("[object Blob]")
  // whereas both jsdom and the browser handle Uint8Array bodies correctly.
  const merged = concatChunks(chunks, loaded);
  const cached = new Response(merged, {
    status: 200,
    statusText: response.statusText || "OK",
    headers: response.headers,
  });

  const cache = await cachesImpl.open(cacheNameForBook(bookId));
  await cache.put(url, cached);

  const completeRecord: DownloadRecord = {
    id: bookId,
    format,
    status: "complete",
    bytes: loaded,
    pageCount: 1,
    downloadedAt: Date.now(),
  };
  await putDownload(completeRecord);
  broadcastDownloadsChange({ kind: "put", record: completeRecord });
  // T9: request persistent storage once per session, opportunistically.
  void requestStoragePersistence();

  return { bookId, bytes: loaded };
}

export interface ComicDownloadOptions {
  bookId: string;
  format: ComicFormat;
  /** Total page count from book metadata; must be >= 1. */
  pageCount: number;
  signal?: AbortSignal;
  /**
   * Reports `{ loaded: pagesDone, total: pageCount }` after each page lands.
   */
  onProgress?: (progress: ProgressUpdate) => void;
  /** Max concurrent page fetches. Defaults to 5. */
  concurrency?: number;
  fetch?: typeof globalThis.fetch;
  caches?: CacheStorage;
}

/**
 * Default concurrency for per-page comic downloads. Tuned to balance
 * throughput against the backend's per-client connection budget and to
 * stay below most browsers' default 6-connection-per-origin limit.
 */
const DEFAULT_COMIC_CONCURRENCY = 5;

export async function downloadComicBook(
  options: ComicDownloadOptions,
): Promise<DownloadResult> {
  const {
    bookId,
    format,
    pageCount,
    signal,
    onProgress,
    concurrency = DEFAULT_COMIC_CONCURRENCY,
  } = options;
  const fetchImpl = options.fetch ?? globalThis.fetch.bind(globalThis);
  const cachesImpl = options.caches ?? globalThis.caches;
  if (!cachesImpl) {
    throw new Error("Cache Storage is not available in this environment");
  }
  if (!Number.isInteger(pageCount) || pageCount < 1) {
    throw new Error(`Invalid pageCount: ${pageCount}`);
  }

  const startRecord: DownloadRecord = {
    id: bookId,
    format,
    status: "downloading",
    bytes: 0,
    pageCount,
  };
  await putDownload(startRecord);
  broadcastDownloadsChange({ kind: "put", record: startRecord });

  // Compose external + internal abort signals so a per-page failure can
  // cancel the in-flight siblings without affecting the caller's signal.
  const internalController = new AbortController();
  const externalAbortHandler = () => internalController.abort();
  if (signal) {
    if (signal.aborted) internalController.abort();
    else signal.addEventListener("abort", externalAbortHandler);
  }

  const cache = await cachesImpl.open(cacheNameForBook(bookId));

  let totalBytes = 0;
  let pagesDone = 0;
  let firstFailure: Error | null = null;
  let nextIndex = 0;

  async function worker() {
    while (true) {
      const i = nextIndex++;
      if (i >= pageCount) return;
      if (internalController.signal.aborted) return;
      const pageNumber = i + 1;
      const url = bookPageUrl(bookId, pageNumber);
      try {
        const response = await fetchImpl(url, {
          signal: internalController.signal,
        });
        if (!response.ok) {
          throw new Error(
            `HTTP ${response.status} fetching page ${pageNumber} of book ${bookId}`,
          );
        }
        const buffer = await response.arrayBuffer();
        const body = new Uint8Array(buffer);
        const headers = new Headers(response.headers);
        const cached = new Response(body, {
          status: 200,
          statusText: response.statusText || "OK",
          headers,
        });
        await cache.put(url, cached);
        totalBytes += body.byteLength;
        pagesDone += 1;
        onProgress?.({ loaded: pagesDone, total: pageCount });
      } catch (err) {
        if (firstFailure === null && !signal?.aborted) {
          firstFailure = err instanceof Error ? err : new Error(String(err));
        }
        internalController.abort();
        return;
      }
    }
  }

  try {
    const workerCount = Math.min(Math.max(1, concurrency), pageCount);
    await Promise.all(Array.from({ length: workerCount }, () => worker()));
  } finally {
    if (signal) signal.removeEventListener("abort", externalAbortHandler);
  }

  if (signal?.aborted) {
    await cleanupAfterAbort(bookId, cachesImpl);
    throw abortError(undefined);
  }
  if (firstFailure) {
    await recordError(startRecord, firstFailure);
    // Partial caches are useless for reading (the reader needs every page),
    // so evict the whole per-book cache. The IDB row stays at status=error
    // so the Downloads page (T7) can show what went wrong.
    await cachesImpl.delete(cacheNameForBook(bookId));
    throw firstFailure;
  }

  const completeRecord: DownloadRecord = {
    id: bookId,
    format,
    status: "complete",
    bytes: totalBytes,
    pageCount,
    downloadedAt: Date.now(),
  };
  await putDownload(completeRecord);
  broadcastDownloadsChange({ kind: "put", record: completeRecord });
  // T9: request persistent storage once per session, opportunistically.
  void requestStoragePersistence();

  return { bookId, bytes: totalBytes };
}

async function recordError(base: DownloadRecord, err: unknown): Promise<void> {
  const errorRecord: DownloadRecord = {
    ...base,
    status: "error",
    error: err instanceof Error ? err.message : String(err),
  };
  await putDownload(errorRecord);
  broadcastDownloadsChange({ kind: "put", record: errorRecord });
}

async function cleanupAfterAbort(
  bookId: string,
  cachesImpl: CacheStorage,
): Promise<void> {
  await deleteDownload(bookId);
  broadcastDownloadsChange({ kind: "delete", id: bookId });
  await cachesImpl.delete(cacheNameForBook(bookId));
}

function abortError(original: unknown): DOMException {
  if (original instanceof DOMException && original.name === "AbortError") {
    return original;
  }
  return new DOMException("Download aborted", "AbortError");
}

function normalizeError(err: unknown): Error {
  return err instanceof Error ? err : new Error(String(err));
}

function concatChunks(
  chunks: Uint8Array[],
  total: number,
): Uint8Array<ArrayBuffer> {
  // Force `Uint8Array<ArrayBuffer>` (not `Uint8Array<ArrayBufferLike>`) so
  // the value satisfies `BodyInit`'s `BufferSource` constraint in TS 5.7+.
  const out = new Uint8Array(new ArrayBuffer(total));
  let offset = 0;
  for (const chunk of chunks) {
    out.set(chunk, offset);
    offset += chunk.length;
  }
  return out;
}
