/**
 * Page-side download manager for the offline-reading feature (Phase 12, T3).
 *
 * `downloadSingleFileBook` handles EPUB and PDF, which the backend serves as
 * a single response from `/api/v1/books/{id}/file`. The body is streamed via
 * `ReadableStream.getReader()` so progress can be reported against
 * `Content-Length`; the assembled blob is stashed in a per-book Cache Storage
 * entry that the service worker's downloaded-book route then serves
 * CacheFirst, transparent to the reader.
 *
 * The IDB metadata row is set to `downloading` immediately, then flipped to
 * `complete` (with the final byte count and timestamp) once the cache write
 * lands. On abort, both the IDB row and the per-book cache are cleaned up so
 * the user sees a fresh slate on retry. On other failures (network, non-2xx
 * response), the row is set to `error` with the message so the Downloads
 * page (T7) can surface what went wrong.
 *
 * Comic per-page downloads (T4) and series batch (T5) build on this manager
 * but require their own entry points; they are not part of this slice.
 */

import {
  broadcastDownloadsChange,
  type DownloadRecord,
  deleteDownload,
  putDownload,
} from "./db";
import { cacheNameForBook } from "./routeMatcher";

export type SingleFileFormat = "epub" | "pdf";

export interface ProgressUpdate {
  /** Bytes received so far. */
  loaded: number;
  /** Total bytes from `Content-Length`; null when the header is missing. */
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

export interface SingleFileDownloadResult {
  bookId: string;
  bytes: number;
}

function bookFileUrl(bookId: string): string {
  return `/api/v1/books/${bookId}/file`;
}

export async function downloadSingleFileBook(
  options: SingleFileDownloadOptions,
): Promise<SingleFileDownloadResult> {
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

  return { bookId, bytes: loaded };
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
