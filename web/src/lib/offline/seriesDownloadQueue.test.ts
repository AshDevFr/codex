import { IDBFactory } from "fake-indexeddb";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  _resetForTests,
  getAllDownloads,
  getDownload,
  setDbContext,
} from "./db";
import { _resetPersistenceForTests } from "./downloadManager";
import { cacheNameForBook } from "./routeMatcher";
import {
  type BookQueueState,
  downloadSeriesBatch,
  estimateBookBytes,
  preflightQuota,
  QuotaExceededError,
  type SeriesBookSummary,
  type SeriesQueueState,
} from "./seriesDownloadQueue";

beforeEach(() => {
  setDbContext({ indexedDB: new IDBFactory() });
});

afterEach(() => {
  setDbContext(null);
  _resetForTests();
  _resetPersistenceForTests();
});

// -- Fakes (mirror downloadManager.test.ts) ------------------------------

interface CacheEntry {
  body: Uint8Array;
  status: number;
  headers: Record<string, string>;
}

function makeFakeCaches() {
  const stores = new Map<string, Map<string, CacheEntry>>();
  const cachesImpl = {
    async open(name: string): Promise<Cache> {
      let store = stores.get(name);
      if (!store) {
        store = new Map<string, CacheEntry>();
        stores.set(name, store);
      }
      const cache: Partial<Cache> = {
        put: async (request, response) => {
          const url =
            typeof request === "string" ? request : (request as Request).url;
          const buffer = await response.arrayBuffer();
          const headerObj: Record<string, string> = {};
          response.headers.forEach((value, key) => {
            headerObj[key] = value;
          });
          store!.set(url, {
            body: new Uint8Array(buffer),
            status: response.status,
            headers: headerObj,
          });
        },
        match: async (request) => {
          const url =
            typeof request === "string" ? request : (request as Request).url;
          const entry = store!.get(url);
          if (!entry) return undefined;
          return new Response(entry.body, {
            status: entry.status,
            headers: entry.headers,
          });
        },
      };
      return cache as Cache;
    },
    async delete(name: string): Promise<boolean> {
      return stores.delete(name);
    },
  } as Partial<CacheStorage>;
  return {
    caches: cachesImpl as CacheStorage,
    getStore: (name: string) => stores.get(name),
  };
}

function makeStreamingResponse(
  chunks: Uint8Array[],
  init: { contentLength?: number; status?: number } = {},
): Response {
  const headers = new Headers();
  if (init.contentLength !== undefined) {
    headers.set("content-length", String(init.contentLength));
  }
  const stream = new ReadableStream<Uint8Array>({
    start(controller) {
      for (const chunk of chunks) {
        controller.enqueue(chunk);
      }
      controller.close();
    },
  });
  return new Response(stream, {
    status: init.status ?? 200,
    headers,
  });
}

/**
 * Build a fetch that resolves single-file downloads with a small payload,
 * and resolves comic page downloads with a one-byte body equal to the page
 * number. Tracks the per-book request count so we can assert one cache hit
 * per book.
 */
function makeFakeFetch() {
  const calls: string[] = [];
  const fakeFetch = vi.fn(
    async (input: RequestInfo | URL, init?: RequestInit) => {
      const url = typeof input === "string" ? input : (input as URL).toString();
      calls.push(url);
      if (init?.signal?.aborted) {
        throw new DOMException("Aborted", "AbortError");
      }
      if (/\/pages\/(\d+)$/.test(url)) {
        const n = Number(url.match(/\/pages\/(\d+)$/)![1]);
        return new Response(new Uint8Array([n]), {
          status: 200,
          headers: { "content-type": "image/jpeg" },
        });
      }
      // Single-file: tiny EPUB/PDF body.
      return makeStreamingResponse([new Uint8Array([1, 2, 3, 4])], {
        contentLength: 4,
      });
    },
  );
  return { fakeFetch, calls };
}

// -- estimateBookBytes ---------------------------------------------------

describe("estimateBookBytes", () => {
  it("uses fileSize for EPUB when available", () => {
    expect(
      estimateBookBytes({
        id: "x",
        fileFormat: "epub",
        pageCount: 1,
        fileSize: 5 * 1024 * 1024,
      }),
    ).toBe(5 * 1024 * 1024);
  });

  it("falls back to one avgPageBytes for EPUB when fileSize missing", () => {
    expect(
      estimateBookBytes({ id: "x", fileFormat: "pdf", pageCount: 1 }, 1000),
    ).toBe(1000);
  });

  it("uses pageCount * avgPageBytes for comics", () => {
    expect(
      estimateBookBytes({ id: "x", fileFormat: "cbz", pageCount: 20 }, 500),
    ).toBe(10_000);
  });

  it("returns 0 for unknown formats", () => {
    expect(
      estimateBookBytes({ id: "x", fileFormat: "mobi", pageCount: 100 }),
    ).toBe(0);
  });
});

// -- preflightQuota ------------------------------------------------------

describe("preflightQuota", () => {
  it("passes when usage + estimated <= quota * threshold", async () => {
    const storage = {
      estimate: vi.fn(async () => ({ usage: 0, quota: 1_000_000 })),
    } as unknown as StorageManager;
    await expect(
      preflightQuota(
        [{ id: "1", fileFormat: "epub", pageCount: 1, fileSize: 100_000 }],
        { storage, quotaThreshold: 0.9 },
      ),
    ).resolves.toBeUndefined();
  });

  it("throws QuotaExceededError when projected usage exceeds threshold", async () => {
    const storage = {
      estimate: vi.fn(async () => ({ usage: 800_000, quota: 1_000_000 })),
    } as unknown as StorageManager;
    await expect(
      preflightQuota(
        [{ id: "1", fileFormat: "epub", pageCount: 1, fileSize: 200_000 }],
        { storage, quotaThreshold: 0.9 },
      ),
    ).rejects.toBeInstanceOf(QuotaExceededError);
  });

  it("treats missing StorageManager as unknown and lets the queue proceed", async () => {
    await expect(
      preflightQuota(
        [{ id: "1", fileFormat: "epub", pageCount: 1, fileSize: 999 }],
        { storage: undefined as unknown as StorageManager },
      ),
    ).resolves.toBeUndefined();
  });

  it("treats a 0-quota estimate as unknown rather than blocking", async () => {
    const storage = {
      estimate: vi.fn(async () => ({ usage: 0, quota: 0 })),
    } as unknown as StorageManager;
    await expect(
      preflightQuota(
        [{ id: "1", fileFormat: "epub", pageCount: 1, fileSize: 1 }],
        { storage },
      ),
    ).resolves.toBeUndefined();
  });
});

// -- downloadSeriesBatch -------------------------------------------------

const books3: SeriesBookSummary[] = [
  { id: "a", fileFormat: "epub", pageCount: 1, fileSize: 4 },
  { id: "b", fileFormat: "epub", pageCount: 1, fileSize: 4 },
  { id: "c", fileFormat: "epub", pageCount: 1, fileSize: 4 },
];

describe("downloadSeriesBatch: success path", () => {
  it("downloads every book sequentially and resolves with all three completed", async () => {
    const { caches: cachesImpl, getStore } = makeFakeCaches();
    const { fakeFetch } = makeFakeFetch();
    const controller = await downloadSeriesBatch({
      seriesId: "series-1",
      books: books3,
      fetch: fakeFetch as typeof globalThis.fetch,
      caches: cachesImpl,
    });
    const result = await controller.done;
    expect(result.completed.sort()).toEqual(["a", "b", "c"]);
    expect(result.failed).toEqual([]);
    expect(result.cancelled).toEqual([]);
    for (const id of ["a", "b", "c"]) {
      expect(await getDownload(id)).toMatchObject({ status: "complete" });
      expect(getStore(cacheNameForBook(id))).toBeDefined();
    }
  });

  it("emits state updates to subscribers as books progress", async () => {
    const { caches: cachesImpl } = makeFakeCaches();
    const { fakeFetch } = makeFakeFetch();
    const controller = await downloadSeriesBatch({
      seriesId: "series-1",
      books: books3,
      fetch: fakeFetch as typeof globalThis.fetch,
      caches: cachesImpl,
    });
    const snapshots: Array<{ completed: number; total: number }> = [];
    controller.subscribe((s) => {
      snapshots.push({ completed: s.completed, total: s.total });
    });
    await controller.done;
    // First snapshot is the synchronous push; final must show 3 completed.
    expect(snapshots[0]).toMatchObject({ total: 3 });
    expect(snapshots[snapshots.length - 1]).toMatchObject({
      completed: 3,
      total: 3,
    });
  });

  it("marks unsupported formats as skipped without trying to download them", async () => {
    const { caches: cachesImpl } = makeFakeCaches();
    const { fakeFetch, calls } = makeFakeFetch();
    const books: SeriesBookSummary[] = [
      { id: "a", fileFormat: "epub", pageCount: 1, fileSize: 4 },
      { id: "b", fileFormat: "mobi", pageCount: 1, fileSize: 4 },
    ];
    const controller = await downloadSeriesBatch({
      seriesId: "series-mix",
      books,
      fetch: fakeFetch as typeof globalThis.fetch,
      caches: cachesImpl,
    });
    await controller.done;
    const state = controller.getState();
    expect(state.perBook.get("b")?.status).toBe("skipped");
    expect(state.perBook.get("a")?.status).toBe("complete");
    expect(calls.some((u) => u.includes("/books/b/"))).toBe(false);
  });
});

describe("downloadSeriesBatch: per-book cancel", () => {
  it("cancelling the middle book lets the other two complete", async () => {
    const { caches: cachesImpl } = makeFakeCaches();
    // Stage the fetch so we can cancel book `b` while it is in flight.
    let releaseB!: () => void;
    const bStarted = new Promise<void>((resolve) => {
      releaseB = resolve;
    });
    const fakeFetch = vi.fn(
      async (input: RequestInfo | URL, init?: RequestInit) => {
        const url =
          typeof input === "string" ? input : (input as URL).toString();
        if (url.includes("/books/b/")) {
          releaseB();
          await new Promise<void>((_resolve, reject) => {
            const onAbort = () => {
              reject(new DOMException("Aborted", "AbortError"));
            };
            if (init?.signal?.aborted) {
              onAbort();
              return;
            }
            init?.signal?.addEventListener("abort", onAbort);
          });
        }
        return makeStreamingResponse([new Uint8Array([1, 2, 3, 4])], {
          contentLength: 4,
        });
      },
    );

    const controller = await downloadSeriesBatch({
      seriesId: "series-cancel",
      books: books3,
      fetch: fakeFetch as typeof globalThis.fetch,
      caches: cachesImpl,
    });
    await bStarted;
    controller.cancelBook("b");
    const result = await controller.done;
    expect(result.completed.sort()).toEqual(["a", "c"]);
    expect(result.cancelled).toEqual(["b"]);
    expect(await getDownload("b")).toBeUndefined();
    expect(await getDownload("a")).toMatchObject({ status: "complete" });
    expect(await getDownload("c")).toMatchObject({ status: "complete" });
  });

  it("cancelling a queued book flips its state without invoking the manager", async () => {
    const { caches: cachesImpl } = makeFakeCaches();
    // Pause the first book so the others stay queued long enough to cancel.
    let releaseA!: () => void;
    const aHolding = new Promise<void>((resolve) => {
      releaseA = resolve;
    });
    const fakeFetch = vi.fn(
      async (input: RequestInfo | URL, init?: RequestInit) => {
        const url =
          typeof input === "string" ? input : (input as URL).toString();
        if (url.includes("/books/a/")) {
          await aHolding;
        }
        if (init?.signal?.aborted)
          throw new DOMException("Aborted", "AbortError");
        return makeStreamingResponse([new Uint8Array([1, 2, 3, 4])], {
          contentLength: 4,
        });
      },
    );
    const controller = await downloadSeriesBatch({
      seriesId: "series-queued",
      books: books3,
      fetch: fakeFetch as typeof globalThis.fetch,
      caches: cachesImpl,
    });
    controller.cancelBook("c");
    releaseA();
    const result = await controller.done;
    expect(result.cancelled).toEqual(["c"]);
    expect(result.completed.sort()).toEqual(["a", "b"]);
    // Cancelled-before-start book never made it into IDB.
    expect(await getDownload("c")).toBeUndefined();
  });
});

describe("downloadSeriesBatch: pre-flight quota check", () => {
  it("refuses with no IDB writes when projected usage exceeds 90% of quota", async () => {
    const { caches: cachesImpl } = makeFakeCaches();
    const { fakeFetch } = makeFakeFetch();
    const storage = {
      estimate: vi.fn(async () => ({ usage: 900_000, quota: 1_000_000 })),
    } as unknown as StorageManager;

    const books: SeriesBookSummary[] = [
      { id: "huge", fileFormat: "epub", pageCount: 1, fileSize: 500_000 },
    ];
    await expect(
      downloadSeriesBatch({
        seriesId: "series-over",
        books,
        fetch: fakeFetch as typeof globalThis.fetch,
        caches: cachesImpl,
        storage,
      }),
    ).rejects.toBeInstanceOf(QuotaExceededError);
    expect(await getAllDownloads()).toEqual([]);
  });

  it("proceeds when projected usage is below the threshold", async () => {
    const { caches: cachesImpl } = makeFakeCaches();
    const { fakeFetch } = makeFakeFetch();
    const storage = {
      estimate: vi.fn(async () => ({ usage: 0, quota: 1_000_000 })),
    } as unknown as StorageManager;
    const controller = await downloadSeriesBatch({
      seriesId: "series-ok",
      books: books3,
      fetch: fakeFetch as typeof globalThis.fetch,
      caches: cachesImpl,
      storage,
    });
    const result = await controller.done;
    expect(result.completed.sort()).toEqual(["a", "b", "c"]);
  });
});

describe("downloadSeriesBatch: subscribe", () => {
  it("synchronously pushes the current state to new subscribers", async () => {
    const { caches: cachesImpl } = makeFakeCaches();
    const { fakeFetch } = makeFakeFetch();
    const controller = await downloadSeriesBatch({
      seriesId: "series-sub",
      books: [{ id: "x", fileFormat: "epub", pageCount: 1, fileSize: 4 }],
      fetch: fakeFetch as typeof globalThis.fetch,
      caches: cachesImpl,
    });
    let received: SeriesQueueState | null = null;
    const unsubscribe = controller.subscribe((s) => {
      received = s;
    });
    expect(received).not.toBeNull();
    expect(received!.seriesId).toBe("series-sub");
    unsubscribe();
    await controller.done;
  });
});

describe("downloadSeriesBatch: cancelAll", () => {
  it("aborts in-flight and queued books, resolves with cancelled list", async () => {
    const { caches: cachesImpl } = makeFakeCaches();
    let releaseA!: () => void;
    const aHolding = new Promise<void>((resolve) => {
      releaseA = resolve;
    });
    const fakeFetch = vi.fn(
      async (input: RequestInfo | URL, init?: RequestInit) => {
        const url =
          typeof input === "string" ? input : (input as URL).toString();
        if (url.includes("/books/a/")) {
          await new Promise<void>((_resolve, reject) => {
            const onAbort = () =>
              reject(new DOMException("Aborted", "AbortError"));
            if (init?.signal?.aborted) {
              onAbort();
              return;
            }
            init?.signal?.addEventListener("abort", onAbort);
            releaseA();
          });
        }
        return makeStreamingResponse([new Uint8Array([1, 2, 3, 4])], {
          contentLength: 4,
        });
      },
    );
    const controller = await downloadSeriesBatch({
      seriesId: "series-all",
      books: books3,
      fetch: fakeFetch as typeof globalThis.fetch,
      caches: cachesImpl,
    });
    await aHolding;
    controller.cancelAll();
    const result = await controller.done;
    expect(result.cancelled.sort()).toEqual(["a", "b", "c"]);
    expect(result.completed).toEqual([]);
  });
});

describe("downloadSeriesBatch: mixed result", () => {
  it("captures per-book error and lets the rest finish", async () => {
    const { caches: cachesImpl } = makeFakeCaches();
    const fakeFetch = vi.fn(async (input: RequestInfo | URL) => {
      const url = typeof input === "string" ? input : (input as URL).toString();
      if (url.includes("/books/b/")) {
        return new Response("forbidden", { status: 403 });
      }
      return makeStreamingResponse([new Uint8Array([1, 2, 3, 4])], {
        contentLength: 4,
      });
    });
    const controller = await downloadSeriesBatch({
      seriesId: "series-err",
      books: books3,
      fetch: fakeFetch as typeof globalThis.fetch,
      caches: cachesImpl,
    });
    const result = await controller.done;
    expect(result.completed.sort()).toEqual(["a", "c"]);
    expect(result.failed.length).toBe(1);
    expect(result.failed[0]?.bookId).toBe("b");
    expect(result.failed[0]?.error).toMatch(/HTTP 403/);
    const states: BookQueueState[] = Array.from(
      controller.getState().perBook.values(),
    );
    expect(states.find((s) => s.bookId === "b")?.status).toBe("error");
  });
});
