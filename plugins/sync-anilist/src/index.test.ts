import { afterEach, describe, expect, it, vi } from "vitest";
import type { AniListClient } from "./anilist.js";
import { applyStaleness, provider, setClient, setSearchFallback, setViewerId } from "./index.js";

// =============================================================================
// applyStaleness Tests
// =============================================================================

describe("applyStaleness", () => {
  // Helper: returns a timestamp N days ago from a fixed reference point
  const now = new Date("2026-02-08T12:00:00Z").getTime();
  const daysAgo = (days: number) => new Date(now - days * 24 * 60 * 60 * 1000).toISOString();

  describe("passthrough cases", () => {
    it("returns status unchanged when not reading", () => {
      expect(applyStaleness("completed", daysAgo(100), 30, 60, now)).toBe("completed");
      expect(applyStaleness("on_hold", daysAgo(100), 30, 60, now)).toBe("on_hold");
      expect(applyStaleness("dropped", daysAgo(100), 30, 60, now)).toBe("dropped");
      expect(applyStaleness("plan_to_read", daysAgo(100), 30, 60, now)).toBe("plan_to_read");
    });

    it("returns reading when both thresholds are 0 (disabled)", () => {
      expect(applyStaleness("reading", daysAgo(365), 0, 0, now)).toBe("reading");
    });

    it("returns reading when latestUpdatedAt is undefined", () => {
      expect(applyStaleness("reading", undefined, 30, 60, now)).toBe("reading");
    });

    it("returns reading when latestUpdatedAt is invalid", () => {
      expect(applyStaleness("reading", "not-a-date", 30, 60, now)).toBe("reading");
    });

    it("returns reading when activity is recent", () => {
      expect(applyStaleness("reading", daysAgo(5), 30, 60, now)).toBe("reading");
    });
  });

  describe("pause only (drop disabled)", () => {
    it("pauses after threshold", () => {
      expect(applyStaleness("reading", daysAgo(31), 30, 0, now)).toBe("on_hold");
    });

    it("pauses at exact threshold", () => {
      expect(applyStaleness("reading", daysAgo(30), 30, 0, now)).toBe("on_hold");
    });

    it("does not pause below threshold", () => {
      expect(applyStaleness("reading", daysAgo(29), 30, 0, now)).toBe("reading");
    });
  });

  describe("drop only (pause disabled)", () => {
    it("drops after threshold", () => {
      expect(applyStaleness("reading", daysAgo(61), 0, 60, now)).toBe("dropped");
    });

    it("drops at exact threshold", () => {
      expect(applyStaleness("reading", daysAgo(60), 0, 60, now)).toBe("dropped");
    });

    it("does not drop below threshold", () => {
      expect(applyStaleness("reading", daysAgo(59), 0, 60, now)).toBe("reading");
    });
  });

  describe("both pause and drop enabled", () => {
    it("pauses when inactive past pause but not drop threshold", () => {
      // pause=30, drop=60, inactive=45 → pause
      expect(applyStaleness("reading", daysAgo(45), 30, 60, now)).toBe("on_hold");
    });

    it("drops when inactive past both thresholds (drop takes priority)", () => {
      // pause=30, drop=60, inactive=90 → drop (stronger action)
      expect(applyStaleness("reading", daysAgo(90), 30, 60, now)).toBe("dropped");
    });

    it("drops at exact drop threshold even when pause threshold is also met", () => {
      expect(applyStaleness("reading", daysAgo(60), 30, 60, now)).toBe("dropped");
    });

    it("does nothing when active within both thresholds", () => {
      expect(applyStaleness("reading", daysAgo(10), 30, 60, now)).toBe("reading");
    });
  });

  describe("edge cases", () => {
    it("handles future latestUpdatedAt (0 days inactive)", () => {
      const future = new Date(now + 24 * 60 * 60 * 1000).toISOString();
      expect(applyStaleness("reading", future, 30, 60, now)).toBe("reading");
    });

    it("handles very old latestUpdatedAt", () => {
      expect(applyStaleness("reading", "2020-01-01T00:00:00Z", 30, 60, now)).toBe("dropped");
    });

    it("uses Date.now() when now parameter is omitted", () => {
      // Activity 1000 days ago with threshold of 1 day → should pause
      const veryOld = new Date(Date.now() - 1000 * 24 * 60 * 60 * 1000).toISOString();
      expect(applyStaleness("reading", veryOld, 1, 0)).toBe("on_hold");
    });
  });
});

// =============================================================================
// pushProgress — searchFallback toggle Tests
// =============================================================================

describe("pushProgress searchFallback", () => {
  function makeMockClient(overrides?: {
    searchManga?: AniListClient["searchManga"];
    saveEntry?: AniListClient["saveEntry"];
    getMangaList?: AniListClient["getMangaList"];
  }) {
    return {
      getViewer: vi.fn(),
      getMangaList:
        overrides?.getMangaList ??
        vi.fn().mockResolvedValue({
          pageInfo: { total: 0, currentPage: 1, lastPage: 1, hasNextPage: false },
          entries: [],
        }),
      saveEntry:
        overrides?.saveEntry ??
        vi.fn().mockResolvedValue({
          id: 1,
          mediaId: 42,
          status: "CURRENT",
          score: 0,
          progress: 0,
          progressVolumes: 1,
        }),
      searchManga: overrides?.searchManga ?? vi.fn().mockResolvedValue(null),
    } as unknown as AniListClient;
  }

  afterEach(() => {
    setClient(null);
    setViewerId(null);
    setSearchFallback(false); // restore default
  });

  it("resolves entry via searchManga when searchFallback=true and externalId is empty", async () => {
    setSearchFallback(true);
    const mockClient = makeMockClient({
      searchManga: vi.fn().mockResolvedValue({ id: 42, title: { english: "One Piece" } }),
    });
    setClient(mockClient);
    setViewerId(1);

    const result = await provider.pushProgress({
      entries: [
        {
          externalId: "",
          title: "One Piece",
          status: "reading",
          progress: { volumes: 5 },
        },
      ],
    });

    expect(result.success).toHaveLength(1);
    expect(result.failed).toHaveLength(0);
    expect(result.success[0].externalId).toBe("42");
    expect(result.success[0].status).toBe("created");
    expect(mockClient.searchManga).toHaveBeenCalledWith("One Piece");
  });

  it("fails entry when searchFallback=false and externalId is empty", async () => {
    setSearchFallback(false);
    const mockClient = makeMockClient({
      searchManga: vi.fn().mockResolvedValue({ id: 42, title: { english: "One Piece" } }),
    });
    setClient(mockClient);
    setViewerId(1);

    const result = await provider.pushProgress({
      entries: [
        {
          externalId: "",
          title: "One Piece",
          status: "reading",
          progress: { volumes: 5 },
        },
      ],
    });

    expect(result.success).toHaveLength(0);
    expect(result.failed).toHaveLength(1);
    expect(result.failed[0].status).toBe("failed");
    expect(result.failed[0].error).toContain("Invalid media ID");
    expect(mockClient.searchManga).not.toHaveBeenCalled();
  });

  it("fails entry when searchFallback=true but search returns no result", async () => {
    setSearchFallback(true);
    const mockClient = makeMockClient({
      searchManga: vi.fn().mockResolvedValue(null),
    });
    setClient(mockClient);
    setViewerId(1);

    const result = await provider.pushProgress({
      entries: [
        {
          externalId: "",
          title: "Obscure Manga",
          status: "reading",
          progress: { volumes: 1 },
        },
      ],
    });

    expect(result.success).toHaveLength(0);
    expect(result.failed).toHaveLength(1);
    expect(result.failed[0].error).toContain("No AniList match found");
    expect(mockClient.searchManga).toHaveBeenCalledWith("Obscure Manga");
  });

  it("does not call searchManga when externalId is a valid number", async () => {
    setSearchFallback(true);
    const mockClient = makeMockClient();
    setClient(mockClient);
    setViewerId(1);

    const result = await provider.pushProgress({
      entries: [
        {
          externalId: "42",
          status: "reading",
          progress: { volumes: 3 },
        },
      ],
    });

    expect(result.success).toHaveLength(1);
    expect(result.success[0].externalId).toBe("42");
    expect(mockClient.searchManga).not.toHaveBeenCalled();
  });

  it("reports 'updated' when mediaId already exists in user list", async () => {
    setSearchFallback(true);
    const mockClient = makeMockClient({
      searchManga: vi.fn().mockResolvedValue({ id: 100, title: { english: "Known" } }),
      getMangaList: vi.fn().mockResolvedValue({
        pageInfo: { total: 1, currentPage: 1, lastPage: 1, hasNextPage: false },
        entries: [{ mediaId: 100 }],
      }),
    });
    setClient(mockClient);
    setViewerId(1);

    const result = await provider.pushProgress({
      entries: [
        {
          externalId: "",
          title: "Known",
          status: "reading",
          progress: { volumes: 2 },
        },
      ],
    });

    expect(result.success).toHaveLength(1);
    expect(result.success[0].status).toBe("updated");
  });
});
