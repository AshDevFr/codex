import { describe, expect, it, vi } from "vitest";
import { type FeedRequest, type FeedResponse, feedUrl, fetchFeedPage } from "./fetcher.js";

// -----------------------------------------------------------------------------
// feedUrl
// -----------------------------------------------------------------------------

describe("feedUrl", () => {
  it("appends the feed path", () => {
    expect(feedUrl("https://t.example.com")).toBe("https://t.example.com/api/v1/series/feed");
  });

  it("strips trailing slashes from the base URL", () => {
    expect(feedUrl("https://t.example.com///")).toBe("https://t.example.com/api/v1/series/feed");
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

function req(overrides: Partial<FeedRequest> = {}): FeedRequest {
  return { externalIds: ["mangabaka:9741"], cursor: null, limit: 100, ...overrides };
}

/** Read the JSON body of the first call to a mocked fetch. */
function calledBody(fetchImpl: typeof fetch): Record<string, unknown> {
  const init = (fetchImpl as unknown as ReturnType<typeof vi.fn>).mock.calls[0][1] as RequestInit;
  return JSON.parse(init.body as string);
}

function calledInit(fetchImpl: typeof fetch): RequestInit {
  return (fetchImpl as unknown as ReturnType<typeof vi.fn>).mock.calls[0][1] as RequestInit;
}

describe("fetchFeedPage", () => {
  it("returns ok with the parsed page on 200", async () => {
    const fetchImpl = vi
      .fn()
      .mockResolvedValue(jsonResponse(samplePage)) as unknown as typeof fetch;
    const result = await fetchFeedPage("https://t.example.com", req(), { fetchImpl });

    expect(result.kind).toBe("ok");
    if (result.kind !== "ok") throw new Error("expected ok");
    expect(result.data.hasMore).toBe(true);
    expect(result.data.nextCursor).toBe("next-cursor-token");
    expect(result.data.items[0].seriesId).toBe(87);
  });

  it("POSTs the external-id filter, cursor and limit in the body", async () => {
    const fetchImpl = vi
      .fn()
      .mockResolvedValue(jsonResponse(samplePage)) as unknown as typeof fetch;
    await fetchFeedPage(
      "https://t.example.com",
      { externalIds: ["mangabaka:9741", "mal:5"], cursor: "cur-1", limit: 250 },
      { fetchImpl },
    );

    const calledUrl = (fetchImpl as unknown as ReturnType<typeof vi.fn>).mock.calls[0][0] as string;
    expect(calledUrl).toBe("https://t.example.com/api/v1/series/feed");
    const init = calledInit(fetchImpl);
    expect(init.method).toBe("POST");
    expect((init.headers as Record<string, string>).Accept).toBe("application/json");
    expect((init.headers as Record<string, string>)["Content-Type"]).toBe("application/json");

    const body = calledBody(fetchImpl);
    expect(body.externalIds).toEqual(["mangabaka:9741", "mal:5"]);
    expect(body.cursor).toBe("cur-1");
    expect(body.limit).toBe(250);
  });

  it("sends a null cursor for the first page", async () => {
    const fetchImpl = vi
      .fn()
      .mockResolvedValue(jsonResponse(samplePage)) as unknown as typeof fetch;
    await fetchFeedPage("https://t.example.com", req({ cursor: null }), { fetchImpl });
    expect(calledBody(fetchImpl).cursor).toBeNull();
  });

  it("maps a non-200 status to an error result", async () => {
    const fetchImpl = vi
      .fn()
      .mockResolvedValue(new Response("nope", { status: 503 })) as unknown as typeof fetch;
    const result = await fetchFeedPage("https://t.example.com", req(), { fetchImpl });

    expect(result.kind).toBe("error");
    if (result.kind !== "error") throw new Error("expected error");
    expect(result.status).toBe(503);
  });

  it("maps a network throw to status 0", async () => {
    const fetchImpl = vi
      .fn()
      .mockRejectedValue(new Error("ECONNREFUSED")) as unknown as typeof fetch;
    const result = await fetchFeedPage("https://t.example.com", req(), { fetchImpl });

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
    const result = await fetchFeedPage("https://t.example.com", req(), { fetchImpl });

    expect(result.kind).toBe("error");
    if (result.kind !== "error") throw new Error("expected error");
    expect(result.status).toBe(200);
    expect(result.message).toContain("parse");
  });

  it("errors on a 200 whose body is missing items[]", async () => {
    const fetchImpl = vi
      .fn()
      .mockResolvedValue(jsonResponse({ hasMore: false })) as unknown as typeof fetch;
    const result = await fetchFeedPage("https://t.example.com", req(), { fetchImpl });

    expect(result.kind).toBe("error");
    if (result.kind !== "error") throw new Error("expected error");
    expect(result.message).toContain("malformed");
  });
});
