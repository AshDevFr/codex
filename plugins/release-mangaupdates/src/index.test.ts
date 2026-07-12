import { HostRpcClient, type TrackedSeriesEntry } from "@ashdev/codex-plugin-sdk";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { _resetState, poll, pollSeries } from "./index.js";
import { EXTERNAL_ID_SOURCE_MANGAUPDATES } from "./manifest.js";

// -----------------------------------------------------------------------------
// Helpers
// -----------------------------------------------------------------------------

function trackedEntry(seriesId: string, muId: string | null = "12345"): TrackedSeriesEntry {
  return {
    seriesId,
    ...(muId
      ? { externalIds: { [EXTERNAL_ID_SOURCE_MANGAUPDATES]: muId } as Record<string, string> }
      : {}),
  };
}

const multilingualFeedXml = `<?xml version="1.0"?>
<rss version="2.0">
  <channel>
    <item>
      <title><![CDATA[c.142 by EnglishGroup (en)]]></title>
      <link>https://www.mangaupdates.com/release.html?id=1001</link>
      <guid isPermaLink="false">1001</guid>
      <pubDate>Mon, 04 May 2026 01:00:00 GMT</pubDate>
    </item>
    <item>
      <title><![CDATA[c.144 by SpanishGroup (es)]]></title>
      <link>https://www.mangaupdates.com/release.html?id=1002</link>
      <guid isPermaLink="false">1002</guid>
      <pubDate>Sun, 03 May 2026 12:00:00 GMT</pubDate>
    </item>
    <item>
      <title><![CDATA[c.145 by BlockedGroup (en)]]></title>
      <link>https://www.mangaupdates.com/release.html?id=1003</link>
      <guid isPermaLink="false">1003</guid>
      <pubDate>Sat, 02 May 2026 22:00:00 GMT</pubDate>
    </item>
  </channel>
</rss>`;

interface CapturedCall {
  method: string;
  params: unknown;
}

/**
 * Build an `HostRpcClient` whose calls are intercepted in-memory. Each call
 * is recorded; the response is provided by `respond`.
 */
function makeMockRpc(respond: (method: string, params: unknown) => unknown): {
  rpc: HostRpcClient;
  calls: CapturedCall[];
} {
  const calls: CapturedCall[] = [];
  // We bypass the wire format entirely: provide a custom `writeFn` that
  // captures the request, then synthesize a matching response and feed it
  // back via `handleResponse`. This exercises the real id-correlation path.
  // eslint-disable-next-line prefer-const
  let rpc: HostRpcClient;
  const writeFn = (line: string) => {
    const req = JSON.parse(line.trim()) as {
      id: number;
      method: string;
      params: unknown;
    };
    calls.push({ method: req.method, params: req.params });
    let result: unknown;
    let error: { code: number; message: string } | null = null;
    try {
      result = respond(req.method, req.params);
    } catch (err) {
      error = {
        code: -32_000,
        message: err instanceof Error ? err.message : "synthetic error",
      };
    }
    setImmediate(() => {
      const payload = error
        ? { jsonrpc: "2.0", id: req.id, error }
        : { jsonrpc: "2.0", id: req.id, result };
      rpc.handleResponse(JSON.stringify(payload));
    });
  };
  rpc = new HostRpcClient(writeFn);
  return { rpc, calls };
}

function mockFetchOk(body: string, etag?: string): typeof fetch {
  return vi.fn().mockResolvedValue(
    new Response(body, {
      status: 200,
      headers: etag ? { etag } : {},
    }),
  ) as unknown as typeof fetch;
}

/**
 * Build a minimal Response-like stub. `Response`'s constructor refuses some
 * status codes (304, 204) since they're "null body status" codes. We only
 * need a handful of fields to drive `fetcher.ts`.
 */
function stubResponse(status: number, body = "", headers: Record<string, string> = {}): Response {
  const h = new Headers(headers);
  return {
    status,
    statusText: "",
    headers: h,
    text: async () => body,
  } as unknown as Response;
}

// -----------------------------------------------------------------------------
// pollSeries
// -----------------------------------------------------------------------------

describe("pollSeries", () => {
  it("skips a series that has no MangaUpdates external ID", async () => {
    const { rpc, calls } = makeMockRpc(() => ({ ledgerId: "x", deduped: false }));
    const out = await pollSeries(rpc, "src-1", trackedEntry("series-1", null), {
      blockedGroups: [],
      timeoutMs: 1000,
      fetchImpl: vi.fn() as unknown as typeof fetch, // never called
    });
    expect(out.fetched).toBe(false);
    expect(out.error).toContain("missing mangaupdates external ID");
    expect(calls).toHaveLength(0);
  });

  it("records candidates for in-language items and skips blocked groups", async () => {
    const { rpc, calls } = makeMockRpc(() => ({ ledgerId: "ld", deduped: false }));
    const out = await pollSeries(rpc, "src-1", trackedEntry("series-1", "999"), {
      // Effective languages from the host gate is empty here (the plugin's
      // client-side language filter is a host-handoff stub today). The
      // group blocklist still applies.
      blockedGroups: ["BlockedGroup"],
      timeoutMs: 1000,
      fetchImpl: mockFetchOk(multilingualFeedXml, '"new-etag"'),
    });
    expect(out.fetched).toBe(true);
    expect(out.notModified).toBe(false);
    expect(out.parsed).toBe(3); // 3 items in the fixture
    // Blocked group dropped, the other two are recorded.
    expect(out.recorded).toBe(2);
    expect(out.etag).toBe('"new-etag"');

    const recordCalls = calls.filter((c) => c.method === "releases/record");
    expect(recordCalls).toHaveLength(2);
    const groups = recordCalls.map((c) => {
      const params = c.params as { candidate: { groupOrUploader: string | null } };
      return params.candidate.groupOrUploader;
    });
    expect(groups).toEqual(["EnglishGroup", "SpanishGroup"]);
  });

  it("returns notModified when upstream replies 304", async () => {
    const { rpc } = makeMockRpc(() => ({ ledgerId: "x", deduped: false }));
    const fetchImpl = vi.fn().mockResolvedValue(stubResponse(304));
    const out = await pollSeries(rpc, "src-1", trackedEntry("series-1"), {
      blockedGroups: [],
      timeoutMs: 1000,
      fetchImpl: fetchImpl as unknown as typeof fetch,
    });
    expect(out.notModified).toBe(true);
    expect(out.parsed).toBe(0);
    expect(out.recorded).toBe(0);
    expect(out.upstreamStatus).toBe(304);
  });

  it("propagates upstream 429 status without recording", async () => {
    const { rpc, calls } = makeMockRpc(() => ({ ledgerId: "x", deduped: false }));
    const fetchImpl = vi.fn().mockResolvedValue(stubResponse(429));
    const out = await pollSeries(rpc, "src-1", trackedEntry("series-1"), {
      blockedGroups: [],
      timeoutMs: 1000,
      fetchImpl: fetchImpl as unknown as typeof fetch,
    });
    expect(out.fetched).toBe(false);
    expect(out.upstreamStatus).toBe(429);
    expect(calls.filter((c) => c.method === "releases/record")).toHaveLength(0);
  });

  it("survives a record() error and continues to next item", async () => {
    let recordCalls = 0;
    const { rpc } = makeMockRpc((method) => {
      if (method === "releases/record") {
        recordCalls++;
        if (recordCalls === 1) {
          // Synthesize a JSON-RPC error for the first record() call. The
          // mock writeFn catches the throw and turns it into an `error`
          // response, mimicking what the host would emit on rejection.
          throw new Error("simulated host error");
        }
      }
      return { ledgerId: "ld", deduped: false };
    });
    const fetchImpl = mockFetchOk(multilingualFeedXml, '"e"');
    const out = await pollSeries(rpc, "src-1", trackedEntry("series-1"), {
      blockedGroups: [],
      timeoutMs: 1000,
      fetchImpl,
    });
    // 3 items parsed; first record() failed so recorded reflects only the
    // remaining two successful inserts.
    expect(out.parsed).toBe(3);
    expect(out.recorded).toBe(2);
  });

  it("counts deduped records as not-newly-recorded", async () => {
    const { rpc } = makeMockRpc(() => ({ ledgerId: "ld", deduped: true }));
    const fetchImpl = mockFetchOk(multilingualFeedXml, '"e"');
    const out = await pollSeries(rpc, "src-1", trackedEntry("series-1"), {
      blockedGroups: [],
      timeoutMs: 1000,
      fetchImpl,
    });
    expect(out.parsed).toBe(3);
    expect(out.recorded).toBe(0); // every record returned deduped:true
  });

  it("uses the channel-level <link> as payloadUrl on the v1 RSS feed shape", async () => {
    // The current MU v1 feed has no per-item <link>. The plugin should
    // fall through to the channel-level link (the series page) rather
    // than emitting an opaque `urn:mu:` URN, which is useless for the
    // user clicking through from the inbox.
    const v1Feed = `<?xml version="1.0"?>
      <rss version="2.0">
        <channel>
          <link>https://www.mangaupdates.com/series/uu4rl66/series-slug</link>
          <item>
            <title>Series v.13 c.116</title>
            <description>Galaxy Degen Scans</description>
          </item>
          <item>
            <title>Series c.113a</title>
            <description>Comikey</description>
          </item>
          <item>
            <title>Series</title>
            <description>OneshotGroup</description>
          </item>
        </channel>
      </rss>`;
    const { rpc, calls } = makeMockRpc(() => ({ ledgerId: "ld", deduped: false }));
    const out = await pollSeries(rpc, "src-1", trackedEntry("series-1"), {
      blockedGroups: [],
      timeoutMs: 1000,
      fetchImpl: mockFetchOk(v1Feed),
    });
    // Title-only item (no chapter/volume) is dropped before reaching record().
    expect(out.parsed).toBe(2);
    expect(out.recorded).toBe(2);

    const recordCalls = calls.filter((c) => c.method === "releases/record");
    expect(recordCalls).toHaveLength(2);
    for (const call of recordCalls) {
      const params = call.params as { candidate: { payloadUrl: string } };
      expect(params.candidate.payloadUrl).toBe(
        "https://www.mangaupdates.com/series/uu4rl66/series-slug",
      );
    }
  });

  it("emits distinct candidates for the same chapter from different groups", async () => {
    // Three groups releasing the same chapter must surface as three
    // ledger rows. The externalReleaseId hash includes the group so the
    // host's `(source_id, external_release_id)` dedup doesn't collapse
    // them into one.
    const sameChapterDifferentGroups = `<?xml version="1.0"?>
      <rss version="2.0">
        <channel>
          <link>https://www.mangaupdates.com/series/abc/series</link>
          <item>
            <title>Series c.200</title>
            <description>Asura</description>
          </item>
          <item>
            <title>Series c.200</title>
            <description>FLAME-SCANS</description>
          </item>
          <item>
            <title>Series c.200</title>
            <description>LeviatanScans</description>
          </item>
        </channel>
      </rss>`;
    const { rpc, calls } = makeMockRpc(() => ({ ledgerId: "ld", deduped: false }));
    const out = await pollSeries(rpc, "src-1", trackedEntry("series-1"), {
      blockedGroups: [],
      timeoutMs: 1000,
      fetchImpl: mockFetchOk(sameChapterDifferentGroups),
    });
    expect(out.parsed).toBe(3);
    expect(out.recorded).toBe(3);

    const ids = calls
      .filter((c) => c.method === "releases/record")
      .map(
        (c) =>
          (c.params as { candidate: { externalReleaseId: string } }).candidate.externalReleaseId,
      );
    expect(new Set(ids).size).toBe(3);
  });
});

// -----------------------------------------------------------------------------
// poll (top-level): count_tracked + report_progress integration
// -----------------------------------------------------------------------------

describe("poll", () => {
  beforeEach(() => {
    _resetState();
  });

  it("calls count_tracked once and report_progress per series with the right denominator", async () => {
    // Two tracked series, both with MU IDs, both upstream-200 with one item.
    const tracked: TrackedSeriesEntry[] = [
      {
        seriesId: "series-1",
        externalIds: { [EXTERNAL_ID_SOURCE_MANGAUPDATES]: "11111" } as Record<string, string>,
      },
      {
        seriesId: "series-2",
        externalIds: { [EXTERNAL_ID_SOURCE_MANGAUPDATES]: "22222" } as Record<string, string>,
      },
    ];
    const v1Feed = `<?xml version="1.0"?>
      <rss version="2.0">
        <channel>
          <link>https://www.mangaupdates.com/series/abc/series</link>
          <item>
            <title>Series c.1</title>
            <description>SomeGroup</description>
          </item>
        </channel>
      </rss>`;

    const { rpc, calls } = makeMockRpc((method) => {
      if (method === "releases/count_tracked") return { total: tracked.length };
      if (method === "releases/list_tracked") return { tracked, nextOffset: undefined };
      if (method === "releases/record") return { ledgerId: "ld", deduped: false };
      if (method === "releases/report_progress") return { emitted: true };
      throw new Error(`unexpected method: ${method}`);
    });

    // Initialize plugin state directly (bypass createReleaseSourcePlugin).
    // The SDK normally injects state through `onInitialize`; for this test
    // we only need the RPC client wired up, since `poll` reads `state.*`
    // for blocked groups + timeout but works fine with the defaults.
    //
    // `Response.text()` consumes the body, so each `fetch` call needs a
    // fresh `Response` — `mockImplementation` returns a new instance each
    // invocation.
    const fetchImpl = vi
      .fn()
      .mockImplementation(
        async () => new Response(v1Feed, { status: 200, headers: { etag: '"e"' } }),
      ) as unknown as typeof fetch;
    // Replace global fetch for this test so pollSeries -> fetcher uses it.
    const origFetch = globalThis.fetch;
    globalThis.fetch = fetchImpl;
    try {
      await poll({ sourceId: "src-1", sourceKey: "default", config: null, etag: null }, rpc);
    } finally {
      globalThis.fetch = origFetch;
    }

    const countCalls = calls.filter((c) => c.method === "releases/count_tracked");
    expect(countCalls).toHaveLength(1);
    expect((countCalls[0].params as { sourceId: string }).sourceId).toBe("src-1");

    const progressCalls = calls.filter((c) => c.method === "releases/report_progress");
    // One emit per tracked series: 2 total. Denominator equals count.
    expect(progressCalls).toHaveLength(2);
    expect(progressCalls[0]?.params).toMatchObject({ current: 1, total: 2 });
    expect(progressCalls[1]?.params).toMatchObject({ current: 2, total: 2 });
  });

  it("falls back to progressive denominator when count_tracked is unsupported", async () => {
    // Older host: count_tracked returns METHOD_NOT_FOUND. The plugin
    // should keep working and emit progress with `total = current`.
    const tracked: TrackedSeriesEntry[] = [
      {
        seriesId: "series-1",
        externalIds: { [EXTERNAL_ID_SOURCE_MANGAUPDATES]: "11111" } as Record<string, string>,
      },
    ];
    const v1Feed = `<?xml version="1.0"?>
      <rss version="2.0">
        <channel>
          <link>https://www.mangaupdates.com/series/abc/series</link>
          <item><title>Series c.1</title><description>G</description></item>
        </channel>
      </rss>`;

    const { rpc, calls } = makeMockRpc((method) => {
      if (method === "releases/count_tracked") {
        // Synthesize a JSON-RPC METHOD_NOT_FOUND error.
        const err = Object.assign(new Error("Method not found"), { code: -32_601 });
        // Throwing inside `respond` is captured by the mock writeFn and
        // surfaced as an error response; HostRpcClient wraps it in
        // HostRpcError which the plugin catches.
        throw err;
      }
      if (method === "releases/list_tracked") return { tracked, nextOffset: undefined };
      if (method === "releases/record") return { ledgerId: "ld", deduped: false };
      if (method === "releases/report_progress") return { emitted: true };
      throw new Error(`unexpected method: ${method}`);
    });

    const fetchImpl = vi
      .fn()
      .mockResolvedValue(
        new Response(v1Feed, { status: 200, headers: { etag: '"e"' } }),
      ) as unknown as typeof fetch;
    const origFetch = globalThis.fetch;
    globalThis.fetch = fetchImpl;
    try {
      await poll({ sourceId: "src-1", sourceKey: "default", config: null, etag: null }, rpc);
    } finally {
      globalThis.fetch = origFetch;
    }

    const progressCalls = calls.filter((c) => c.method === "releases/report_progress");
    expect(progressCalls).toHaveLength(1);
    // No total known => current == total.
    expect(progressCalls[0]?.params).toMatchObject({ current: 1, total: 1 });
  });
});
