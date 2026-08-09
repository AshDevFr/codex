import type { PluginStorage } from "@ashdev/codex-plugin-sdk";
import { ApiError } from "@ashdev/codex-plugin-sdk";
import { describe, expect, it, vi } from "vitest";
import type { MangaBakaRecommendationClient } from "./api.js";
import { TAG_CACHE_KEY, TagResolver } from "./tags.js";
import type { MbCatalogueTag } from "./types.js";

const CATALOGUE: MbCatalogueTag[] = [
  { id: 39, name: "Action" },
  { id: 1120, name: "Death Game" },
  { id: 911, name: "Male Oriented" },
  { id: 26, name: "Suspense" },
];

/** A client stub whose `tags` returns the shared catalogue. */
function clientReturning(tags: MbCatalogueTag[] = CATALOGUE) {
  const tagsFn = vi.fn(async () => tags);
  return { client: { tags: tagsFn } as unknown as MangaBakaRecommendationClient, tagsFn };
}

/** Storage backed by a plain object. */
function fakeStorage(initial: unknown = null) {
  const state = { value: initial };
  const storage = {
    get: vi.fn(async () => ({ data: state.value })),
    set: vi.fn(async (_key: string, value: unknown) => {
      state.value = value;
      return { success: true };
    }),
  } as unknown as PluginStorage;
  return { storage, state };
}

describe("TagResolver", () => {
  it("resolves an exact tag name to its numeric ID", async () => {
    // tag_not takes integers; passing a name returns HTTP 400 upstream.
    const { client } = clientReturning();
    const resolver = new TagResolver(client);

    expect(await resolver.resolve(["Death Game"])).toEqual([1120]);
  });

  it("resolves case-insensitively", async () => {
    const { client } = clientReturning();
    const resolver = new TagResolver(client);

    expect(await resolver.resolve(["death game", "ACTION"])).toEqual([1120, 39]);
  });

  it("ignores surrounding whitespace", async () => {
    const { client } = clientReturning();
    const resolver = new TagResolver(client);

    expect(await resolver.resolve(["  Suspense  "])).toEqual([26]);
  });

  it("accepts a numeric ID typed directly", async () => {
    // Not the documented way to use the field, but silently dropping a valid
    // ID because it was not a name would be unhelpful.
    const { client } = clientReturning();
    const resolver = new TagResolver(client);

    expect(await resolver.resolve(["1120"])).toEqual([1120]);
  });

  it("skips names it cannot resolve and keeps the rest", async () => {
    const { client } = clientReturning();
    const resolver = new TagResolver(client);

    expect(await resolver.resolve(["Action", "Not A Real Tag"])).toEqual([39]);
  });

  it("returns nothing for an empty request without calling upstream", async () => {
    const { client, tagsFn } = clientReturning();
    const resolver = new TagResolver(client);

    expect(await resolver.resolve([])).toEqual([]);
    expect(tagsFn).not.toHaveBeenCalled();
  });

  it("de-duplicates repeated names", async () => {
    const { client } = clientReturning();
    const resolver = new TagResolver(client);

    expect(await resolver.resolve(["Action", "action"])).toEqual([39]);
  });

  it("fetches the catalogue only once per process", async () => {
    const { client, tagsFn } = clientReturning();
    const resolver = new TagResolver(client);

    await resolver.resolve(["Action"]);
    await resolver.resolve(["Suspense"]);

    expect(tagsFn).toHaveBeenCalledTimes(1);
  });

  it("returns nothing when the catalogue cannot be fetched", async () => {
    // Losing a tag filter is a degraded result; failing the run is worse.
    const client = {
      tags: vi.fn(async () => {
        throw new ApiError("API error: 503", 503);
      }),
    } as unknown as MangaBakaRecommendationClient;
    const resolver = new TagResolver(client);

    expect(await resolver.resolve(["Action"])).toEqual([]);
  });

  it("does not retry a failed catalogue fetch on every call", async () => {
    const tags = vi.fn(async () => {
      throw new ApiError("API error: 503", 503);
    });
    const client = { tags } as unknown as MangaBakaRecommendationClient;
    const resolver = new TagResolver(client);

    await resolver.resolve(["Action"]);
    await resolver.resolve(["Suspense"]);

    expect(tags).toHaveBeenCalledTimes(1);
  });
});

describe("TagResolver storage cache", () => {
  it("serves a cached catalogue without calling upstream", async () => {
    const { client, tagsFn } = clientReturning();
    const { storage } = fakeStorage({ "death game": 1120 });
    const resolver = new TagResolver(client, storage);

    expect(await resolver.resolve(["Death Game"])).toEqual([1120]);
    expect(tagsFn).not.toHaveBeenCalled();
  });

  it("reads from the documented cache key", async () => {
    const { client } = clientReturning();
    const { storage } = fakeStorage(null);
    const resolver = new TagResolver(client, storage);

    await resolver.resolve(["Action"]);

    expect(storage.get).toHaveBeenCalledWith(TAG_CACHE_KEY);
  });

  it("persists a freshly fetched catalogue with an expiry", async () => {
    const { client } = clientReturning();
    const { storage, state } = fakeStorage(null);
    const resolver = new TagResolver(client, storage);

    await resolver.resolve(["Action"]);

    expect(storage.set).toHaveBeenCalled();
    expect(state.value).toMatchObject({ action: 39 });
    // Third argument is the TTL expiry; the catalogue is stable but not frozen.
    const expiry = (storage.set as unknown as { mock: { calls: unknown[][] } }).mock.calls[0][2];
    expect(typeof expiry).toBe("string");
    expect(Number.isNaN(Date.parse(expiry as string))).toBe(false);
  });

  it("refetches when the cached value is not a usable map", async () => {
    const { client, tagsFn } = clientReturning();
    const { storage } = fakeStorage(["not", "a", "map"]);
    const resolver = new TagResolver(client, storage);

    expect(await resolver.resolve(["Action"])).toEqual([39]);
    expect(tagsFn).toHaveBeenCalledTimes(1);
  });

  it("survives a storage read failure by fetching upstream", async () => {
    const { client, tagsFn } = clientReturning();
    const storage = {
      get: vi.fn(async () => {
        throw new Error("storage down");
      }),
      set: vi.fn(async () => ({ success: true })),
    } as unknown as PluginStorage;
    const resolver = new TagResolver(client, storage);

    expect(await resolver.resolve(["Action"])).toEqual([39]);
    expect(tagsFn).toHaveBeenCalledTimes(1);
  });

  it("survives a storage write failure", async () => {
    const { client } = clientReturning();
    const storage = {
      get: vi.fn(async () => ({ data: null })),
      set: vi.fn(async () => {
        throw new Error("storage down");
      }),
    } as unknown as PluginStorage;
    const resolver = new TagResolver(client, storage);

    expect(await resolver.resolve(["Action"])).toEqual([39]);
  });

  it("works with no storage attached", async () => {
    const { client } = clientReturning();
    const resolver = new TagResolver(client);

    expect(await resolver.resolve(["Action"])).toEqual([39]);
  });
});
