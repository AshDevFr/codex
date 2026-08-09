import { ApiError, NotFoundError, RateLimitError } from "@ashdev/codex-plugin-sdk";
import { afterEach, describe, expect, it, vi } from "vitest";
import { DEFAULT_BASE_URL, MangaBakaRecommendationClient } from "./api.js";

/** Stub `fetch` with a JSON payload and capture the requested URLs. */
function stubFetch(payload: unknown): { urls: string[]; headers: HeadersInit[] } {
  const urls: string[] = [];
  const headers: HeadersInit[] = [];
  vi.stubGlobal(
    "fetch",
    vi.fn((url: string, init?: RequestInit) => {
      urls.push(String(url));
      if (init?.headers) headers.push(init.headers);
      return Promise.resolve({
        ok: true,
        status: 200,
        statusText: "OK",
        headers: new Headers(),
        json: () => Promise.resolve(payload),
        text: () => Promise.resolve(JSON.stringify(payload)),
      } as unknown as Response);
    }),
  );
  return { urls, headers };
}

/** Stub `fetch` with a non-2xx response. */
function stubFetchStatus(status: number, responseHeaders: Record<string, string> = {}): void {
  vi.stubGlobal(
    "fetch",
    vi.fn(() =>
      Promise.resolve({
        ok: false,
        status,
        statusText: "Error",
        headers: new Headers(responseHeaders),
        json: () => Promise.resolve({}),
        text: () => Promise.resolve("error body"),
      } as unknown as Response),
    ),
  );
}

/** Parse the query string of a captured URL into a list of [key, value] pairs. */
function queryPairs(url: string): [string, string][] {
  return [...new URL(url).searchParams.entries()];
}

const OK: unknown = { status: 200, data: [] };

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("MangaBakaRecommendationClient.mix", () => {
  it("targets the public mix endpoint on the default host", async () => {
    const { urls } = stubFetch(OK);

    await new MangaBakaRecommendationClient().mix({ series: [1, 2] });

    expect(urls[0]).toContain(`${DEFAULT_BASE_URL}/v1/series/mix`);
  });

  it("serialises seeds as repeated series params", async () => {
    const { urls } = stubFetch(OK);

    await new MangaBakaRecommendationClient().mix({ series: [1, 2, 3] });

    const seeds = queryPairs(urls[0])
      .filter(([k]) => k === "series")
      .map(([, v]) => v);
    expect(seeds).toEqual(["1", "2", "3"]);
  });

  it("sends no API key header (these endpoints are public)", async () => {
    const { headers } = stubFetch(OK);

    await new MangaBakaRecommendationClient().mix({ series: [1] });

    expect(JSON.stringify(headers[0])).not.toContain("x-api-key");
  });

  it("omits filter params that are empty or unset", async () => {
    const { urls } = stubFetch(OK);

    await new MangaBakaRecommendationClient().mix({
      series: [1],
      contentRating: [],
      tagNot: [],
      genreNot: [],
    });

    const keys = queryPairs(urls[0]).map(([k]) => k);
    expect(keys).not.toContain("content_rating");
    expect(keys).not.toContain("tag_not");
    expect(keys).not.toContain("genre_not");
  });

  it("maps camelCase params onto the upstream snake_case names", async () => {
    const { urls } = stubFetch(OK);

    await new MangaBakaRecommendationClient().mix({
      series: [1],
      limit: 50,
      strict: false,
      contentRating: ["safe", "suggestive"],
      tagNot: [1120, 39],
      type: ["manga"],
      typeNot: ["novel"],
      status: ["releasing"],
      statusNot: ["cancelled"],
      genreNot: ["hentai"],
      ratingLower: 60,
      ratingUpper: 95,
    });

    const pairs = queryPairs(urls[0]);
    const get = (k: string) => pairs.filter(([key]) => key === k).map(([, v]) => v);

    expect(get("limit")).toEqual(["50"]);
    expect(get("strict")).toEqual(["false"]);
    expect(get("content_rating")).toEqual(["safe", "suggestive"]);
    expect(get("tag_not")).toEqual(["1120", "39"]);
    expect(get("type")).toEqual(["manga"]);
    expect(get("type_not")).toEqual(["novel"]);
    expect(get("status")).toEqual(["releasing"]);
    expect(get("status_not")).toEqual(["cancelled"]);
    expect(get("genre_not")).toEqual(["hentai"]);
    expect(get("rating_lower")).toEqual(["60"]);
    expect(get("rating_upper")).toEqual(["95"]);
  });

  it("clamps limit to the documented 1-50 range", async () => {
    const { urls } = stubFetch(OK);
    const client = new MangaBakaRecommendationClient();

    await client.mix({ series: [1], limit: 500 });
    await client.mix({ series: [1], limit: 0 });

    expect(queryPairs(urls[0]).find(([k]) => k === "limit")?.[1]).toBe("50");
    expect(queryPairs(urls[1]).find(([k]) => k === "limit")?.[1]).toBe("1");
  });

  it("rejects a call with no seeds rather than issuing a doomed request", async () => {
    // Upstream requires at least one seed or include-tag and answers 400
    // otherwise. Failing locally keeps a pointless round trip off the wire.
    stubFetch(OK);

    await expect(new MangaBakaRecommendationClient().mix({ series: [] })).rejects.toThrow(ApiError);
  });

  it("returns an empty list when data is missing entirely", async () => {
    // Beta endpoint: a shape change must not throw.
    stubFetch({ status: 200 });

    const result = await new MangaBakaRecommendationClient().mix({ series: [1] });

    expect(result).toEqual([]);
  });

  it("drops entries with no usable series object", async () => {
    stubFetch({
      status: 200,
      data: [{ score: 0.5 }, { score: 0.4, series: { id: 7 } }, { score: 0.3, series: {} }],
    });

    const result = await new MangaBakaRecommendationClient().mix({ series: [1] });

    expect(result).toHaveLength(1);
    expect(result[0].series.id).toBe(7);
  });

  it("honours a base URL override", async () => {
    const { urls } = stubFetch(OK);

    await new MangaBakaRecommendationClient({ baseUrl: "https://mb.example.test" }).mix({
      series: [1],
    });

    expect(urls[0]).toContain("https://mb.example.test/v1/series/mix");
  });

  it("strips a trailing slash from the base URL", async () => {
    const { urls } = stubFetch(OK);

    await new MangaBakaRecommendationClient({ baseUrl: "https://mb.example.test/" }).mix({
      series: [1],
    });

    expect(urls[0]).not.toContain("test//v1");
  });
});

describe("MangaBakaRecommendationClient.readersAlsoLike", () => {
  it("targets the per-series collaborative endpoint", async () => {
    const { urls } = stubFetch(OK);

    await new MangaBakaRecommendationClient().readersAlsoLike(84926);

    expect(urls[0]).toContain("/v1/series/84926/readers-also-like");
  });

  it("clamps limit to the documented 1-24 range", async () => {
    const { urls } = stubFetch(OK);

    await new MangaBakaRecommendationClient().readersAlsoLike(1, { limit: 100 });

    expect(queryPairs(urls[0]).find(([k]) => k === "limit")?.[1]).toBe("24");
  });

  it("passes the shared content rating and tag filters", async () => {
    const { urls } = stubFetch(OK);

    await new MangaBakaRecommendationClient().readersAlsoLike(1, {
      contentRating: ["safe"],
      tagNot: [42],
    });

    const pairs = queryPairs(urls[0]);
    expect(pairs).toContainEqual(["content_rating", "safe"]);
    expect(pairs).toContainEqual(["tag_not", "42"]);
  });
});

describe("MangaBakaRecommendationClient.tags", () => {
  it("targets the stable tag catalogue endpoint", async () => {
    const { urls } = stubFetch(OK);

    await new MangaBakaRecommendationClient().tags();

    expect(urls[0]).toContain("/v1/tags");
  });

  it("drops catalogue entries missing an id or name", async () => {
    stubFetch({
      status: 200,
      data: [{ id: 1, name: "Action" }, { id: 2 }, { name: "Orphan" }],
    });

    const result = await new MangaBakaRecommendationClient().tags();

    expect(result).toEqual([{ id: 1, name: "Action" }]);
  });
});

describe("MangaBakaRecommendationClient error handling", () => {
  it("raises RateLimitError honouring Retry-After on 429", async () => {
    stubFetchStatus(429, { "Retry-After": "30" });

    const error = await new MangaBakaRecommendationClient()
      .mix({ series: [1] })
      .catch((e: unknown) => e);

    expect(error).toBeInstanceOf(RateLimitError);
    expect((error as RateLimitError).retryAfterSeconds).toBe(30);
  });

  it("defaults the retry delay when Retry-After is absent or unparseable", async () => {
    stubFetchStatus(429, { "Retry-After": "not-a-number" });

    const error = await new MangaBakaRecommendationClient()
      .mix({ series: [1] })
      .catch((e: unknown) => e);

    expect(error).toBeInstanceOf(RateLimitError);
    expect((error as RateLimitError).retryAfterSeconds).toBe(60);
  });

  it("raises NotFoundError on 404", async () => {
    stubFetchStatus(404);

    await expect(
      new MangaBakaRecommendationClient().readersAlsoLike(999999999),
    ).rejects.toBeInstanceOf(NotFoundError);
  });

  it("raises ApiError carrying the status code on 5xx", async () => {
    stubFetchStatus(503);

    await expect(new MangaBakaRecommendationClient().mix({ series: [1] })).rejects.toMatchObject({
      statusCode: 503,
    });
  });

  it("raises ApiError on 400, which is how upstream rejects bad params", async () => {
    // Passing a tag *name* to tag_not instead of a numeric ID lands here.
    stubFetchStatus(400);

    await expect(new MangaBakaRecommendationClient().mix({ series: [1] })).rejects.toMatchObject({
      statusCode: 400,
    });
  });

  it("raises ApiError when the request times out", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(() => {
        const err = new Error("aborted");
        err.name = "AbortError";
        return Promise.reject(err);
      }),
    );

    await expect(
      new MangaBakaRecommendationClient({ timeout: 1 }).mix({ series: [1] }),
    ).rejects.toThrow(/timed out/);
  });

  it("wraps unexpected transport failures as ApiError", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(() => Promise.reject(new Error("socket hang up"))),
    );

    await expect(new MangaBakaRecommendationClient().mix({ series: [1] })).rejects.toBeInstanceOf(
      ApiError,
    );
  });
});
