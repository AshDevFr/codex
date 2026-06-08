import { HostRpcClient, type PluginStorage } from "@ashdev/codex-plugin-sdk";
import { describe, expect, it, vi } from "vitest";
import type { FeedItem, FeedResponse } from "./fetcher.js";
import {
  CURSOR_STORAGE_KEY,
  loadCursor,
  normalizeBaseUrl,
  poll,
  registerSources,
  saveCursor,
} from "./index.js";

// -----------------------------------------------------------------------------
// Mock host RPC
// -----------------------------------------------------------------------------

interface CapturedCall {
  method: string;
  params: unknown;
}

type Responder = (
  method: string,
  params: unknown,
  attempt: number,
) => unknown | { __error: { code: number; message: string } };

/**
 * Build a `HostRpcClient` whose calls are intercepted in-memory. The custom
 * `writeFn` captures each request and synthesizes a JSON-RPC response (result
 * or error) via the real id-correlation path. `respond` may return a normal
 * result, or `{ __error: { code, message } }` to drive an error response with
 * a specific code (e.g. -32601 for METHOD_NOT_FOUND).
 */
function makeMockRpc(respond: Responder): {
  rpc: HostRpcClient;
  calls: CapturedCall[];
} {
  const calls: CapturedCall[] = [];
  let attemptByMethod: Record<string, number> = {};
  // `rpc` is referenced inside writeFn (a closure) before its assignment runs,
  // so it must be declared with `let` and initialized after writeFn is built.
  let rpc: HostRpcClient;
  const writeFn = (line: string) => {
    const req = JSON.parse(line.trim()) as {
      id: number;
      method: string;
      params: unknown;
    };
    calls.push({ method: req.method, params: req.params });
    attemptByMethod[req.method] = (attemptByMethod[req.method] ?? 0) + 1;
    const outcome = respond(req.method, req.params, attemptByMethod[req.method]);
    setImmediate(() => {
      const isError =
        outcome !== null &&
        typeof outcome === "object" &&
        "__error" in (outcome as Record<string, unknown>);
      const payload = isError
        ? {
            jsonrpc: "2.0",
            id: req.id,
            error: (outcome as { __error: { code: number; message: string } }).__error,
          }
        : { jsonrpc: "2.0", id: req.id, result: outcome };
      rpc.handleResponse(JSON.stringify(payload));
    });
  };
  rpc = new HostRpcClient(writeFn);
  attemptByMethod = {};
  return { rpc, calls };
}

// -----------------------------------------------------------------------------
// normalizeBaseUrl
// -----------------------------------------------------------------------------

describe("normalizeBaseUrl", () => {
  it("strips trailing slashes and trims whitespace", () => {
    expect(normalizeBaseUrl("https://t.example.com/")).toBe("https://t.example.com");
    expect(normalizeBaseUrl("  https://t.example.com///  ")).toBe("https://t.example.com");
    expect(normalizeBaseUrl("https://t.example.com")).toBe("https://t.example.com");
  });
});

// -----------------------------------------------------------------------------
// Cursor persistence
// -----------------------------------------------------------------------------

/** Minimal in-memory `PluginStorage` double exposing only get/set. */
function makeFakeStorage(initial?: unknown): {
  storage: PluginStorage;
  get: ReturnType<typeof vi.fn>;
  set: ReturnType<typeof vi.fn>;
} {
  const get = vi.fn(async () => ({ data: initial ?? null }));
  const set = vi.fn(async () => ({ success: true }));
  const storage = { get, set } as unknown as PluginStorage;
  return { storage, get, set };
}

describe("loadCursor", () => {
  it("returns the stored cursor string", async () => {
    const { storage, get } = makeFakeStorage("cursor-42");
    expect(await loadCursor(storage)).toBe("cursor-42");
    expect(get).toHaveBeenCalledWith(CURSOR_STORAGE_KEY);
  });

  it("returns null when no cursor is stored", async () => {
    const { storage } = makeFakeStorage(null);
    expect(await loadCursor(storage)).toBeNull();
  });

  it("returns null for a non-string / empty stored value", async () => {
    expect(await loadCursor(makeFakeStorage("").storage)).toBeNull();
    expect(await loadCursor(makeFakeStorage(123).storage)).toBeNull();
  });

  it("returns null and does not throw when the read fails", async () => {
    const storage = {
      get: vi.fn(async () => {
        throw new Error("kv down");
      }),
      set: vi.fn(),
    } as unknown as PluginStorage;
    expect(await loadCursor(storage)).toBeNull();
  });
});

describe("saveCursor", () => {
  it("writes the cursor under the feed-cursor key", async () => {
    const { storage, set } = makeFakeStorage();
    await saveCursor(storage, "cursor-99");
    expect(set).toHaveBeenCalledWith(CURSOR_STORAGE_KEY, "cursor-99");
  });

  it("swallows a write failure without throwing", async () => {
    const storage = {
      get: vi.fn(),
      set: vi.fn(async () => {
        throw new Error("kv full");
      }),
    } as unknown as PluginStorage;
    await expect(saveCursor(storage, "cursor-99")).resolves.toBeUndefined();
  });
});

// -----------------------------------------------------------------------------
// registerSources
// -----------------------------------------------------------------------------

describe("registerSources", () => {
  it("registers exactly one api-feed source keyed 'default'", async () => {
    const { rpc, calls } = makeMockRpc(() => ({ registered: 1, pruned: 0 }));
    const result = await registerSources(rpc);

    expect(result).toEqual({ registered: 1, pruned: 0 });
    expect(calls).toHaveLength(1);
    expect(calls[0].method).toBe("releases/register_sources");
    const params = calls[0].params as { sources: Array<Record<string, unknown>> };
    expect(params.sources).toHaveLength(1);
    expect(params.sources[0]).toMatchObject({
      sourceKey: "default",
      displayName: "Tsundoku Releases",
      kind: "api-feed",
    });
  });

  it("retries on METHOD_NOT_FOUND then succeeds", async () => {
    const { rpc, calls } = makeMockRpc((_m, _p, attempt) =>
      attempt < 3
        ? { __error: { code: -32601, message: "method not found" } }
        : { registered: 1, pruned: 0 },
    );
    const result = await registerSources(rpc);

    expect(result).toEqual({ registered: 1, pruned: 0 });
    expect(calls.length).toBe(3);
  });

  it("returns null after exhausting retries on METHOD_NOT_FOUND", async () => {
    const { rpc, calls } = makeMockRpc(() => ({
      __error: { code: -32601, message: "method not found" },
    }));
    const result = await registerSources(rpc);

    expect(result).toBeNull();
    expect(calls.length).toBe(5);
  });

  it("does not retry on a non-METHOD_NOT_FOUND error", async () => {
    const { rpc, calls } = makeMockRpc(() => ({
      __error: { code: -32000, message: "db error" },
    }));
    const result = await registerSources(rpc);

    expect(result).toBeNull();
    expect(calls.length).toBe(1);
  });
});

// -----------------------------------------------------------------------------
// poll
// -----------------------------------------------------------------------------

/** Stateful in-memory storage double recording every `set`. */
function makePollStorage(initialCursor: string | null = null): {
  storage: PluginStorage;
  sets: string[];
  current: () => string | null;
} {
  let value = initialCursor;
  const sets: string[] = [];
  const storage = {
    get: vi.fn(async () => ({ data: value })),
    set: vi.fn(async (_key: string, data: unknown) => {
      value = data as string;
      sets.push(value);
      return { success: true };
    }),
  } as unknown as PluginStorage;
  return { storage, sets, current: () => value };
}

/** A `fetch` impl that returns the given pages in order, then empty pages. */
function makeFetchSequence(pages: FeedResponse[]): {
  fetchImpl: typeof fetch;
  urls: string[];
} {
  const urls: string[] = [];
  let i = 0;
  const fetchImpl = vi.fn(async (url: string) => {
    urls.push(url);
    const page = pages[i++] ?? { items: [], hasMore: false, nextCursor: null };
    return new Response(JSON.stringify(page), {
      status: 200,
      headers: { "content-type": "application/json" },
    });
  }) as unknown as typeof fetch;
  return { fetchImpl, urls };
}

function item(
  seriesId: number,
  provider: string,
  externalId: string,
  highestVolume: number | null = 1,
): FeedItem {
  return {
    seriesId,
    canonicalTitle: `Series ${seriesId}`,
    externalIds: [{ provider, externalId, fetchedAt: 1_700_000_000 }],
    volumeCoverage: highestVolume === null ? [] : [{ start: 1, end: highestVolume }],
    chapterCoverage: [],
    highestVolume,
    highestChapter: null,
    updatedAt: 1_700_000_000,
  };
}

/**
 * Mock host RPC for poll tests. `listTracked` supplies one page of tracked
 * series (no `nextOffset`, so the sweep stops); `record` returns a result
 * computed by `onRecord` (default: a fresh insert).
 */
function makePollRpc(opts: {
  tracked: Array<{ seriesId: string; externalIds?: Record<string, string> }>;
  onRecord?: (n: number) => { ledgerId: string; deduped: boolean } | { __error: object };
}): { rpc: HostRpcClient; calls: CapturedCall[] } {
  let recordCount = 0;
  return makeMockRpc((method) => {
    if (method === "releases/list_tracked") {
      return { tracked: opts.tracked };
    }
    if (method === "releases/record") {
      recordCount++;
      return opts.onRecord
        ? opts.onRecord(recordCount)
        : { ledgerId: `l${recordCount}`, deduped: false };
    }
    if (method === "releases/report_progress") {
      return { emitted: true };
    }
    return {};
  });
}

const pollDeps = (storage: PluginStorage, fetchImpl: typeof fetch) => ({
  storage,
  baseUrl: "https://t.example.com",
  language: "en",
  pageLimit: 100,
  timeoutMs: 5_000,
  fetchImpl,
});

describe("poll", () => {
  it("walks pages, matches by external id, records, and persists the cursor per page", async () => {
    const { rpc } = makePollRpc({
      tracked: [{ seriesId: "uuid-a", externalIds: { mangabaka: "9741" } }],
    });
    const { storage, sets } = makePollStorage();
    const { fetchImpl } = makeFetchSequence([
      {
        items: [item(87, "mangabaka", "9741", 16), item(99, "anilist", "999")],
        hasMore: true,
        nextCursor: "c1",
      },
      { items: [item(87, "mangabaka", "9741", 17)], hasMore: false, nextCursor: "c2" },
    ]);

    const res = await poll({ sourceId: "src-1" }, rpc, pollDeps(storage, fetchImpl));

    expect(res).toMatchObject({
      parsed: 3,
      matched: 2,
      recorded: 2,
      deduped: 0,
      upstreamStatus: 200,
    });
    expect(sets).toEqual(["c1", "c2"]);
  });

  it("counts host dedup separately from inserts", async () => {
    const { rpc } = makePollRpc({
      tracked: [{ seriesId: "uuid-a", externalIds: { mangabaka: "9741" } }],
      onRecord: (n) => ({ ledgerId: `l${n}`, deduped: n > 1 }),
    });
    const { storage } = makePollStorage();
    const { fetchImpl } = makeFetchSequence([
      {
        items: [item(87, "mangabaka", "9741", 16), item(87, "mangabaka", "9741", 17)],
        hasMore: false,
        nextCursor: "c1",
      },
    ]);

    const res = await poll({ sourceId: "src-1" }, rpc, pollDeps(storage, fetchImpl));
    expect(res).toMatchObject({ matched: 2, recorded: 1, deduped: 1 });
  });

  it("skips items with no tracked match (no record calls)", async () => {
    const { rpc, calls } = makePollRpc({
      tracked: [{ seriesId: "uuid-a", externalIds: { mangabaka: "9741" } }],
    });
    const { storage } = makePollStorage();
    const { fetchImpl } = makeFetchSequence([
      { items: [item(99, "anilist", "999")], hasMore: false, nextCursor: "c1" },
    ]);

    const res = await poll({ sourceId: "src-1" }, rpc, pollDeps(storage, fetchImpl));
    expect(res).toMatchObject({ parsed: 1, matched: 0, recorded: 0 });
    expect(calls.some((c) => c.method === "releases/record")).toBe(false);
  });

  it("tolerates a record failure without aborting the walk", async () => {
    const { rpc } = makePollRpc({
      tracked: [{ seriesId: "uuid-a", externalIds: { mangabaka: "9741" } }],
      onRecord: () => ({ __error: { code: -32000, message: "ledger down" } }),
    });
    const { storage, sets } = makePollStorage();
    const { fetchImpl } = makeFetchSequence([
      { items: [item(87, "mangabaka", "9741", 16)], hasMore: false, nextCursor: "c1" },
    ]);

    const res = await poll({ sourceId: "src-1" }, rpc, pollDeps(storage, fetchImpl));
    expect(res).toMatchObject({ parsed: 1, matched: 1, recorded: 0, deduped: 0 });
    expect(sets).toEqual(["c1"]); // walk still completed and advanced the cursor
  });

  it("passes the stored cursor to the first fetch", async () => {
    const { rpc } = makePollRpc({ tracked: [] });
    const { storage } = makePollStorage("resume-here");
    const { fetchImpl, urls } = makeFetchSequence([
      { items: [], hasMore: false, nextCursor: null },
    ]);

    await poll({ sourceId: "src-1" }, rpc, pollDeps(storage, fetchImpl));
    expect(new URL(urls[0]).searchParams.get("cursor")).toBe("resume-here");
  });

  it("stops and preserves the cursor on a fetch error", async () => {
    const { rpc } = makePollRpc({
      tracked: [{ seriesId: "uuid-a", externalIds: { mangabaka: "9741" } }],
    });
    const { storage, sets } = makePollStorage("kept");
    const fetchImpl = vi
      .fn()
      .mockResolvedValue(new Response("boom", { status: 503 })) as unknown as typeof fetch;

    const res = await poll({ sourceId: "src-1" }, rpc, pollDeps(storage, fetchImpl));
    expect(res).toMatchObject({ parsed: 0, upstreamStatus: 503 });
    expect(sets).toEqual([]); // never advanced past the failing page
  });
});
