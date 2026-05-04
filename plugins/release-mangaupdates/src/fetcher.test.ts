import { describe, expect, it, vi } from "vitest";
import { feedUrl, fetchSeriesFeed, MANGAUPDATES_RSS_BASE } from "./fetcher.js";

function mockResponse(status: number, body = "", headers: Record<string, string> = {}): Response {
  // Some status codes (204, 304) can't be set on a constructed `Response`
  // because they're "null body status" codes. We synthesize a minimal
  // duck-typed object instead — only `status`, `statusText`, `headers`, and
  // `text()` are read by `fetchSeriesFeed`.
  const h = new Headers(headers);
  return {
    status,
    statusText: "",
    headers: h,
    text: async () => body,
  } as unknown as Response;
}

describe("feedUrl", () => {
  it("builds the per-series RSS URL", () => {
    expect(feedUrl("12345")).toBe(`${MANGAUPDATES_RSS_BASE}/12345/rss`);
  });
});

describe("fetchSeriesFeed", () => {
  it("sends If-None-Match when a previous ETag is given", async () => {
    const fetchImpl = vi.fn().mockResolvedValue(mockResponse(200, "<rss/>", { etag: '"def"' }));
    await fetchSeriesFeed("99", '"abc"', { fetchImpl });
    const callArgs = fetchImpl.mock.calls[0];
    expect(callArgs).toBeDefined();
    if (!callArgs) return;
    const [url, init] = callArgs as [string, RequestInit];
    expect(url).toBe(feedUrl("99"));
    const headers = init.headers as Record<string, string>;
    expect(headers["If-None-Match"]).toBe('"abc"');
    expect(headers.Accept).toContain("rss");
  });

  it("omits If-None-Match on the first poll (no previous etag)", async () => {
    const fetchImpl = vi.fn().mockResolvedValue(mockResponse(200, "<rss/>"));
    await fetchSeriesFeed("99", null, { fetchImpl });
    const callArgs = fetchImpl.mock.calls[0];
    if (!callArgs) return;
    const headers = (callArgs[1] as RequestInit).headers as Record<string, string>;
    expect(headers["If-None-Match"]).toBeUndefined();
  });

  it("returns ok with body and etag on 200", async () => {
    const fetchImpl = vi
      .fn()
      .mockResolvedValue(mockResponse(200, "<rss>body</rss>", { etag: '"new-etag"' }));
    const result = await fetchSeriesFeed("99", null, { fetchImpl });
    expect(result.kind).toBe("ok");
    if (result.kind !== "ok") return;
    expect(result.body).toBe("<rss>body</rss>");
    expect(result.etag).toBe('"new-etag"');
    expect(result.status).toBe(200);
  });

  it("returns notModified on 304", async () => {
    const fetchImpl = vi.fn().mockResolvedValue(mockResponse(304));
    const result = await fetchSeriesFeed("99", '"abc"', { fetchImpl });
    expect(result.kind).toBe("notModified");
    expect(result.status).toBe(304);
  });

  it("returns error with the upstream status on 429", async () => {
    const fetchImpl = vi.fn().mockResolvedValue(mockResponse(429));
    const result = await fetchSeriesFeed("99", null, { fetchImpl });
    expect(result.kind).toBe("error");
    expect(result.status).toBe(429);
  });

  it("returns error with status 0 on transport-level failure", async () => {
    const fetchImpl = vi.fn().mockRejectedValue(new Error("ECONNRESET"));
    const result = await fetchSeriesFeed("99", null, { fetchImpl });
    expect(result.kind).toBe("error");
    if (result.kind !== "error") return;
    expect(result.status).toBe(0);
    expect(result.message).toContain("ECONNRESET");
  });

  it("returns error with status 0 on timeout (AbortError)", async () => {
    const fetchImpl = vi.fn().mockImplementation((_url, init: RequestInit) => {
      // Simulate an aborted request: throw the same DOMException-like error
      // that real `fetch` raises when the AbortSignal triggers.
      return new Promise((_, reject) => {
        const signal = init.signal as AbortSignal;
        signal.addEventListener("abort", () => {
          reject(new DOMException("aborted", "AbortError"));
        });
      });
    });
    const result = await fetchSeriesFeed("99", null, { fetchImpl, timeoutMs: 10 });
    expect(result.kind).toBe("error");
    if (result.kind !== "error") return;
    expect(result.status).toBe(0);
  });
});
