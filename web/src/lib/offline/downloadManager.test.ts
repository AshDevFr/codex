import { IDBFactory } from "fake-indexeddb";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  _resetForTests,
  getAllDownloads,
  getDownload,
  setDbContext,
} from "./db";
import {
  _resetPersistenceForTests,
  downloadComicBook,
  downloadSingleFileBook,
  getStoragePersistence,
  requestStoragePersistence,
} from "./downloadManager";
import { cacheNameForBook } from "./routeMatcher";

beforeEach(() => {
  setDbContext({ indexedDB: new IDBFactory() });
});

afterEach(() => {
  setDbContext(null);
  _resetForTests();
  _resetPersistenceForTests();
});

// -- Fake CacheStorage ----------------------------------------------------
// jsdom does not ship the Cache API. The downloadManager only needs `open`
// and `delete`, and Cache only needs `put` / `match` for our purposes.

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

// -- Fetch helpers --------------------------------------------------------

function makeStreamingResponse(
  chunks: Uint8Array[],
  init: {
    contentLength?: number | null;
    contentType?: string;
    status?: number;
  } = {},
): Response {
  const headers = new Headers();
  if (init.contentType) headers.set("content-type", init.contentType);
  if (init.contentLength !== null && init.contentLength !== undefined) {
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

describe("downloadSingleFileBook: success path", () => {
  it("streams the body, caches it under codex-book-<id>, and writes a complete IDB row", async () => {
    const { caches: cachesImpl, getStore } = makeFakeCaches();
    const payload = new Uint8Array([1, 2, 3, 4, 5, 6, 7, 8]);
    const fakeFetch = vi.fn(async () =>
      makeStreamingResponse([payload.slice(0, 4), payload.slice(4)], {
        contentLength: payload.length,
        contentType: "application/epub+zip",
      }),
    );

    const result = await downloadSingleFileBook({
      bookId: "book-1",
      format: "epub",
      fetch: fakeFetch as typeof globalThis.fetch,
      caches: cachesImpl,
    });

    expect(result).toEqual({ bookId: "book-1", bytes: 8 });
    expect(fakeFetch).toHaveBeenCalledWith(
      "/api/v1/books/book-1/file",
      expect.objectContaining({}),
    );

    const record = await getDownload("book-1");
    expect(record?.status).toBe("complete");
    expect(record?.bytes).toBe(8);
    expect(record?.format).toBe("epub");
    expect(record?.pageCount).toBe(1);
    expect(record?.downloadedAt).toBeGreaterThan(0);

    const store = getStore(cacheNameForBook("book-1"));
    expect(store?.has("/api/v1/books/book-1/file")).toBe(true);
    const entry = store?.get("/api/v1/books/book-1/file");
    expect(Array.from(entry?.body ?? [])).toEqual(Array.from(payload));
    expect(entry?.headers["content-type"]).toBe("application/epub+zip");
  });

  it("invokes onProgress with monotonically increasing loaded values and the correct total", async () => {
    const { caches: cachesImpl } = makeFakeCaches();
    const chunks = [
      new Uint8Array([1, 2, 3]),
      new Uint8Array([4, 5]),
      new Uint8Array([6, 7, 8, 9, 10]),
    ];
    const total = chunks.reduce((acc, c) => acc + c.length, 0);
    const fakeFetch = vi.fn(async () =>
      makeStreamingResponse(chunks, { contentLength: total }),
    );
    const progress: { loaded: number; total: number | null }[] = [];

    await downloadSingleFileBook({
      bookId: "book-2",
      format: "pdf",
      onProgress: (p) => progress.push({ ...p }),
      fetch: fakeFetch as typeof globalThis.fetch,
      caches: cachesImpl,
    });

    expect(progress.map((p) => p.loaded)).toEqual([3, 5, 10]);
    expect(progress.every((p) => p.total === total)).toBe(true);
  });

  it("reports total: null when Content-Length is missing", async () => {
    const { caches: cachesImpl } = makeFakeCaches();
    const fakeFetch = vi.fn(async () =>
      makeStreamingResponse([new Uint8Array([1, 2, 3])], {
        contentLength: null,
      }),
    );
    const progress: { loaded: number; total: number | null }[] = [];

    await downloadSingleFileBook({
      bookId: "book-3",
      format: "epub",
      onProgress: (p) => progress.push({ ...p }),
      fetch: fakeFetch as typeof globalThis.fetch,
      caches: cachesImpl,
    });

    expect(progress[0]?.total).toBeNull();
  });

  it("flips the IDB record from downloading -> complete in two writes", async () => {
    const { caches: cachesImpl } = makeFakeCaches();
    const states: string[] = [];
    const fakeFetch = vi.fn(async () => {
      // Capture the IDB row state at the time fetch is invoked: by then
      // putDownload should have already written the `downloading` row.
      const mid = await getDownload("book-4");
      if (mid) states.push(mid.status);
      return makeStreamingResponse([new Uint8Array([1])], { contentLength: 1 });
    });

    await downloadSingleFileBook({
      bookId: "book-4",
      format: "epub",
      fetch: fakeFetch as typeof globalThis.fetch,
      caches: cachesImpl,
    });

    const final = await getDownload("book-4");
    states.push(final?.status ?? "missing");
    expect(states).toEqual(["downloading", "complete"]);
  });

  it("supports independent concurrent downloads in separate caches", async () => {
    const { caches: cachesImpl, getStore } = makeFakeCaches();
    const fakeFetch = vi.fn(async (input: RequestInfo | URL) => {
      const url = typeof input === "string" ? input : (input as URL).toString();
      const body = url.includes("book-a")
        ? new Uint8Array([0xa])
        : new Uint8Array([0xb, 0xb]);
      return makeStreamingResponse([body], { contentLength: body.length });
    });

    await Promise.all([
      downloadSingleFileBook({
        bookId: "book-a",
        format: "epub",
        fetch: fakeFetch as typeof globalThis.fetch,
        caches: cachesImpl,
      }),
      downloadSingleFileBook({
        bookId: "book-b",
        format: "pdf",
        fetch: fakeFetch as typeof globalThis.fetch,
        caches: cachesImpl,
      }),
    ]);

    const all = await getAllDownloads();
    expect(all.map((r) => r.id).sort()).toEqual(["book-a", "book-b"]);
    expect(getStore(cacheNameForBook("book-a"))?.size).toBe(1);
    expect(getStore(cacheNameForBook("book-b"))?.size).toBe(1);
  });
});

describe("downloadSingleFileBook: error paths", () => {
  it("records an error and rethrows when fetch throws", async () => {
    const { caches: cachesImpl } = makeFakeCaches();
    const fakeFetch = vi.fn(async () => {
      throw new Error("network down");
    });

    await expect(
      downloadSingleFileBook({
        bookId: "book-err",
        format: "epub",
        fetch: fakeFetch as typeof globalThis.fetch,
        caches: cachesImpl,
      }),
    ).rejects.toThrow("network down");

    const record = await getDownload("book-err");
    expect(record?.status).toBe("error");
    expect(record?.error).toBe("network down");
  });

  it("records an error and rethrows on a non-OK response", async () => {
    const { caches: cachesImpl, getStore } = makeFakeCaches();
    const fakeFetch = vi.fn(
      async () => new Response("forbidden", { status: 403 }),
    );

    await expect(
      downloadSingleFileBook({
        bookId: "book-403",
        format: "pdf",
        fetch: fakeFetch as typeof globalThis.fetch,
        caches: cachesImpl,
      }),
    ).rejects.toThrow(/HTTP 403/);

    const record = await getDownload("book-403");
    expect(record?.status).toBe("error");
    // Nothing was cached.
    expect(getStore(cacheNameForBook("book-403"))).toBeUndefined();
  });

  it("records an error if the stream errors mid-download", async () => {
    const { caches: cachesImpl, getStore } = makeFakeCaches();
    const fakeFetch = vi.fn(async () => {
      const stream = new ReadableStream<Uint8Array>({
        start(controller) {
          controller.enqueue(new Uint8Array([1, 2]));
          controller.error(new Error("stream broke"));
        },
      });
      return new Response(stream, {
        status: 200,
        headers: { "content-length": "8" },
      });
    });

    await expect(
      downloadSingleFileBook({
        bookId: "book-stream",
        format: "epub",
        fetch: fakeFetch as typeof globalThis.fetch,
        caches: cachesImpl,
      }),
    ).rejects.toThrow("stream broke");

    const record = await getDownload("book-stream");
    expect(record?.status).toBe("error");
    expect(getStore(cacheNameForBook("book-stream"))).toBeUndefined();
  });
});

describe("downloadSingleFileBook: cancellation", () => {
  it("aborting before the stream finishes deletes the IDB row and the per-book cache", async () => {
    const { caches: cachesImpl, getStore } = makeFakeCaches();
    const controller = new AbortController();

    const fakeFetch = vi.fn(async (_input, init?: RequestInit) => {
      const signal = init?.signal;
      const stream = new ReadableStream<Uint8Array>({
        async start(streamController) {
          streamController.enqueue(new Uint8Array([1, 2]));
          // Wait then abort, triggering a stream error on the reader.
          await new Promise<void>((resolve) => setTimeout(resolve, 5));
          controller.abort();
          if (signal?.aborted) {
            streamController.error(new DOMException("Aborted", "AbortError"));
          } else {
            streamController.enqueue(new Uint8Array([3, 4]));
            streamController.close();
          }
        },
      });
      return new Response(stream, {
        status: 200,
        headers: { "content-length": "4" },
      });
    });

    await expect(
      downloadSingleFileBook({
        bookId: "book-abort",
        format: "epub",
        signal: controller.signal,
        fetch: fakeFetch as typeof globalThis.fetch,
        caches: cachesImpl,
      }),
    ).rejects.toMatchObject({ name: "AbortError" });

    expect(await getDownload("book-abort")).toBeUndefined();
    expect(getStore(cacheNameForBook("book-abort"))).toBeUndefined();
  });
});

// -- Comic per-page download ---------------------------------------------

function makePageResponse(
  bytes: Uint8Array,
  contentType = "image/jpeg",
): Response {
  return new Response(bytes, {
    status: 200,
    headers: { "content-type": contentType },
  });
}

function parsePageNumber(url: string): number {
  const match = url.match(/\/pages\/(\d+)$/);
  if (!match) throw new Error(`Not a page URL: ${url}`);
  return Number(match[1]);
}

describe("downloadComicBook: success path", () => {
  it("fetches every page, stores each under the per-book cache, and writes a complete IDB row", async () => {
    const { caches: cachesImpl, getStore } = makeFakeCaches();
    const pageCount = 12;
    const fakeFetch = vi.fn(async (input: RequestInfo | URL) => {
      const url = typeof input === "string" ? input : (input as URL).toString();
      const n = parsePageNumber(url);
      // Page N body = a single byte equal to N (test-friendly).
      return makePageResponse(new Uint8Array([n]));
    });

    const result = await downloadComicBook({
      bookId: "book-1",
      format: "cbz",
      pageCount,
      fetch: fakeFetch as typeof globalThis.fetch,
      caches: cachesImpl,
    });

    expect(result).toEqual({ bookId: "book-1", bytes: pageCount });
    expect(fakeFetch).toHaveBeenCalledTimes(pageCount);

    const store = getStore(cacheNameForBook("book-1"));
    expect(store?.size).toBe(pageCount);
    for (let n = 1; n <= pageCount; n++) {
      const entry = store?.get(`/api/v1/books/book-1/pages/${n}`);
      expect(entry).toBeDefined();
      expect(Array.from(entry?.body ?? [])).toEqual([n]);
    }

    const record = await getDownload("book-1");
    expect(record?.status).toBe("complete");
    expect(record?.format).toBe("cbz");
    expect(record?.pageCount).toBe(pageCount);
    expect(record?.bytes).toBe(pageCount);
    expect(record?.downloadedAt).toBeGreaterThan(0);
  });

  it("respects the concurrency cap (no more than `concurrency` requests in flight)", async () => {
    const { caches: cachesImpl } = makeFakeCaches();
    let inFlight = 0;
    let peak = 0;
    const fakeFetch = vi.fn(async (input: RequestInfo | URL) => {
      inFlight++;
      peak = Math.max(peak, inFlight);
      // Give the event loop a microtask break so concurrency can actually
      // ramp up (sync resolves would all run in one tick at peak=1).
      await new Promise((r) => setTimeout(r, 1));
      inFlight--;
      const n = parsePageNumber(
        typeof input === "string" ? input : (input as URL).toString(),
      );
      return makePageResponse(new Uint8Array([n]));
    });

    await downloadComicBook({
      bookId: "book-conc",
      format: "cbz",
      pageCount: 20,
      concurrency: 4,
      fetch: fakeFetch as typeof globalThis.fetch,
      caches: cachesImpl,
    });

    expect(peak).toBeLessThanOrEqual(4);
    expect(peak).toBeGreaterThan(1);
  });

  it("reports progress as pages-done / pageCount, monotonically increasing", async () => {
    const { caches: cachesImpl } = makeFakeCaches();
    const fakeFetch = vi.fn(async (input: RequestInfo | URL) => {
      const n = parsePageNumber(
        typeof input === "string" ? input : (input as URL).toString(),
      );
      return makePageResponse(new Uint8Array([n]));
    });
    const progress: { loaded: number; total: number | null }[] = [];

    await downloadComicBook({
      bookId: "book-prog",
      format: "cbz",
      pageCount: 5,
      concurrency: 1,
      onProgress: (p) => progress.push({ ...p }),
      fetch: fakeFetch as typeof globalThis.fetch,
      caches: cachesImpl,
    });

    expect(progress.map((p) => p.loaded)).toEqual([1, 2, 3, 4, 5]);
    expect(progress.every((p) => p.total === 5)).toBe(true);
  });

  it("rejects pageCount < 1 without touching IDB or cache", async () => {
    const { caches: cachesImpl, getStore } = makeFakeCaches();
    await expect(
      downloadComicBook({
        bookId: "bad",
        format: "cbz",
        pageCount: 0,
        fetch: vi.fn() as unknown as typeof globalThis.fetch,
        caches: cachesImpl,
      }),
    ).rejects.toThrow(/Invalid pageCount/);
    expect(await getDownload("bad")).toBeUndefined();
    expect(getStore(cacheNameForBook("bad"))).toBeUndefined();
  });
});

describe("downloadComicBook: page failure", () => {
  it("aborts on a 404 page, sets IDB to error with the page number, and evicts the partial cache", async () => {
    const { caches: cachesImpl, getStore } = makeFakeCaches();
    const fakeFetch = vi.fn(async (input: RequestInfo | URL) => {
      const url = typeof input === "string" ? input : (input as URL).toString();
      const n = parsePageNumber(url);
      if (n === 3) return new Response("missing", { status: 404 });
      // Pause briefly so siblings actually get a chance to start before
      // the failure aborts them, otherwise the test trivially passes with
      // page 3 being the only attempt.
      await new Promise((r) => setTimeout(r, 1));
      return makePageResponse(new Uint8Array([n]));
    });

    await expect(
      downloadComicBook({
        bookId: "book-fail",
        format: "cbz",
        pageCount: 5,
        concurrency: 2,
        fetch: fakeFetch as typeof globalThis.fetch,
        caches: cachesImpl,
      }),
    ).rejects.toThrow(/HTTP 404.*page 3/);

    const record = await getDownload("book-fail");
    expect(record?.status).toBe("error");
    expect(record?.error).toMatch(/page 3/);
    // The per-book cache must be cleared so the reader never sees an
    // incomplete download (partial caches are useless for comic reading).
    expect(getStore(cacheNameForBook("book-fail"))).toBeUndefined();
  });

  it("does not start additional pages after the first failure (in-flight workers exit)", async () => {
    const { caches: cachesImpl } = makeFakeCaches();
    const fakeFetch = vi.fn(async (input: RequestInfo | URL) => {
      const url = typeof input === "string" ? input : (input as URL).toString();
      const n = parsePageNumber(url);
      if (n === 2) return new Response("nope", { status: 500 });
      return makePageResponse(new Uint8Array([n]));
    });

    await expect(
      downloadComicBook({
        bookId: "book-stop",
        format: "cbz",
        pageCount: 100,
        concurrency: 2,
        fetch: fakeFetch as typeof globalThis.fetch,
        caches: cachesImpl,
      }),
    ).rejects.toThrow(/HTTP 500/);

    // With concurrency=2 and an immediate failure on page 2, the worker
    // pool should not have fanned out to anywhere near all 100 pages.
    expect(fakeFetch.mock.calls.length).toBeLessThan(20);
  });
});

describe("downloadComicBook: cancellation", () => {
  it("aborting during the download deletes the IDB row and the per-book cache", async () => {
    const { caches: cachesImpl, getStore } = makeFakeCaches();
    const controller = new AbortController();

    const fakeFetch = vi.fn(
      async (input: RequestInfo | URL, init?: RequestInit) => {
        const url =
          typeof input === "string" ? input : (input as URL).toString();
        const n = parsePageNumber(url);
        // Abort once page 3 starts; pages already-finished stay in cache
        // (the cleanup runs after Promise.all resolves).
        if (n === 3) controller.abort();
        if (init?.signal?.aborted) {
          throw new DOMException("Aborted", "AbortError");
        }
        return makePageResponse(new Uint8Array([n]));
      },
    );

    await expect(
      downloadComicBook({
        bookId: "book-abort",
        format: "cbz",
        pageCount: 10,
        concurrency: 1,
        signal: controller.signal,
        fetch: fakeFetch as typeof globalThis.fetch,
        caches: cachesImpl,
      }),
    ).rejects.toMatchObject({ name: "AbortError" });

    expect(await getDownload("book-abort")).toBeUndefined();
    expect(getStore(cacheNameForBook("book-abort"))).toBeUndefined();
  });
});

// -- Storage persistence -------------------------------------------------

describe("requestStoragePersistence", () => {
  it("calls persist() once and caches the result for subsequent calls", async () => {
    const persistSpy = vi.fn(async () => true);
    const fakeStorage = { persist: persistSpy } as unknown as StorageManager;

    const first = await requestStoragePersistence(fakeStorage);
    const second = await requestStoragePersistence(fakeStorage);

    expect(first).toBe(true);
    expect(second).toBe(true);
    expect(persistSpy).toHaveBeenCalledTimes(1);
    expect(getStoragePersistence()).toBe(true);
  });

  it("returns null when the StorageManager API is unavailable", async () => {
    const result = await requestStoragePersistence(
      undefined as unknown as StorageManager,
    );
    expect(result).toBeNull();
    expect(getStoragePersistence()).toBeNull();
  });

  it("returns false when persist() throws", async () => {
    const fakeStorage = {
      persist: vi.fn(async () => {
        throw new Error("denied");
      }),
    } as unknown as StorageManager;

    const result = await requestStoragePersistence(fakeStorage);
    expect(result).toBe(false);
    expect(getStoragePersistence()).toBe(false);
  });

  it("returns false when persist() resolves to false (denied)", async () => {
    const fakeStorage = {
      persist: vi.fn(async () => false),
    } as unknown as StorageManager;

    const result = await requestStoragePersistence(fakeStorage);
    expect(result).toBe(false);
    expect(getStoragePersistence()).toBe(false);
  });

  it("deduplicates concurrent in-flight calls", async () => {
    let resolvePersist: ((granted: boolean) => void) | null = null;
    const persistSpy = vi.fn(
      () =>
        new Promise<boolean>((res) => {
          resolvePersist = res;
        }),
    );
    const fakeStorage = { persist: persistSpy } as unknown as StorageManager;

    const a = requestStoragePersistence(fakeStorage);
    const b = requestStoragePersistence(fakeStorage);
    expect(persistSpy).toHaveBeenCalledTimes(1);

    resolvePersist?.(true);
    expect(await a).toBe(true);
    expect(await b).toBe(true);
  });
});

describe("downloadSingleFileBook + persistence", () => {
  it("requests storage persistence on first successful download", async () => {
    const { caches: cachesImpl } = makeFakeCaches();
    const persistSpy = vi.fn(async () => true);
    Object.defineProperty(globalThis.navigator, "storage", {
      configurable: true,
      value: { persist: persistSpy } as unknown as StorageManager,
    });

    const fakeFetch = vi.fn(async () =>
      makeStreamingResponse([new Uint8Array([1])], { contentLength: 1 }),
    );

    try {
      await downloadSingleFileBook({
        bookId: "book-persist",
        format: "epub",
        fetch: fakeFetch as typeof globalThis.fetch,
        caches: cachesImpl,
      });
      // Allow the fire-and-forget persistence request to settle before
      // asserting the spy was called.
      await new Promise((r) => setTimeout(r, 0));
      expect(persistSpy).toHaveBeenCalledTimes(1);
      expect(getStoragePersistence()).toBe(true);
    } finally {
      Object.defineProperty(globalThis.navigator, "storage", {
        configurable: true,
        value: undefined,
      });
    }
  });
});
