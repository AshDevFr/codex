import { HostRpcClient, type TrackedSeriesEntry } from "@ashdev/codex-plugin-sdk";
import { describe, expect, it, vi } from "vitest";
import { pollSeries } from "./index.js";
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
});
