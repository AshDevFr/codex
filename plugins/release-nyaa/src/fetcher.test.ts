import { describe, expect, it, vi } from "vitest";
import {
  feedUrl,
  fetchSubscriptionFeed,
  parseSubscriptionList,
  parseSubscriptionToken,
} from "./fetcher.js";

// -----------------------------------------------------------------------------
// parseSubscriptionToken / parseSubscriptionList
// -----------------------------------------------------------------------------

describe("parseSubscriptionToken", () => {
  it("returns null for empty / whitespace-only input", () => {
    expect(parseSubscriptionToken("")).toBeNull();
    expect(parseSubscriptionToken("   ")).toBeNull();
  });

  it("treats a bare identifier as a user feed", () => {
    expect(parseSubscriptionToken("1r0n")).toEqual({ kind: "user", identifier: "1r0n" });
  });

  it("treats `q:<query>` as a search query", () => {
    expect(parseSubscriptionToken("q:LuminousScans")).toEqual({
      kind: "query",
      identifier: "LuminousScans",
    });
  });

  it("treats `query:<query>` (long form) as a search query", () => {
    expect(parseSubscriptionToken("query:Manga Group")).toEqual({
      kind: "query",
      identifier: "Manga Group",
    });
  });

  it("rejects an empty query body", () => {
    expect(parseSubscriptionToken("q:")).toBeNull();
    expect(parseSubscriptionToken("query:   ")).toBeNull();
  });
});

describe("parseSubscriptionList", () => {
  it("parses a comma-separated list and dedupes (case-insensitive)", () => {
    const list = parseSubscriptionList("1r0n, TankobonBlur ,1r0n,q:LuminousScans");
    expect(list).toEqual([
      { kind: "user", identifier: "1r0n" },
      { kind: "user", identifier: "TankobonBlur" },
      { kind: "query", identifier: "LuminousScans" },
    ]);
  });

  it("returns an empty list for non-string input", () => {
    expect(parseSubscriptionList(undefined)).toEqual([]);
    expect(parseSubscriptionList(null)).toEqual([]);
    expect(parseSubscriptionList(42)).toEqual([]);
  });

  it("drops empty tokens (trailing comma, double commas)", () => {
    expect(parseSubscriptionList(",,,foo,,,bar,,")).toEqual([
      { kind: "user", identifier: "foo" },
      { kind: "user", identifier: "bar" },
    ]);
  });
});

// -----------------------------------------------------------------------------
// feedUrl
// -----------------------------------------------------------------------------

describe("feedUrl", () => {
  it("builds a user-feed URL", () => {
    const url = feedUrl({ kind: "user", identifier: "1r0n" });
    expect(url).toBe("https://nyaa.si/?page=rss&u=1r0n");
  });

  it("builds a search-feed URL with URL-encoded query", () => {
    const url = feedUrl({ kind: "query", identifier: "Luminous Scans" });
    expect(url).toBe("https://nyaa.si/?page=rss&q=Luminous%20Scans");
  });

  it("respects a custom base URL with trailing slash trimming", () => {
    const url = feedUrl({ kind: "user", identifier: "x" }, "https://mirror.example/");
    expect(url).toBe("https://mirror.example/?page=rss&u=x");
  });
});

// -----------------------------------------------------------------------------
// fetchSubscriptionFeed
// -----------------------------------------------------------------------------

function stubResponse(status: number, body = "", headers: Record<string, string> = {}): Response {
  const h = new Headers(headers);
  return {
    status,
    statusText: "",
    headers: h,
    text: async () => body,
  } as unknown as Response;
}

describe("fetchSubscriptionFeed", () => {
  it("returns ok with body, etag, and last-modified on 200", async () => {
    const fetchImpl = vi.fn().mockResolvedValue(
      stubResponse(200, "<rss></rss>", {
        etag: '"v1"',
        "last-modified": "Mon, 04 May 2026 02:31:00 GMT",
      }),
    );
    const r = await fetchSubscriptionFeed({ kind: "user", identifier: "1r0n" }, null, null, {
      fetchImpl: fetchImpl as unknown as typeof fetch,
    });
    expect(r.kind).toBe("ok");
    if (r.kind !== "ok") return;
    expect(r.body).toBe("<rss></rss>");
    expect(r.etag).toBe('"v1"');
    expect(r.lastModified).toBe("Mon, 04 May 2026 02:31:00 GMT");
  });

  it("returns notModified on 304", async () => {
    const fetchImpl = vi.fn().mockResolvedValue(stubResponse(304));
    const r = await fetchSubscriptionFeed({ kind: "user", identifier: "1r0n" }, '"v1"', null, {
      fetchImpl: fetchImpl as unknown as typeof fetch,
    });
    expect(r.kind).toBe("notModified");
  });

  it("forwards 429 / 5xx as an error result with the upstream status", async () => {
    const fetchImpl = vi.fn().mockResolvedValue(stubResponse(429));
    const r = await fetchSubscriptionFeed({ kind: "user", identifier: "1r0n" }, null, null, {
      fetchImpl: fetchImpl as unknown as typeof fetch,
    });
    expect(r.kind).toBe("error");
    if (r.kind !== "error") return;
    expect(r.status).toBe(429);
  });

  it("returns status=0 on transport error / abort", async () => {
    const fetchImpl = vi.fn().mockRejectedValue(new Error("network down"));
    const r = await fetchSubscriptionFeed({ kind: "user", identifier: "1r0n" }, null, null, {
      fetchImpl: fetchImpl as unknown as typeof fetch,
    });
    expect(r.kind).toBe("error");
    if (r.kind !== "error") return;
    expect(r.status).toBe(0);
    expect(r.message).toContain("network down");
  });

  it("attaches If-None-Match and If-Modified-Since headers when previous values are passed", async () => {
    const fetchImpl = vi.fn().mockResolvedValue(stubResponse(200, "<rss></rss>"));
    await fetchSubscriptionFeed(
      { kind: "user", identifier: "1r0n" },
      '"v1"',
      "Sat, 01 May 2026 00:00:00 GMT",
      { fetchImpl: fetchImpl as unknown as typeof fetch },
    );
    const callArgs = fetchImpl.mock.calls[0];
    expect(callArgs).toBeDefined();
    if (!callArgs) return;
    const [, init] = callArgs as [string, RequestInit];
    const headers = init.headers as Record<string, string>;
    expect(headers["If-None-Match"]).toBe('"v1"');
    expect(headers["If-Modified-Since"]).toBe("Sat, 01 May 2026 00:00:00 GMT");
  });
});
