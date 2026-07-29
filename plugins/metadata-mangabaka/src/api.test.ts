import { afterEach, describe, expect, it, vi } from "vitest";
import { MangaBakaClient } from "./api.js";

/**
 * Stub `fetch` with a single JSON payload and capture the requested URL.
 */
function stubFetch(payload: unknown): { urls: string[] } {
  const urls: string[] = [];
  vi.stubGlobal(
    "fetch",
    vi.fn((url: string) => {
      urls.push(String(url));
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
  return { urls };
}

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("MangaBakaClient.search", () => {
  it("targets the api.mangabaka.org host", async () => {
    const { urls } = stubFetch({ status: 200, data: [], pagination: null });

    await new MangaBakaClient("key").search("one piece");

    expect(urls[0]).toContain("https://api.mangabaka.org/v1/series/search");
  });

  it("maps the count/next pagination envelope", async () => {
    stubFetch({
      status: 200,
      data: [{ id: 377 }, { id: 378 }, { id: 379 }],
      pagination: {
        count: 599,
        next: "https://api.mangabaka.org/v1/series/search?limit=3&page=3&q=one+piece",
        previous: "https://api.mangabaka.org/v1/series/search?limit=3&page=1&q=one+piece",
        page: 2,
        limit: 3,
      },
    });

    const result = await new MangaBakaClient("key").search("one piece", 2, 3);

    expect(result.total).toBe(599);
    expect(result.page).toBe(2);
    expect(result.hasNextPage).toBe(true);
  });

  it("reports no next page on the final page", async () => {
    stubFetch({
      status: 200,
      data: [{ id: 1 }],
      pagination: { count: 1, next: null, previous: null, page: 1, limit: 3 },
    });

    const result = await new MangaBakaClient("key").search("dice");

    expect(result.total).toBe(1);
    expect(result.hasNextPage).toBe(false);
  });

  it("reports no next page for an empty result set", async () => {
    stubFetch({
      status: 200,
      data: [],
      pagination: { count: 0, next: null, previous: null, page: 1, limit: 3 },
    });

    const result = await new MangaBakaClient("key").search("zzzqqxnonexistent");

    expect(result.total).toBe(0);
    expect(result.hasNextPage).toBe(false);
  });

  it("honors a base_url override", async () => {
    const { urls } = stubFetch({ status: 200, data: [], pagination: null });

    await new MangaBakaClient("key", { baseUrl: "https://mb.example.test" }).search("air");

    expect(urls[0]).toContain("https://mb.example.test/v1/series/search");
  });

  it("trims a trailing slash from a base_url override", async () => {
    const { urls } = stubFetch({ status: 200, data: [], pagination: null });

    await new MangaBakaClient("key", { baseUrl: "https://mb.example.test/" }).search("air");

    expect(urls[0]).toContain("https://mb.example.test/v1/series/search");
    expect(urls[0]).not.toContain("//v1/series");
  });

  it("ignores a blank base_url override", async () => {
    const { urls } = stubFetch({ status: 200, data: [], pagination: null });

    await new MangaBakaClient("key", { baseUrl: "  " }).search("air");

    expect(urls[0]).toContain("https://api.mangabaka.org/v1/series/search");
  });

  it("falls back safely when the pagination envelope is absent", async () => {
    stubFetch({ status: 200, data: [{ id: 1 }, { id: 2 }] });

    const result = await new MangaBakaClient("key").search("air");

    expect(result.total).toBe(2);
    expect(result.page).toBe(1);
    expect(result.hasNextPage).toBe(false);
  });
});
