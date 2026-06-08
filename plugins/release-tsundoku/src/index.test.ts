import { HostRpcClient } from "@ashdev/codex-plugin-sdk";
import { describe, expect, it, vi } from "vitest";
import type { FeedItem, FeedResponse } from "./fetcher.js";
import { loadCursor, normalizeBaseUrl, poll, registerSources, saveCursor } from "./index.js";

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
// Cursor persistence (per-source state / etag slot)
// -----------------------------------------------------------------------------

describe("loadCursor", () => {
  it("returns the etag the host passed back from the last poll", () => {
    expect(loadCursor({ sourceId: "s", etag: "cursor-42" })).toBe("cursor-42");
  });

  it("returns null when no etag was supplied", () => {
    expect(loadCursor({ sourceId: "s" })).toBeNull();
    expect(loadCursor({ sourceId: "s", etag: "" })).toBeNull();
  });
});

describe("saveCursor", () => {
  it("persists the cursor into the source-state etag slot", async () => {
    const { rpc, calls } = makeMockRpc(() => ({ success: true }));
    await saveCursor(rpc, "src-1", "cursor-99");
    expect(calls).toHaveLength(1);
    expect(calls[0].method).toBe("releases/source_state/set");
    expect(calls[0].params).toEqual({ sourceId: "src-1", etag: "cursor-99" });
  });

  it("swallows a write failure without throwing", async () => {
    const { rpc } = makeMockRpc(() => ({ __error: { code: -32000, message: "db error" } }));
    await expect(saveCursor(rpc, "src-1", "cursor-99")).resolves.toBeUndefined();
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

type PageOrError = FeedResponse | { errorStatus: number };

/** A `fetch` impl that returns the given pages/errors in order, then empties. */
function makeFetchSequence(pages: PageOrError[]): {
  fetchImpl: typeof fetch;
  urls: string[];
} {
  const urls: string[] = [];
  let i = 0;
  const fetchImpl = vi.fn(async (url: string) => {
    urls.push(url);
    const page = pages[i++] ?? { items: [], hasMore: false, nextCursor: null };
    if ("errorStatus" in page) {
      return new Response("err", { status: page.errorStatus });
    }
    return new Response(JSON.stringify(page), {
      status: 200,
      headers: { "content-type": "application/json" },
    });
  }) as unknown as typeof fetch;
  return { fetchImpl, urls };
}

/** Cursors persisted via `releases/source_state/set`, in order. */
function cursorsPersisted(calls: CapturedCall[]): string[] {
  return calls
    .filter((c) => c.method === "releases/source_state/set")
    .map((c) => (c.params as { etag: string }).etag);
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
    if (method === "releases/source_state/set") {
      return { success: true };
    }
    return {};
  });
}

const pollDeps = (fetchImpl: typeof fetch) => ({
  baseUrl: "https://t.example.com",
  language: "en",
  pageLimit: 100,
  timeoutMs: 5_000,
  fetchImpl,
});

describe("poll", () => {
  it("walks pages, matches by external id, records, and persists the cursor per page", async () => {
    const { rpc, calls } = makePollRpc({
      tracked: [{ seriesId: "uuid-a", externalIds: { mangabaka: "9741" } }],
    });
    const { fetchImpl } = makeFetchSequence([
      {
        items: [item(87, "mangabaka", "9741", 16), item(99, "anilist", "999")],
        hasMore: true,
        nextCursor: "c1",
      },
      { items: [item(87, "mangabaka", "9741", 17)], hasMore: false, nextCursor: "c2" },
    ]);

    const res = await poll({ sourceId: "src-1" }, rpc, pollDeps(fetchImpl));

    expect(res).toMatchObject({
      parsed: 3,
      matched: 2,
      recorded: 2,
      deduped: 0,
      upstreamStatus: 200,
      etag: "c2",
    });
    expect(cursorsPersisted(calls)).toEqual(["c1", "c2"]);
  });

  it("counts host dedup separately from inserts", async () => {
    const { rpc } = makePollRpc({
      tracked: [{ seriesId: "uuid-a", externalIds: { mangabaka: "9741" } }],
      onRecord: (n) => ({ ledgerId: `l${n}`, deduped: n > 1 }),
    });
    const { fetchImpl } = makeFetchSequence([
      {
        items: [item(87, "mangabaka", "9741", 16), item(87, "mangabaka", "9741", 17)],
        hasMore: false,
        nextCursor: "c1",
      },
    ]);

    const res = await poll({ sourceId: "src-1" }, rpc, pollDeps(fetchImpl));
    expect(res).toMatchObject({ matched: 2, recorded: 1, deduped: 1 });
  });

  it("skips items with no tracked match (no record calls)", async () => {
    const { rpc, calls } = makePollRpc({
      tracked: [{ seriesId: "uuid-a", externalIds: { mangabaka: "9741" } }],
    });
    const { fetchImpl } = makeFetchSequence([
      { items: [item(99, "anilist", "999")], hasMore: false, nextCursor: "c1" },
    ]);

    const res = await poll({ sourceId: "src-1" }, rpc, pollDeps(fetchImpl));
    expect(res).toMatchObject({ parsed: 1, matched: 0, recorded: 0 });
    expect(calls.some((c) => c.method === "releases/record")).toBe(false);
  });

  it("tolerates a record failure without aborting the walk", async () => {
    const { rpc, calls } = makePollRpc({
      tracked: [{ seriesId: "uuid-a", externalIds: { mangabaka: "9741" } }],
      onRecord: () => ({ __error: { code: -32000, message: "ledger down" } }),
    });
    const { fetchImpl } = makeFetchSequence([
      { items: [item(87, "mangabaka", "9741", 16)], hasMore: false, nextCursor: "c1" },
    ]);

    const res = await poll({ sourceId: "src-1" }, rpc, pollDeps(fetchImpl));
    expect(res).toMatchObject({ parsed: 1, matched: 1, recorded: 0, deduped: 0 });
    expect(cursorsPersisted(calls)).toEqual(["c1"]); // walk still completed and advanced the cursor
  });

  it("resumes from the etag the host passed in", async () => {
    const { rpc } = makePollRpc({ tracked: [] });
    const { fetchImpl, urls } = makeFetchSequence([
      { items: [], hasMore: false, nextCursor: null },
    ]);

    await poll({ sourceId: "src-1", etag: "resume-here" }, rpc, pollDeps(fetchImpl));
    expect(new URL(urls[0]).searchParams.get("cursor")).toBe("resume-here");
  });

  it("throws when even the first page can't be fetched (so the source shows last_error)", async () => {
    const { rpc, calls } = makePollRpc({
      tracked: [{ seriesId: "uuid-a", externalIds: { mangabaka: "9741" } }],
    });
    const { fetchImpl } = makeFetchSequence([{ errorStatus: 503 }]);

    await expect(poll({ sourceId: "src-1" }, rpc, pollDeps(fetchImpl))).rejects.toThrow(/503/);
    expect(cursorsPersisted(calls)).toEqual([]); // nothing persisted on a hard failure
  });

  it("stops without throwing on a mid-walk fetch error, keeping prior progress", async () => {
    const { rpc, calls } = makePollRpc({
      tracked: [{ seriesId: "uuid-a", externalIds: { mangabaka: "9741" } }],
    });
    const { fetchImpl } = makeFetchSequence([
      { items: [item(87, "mangabaka", "9741", 16)], hasMore: true, nextCursor: "c1" },
      { errorStatus: 503 },
    ]);

    const res = await poll({ sourceId: "src-1" }, rpc, pollDeps(fetchImpl));
    expect(res).toMatchObject({
      parsed: 1,
      matched: 1,
      recorded: 1,
      upstreamStatus: 503,
      etag: "c1",
    });
    expect(cursorsPersisted(calls)).toEqual(["c1"]);
  });
});
