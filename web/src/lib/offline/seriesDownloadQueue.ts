/**
 * Series batch download queue.
 *
 * Wraps the per-book download functions from `./downloadManager` in a small
 * in-process queue so a "Download series" action can fan out across every
 * book in a series without blowing past quota or kicking off N concurrent
 * fetches. The queue is intentionally minimal: it lives only as long as the
 * caller holds the returned controller, runs sequentially by default (the
 * plan calls for 1-2 books in flight; 1 is the right phone default), and
 * does not survive a tab close. The persisted state of partial completes
 * lives in the per-book IDB rows and per-book cache that the individual
 * download functions already maintain.
 *
 * Pre-flight quota check: sum the estimated bytes for every queued book,
 * compare against `navigator.storage.estimate()`, and refuse with a typed
 * `QuotaExceededError` if the queue would push usage past 90% of quota.
 * Browsers that do not implement `navigator.storage.estimate()` (Safari
 * historically) report `null` quota; treat as "unknown" and let the queue
 * proceed rather than blocking on missing data.
 *
 * Per-book cancel composes the same way the comic per-page worker does in
 * `downloadManager.ts`: one AbortController per book, plus a queue-level
 * controller for "cancel everything." Cancelling one book leaves the rest
 * intact.
 */

import {
  type ComicFormat,
  downloadComicBook,
  downloadSingleFileBook,
  type ProgressUpdate,
  type SingleFileFormat,
} from "./downloadManager";

/**
 * Per-book input to the queue. Pulled from the series' book list at the
 * call site so the queue does not need to know about the series API.
 */
export interface SeriesBookSummary {
  id: string;
  /** Lowercase format from the API (`"cbz" | "cbr" | "epub" | "pdf"`). */
  fileFormat: string;
  /** Page count from book metadata. Required for comics. */
  pageCount: number;
  /** Single-file size in bytes (EPUB/PDF). Optional for comics. */
  fileSize?: number | null;
}

export type BookQueueStatus =
  | "queued"
  | "downloading"
  | "complete"
  | "error"
  | "cancelled"
  | "skipped";

export interface BookQueueState {
  bookId: string;
  status: BookQueueStatus;
  /** Pages for comics, bytes for single-file. Matches `ProgressUpdate`. */
  loaded: number;
  /** Pages or bytes; `null` when unknown (single-file w/o Content-Length). */
  total: number | null;
  error?: string;
}

export interface SeriesQueueState {
  seriesId: string;
  total: number;
  completed: number;
  failed: number;
  cancelled: number;
  /** Per-book state keyed by book id. Insertion order preserved. */
  perBook: Map<string, BookQueueState>;
}

export interface SeriesDownloadResult {
  completed: string[];
  failed: { bookId: string; error: string }[];
  cancelled: string[];
}

export interface SeriesDownloadController {
  /**
   * Cancel a single book. If it has not started yet, it is marked
   * `cancelled` and skipped; if in flight, the per-book controller is
   * aborted (`downloadManager` cleans up its IDB row + cache).
   */
  cancelBook: (bookId: string) => void;
  /** Cancel every book that has not yet completed. */
  cancelAll: () => void;
  /** Subscribe to state-change notifications; returns an unsubscribe fn. */
  subscribe: (listener: (state: SeriesQueueState) => void) => () => void;
  /** Snapshot of the current state. */
  getState: () => SeriesQueueState;
  /** Resolves when every book has reached a terminal state. */
  done: Promise<SeriesDownloadResult>;
}

export interface SeriesDownloadOptions {
  seriesId: string;
  books: SeriesBookSummary[];
  /**
   * Max books processed in parallel. Defaults to 1 (sequential), which is
   * the right default for phones and avoids fanning out 4 books * 5 pages
   * = 20 concurrent fetches on comics.
   */
  concurrency?: number;
  /**
   * Rough bytes-per-page used to estimate a comic's total size for the
   * pre-flight quota check when `fileSize` is unknown. Comics rendered
   * server-side at ~1080px land around 300 KB / page in practice; we
   * default a little high (400 KB) to bias towards refusing borderline
   * queues rather than failing mid-download.
   */
  avgPageBytes?: number;
  /**
   * Refuse the queue when `usage + estimatedBytes > quota * quotaThreshold`.
   * Defaults to 0.9 per the plan; lowered in tests so the assertion does
   * not have to fabricate huge byte counts.
   */
  quotaThreshold?: number;
  /** Injection points for tests. Default to globals. */
  fetch?: typeof globalThis.fetch;
  caches?: CacheStorage;
  storage?: StorageManager;
}

/**
 * Thrown by `downloadSeriesBatch` from the pre-flight quota check. Carries
 * the numbers needed for a clear user-facing message.
 */
export class QuotaExceededError extends Error {
  readonly estimatedBytes: number;
  readonly usage: number;
  readonly quota: number;
  readonly threshold: number;
  constructor(args: {
    estimatedBytes: number;
    usage: number;
    quota: number;
    threshold: number;
  }) {
    super(
      `Series download would exceed storage quota (${args.usage + args.estimatedBytes} of ${args.quota} bytes would be used; threshold ${Math.round(args.threshold * 100)}%).`,
    );
    this.name = "QuotaExceededError";
    this.estimatedBytes = args.estimatedBytes;
    this.usage = args.usage;
    this.quota = args.quota;
    this.threshold = args.threshold;
  }
}

const DEFAULT_AVG_PAGE_BYTES = 400 * 1024;
const DEFAULT_QUOTA_THRESHOLD = 0.9;

function isSingleFileFormat(format: string): format is SingleFileFormat {
  return format === "epub" || format === "pdf";
}

function isComicFormat(format: string): format is ComicFormat {
  return format === "cbz" || format === "cbr";
}

/**
 * Estimate the bytes a book will consume in the per-book cache. EPUB/PDF
 * uses `fileSize` if known, falling back to a single average-page guess.
 * Comics use `pageCount * avgPageBytes` (the backend renders one image per
 * page; total size scales linearly).
 */
export function estimateBookBytes(
  book: SeriesBookSummary,
  avgPageBytes: number = DEFAULT_AVG_PAGE_BYTES,
): number {
  if (isSingleFileFormat(book.fileFormat)) {
    if (typeof book.fileSize === "number" && book.fileSize > 0) {
      return book.fileSize;
    }
    return avgPageBytes;
  }
  if (isComicFormat(book.fileFormat)) {
    return Math.max(1, book.pageCount) * avgPageBytes;
  }
  return 0;
}

async function readQuotaEstimate(
  storage: StorageManager | undefined,
): Promise<{ usage: number; quota: number } | null> {
  if (!storage || typeof storage.estimate !== "function") return null;
  try {
    const est = await storage.estimate();
    const quota = typeof est.quota === "number" ? est.quota : 0;
    const usage = typeof est.usage === "number" ? est.usage : 0;
    if (quota <= 0) return null;
    return { usage, quota };
  } catch {
    return null;
  }
}

/**
 * Run a pre-flight quota check. Returns nothing on success; throws
 * `QuotaExceededError` if the queue would push past `quotaThreshold`.
 * Treats an unavailable estimate as "unknown" and lets the caller proceed.
 */
export async function preflightQuota(
  books: SeriesBookSummary[],
  options: {
    avgPageBytes?: number;
    quotaThreshold?: number;
    storage?: StorageManager;
  } = {},
): Promise<void> {
  const avg = options.avgPageBytes ?? DEFAULT_AVG_PAGE_BYTES;
  const threshold = options.quotaThreshold ?? DEFAULT_QUOTA_THRESHOLD;
  const storage = options.storage ?? globalThis.navigator?.storage;
  const estimated = books.reduce(
    (acc, b) => acc + estimateBookBytes(b, avg),
    0,
  );
  if (estimated === 0) return;
  const est = await readQuotaEstimate(storage);
  if (!est) return;
  if (est.usage + estimated > est.quota * threshold) {
    throw new QuotaExceededError({
      estimatedBytes: estimated,
      usage: est.usage,
      quota: est.quota,
      threshold,
    });
  }
}

/**
 * Kick off a series batch download. Returns synchronously with a
 * controller; `controller.done` resolves when every book reaches a
 * terminal state. Throws `QuotaExceededError` from the pre-flight check
 * without writing any IDB rows.
 */
export async function downloadSeriesBatch(
  options: SeriesDownloadOptions,
): Promise<SeriesDownloadController> {
  const {
    seriesId,
    books,
    concurrency = 1,
    avgPageBytes = DEFAULT_AVG_PAGE_BYTES,
    quotaThreshold = DEFAULT_QUOTA_THRESHOLD,
    fetch: fetchImpl,
    caches: cachesImpl,
    storage,
  } = options;

  await preflightQuota(books, { avgPageBytes, quotaThreshold, storage });

  // Build initial state. Books with unsupported formats are skipped up front
  // so the UI can show "1 of N skipped" without each one logging through the
  // queue lifecycle.
  const perBook = new Map<string, BookQueueState>();
  const supportedBooks: SeriesBookSummary[] = [];
  for (const b of books) {
    if (isSingleFileFormat(b.fileFormat) || isComicFormat(b.fileFormat)) {
      perBook.set(b.id, {
        bookId: b.id,
        status: "queued",
        loaded: 0,
        total: isComicFormat(b.fileFormat) ? b.pageCount : null,
      });
      supportedBooks.push(b);
    } else {
      perBook.set(b.id, {
        bookId: b.id,
        status: "skipped",
        loaded: 0,
        total: null,
      });
    }
  }

  const state: SeriesQueueState = {
    seriesId,
    total: books.length,
    completed: 0,
    failed: 0,
    cancelled: 0,
    perBook,
  };

  const listeners = new Set<(s: SeriesQueueState) => void>();
  const notify = () => {
    // Hand listeners a stable snapshot — perBook is a Map, so React
    // consumers should re-render via the listener call rather than by
    // identity-checking the object.
    for (const l of Array.from(listeners)) {
      try {
        l(state);
      } catch {
        // Listener errors should never break the queue.
      }
    }
  };

  // Per-book controllers so `cancelBook(id)` aborts exactly one fetch.
  const controllers = new Map<string, AbortController>();
  // Queue-level "cancel everything" flag.
  let everythingCancelled = false;
  // Books that have not yet started; cancelling these flips the state
  // without ever asking the manager to do anything.
  const queuedIds = new Set<string>(supportedBooks.map((b) => b.id));

  function setBookState(
    bookId: string,
    next: Partial<BookQueueState> & { status?: BookQueueStatus },
  ): void {
    const prev = perBook.get(bookId);
    if (!prev) return;
    const merged: BookQueueState = { ...prev, ...next };
    perBook.set(bookId, merged);
    notify();
  }

  function bumpTerminal(prev: BookQueueStatus, next: BookQueueStatus): void {
    if (prev === next) return;
    if (next === "complete") state.completed += 1;
    else if (next === "error") state.failed += 1;
    else if (next === "cancelled") state.cancelled += 1;
  }

  async function runOne(book: SeriesBookSummary): Promise<void> {
    const prev = perBook.get(book.id);
    if (!prev) return;
    if (prev.status === "cancelled" || prev.status === "skipped") return;
    if (everythingCancelled) {
      bumpTerminal(prev.status, "cancelled");
      setBookState(book.id, { status: "cancelled" });
      return;
    }

    queuedIds.delete(book.id);

    const controller = new AbortController();
    controllers.set(book.id, controller);
    setBookState(book.id, { status: "downloading", loaded: 0 });

    const onProgress = (p: ProgressUpdate) => {
      const cur = perBook.get(book.id);
      if (!cur || cur.status !== "downloading") return;
      setBookState(book.id, { loaded: p.loaded, total: p.total });
    };

    try {
      if (isSingleFileFormat(book.fileFormat)) {
        await downloadSingleFileBook({
          bookId: book.id,
          format: book.fileFormat,
          signal: controller.signal,
          onProgress,
          fetch: fetchImpl,
          caches: cachesImpl,
        });
      } else if (isComicFormat(book.fileFormat)) {
        await downloadComicBook({
          bookId: book.id,
          format: book.fileFormat,
          pageCount: book.pageCount,
          signal: controller.signal,
          onProgress,
          fetch: fetchImpl,
          caches: cachesImpl,
        });
      } else {
        // Should not happen — unsupported formats are filtered above — but
        // keep the branch so future formats fail loudly.
        throw new Error(
          `Unsupported format for offline download: ${book.fileFormat}`,
        );
      }
      bumpTerminal(perBook.get(book.id)?.status ?? "downloading", "complete");
      setBookState(book.id, { status: "complete" });
    } catch (err) {
      if (err instanceof DOMException && err.name === "AbortError") {
        bumpTerminal(
          perBook.get(book.id)?.status ?? "downloading",
          "cancelled",
        );
        setBookState(book.id, { status: "cancelled" });
      } else {
        const message = err instanceof Error ? err.message : String(err);
        bumpTerminal(perBook.get(book.id)?.status ?? "downloading", "error");
        setBookState(book.id, { status: "error", error: message });
      }
    } finally {
      controllers.delete(book.id);
    }
  }

  // Worker pool driven off a shared index so concurrency is honoured even
  // when individual downloads finish out of order.
  let nextIndex = 0;
  async function worker() {
    while (true) {
      const i = nextIndex++;
      if (i >= supportedBooks.length) return;
      const book = supportedBooks[i];
      if (!book) return;
      await runOne(book);
    }
  }

  const workerCount = Math.max(
    1,
    Math.min(concurrency, supportedBooks.length || 1),
  );

  const done: Promise<SeriesDownloadResult> = (async () => {
    if (supportedBooks.length === 0) {
      // Nothing to do — every book was unsupported. Resolve immediately
      // with the skipped list so the UI can render a "0 of N supported"
      // message instead of spinning forever.
      return summarise(state);
    }
    await Promise.all(Array.from({ length: workerCount }, () => worker()));
    return summarise(state);
  })();

  return {
    cancelBook(bookId: string) {
      const cur = perBook.get(bookId);
      if (!cur) return;
      if (
        cur.status === "complete" ||
        cur.status === "cancelled" ||
        cur.status === "error" ||
        cur.status === "skipped"
      ) {
        return;
      }
      const controller = controllers.get(bookId);
      if (controller) {
        controller.abort();
        return; // The catch arm in runOne flips status to "cancelled".
      }
      // Not started yet — flip directly so the worker skips it when it
      // pops the index.
      bumpTerminal(cur.status, "cancelled");
      setBookState(bookId, { status: "cancelled" });
    },
    cancelAll() {
      everythingCancelled = true;
      for (const [id, controller] of controllers.entries()) {
        controller.abort();
        // Defensive: if the abort never propagates (synchronous resolve in
        // a test), mark it now so the snapshot is consistent.
        const cur = perBook.get(id);
        if (cur && cur.status === "downloading") {
          bumpTerminal(cur.status, "cancelled");
          setBookState(id, { status: "cancelled" });
        }
      }
      for (const id of Array.from(queuedIds)) {
        const cur = perBook.get(id);
        if (cur && cur.status === "queued") {
          bumpTerminal(cur.status, "cancelled");
          setBookState(id, { status: "cancelled" });
        }
        queuedIds.delete(id);
      }
    },
    subscribe(listener) {
      listeners.add(listener);
      // Push the current snapshot synchronously so subscribers do not have
      // to render once with an empty state then wait for the next change.
      try {
        listener(state);
      } catch {
        /* ignore */
      }
      return () => {
        listeners.delete(listener);
      };
    },
    getState() {
      return state;
    },
    done,
  };
}

function summarise(state: SeriesQueueState): SeriesDownloadResult {
  const completed: string[] = [];
  const failed: { bookId: string; error: string }[] = [];
  const cancelled: string[] = [];
  for (const book of state.perBook.values()) {
    if (book.status === "complete") completed.push(book.bookId);
    else if (book.status === "error")
      failed.push({
        bookId: book.bookId,
        error: book.error ?? "unknown error",
      });
    else if (book.status === "cancelled") cancelled.push(book.bookId);
  }
  return { completed, failed, cancelled };
}
