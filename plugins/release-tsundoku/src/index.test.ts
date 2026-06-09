import { HostRpcClient } from "@ashdev/codex-plugin-sdk";
import { describe, expect, it, vi } from "vitest";
import type { FeedItem, FeedResponse } from "./fetcher.js";
import { normalizeBaseUrl, poll, registerSources } from "./index.js";

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

  it("issues a single call (no retry) and returns null on failure", async () => {
    // The host's readiness barrier makes registration race-free, so the
    // plugin no longer retries: one call, and a failure surfaces as null.
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

/**
 * A `fetch` impl that returns the given pages/errors in order, then empties.
 * Captures each POST request body so tests can assert the posted filter set
 * and the in-poll pagination cursor.
 */
function makeFetchSequence(pages: PageOrError[]): {
  fetchImpl: typeof fetch;
  bodies: Array<Record<string, unknown>>;
} {
  const bodies: Array<Record<string, unknown>> = [];
  let i = 0;
  const fetchImpl = vi.fn(async (_url: string, init?: RequestInit) => {
    bodies.push(init?.body ? JSON.parse(init.body as string) : {});
    const page = pages[i++] ?? { items: [], hasMore: false, nextCursor: null };
    if ("errorStatus" in page) {
      return new Response("err", { status: page.errorStatus });
    }
    return new Response(JSON.stringify(page), {
      status: 200,
      headers: { "content-type": "application/json" },
    });
  }) as unknown as typeof fetch;
  return { fetchImpl, bodies };
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

const pollDeps = (fetchImpl: typeof fetch) => ({
  baseUrl: "https://t.example.com",
  language: "en",
  pageLimit: 100,
  timeoutMs: 5_000,
  fetchImpl,
});

describe("poll", () => {
  it("posts the tracked external-id filter, walks pages, and records matches", async () => {
    const { rpc } = makePollRpc({
      tracked: [
        { seriesId: "uuid-a", externalIds: { mangabaka: "9741" } },
        { seriesId: "uuid-b", externalIds: { mangabaka: "5555" } },
      ],
    });
    const { fetchImpl, bodies } = makeFetchSequence([
      {
        items: [item(87, "mangabaka", "9741", 16), item(99, "anilist", "999")],
        hasMore: true,
        nextCursor: "c1",
      },
      { items: [item(88, "mangabaka", "5555", 3)], hasMore: false, nextCursor: "c2" },
    ]);

    const res = await poll({ sourceId: "src-1" }, rpc, pollDeps(fetchImpl));

    expect(res).toMatchObject({
      parsed: 3, // 87, 99, 88
      matched: 2, // 87 -> uuid-a, 88 -> uuid-b; 99 unmatched
      recorded: 2,
      deduped: 0,
      upstreamStatus: 200,
    });
    // Posted the tracked external-id filter, and paginated within the poll
    // (first page cursor null, second page the prior response's nextCursor).
    expect(new Set(bodies[0].externalIds as string[])).toEqual(
      new Set(["mangabaka:9741", "mangabaka:5555"]),
    );
    expect(bodies[0].cursor).toBeNull();
    expect(bodies[1].cursor).toBe("c1");
  });

  it("skips the fetch entirely when no tracked series carry a known id", async () => {
    const { rpc } = makePollRpc({ tracked: [] });
    const { fetchImpl, bodies } = makeFetchSequence([{ items: [], hasMore: false }]);

    const res = await poll({ sourceId: "src-1" }, rpc, pollDeps(fetchImpl));
    expect(res).toMatchObject({ parsed: 0, matched: 0, recorded: 0 });
    // Never POSTed — an empty filter would mean "whole catalog" upstream.
    expect(bodies).toHaveLength(0);
  });

  it("counts host dedup separately from inserts", async () => {
    const { rpc } = makePollRpc({
      tracked: [{ seriesId: "uuid-a", externalIds: { mangabaka: "9741" } }],
      onRecord: () => ({ ledgerId: "l1", deduped: true }),
    });
    const { fetchImpl } = makeFetchSequence([
      { items: [item(87, "mangabaka", "9741", 16)], hasMore: false, nextCursor: "c1" },
    ]);

    const res = await poll({ sourceId: "src-1" }, rpc, pollDeps(fetchImpl));
    expect(res).toMatchObject({ matched: 1, recorded: 0, deduped: 1 });
  });

  it("resolves a collision to the highest-scoring feed entry", async () => {
    // Two different Tsundoku series both map to uuid-a: #87 via mangabaka
    // (score 3) and #88 via mal (score 1). The mangabaka match wins; the mal
    // one is superseded, so only one record call is made — for series #87.
    const { rpc, calls } = makePollRpc({
      tracked: [{ seriesId: "uuid-a", externalIds: { mangabaka: "9741", mal: "555" } }],
    });
    const { fetchImpl } = makeFetchSequence([
      {
        items: [item(87, "mangabaka", "9741", 16), item(88, "mal", "555", 9)],
        hasMore: false,
        nextCursor: "c1",
      },
    ]);

    const res = await poll({ sourceId: "src-1" }, rpc, pollDeps(fetchImpl));
    expect(res).toMatchObject({ parsed: 2, matched: 1, recorded: 1 });

    const recordCalls = calls.filter((c) => c.method === "releases/record");
    expect(recordCalls).toHaveLength(1);
    const candidate = (recordCalls[0].params as { candidate: { externalReleaseId: string } })
      .candidate;
    expect(candidate.externalReleaseId).toContain("tsundoku:87:");
  });

  it("skips an ambiguous collision (different series tie at the same score)", async () => {
    // Both #87 and #88 match uuid-a only via the same low-trust mal id (score
    // 1 each) — genuinely ambiguous, so neither is recorded.
    const { rpc, calls } = makePollRpc({
      tracked: [{ seriesId: "uuid-a", externalIds: { mal: "555" } }],
    });
    const { fetchImpl } = makeFetchSequence([
      {
        items: [item(87, "mal", "555", 16), item(88, "mal", "555", 9)],
        hasMore: false,
        nextCursor: "c1",
      },
    ]);

    const res = await poll({ sourceId: "src-1" }, rpc, pollDeps(fetchImpl));
    expect(res).toMatchObject({ parsed: 2, matched: 0, recorded: 0 });
    expect(calls.some((c) => c.method === "releases/record")).toBe(false);
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
    const { rpc } = makePollRpc({
      tracked: [{ seriesId: "uuid-a", externalIds: { mangabaka: "9741" } }],
      onRecord: () => ({ __error: { code: -32000, message: "ledger down" } }),
    });
    const { fetchImpl } = makeFetchSequence([
      { items: [item(87, "mangabaka", "9741", 16)], hasMore: false, nextCursor: "c1" },
    ]);

    const res = await poll({ sourceId: "src-1" }, rpc, pollDeps(fetchImpl));
    expect(res).toMatchObject({ parsed: 1, matched: 1, recorded: 0, deduped: 0 });
  });

  it("throws when even the first page can't be fetched (so the source shows last_error)", async () => {
    const { rpc } = makePollRpc({
      tracked: [{ seriesId: "uuid-a", externalIds: { mangabaka: "9741" } }],
    });
    const { fetchImpl } = makeFetchSequence([{ errorStatus: 503 }]);

    await expect(poll({ sourceId: "src-1" }, rpc, pollDeps(fetchImpl))).rejects.toThrow(/503/);
  });

  it("stops without throwing on a mid-walk fetch error, keeping prior progress", async () => {
    const { rpc } = makePollRpc({
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
    });
  });
});
