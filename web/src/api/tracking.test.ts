import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { api } from "./client";
import { trackingApi } from "./tracking";

vi.mock("./client", () => ({
  api: {
    get: vi.fn(),
    patch: vi.fn(),
    post: vi.fn(),
    delete: vi.fn(),
  },
}));

describe("trackingApi.listAliases", () => {
  beforeEach(() => vi.clearAllMocks());
  afterEach(() => vi.restoreAllMocks());

  it("returns the aliases array when present", async () => {
    const aliases = [
      { id: "a1", seriesId: "s1", alias: "alt-name", source: "user" },
    ];
    vi.mocked(api.get).mockResolvedValueOnce({ data: { aliases } });

    const result = await trackingApi.listAliases("s1");

    expect(api.get).toHaveBeenCalledWith("/series/s1/aliases");
    expect(result).toEqual(aliases);
  });

  it("returns [] when the response body omits the aliases wrapper", async () => {
    // Reproduces the production bug where TanStack Query rejected
    // `undefined` from `response.data.aliases` and surfaced
    // "Query data cannot be undefined" for every series detail visit.
    vi.mocked(api.get).mockResolvedValueOnce({ data: {} });

    const result = await trackingApi.listAliases("s1");

    expect(result).toEqual([]);
  });

  it("returns [] when the response body is null", async () => {
    vi.mocked(api.get).mockResolvedValueOnce({ data: null });

    const result = await trackingApi.listAliases("s1");

    expect(result).toEqual([]);
  });
});
