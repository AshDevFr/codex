import { HostRpcClient, HostRpcError } from "@ashdev/codex-plugin-sdk";
import { describe, expect, it, vi } from "vitest";
import { pollSubscription, registerSources } from "./index.js";
import type { AliasCandidate } from "./matcher.js";

// -----------------------------------------------------------------------------
// Helpers — mirrors the makeMockRpc shape used by release-mangaupdates so the
// two suites stay readable side-by-side.
// -----------------------------------------------------------------------------

interface CapturedCall {
  method: string;
  params: unknown;
}

function makeMockRpc(respond: (method: string, params: unknown) => unknown): {
  rpc: HostRpcClient;
  calls: CapturedCall[];
} {
  const calls: CapturedCall[] = [];
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
      // Preserve HostRpcError.code so tests can simulate METHOD_NOT_FOUND etc.
      const code = err instanceof HostRpcError ? err.code : -32_000;
      error = {
        code,
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
// Fixtures — uses the user's 1r0n example shapes.
// -----------------------------------------------------------------------------

const uploaderFeedXml = `<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0" xmlns:nyaa="https://nyaa.si/xmlns/nyaa">
  <channel>
    <item>
      <title><![CDATA[[1r0n] Boruto - Two Blue Vortex - Volume 02 (Digital) (1r0n)]]></title>
      <link>https://nyaa.si/download/1.torrent</link>
      <guid isPermaLink="true">https://nyaa.si/view/1</guid>
      <pubDate>Mon, 04 May 2026 02:31:00 GMT</pubDate>
      <nyaa:infoHash>aaa</nyaa:infoHash>
    </item>
    <item>
      <title><![CDATA[[1r0n] Dandadan c126-142 (Digital)]]></title>
      <link>https://nyaa.si/download/2.torrent</link>
      <guid isPermaLink="true">https://nyaa.si/view/2</guid>
      <pubDate>Sun, 03 May 2026 12:00:00 GMT</pubDate>
      <nyaa:infoHash>bbb</nyaa:infoHash>
    </item>
    <item>
      <title><![CDATA[[1r0n] Some Untracked Series v1 (Digital)]]></title>
      <link>https://nyaa.si/download/3.torrent</link>
      <guid isPermaLink="true">https://nyaa.si/view/3</guid>
      <pubDate>Sat, 02 May 2026 22:00:00 GMT</pubDate>
      <nyaa:infoHash>ccc</nyaa:infoHash>
    </item>
  </channel>
</rss>`;

const trackedCandidates: AliasCandidate[] = [
  { seriesId: "s-boruto", aliases: ["Boruto: Two Blue Vortex", "Boruto Two Blue Vortex"] },
  { seriesId: "s-dandadan", aliases: ["Dandadan", "ダンダダン"] },
];

// -----------------------------------------------------------------------------
// pollSubscription
// -----------------------------------------------------------------------------

describe("pollSubscription", () => {
  it("matches and records candidates for tracked series, skipping untracked", async () => {
    const { rpc, calls } = makeMockRpc(() => ({ ledgerId: "ld", deduped: false }));
    const out = await pollSubscription(
      rpc,
      "src-1",
      { kind: "user", identifier: "1r0n" },
      trackedCandidates,
      {
        previousEtag: null,
        timeoutMs: 1000,
        minConfidence: 0.7,
        fetchImpl: mockFetchOk(uploaderFeedXml, '"new-etag"'),
      },
    );
    expect(out.fetched).toBe(true);
    expect(out.notModified).toBe(false);
    expect(out.parsed).toBe(3);
    // Boruto + Dandadan match; "Some Untracked Series" doesn't.
    expect(out.matched).toBe(2);
    expect(out.recorded).toBe(2);
    expect(out.etag).toBe('"new-etag"');

    const recordCalls = calls.filter((c) => c.method === "releases/record");
    expect(recordCalls).toHaveLength(2);
    const matched = recordCalls.map((c) => {
      const p = c.params as { candidate: { seriesMatch: { codexSeriesId: string } } };
      return p.candidate.seriesMatch.codexSeriesId;
    });
    expect(matched.sort()).toEqual(["s-boruto", "s-dandadan"]);
  });

  it("returns notModified when upstream replies 304", async () => {
    const { rpc, calls } = makeMockRpc(() => ({ ledgerId: "x", deduped: false }));
    const fetchImpl = vi.fn().mockResolvedValue(stubResponse(304));
    const out = await pollSubscription(
      rpc,
      "src-1",
      { kind: "user", identifier: "1r0n" },
      trackedCandidates,
      {
        previousEtag: '"v1"',
        timeoutMs: 1000,
        minConfidence: 0.7,
        fetchImpl: fetchImpl as unknown as typeof fetch,
      },
    );
    expect(out.notModified).toBe(true);
    expect(out.parsed).toBe(0);
    expect(out.matched).toBe(0);
    expect(out.upstreamStatus).toBe(304);
    expect(calls.filter((c) => c.method === "releases/record")).toHaveLength(0);
  });

  it("propagates upstream 429 status without recording", async () => {
    const { rpc, calls } = makeMockRpc(() => ({ ledgerId: "x", deduped: false }));
    const fetchImpl = vi.fn().mockResolvedValue(stubResponse(429));
    const out = await pollSubscription(
      rpc,
      "src-1",
      { kind: "user", identifier: "1r0n" },
      trackedCandidates,
      {
        previousEtag: null,
        timeoutMs: 1000,
        minConfidence: 0.7,
        fetchImpl: fetchImpl as unknown as typeof fetch,
      },
    );
    expect(out.fetched).toBe(false);
    expect(out.upstreamStatus).toBe(429);
    expect(calls.filter((c) => c.method === "releases/record")).toHaveLength(0);
  });

  it("attaches infoHash and format hints to the candidate payload", async () => {
    const { rpc, calls } = makeMockRpc(() => ({ ledgerId: "ld", deduped: false }));
    const fetchImpl = mockFetchOk(uploaderFeedXml);
    await pollSubscription(rpc, "src-1", { kind: "user", identifier: "1r0n" }, trackedCandidates, {
      previousEtag: null,
      timeoutMs: 1000,
      minConfidence: 0.7,
      fetchImpl,
    });
    const recordCalls = calls.filter((c) => c.method === "releases/record");
    const boruto = recordCalls.find((c) => {
      const p = c.params as { candidate: { seriesMatch: { codexSeriesId: string } } };
      return p.candidate.seriesMatch.codexSeriesId === "s-boruto";
    });
    expect(boruto).toBeDefined();
    if (!boruto) return;
    const params = boruto.params as {
      candidate: {
        infoHash: string | null;
        formatHints: Record<string, unknown>;
        volume: number | null;
        payloadUrl: string;
        mediaUrl?: string | null;
        mediaUrlKind?: string | null;
      };
    };
    expect(params.candidate.infoHash).toBe("aaa");
    expect(params.candidate.formatHints.digital).toBe(true);
    expect(params.candidate.formatHints.subscription).toBe("user:1r0n");
    expect(params.candidate.volume).toBe(2);
    // Page url -> payloadUrl, .torrent -> mediaUrl with kind=torrent.
    expect(params.candidate.payloadUrl).toBe("https://nyaa.si/view/1");
    expect(params.candidate.mediaUrl).toBe("https://nyaa.si/download/1.torrent");
    expect(params.candidate.mediaUrlKind).toBe("torrent");
  });

  it("falls back to torrent link as payloadUrl when guid permalink is missing", async () => {
    const noPermalinkXml = `<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0" xmlns:nyaa="https://nyaa.si/xmlns/nyaa">
  <channel>
    <item>
      <title><![CDATA[[1r0n] Dandadan c126-142 (Digital)]]></title>
      <link>https://nyaa.si/download/99.torrent</link>
      <guid>nyaa-99</guid>
      <pubDate>Sun, 03 May 2026 12:00:00 GMT</pubDate>
      <nyaa:infoHash>zzz</nyaa:infoHash>
    </item>
  </channel>
</rss>`;
    const { rpc, calls } = makeMockRpc(() => ({ ledgerId: "ld", deduped: false }));
    await pollSubscription(rpc, "src-1", { kind: "user", identifier: "1r0n" }, trackedCandidates, {
      previousEtag: null,
      timeoutMs: 1000,
      minConfidence: 0.7,
      fetchImpl: mockFetchOk(noPermalinkXml),
    });
    const record = calls.find((c) => c.method === "releases/record");
    expect(record).toBeDefined();
    const cand = (record?.params as { candidate: Record<string, unknown> }).candidate;
    expect(cand.payloadUrl).toBe("https://nyaa.si/download/99.torrent");
    // Both fields would point at the same URL — skip the duplicate.
    expect(cand.mediaUrl).toBeUndefined();
    expect(cand.mediaUrlKind).toBeUndefined();
  });

  it("counts deduped records as not-newly-recorded", async () => {
    const { rpc } = makeMockRpc(() => ({ ledgerId: "ld", deduped: true }));
    const fetchImpl = mockFetchOk(uploaderFeedXml);
    const out = await pollSubscription(
      rpc,
      "src-1",
      { kind: "user", identifier: "1r0n" },
      trackedCandidates,
      {
        previousEtag: null,
        timeoutMs: 1000,
        minConfidence: 0.7,
        fetchImpl,
      },
    );
    expect(out.matched).toBe(2);
    expect(out.recorded).toBe(0);
  });

  it("skips items with no alias match without recording", async () => {
    const { rpc, calls } = makeMockRpc(() => ({ ledgerId: "x", deduped: false }));
    const fetchImpl = mockFetchOk(uploaderFeedXml);
    const out = await pollSubscription(
      rpc,
      "src-1",
      { kind: "user", identifier: "1r0n" },
      [{ seriesId: "s-other", aliases: ["Completely Unrelated Manga"] }],
      {
        previousEtag: null,
        timeoutMs: 1000,
        minConfidence: 0.7,
        fetchImpl,
      },
    );
    expect(out.parsed).toBe(3);
    expect(out.matched).toBe(0);
    expect(out.recorded).toBe(0);
    expect(calls.filter((c) => c.method === "releases/record")).toHaveLength(0);
  });
});

// -----------------------------------------------------------------------------
// registerSources
// -----------------------------------------------------------------------------

describe("registerSources", () => {
  it("emits one source per subscription with stable kind:identifier keys", async () => {
    const { rpc, calls } = makeMockRpc(() => ({ registered: 3, pruned: 0 }));
    const result = await registerSources(rpc, [
      { kind: "user", identifier: "tsuna69" },
      { kind: "query", identifier: "LuminousScans" },
      { kind: "params", identifier: "c=3_1&q=Berserk" },
    ]);
    expect(result).toEqual({ registered: 3, pruned: 0 });

    const reg = calls.find((c) => c.method === "releases/register_sources");
    expect(reg).toBeDefined();
    if (!reg) return;
    const payload = reg.params as {
      sources: { sourceKey: string; displayName: string; kind: string; config: unknown }[];
    };
    const keys = payload.sources.map((s) => s.sourceKey);
    expect(keys).toEqual(["user:tsuna69", "query:luminousscans", "params:c=3_1&q=berserk"]);
    expect(payload.sources.every((s) => s.kind === "rss-uploader")).toBe(true);
    // Round-trip data: config carries the original (case-preserving) subscription.
    const userSrc = payload.sources[0];
    expect(
      (userSrc?.config as { subscription: { identifier: string } }).subscription.identifier,
    ).toBe("tsuna69");
  });

  it("retries on METHOD_NOT_FOUND while the host installs the handler", async () => {
    let calls = 0;
    const { rpc } = makeMockRpc(() => {
      calls++;
      if (calls < 3) {
        throw new HostRpcError("Method not found", -32601);
      }
      return { registered: 1, pruned: 0 };
    });
    const result = await registerSources(rpc, [{ kind: "user", identifier: "a" }]);
    expect(result).toEqual({ registered: 1, pruned: 0 });
    expect(calls).toBe(3);
  });

  it("does not retry on non-method-not-found errors", async () => {
    let calls = 0;
    const { rpc } = makeMockRpc(() => {
      calls++;
      throw new HostRpcError("server boom", -32000);
    });
    const result = await registerSources(rpc, [{ kind: "user", identifier: "a" }]);
    expect(result).toBeNull();
    expect(calls).toBe(1);
  });

  it("sends an empty list when no subscriptions are configured (host wipes plugin's rows)", async () => {
    const { rpc, calls } = makeMockRpc(() => ({ registered: 0, pruned: 2 }));
    const result = await registerSources(rpc, []);
    expect(result).toEqual({ registered: 0, pruned: 2 });
    const reg = calls.find((c) => c.method === "releases/register_sources");
    expect(reg).toBeDefined();
    expect((reg?.params as { sources: unknown[] }).sources).toEqual([]);
  });
});
