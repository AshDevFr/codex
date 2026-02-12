import { afterEach, describe, expect, it, vi } from "vitest";
import { AniListRecommendationClient, getBestTitle, stripHtml } from "./anilist.js";

describe("getBestTitle", () => {
  it("prefers English title", () => {
    expect(getBestTitle({ romaji: "Shingeki no Kyojin", english: "Attack on Titan" })).toBe(
      "Attack on Titan",
    );
  });

  it("falls back to romaji", () => {
    expect(getBestTitle({ romaji: "Berserk" })).toBe("Berserk");
  });

  it("falls back to romaji when english is empty", () => {
    expect(getBestTitle({ romaji: "Berserk", english: "" })).toBe("Berserk");
  });

  it("returns Unknown when neither is set", () => {
    expect(getBestTitle({})).toBe("Unknown");
  });
});

describe("stripHtml", () => {
  it("strips basic tags", () => {
    expect(stripHtml("<p>Hello <b>world</b></p>")).toBe("Hello world");
  });

  it("converts br to newlines", () => {
    expect(stripHtml("Line 1<br>Line 2<br/>Line 3")).toBe("Line 1\nLine 2\nLine 3");
  });

  it("returns undefined for null", () => {
    expect(stripHtml(null)).toBeUndefined();
  });

  it("returns undefined for empty string after trim", () => {
    expect(stripHtml("   ")).toBe("");
  });

  it("handles complex HTML", () => {
    expect(stripHtml('<i>A story about <a href="#">heroes</a></i>')).toBe("A story about heroes");
  });

  it("decodes named HTML entities", () => {
    expect(stripHtml("Tom &amp; Jerry")).toBe("Tom & Jerry");
    expect(stripHtml("a &lt; b &gt; c")).toBe("a < b > c");
    expect(stripHtml("&quot;quoted&quot;")).toBe('"quoted"');
    expect(stripHtml("it&#39;s")).toBe("it's");
  });

  it("decodes numeric HTML entities", () => {
    expect(stripHtml("&#169; 2026")).toBe("\u00A9 2026");
    expect(stripHtml("&#x2764;")).toBe("\u2764");
  });

  it("decodes entities inside HTML", () => {
    expect(stripHtml("<p>Rock &amp; Roll</p>")).toBe("Rock & Roll");
  });

  it("preserves unknown entities as-is", () => {
    expect(stripHtml("&unknown;")).toBe("&unknown;");
  });

  it("strips nested tags", () => {
    expect(stripHtml("<div><p><b><i>deep</i></b></p></div>")).toBe("deep");
  });

  it("handles br with space before slash", () => {
    expect(stripHtml("A<br />B")).toBe("A\nB");
  });
});

// =============================================================================
// AniListRecommendationClient Fetch Behavior Tests
// =============================================================================

describe("AniListRecommendationClient fetch behavior", () => {
  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("passes AbortSignal.timeout to fetch", async () => {
    const fetchSpy = vi.spyOn(globalThis, "fetch").mockResolvedValue(
      new Response(JSON.stringify({ data: { Viewer: { id: 1, name: "test" } } }), {
        status: 200,
        headers: { "Content-Type": "application/json" },
      }),
    );

    const client = new AniListRecommendationClient("test-token");
    await client.getViewerId();

    expect(fetchSpy).toHaveBeenCalledOnce();
    const init = fetchSpy.mock.calls[0][1] as RequestInit;
    expect(init.signal).toBeDefined();
  });

  it("wraps timeout errors with descriptive message", async () => {
    const timeoutError = new DOMException(
      "The operation was aborted due to timeout",
      "TimeoutError",
    );
    vi.spyOn(globalThis, "fetch").mockRejectedValue(timeoutError);

    const client = new AniListRecommendationClient("test-token");
    await expect(client.getViewerId()).rejects.toThrow(
      "AniList API request timed out after 30 seconds",
    );
  });

  it("re-throws non-timeout fetch errors as-is", async () => {
    vi.spyOn(globalThis, "fetch").mockRejectedValue(new Error("Network failure"));

    const client = new AniListRecommendationClient("test-token");
    await expect(client.getViewerId()).rejects.toThrow("Network failure");
  });

  it("retries once on 429 then succeeds", async () => {
    const fetchSpy = vi
      .spyOn(globalThis, "fetch")
      .mockResolvedValueOnce(new Response("", { status: 429, headers: { "Retry-After": "0" } }))
      .mockResolvedValueOnce(
        new Response(JSON.stringify({ data: { Viewer: { id: 42, name: "test" } } }), {
          status: 200,
          headers: { "Content-Type": "application/json" },
        }),
      );

    const client = new AniListRecommendationClient("test-token");
    const id = await client.getViewerId();

    expect(id).toBe(42);
    expect(fetchSpy).toHaveBeenCalledTimes(2);
  });

  it("throws RateLimitError after retry exhausted on 429", async () => {
    vi.spyOn(globalThis, "fetch").mockResolvedValue(
      new Response("", { status: 429, headers: { "Retry-After": "0" } }),
    );

    const client = new AniListRecommendationClient("test-token");
    await expect(client.getViewerId()).rejects.toThrow("AniList rate limit exceeded");
  });
});

// =============================================================================
// Recommendation Pagination Tests
// =============================================================================

describe("AniListRecommendationClient pagination", () => {
  afterEach(() => {
    vi.restoreAllMocks();
  });

  function makeRecommendationResponse(nodes: Array<{ id: number }>, hasNextPage: boolean) {
    return {
      data: {
        Media: {
          id: 1,
          title: { romaji: "Test", english: "Test" },
          recommendations: {
            pageInfo: { hasNextPage },
            nodes: nodes.map((n) => ({
              rating: 10,
              mediaRecommendation: {
                id: n.id,
                title: { romaji: `Rec ${n.id}` },
                coverImage: { large: null },
                description: null,
                genres: [],
                averageScore: 80,
                popularity: 5000,
                siteUrl: `https://anilist.co/manga/${n.id}`,
                status: null,
                volumes: null,
              },
            })),
          },
        },
      },
    };
  }

  it("fetches multiple pages when hasNextPage is true", async () => {
    const fetchSpy = vi
      .spyOn(globalThis, "fetch")
      .mockResolvedValueOnce(
        new Response(JSON.stringify(makeRecommendationResponse([{ id: 1 }, { id: 2 }], true)), {
          status: 200,
          headers: { "Content-Type": "application/json" },
        }),
      )
      .mockResolvedValueOnce(
        new Response(JSON.stringify(makeRecommendationResponse([{ id: 3 }], false)), {
          status: 200,
          headers: { "Content-Type": "application/json" },
        }),
      );

    const client = new AniListRecommendationClient("test-token");
    const nodes = await client.getRecommendationsForMedia(1, 10, 3);

    expect(nodes).toHaveLength(3);
    expect(fetchSpy).toHaveBeenCalledTimes(2);
  });

  it("stops at maxPages even if hasNextPage is true", async () => {
    const fetchSpy = vi
      .spyOn(globalThis, "fetch")
      .mockResolvedValueOnce(
        new Response(JSON.stringify(makeRecommendationResponse([{ id: 1 }], true)), {
          status: 200,
          headers: { "Content-Type": "application/json" },
        }),
      )
      .mockResolvedValueOnce(
        new Response(JSON.stringify(makeRecommendationResponse([{ id: 2 }], true)), {
          status: 200,
          headers: { "Content-Type": "application/json" },
        }),
      );

    const client = new AniListRecommendationClient("test-token");
    const nodes = await client.getRecommendationsForMedia(1, 10, 2);

    expect(nodes).toHaveLength(2);
    expect(fetchSpy).toHaveBeenCalledTimes(2);
  });

  it("fetches single page when hasNextPage is false", async () => {
    const fetchSpy = vi.spyOn(globalThis, "fetch").mockResolvedValueOnce(
      new Response(JSON.stringify(makeRecommendationResponse([{ id: 1 }], false)), {
        status: 200,
        headers: { "Content-Type": "application/json" },
      }),
    );

    const client = new AniListRecommendationClient("test-token");
    const nodes = await client.getRecommendationsForMedia(1, 10, 3);

    expect(nodes).toHaveLength(1);
    expect(fetchSpy).toHaveBeenCalledTimes(1);
  });
});
