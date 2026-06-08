import { describe, expect, it, vi } from "vitest";
import { type FeedResponse, feedUrl, fetchFeedPage } from "./fetcher.js";

// -----------------------------------------------------------------------------
// feedUrl
// -----------------------------------------------------------------------------

describe("feedUrl", () => {
  it("appends the feed path and limit", () => {
    const url = feedUrl("https://t.example.com", null, 100);
    expect(url).toBe("https://t.example.com/api/v1/series/feed?limit=100");
  });

  it("includes the cursor when provided", () => {
    const url = feedUrl("https://t.example.com", "abc123", 50);
    const parsed = new URL(url);
    expect(parsed.pathname).toBe("/api/v1/series/feed");
    expect(parsed.searchParams.get("limit")).toBe("50");
    expect(parsed.searchParams.get("cursor")).toBe("abc123");
  });

  it("omits the cursor param when null or empty", () => {
    expect(feedUrl("https://t.example.com", "", 10)).not.toContain("cursor=");
    expect(feedUrl("https://t.example.com", null, 10)).not.toContain("cursor=");
  });

  it("strips trailing slashes from the base URL", () => {
    expect(feedUrl("https://t.example.com///", null, 10)).toBe(
      "https://t.example.com/api/v1/series/feed?limit=10",
    );
  });

  it("url-encodes an opaque cursor", () => {
    const url = feedUrl("https://t.example.com", "a b/c+d", 10);
    expect(new URL(url).searchParams.get("cursor")).toBe("a b/c+d");
  });
});

// -----------------------------------------------------------------------------
// fetchFeedPage
// -----------------------------------------------------------------------------

const samplePage: FeedResponse = {
  items: [
    {
      seriesId: 87,
      canonicalTitle: "Example Series",
      externalIds: [{ provider: "mangabaka", externalId: "9741", fetchedAt: 1_780_943_416 }],
      volumeCoverage: [{ start: 1, end: 16 }],
      chapterCoverage: [],
      highestVolume: 16,
      highestChapter: null,
      updatedAt: 1_780_943_416,
    },
  ],
  hasMore: true,
  nextCursor: "next-cursor-token",
};

function jsonResponse(body: unknown, status = 200): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { "content-type": "application/json" },
  });
}

describe("fetchFeedPage", () => {
  it("returns ok with the parsed page on 200", async () => {
    const fetchImpl = vi
      .fn()
      .mockResolvedValue(jsonResponse(samplePage)) as unknown as typeof fetch;
    const result = await fetchFeedPage("https://t.example.com", null, 100, { fetchImpl });

    expect(result.kind).toBe("ok");
    if (result.kind !== "ok") throw new Error("expected ok");
    expect(result.data.hasMore).toBe(true);
    expect(result.data.nextCursor).toBe("next-cursor-token");
    expect(result.data.items).toHaveLength(1);
    expect(result.data.items[0].seriesId).toBe(87);
  });

  it("sends the cursor and limit in the request URL", async () => {
    const fetchImpl = vi
      .fn()
      .mockResolvedValue(jsonResponse(samplePage)) as unknown as typeof fetch;
    await fetchFeedPage("https://t.example.com", "cur-1", 250, { fetchImpl });

    const calledUrl = (fetchImpl as unknown as ReturnType<typeof vi.fn>).mock.calls[0][0] as string;
    const parsed = new URL(calledUrl);
    expect(parsed.searchParams.get("cursor")).toBe("cur-1");
    expect(parsed.searchParams.get("limit")).toBe("250");
  });

  it("requests JSON via the Accept header", async () => {
    const fetchImpl = vi
      .fn()
      .mockResolvedValue(jsonResponse(samplePage)) as unknown as typeof fetch;
    await fetchFeedPage("https://t.example.com", null, 100, { fetchImpl });

    const init = (fetchImpl as unknown as ReturnType<typeof vi.fn>).mock.calls[0][1] as RequestInit;
    expect((init.headers as Record<string, string>).Accept).toBe("application/json");
  });

  it("maps a non-200 status to an error result", async () => {
    const fetchImpl = vi
      .fn()
      .mockResolvedValue(new Response("nope", { status: 503 })) as unknown as typeof fetch;
    const result = await fetchFeedPage("https://t.example.com", null, 100, { fetchImpl });

    expect(result.kind).toBe("error");
    if (result.kind !== "error") throw new Error("expected error");
    expect(result.status).toBe(503);
  });

  it("maps a network throw to status 0", async () => {
    const fetchImpl = vi
      .fn()
      .mockRejectedValue(new Error("ECONNREFUSED")) as unknown as typeof fetch;
    const result = await fetchFeedPage("https://t.example.com", null, 100, { fetchImpl });

    expect(result.kind).toBe("error");
    if (result.kind !== "error") throw new Error("expected error");
    expect(result.status).toBe(0);
    expect(result.message).toContain("ECONNREFUSED");
  });

  it("errors on a 200 with invalid JSON", async () => {
    const fetchImpl = vi
      .fn()
      .mockResolvedValue(
        new Response("not json", { status: 200, headers: { "content-type": "application/json" } }),
      ) as unknown as typeof fetch;
    const result = await fetchFeedPage("https://t.example.com", null, 100, { fetchImpl });

    expect(result.kind).toBe("error");
    if (result.kind !== "error") throw new Error("expected error");
    expect(result.status).toBe(200);
    expect(result.message).toContain("parse");
  });

  it("errors on a 200 whose body is missing items[]", async () => {
    const fetchImpl = vi
      .fn()
      .mockResolvedValue(jsonResponse({ hasMore: false })) as unknown as typeof fetch;
    const result = await fetchFeedPage("https://t.example.com", null, 100, { fetchImpl });

    expect(result.kind).toBe("error");
    if (result.kind !== "error") throw new Error("expected error");
    expect(result.message).toContain("malformed");
  });
});
