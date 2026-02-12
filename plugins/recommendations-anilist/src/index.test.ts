import { EXTERNAL_ID_SOURCE_ANILIST, type UserLibraryEntry } from "@ashdev/codex-plugin-sdk";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { AniListRecommendationNode } from "./anilist.js";
import {
  convertRecommendations,
  dismissedIds,
  mapAniListStatus,
  pickSeedEntries,
  resolveAniListIds,
  setClient,
  setSearchFallback,
} from "./index.js";

// =============================================================================
// Helpers
// =============================================================================

function makeEntry(overrides: Partial<UserLibraryEntry> & { seriesId: string }): UserLibraryEntry {
  return {
    title: `Series ${overrides.seriesId}`,
    alternateTitles: [],
    genres: [],
    tags: [],
    externalIds: [],
    booksRead: 0,
    booksOwned: 0,
    ...overrides,
  };
}

function makeNode(
  overrides: Partial<{
    id: number;
    rating: number;
    averageScore: number | null;
    title: string;
    genres: string[];
    description: string | null;
    siteUrl: string;
    coverImage: string | null;
    popularity: number | null;
    status: AniListRecommendationNode["mediaRecommendation"] extends infer T
      ? T extends { status: infer S }
        ? S
        : never
      : never;
    volumes: number | null;
    mediaRecommendation: AniListRecommendationNode["mediaRecommendation"];
  }>,
): AniListRecommendationNode {
  // Allow passing null mediaRecommendation explicitly
  if ("mediaRecommendation" in overrides && overrides.mediaRecommendation === null) {
    return { rating: overrides.rating ?? 50, mediaRecommendation: null };
  }
  return {
    rating: overrides.rating ?? 50,
    mediaRecommendation: {
      id: overrides.id ?? 100,
      title: { english: overrides.title ?? "Recommended Manga" },
      coverImage: {
        large:
          "coverImage" in overrides
            ? (overrides.coverImage ?? undefined)
            : "https://img.example.com/cover.jpg",
      },
      description: "description" in overrides ? (overrides.description ?? null) : "A great manga",
      genres: overrides.genres ?? ["Action"],
      averageScore: "averageScore" in overrides ? (overrides.averageScore ?? null) : 80,
      popularity: "popularity" in overrides ? (overrides.popularity ?? null) : 5000,
      siteUrl: overrides.siteUrl ?? `https://anilist.co/manga/${overrides.id ?? 100}`,
      status: "status" in overrides ? (overrides.status ?? null) : null,
      volumes: "volumes" in overrides ? (overrides.volumes ?? null) : null,
    },
  };
}

// =============================================================================
// pickSeedEntries Tests
// =============================================================================

describe("pickSeedEntries", () => {
  it("returns empty array for empty library", () => {
    expect(pickSeedEntries([], 10)).toEqual([]);
  });

  it("returns all entries when fewer than maxSeeds", () => {
    const entries = [makeEntry({ seriesId: "a", userRating: 80, booksRead: 5 })];
    const result = pickSeedEntries(entries, 10);
    expect(result).toHaveLength(1);
    expect(result[0].seriesId).toBe("a");
  });

  it("limits to maxSeeds", () => {
    const entries = Array.from({ length: 20 }, (_, i) =>
      makeEntry({ seriesId: `s${i}`, userRating: 50, booksRead: 1 }),
    );
    const result = pickSeedEntries(entries, 5);
    expect(result).toHaveLength(5);
  });

  it("prioritizes by rating descending", () => {
    const entries = [
      makeEntry({ seriesId: "low", userRating: 30, booksRead: 10 }),
      makeEntry({ seriesId: "high", userRating: 90, booksRead: 1 }),
      makeEntry({ seriesId: "mid", userRating: 60, booksRead: 5 }),
    ];
    const result = pickSeedEntries(entries, 3);
    expect(result.map((e) => e.seriesId)).toEqual(["high", "mid", "low"]);
  });

  it("breaks ties by booksRead descending", () => {
    const entries = [
      makeEntry({ seriesId: "fewer", userRating: 80, booksRead: 2 }),
      makeEntry({ seriesId: "more", userRating: 80, booksRead: 10 }),
    ];
    const result = pickSeedEntries(entries, 2);
    expect(result[0].seriesId).toBe("more");
    expect(result[1].seriesId).toBe("fewer");
  });

  it("treats undefined rating as 0", () => {
    const entries = [
      makeEntry({ seriesId: "rated", userRating: 50, booksRead: 1 }),
      makeEntry({ seriesId: "unrated", booksRead: 1 }),
    ];
    const result = pickSeedEntries(entries, 2);
    expect(result[0].seriesId).toBe("rated");
    expect(result[1].seriesId).toBe("unrated");
  });

  it("does not mutate the original array", () => {
    const entries = [
      makeEntry({ seriesId: "b", userRating: 90, booksRead: 1 }),
      makeEntry({ seriesId: "a", userRating: 50, booksRead: 1 }),
    ];
    const originalOrder = entries.map((e) => e.seriesId);
    pickSeedEntries(entries, 2);
    expect(entries.map((e) => e.seriesId)).toEqual(originalOrder);
  });
});

// =============================================================================
// convertRecommendations Tests
// =============================================================================

describe("convertRecommendations", () => {
  beforeEach(() => {
    dismissedIds.clear();
  });

  it("computes score from community rating and average score", () => {
    // rating=80, averageScore=70 → communityScore=0.8, avgScore=0.7
    // score = round((0.8*0.6 + 0.7*0.4) * 100) / 100 = round((0.48+0.28)*100)/100 = 0.76
    const nodes = [makeNode({ id: 1, rating: 80, averageScore: 70 })];
    const results = convertRecommendations(nodes, "Berserk", new Set(), new Set());
    expect(results).toHaveLength(1);
    expect(results[0].score).toBe(0.76);
  });

  it("uses 0.5 fallback when averageScore is null", () => {
    // rating=100, averageScore=null → communityScore=1.0, avgScore=0.5
    // score = round((1.0*0.6 + 0.5*0.4)*100)/100 = round(0.8*100)/100 = 0.8
    const nodes = [makeNode({ id: 1, rating: 100, averageScore: null })];
    const results = convertRecommendations(nodes, "Berserk", new Set(), new Set());
    expect(results).toHaveLength(1);
    expect(results[0].score).toBe(0.8);
  });

  it("uses 0.5 fallback when averageScore is 0 (falsy)", () => {
    // rating=0, averageScore=0 → communityScore=0, avgScore=0.5 (0 is falsy → fallback)
    // score = round((0*0.6 + 0.5*0.4)*100)/100 = 0.2
    const nodes = [makeNode({ id: 1, rating: 0, averageScore: 0 })];
    const results = convertRecommendations(nodes, "Berserk", new Set(), new Set());
    expect(results).toHaveLength(1);
    expect(results[0].score).toBe(0.2);
  });

  it("clamps negative community rating to 0", () => {
    // rating=-50 → communityScore = max(0, min(-50,100))/100 = 0
    // averageScore=80 → avgScore = 0.8
    // score = round((0*0.6 + 0.8*0.4)*100)/100 = 0.32
    const nodes = [makeNode({ id: 1, rating: -50, averageScore: 80 })];
    const results = convertRecommendations(nodes, "Berserk", new Set(), new Set());
    expect(results).toHaveLength(1);
    expect(results[0].score).toBe(0.32);
  });

  it("clamps community rating above 100 to 100", () => {
    // rating=200 → communityScore = max(0, min(200,100))/100 = 1.0
    // averageScore=100 → avgScore = 1.0
    // score = round((1.0*0.6 + 1.0*0.4)*100)/100 = 1.0
    const nodes = [makeNode({ id: 1, rating: 200, averageScore: 100 })];
    const results = convertRecommendations(nodes, "Berserk", new Set(), new Set());
    expect(results).toHaveLength(1);
    expect(results[0].score).toBe(1.0);
  });

  it("clamps final score to [0, 1]", () => {
    // Even with maximum inputs, score should not exceed 1.0
    const nodes = [makeNode({ id: 1, rating: 100, averageScore: 100 })];
    const results = convertRecommendations(nodes, "Berserk", new Set(), new Set());
    expect(results[0].score).toBeLessThanOrEqual(1.0);
    expect(results[0].score).toBeGreaterThanOrEqual(0);
  });

  it("excludes nodes with IDs in excludeIds", () => {
    const nodes = [makeNode({ id: 1, rating: 80 }), makeNode({ id: 2, rating: 90 })];
    const results = convertRecommendations(nodes, "Berserk", new Set(), new Set(["1"]));
    expect(results).toHaveLength(1);
    expect(results[0].externalId).toBe("2");
  });

  it("excludes nodes with IDs in dismissedIds", () => {
    dismissedIds.add("1");
    const nodes = [makeNode({ id: 1, rating: 80 }), makeNode({ id: 2, rating: 90 })];
    const results = convertRecommendations(nodes, "Berserk", new Set(), new Set());
    expect(results).toHaveLength(1);
    expect(results[0].externalId).toBe("2");
  });

  it("filters out nodes with null mediaRecommendation", () => {
    const nodes = [
      makeNode({ mediaRecommendation: null, rating: 50 }),
      makeNode({ id: 2, rating: 90 }),
    ];
    const results = convertRecommendations(nodes, "Berserk", new Set(), new Set());
    expect(results).toHaveLength(1);
    expect(results[0].externalId).toBe("2");
  });

  it("sets inLibrary flag when manga is in userMangaIds", () => {
    const nodes = [makeNode({ id: 1, rating: 80 }), makeNode({ id: 2, rating: 80 })];
    const userMangaIds = new Set([1]);
    const results = convertRecommendations(nodes, "Berserk", userMangaIds, new Set());
    expect(results[0].inLibrary).toBe(true);
    expect(results[1].inLibrary).toBe(false);
  });

  it("returns empty array for empty nodes", () => {
    const results = convertRecommendations([], "Berserk", new Set(), new Set());
    expect(results).toEqual([]);
  });

  it("constructs basedOn field from basedOnTitle", () => {
    const nodes = [makeNode({ id: 1, rating: 80 })];
    const results = convertRecommendations(nodes, "Attack on Titan", new Set(), new Set());
    expect(results[0].basedOn).toEqual(["Attack on Titan"]);
    expect(results[0].reason).toBe("Recommended because you liked Attack on Titan");
  });

  it("sets externalId, externalUrl, title, coverUrl, summary, genres", () => {
    const nodes = [
      makeNode({
        id: 42,
        rating: 50,
        averageScore: 60,
        title: "One Piece",
        genres: ["Adventure", "Comedy"],
        description: "A pirate adventure",
        siteUrl: "https://anilist.co/manga/42",
        coverImage: "https://img.example.com/onepiece.jpg",
      }),
    ];
    const results = convertRecommendations(nodes, "Naruto", new Set(), new Set());
    expect(results).toHaveLength(1);
    const rec = results[0];
    expect(rec.externalId).toBe("42");
    expect(rec.externalUrl).toBe("https://anilist.co/manga/42");
    expect(rec.title).toBe("One Piece");
    expect(rec.coverUrl).toBe("https://img.example.com/onepiece.jpg");
    expect(rec.summary).toBe("A pirate adventure");
    expect(rec.genres).toEqual(["Adventure", "Comedy"]);
  });

  it("handles coverImage.large being undefined", () => {
    const nodes = [makeNode({ id: 1, rating: 50, coverImage: null })];
    // coverImage.large is null → undefined via ?? undefined
    const results = convertRecommendations(nodes, "Test", new Set(), new Set());
    expect(results[0].coverUrl).toBeUndefined();
  });

  it("handles null description via stripHtml", () => {
    const nodes = [makeNode({ id: 1, rating: 50, description: null })];
    const results = convertRecommendations(nodes, "Test", new Set(), new Set());
    expect(results[0].summary).toBeUndefined();
  });

  it("maps RELEASING status to ongoing", () => {
    const nodes = [makeNode({ id: 1, rating: 50, status: "RELEASING" })];
    const results = convertRecommendations(nodes, "Test", new Set(), new Set());
    expect(results[0].status).toBe("ongoing");
  });

  it("maps FINISHED status to ended", () => {
    const nodes = [makeNode({ id: 1, rating: 50, status: "FINISHED" })];
    const results = convertRecommendations(nodes, "Test", new Set(), new Set());
    expect(results[0].status).toBe("ended");
  });

  it("maps HIATUS status to hiatus", () => {
    const nodes = [makeNode({ id: 1, rating: 50, status: "HIATUS" })];
    const results = convertRecommendations(nodes, "Test", new Set(), new Set());
    expect(results[0].status).toBe("hiatus");
  });

  it("maps CANCELLED status to abandoned", () => {
    const nodes = [makeNode({ id: 1, rating: 50, status: "CANCELLED" })];
    const results = convertRecommendations(nodes, "Test", new Set(), new Set());
    expect(results[0].status).toBe("abandoned");
  });

  it("leaves status undefined when null", () => {
    const nodes = [makeNode({ id: 1, rating: 50, status: null })];
    const results = convertRecommendations(nodes, "Test", new Set(), new Set());
    expect(results[0].status).toBeUndefined();
  });

  it("includes totalBookCount from volumes", () => {
    const nodes = [makeNode({ id: 1, rating: 50, volumes: 27 })];
    const results = convertRecommendations(nodes, "Test", new Set(), new Set());
    expect(results[0].totalBookCount).toBe(27);
  });

  it("leaves totalBookCount undefined when volumes is null", () => {
    const nodes = [makeNode({ id: 1, rating: 50, volumes: null })];
    const results = convertRecommendations(nodes, "Test", new Set(), new Set());
    expect(results[0].totalBookCount).toBeUndefined();
  });

  it("includes rating from AniList averageScore", () => {
    const nodes = [makeNode({ id: 1, rating: 50, averageScore: 85 })];
    const results = convertRecommendations(nodes, "Test", new Set(), new Set());
    expect(results[0].rating).toBe(85);
  });

  it("leaves rating undefined when averageScore is null", () => {
    const nodes = [makeNode({ id: 1, rating: 50, averageScore: null })];
    const results = convertRecommendations(nodes, "Test", new Set(), new Set());
    expect(results[0].rating).toBeUndefined();
  });

  it("includes popularity from AniList", () => {
    const nodes = [makeNode({ id: 1, rating: 50, popularity: 120000 })];
    const results = convertRecommendations(nodes, "Test", new Set(), new Set());
    expect(results[0].popularity).toBe(120000);
  });

  it("leaves popularity undefined when null", () => {
    const nodes = [makeNode({ id: 1, rating: 50, popularity: null })];
    const results = convertRecommendations(nodes, "Test", new Set(), new Set());
    expect(results[0].popularity).toBeUndefined();
  });
});

// =============================================================================
// mapAniListStatus Tests
// =============================================================================

describe("mapAniListStatus", () => {
  it("maps RELEASING to ongoing", () => {
    expect(mapAniListStatus("RELEASING")).toBe("ongoing");
  });

  it("maps FINISHED to ended", () => {
    expect(mapAniListStatus("FINISHED")).toBe("ended");
  });

  it("maps HIATUS to hiatus", () => {
    expect(mapAniListStatus("HIATUS")).toBe("hiatus");
  });

  it("maps CANCELLED to abandoned", () => {
    expect(mapAniListStatus("CANCELLED")).toBe("abandoned");
  });

  it("maps NOT_YET_RELEASED to unknown", () => {
    expect(mapAniListStatus("NOT_YET_RELEASED")).toBe("unknown");
  });

  it("returns undefined for null", () => {
    expect(mapAniListStatus(null)).toBeUndefined();
  });
});

// =============================================================================
// resolveAniListIds Tests
// =============================================================================

describe("resolveAniListIds", () => {
  afterEach(() => {
    setClient(null);
  });

  function makeMockClient(overrides: {
    searchManga?: (
      title: string,
    ) => Promise<{ id: number; title: { romaji?: string; english?: string } } | null>;
  }) {
    return {
      getViewerId: vi.fn(),
      getRecommendationsForMedia: vi.fn(),
      getUserMangaIds: vi.fn(),
      searchManga: overrides.searchManga ?? vi.fn().mockResolvedValue(null),
    } as unknown as Parameters<typeof setClient>[0];
  }

  it("resolves entries with api:anilist external ID directly", async () => {
    const mockClient = makeMockClient({});
    setClient(mockClient);

    const entries = [
      makeEntry({
        seriesId: "s1",
        title: "Berserk",
        userRating: 90,
        externalIds: [{ source: EXTERNAL_ID_SOURCE_ANILIST, externalId: "21" }],
      }),
    ];

    const result = await resolveAniListIds(entries);
    expect(result.size).toBe(1);
    expect(result.get("s1")).toEqual({ anilistId: 21, title: "Berserk", rating: 90 });
    // Should NOT call searchManga since external ID was found
    expect(mockClient.searchManga).not.toHaveBeenCalled();
  });

  it("resolves entries with legacy 'anilist' source", async () => {
    const mockClient = makeMockClient({});
    setClient(mockClient);

    const entries = [
      makeEntry({
        seriesId: "s1",
        title: "Berserk",
        externalIds: [{ source: "anilist", externalId: "21" }],
      }),
    ];

    const result = await resolveAniListIds(entries);
    expect(result.size).toBe(1);
    expect(result.get("s1")?.anilistId).toBe(21);
    expect(mockClient.searchManga).not.toHaveBeenCalled();
  });

  it("resolves entries with legacy 'AniList' source (case variation)", async () => {
    const mockClient = makeMockClient({});
    setClient(mockClient);

    const entries = [
      makeEntry({
        seriesId: "s1",
        title: "Berserk",
        externalIds: [{ source: "AniList", externalId: "21" }],
      }),
    ];

    const result = await resolveAniListIds(entries);
    expect(result.size).toBe(1);
    expect(result.get("s1")?.anilistId).toBe(21);
  });

  it("falls back to title search when no external IDs", async () => {
    const mockClient = makeMockClient({
      searchManga: vi.fn().mockResolvedValue({ id: 42, title: { english: "Berserk" } }),
    });
    setClient(mockClient);

    const entries = [makeEntry({ seriesId: "s1", title: "Berserk", userRating: 75 })];

    const result = await resolveAniListIds(entries);
    expect(result.size).toBe(1);
    expect(result.get("s1")).toEqual({ anilistId: 42, title: "Berserk", rating: 75 });
    expect(mockClient.searchManga).toHaveBeenCalledWith("Berserk");
  });

  it("skips entry when external ID is non-numeric and falls back to search", async () => {
    const mockClient = makeMockClient({
      searchManga: vi.fn().mockResolvedValue({ id: 99, title: { english: "Test" } }),
    });
    setClient(mockClient);

    const entries = [
      makeEntry({
        seriesId: "s1",
        title: "Test",
        externalIds: [{ source: EXTERNAL_ID_SOURCE_ANILIST, externalId: "not-a-number" }],
      }),
    ];

    const result = await resolveAniListIds(entries);
    // Should fall through to search since NaN is not valid
    expect(result.size).toBe(1);
    expect(result.get("s1")?.anilistId).toBe(99);
    expect(mockClient.searchManga).toHaveBeenCalledWith("Test");
  });

  it("skips entry when search returns null", async () => {
    const mockClient = makeMockClient({
      searchManga: vi.fn().mockResolvedValue(null),
    });
    setClient(mockClient);

    const entries = [makeEntry({ seriesId: "s1", title: "Obscure Manga" })];

    const result = await resolveAniListIds(entries);
    expect(result.size).toBe(0);
  });

  it("preserves rating and title in result map", async () => {
    const mockClient = makeMockClient({});
    setClient(mockClient);

    const entries = [
      makeEntry({
        seriesId: "s1",
        title: "My Manga",
        userRating: 85,
        externalIds: [{ source: EXTERNAL_ID_SOURCE_ANILIST, externalId: "55" }],
      }),
    ];

    const result = await resolveAniListIds(entries);
    const entry = result.get("s1");
    expect(entry?.title).toBe("My Manga");
    expect(entry?.rating).toBe(85);
  });

  it("treats undefined userRating as 0", async () => {
    const mockClient = makeMockClient({});
    setClient(mockClient);

    const entries = [
      makeEntry({
        seriesId: "s1",
        title: "Unrated",
        externalIds: [{ source: EXTERNAL_ID_SOURCE_ANILIST, externalId: "10" }],
      }),
    ];

    const result = await resolveAniListIds(entries);
    expect(result.get("s1")?.rating).toBe(0);
  });

  it("throws when client is not initialized", async () => {
    setClient(null);
    await expect(resolveAniListIds([])).rejects.toThrow("Plugin not initialized");
  });

  it("resolves multiple entries", async () => {
    const mockClient = makeMockClient({
      searchManga: vi.fn().mockResolvedValue({ id: 77, title: { english: "Found" } }),
    });
    setClient(mockClient);

    const entries = [
      makeEntry({
        seriesId: "s1",
        title: "Has ID",
        externalIds: [{ source: EXTERNAL_ID_SOURCE_ANILIST, externalId: "10" }],
      }),
      makeEntry({ seriesId: "s2", title: "Needs Search" }),
      makeEntry({ seriesId: "s3", title: "Also Needs Search" }),
    ];

    const result = await resolveAniListIds(entries);
    expect(result.size).toBe(3);
    expect(result.get("s1")?.anilistId).toBe(10);
    expect(result.get("s2")?.anilistId).toBe(77);
    expect(result.get("s3")?.anilistId).toBe(77);
    expect(mockClient.searchManga).toHaveBeenCalledTimes(2);
  });
});

// =============================================================================
// resolveAniListIds — searchFallback toggle Tests
// =============================================================================

describe("resolveAniListIds searchFallback toggle", () => {
  afterEach(() => {
    setClient(null);
    setSearchFallback(true); // restore default
  });

  function makeMockClient(overrides: {
    searchManga?: (
      title: string,
    ) => Promise<{ id: number; title: { romaji?: string; english?: string } } | null>;
  }) {
    return {
      getViewerId: vi.fn(),
      getRecommendationsForMedia: vi.fn(),
      getUserMangaIds: vi.fn(),
      searchManga: overrides.searchManga ?? vi.fn().mockResolvedValue(null),
    } as unknown as Parameters<typeof setClient>[0];
  }

  it("calls searchManga when searchFallback is true and no external ID", async () => {
    setSearchFallback(true);
    const mockClient = makeMockClient({
      searchManga: vi.fn().mockResolvedValue({ id: 42, title: { english: "Berserk" } }),
    });
    setClient(mockClient);

    const entries = [makeEntry({ seriesId: "s1", title: "Berserk", userRating: 75 })];
    const result = await resolveAniListIds(entries);

    expect(result.size).toBe(1);
    expect(result.get("s1")?.anilistId).toBe(42);
    expect(mockClient.searchManga).toHaveBeenCalledWith("Berserk");
  });

  it("skips searchManga when searchFallback is false and no external ID", async () => {
    setSearchFallback(false);
    const mockClient = makeMockClient({
      searchManga: vi.fn().mockResolvedValue({ id: 42, title: { english: "Berserk" } }),
    });
    setClient(mockClient);

    const entries = [makeEntry({ seriesId: "s1", title: "Berserk", userRating: 75 })];
    const result = await resolveAniListIds(entries);

    expect(result.size).toBe(0);
    expect(mockClient.searchManga).not.toHaveBeenCalled();
  });

  it("still resolves via external ID when searchFallback is false", async () => {
    setSearchFallback(false);
    const mockClient = makeMockClient({});
    setClient(mockClient);

    const entries = [
      makeEntry({
        seriesId: "s1",
        title: "Berserk",
        externalIds: [{ source: EXTERNAL_ID_SOURCE_ANILIST, externalId: "21" }],
      }),
    ];
    const result = await resolveAniListIds(entries);

    expect(result.size).toBe(1);
    expect(result.get("s1")?.anilistId).toBe(21);
    expect(mockClient.searchManga).not.toHaveBeenCalled();
  });

  it("mixes matched and unmatched entries with searchFallback disabled", async () => {
    setSearchFallback(false);
    const mockClient = makeMockClient({
      searchManga: vi.fn().mockResolvedValue({ id: 99, title: { english: "Found" } }),
    });
    setClient(mockClient);

    const entries = [
      makeEntry({
        seriesId: "s1",
        title: "Has ID",
        externalIds: [{ source: EXTERNAL_ID_SOURCE_ANILIST, externalId: "10" }],
      }),
      makeEntry({ seriesId: "s2", title: "No ID" }),
    ];
    const result = await resolveAniListIds(entries);

    expect(result.size).toBe(1);
    expect(result.has("s1")).toBe(true);
    expect(result.has("s2")).toBe(false);
    expect(mockClient.searchManga).not.toHaveBeenCalled();
  });
});

// =============================================================================
// Score Merging Tests (via convertRecommendations + simulated merge loop)
// =============================================================================

describe("score merging", () => {
  beforeEach(() => {
    dismissedIds.clear();
  });

  /**
   * Simulate the merge loop from provider.get():
   * For each rec, if externalId already exists, boost score by 0.05 and merge basedOn.
   */
  function mergeRecommendations(
    recsBySource: Array<{ basedOnTitle: string; nodes: AniListRecommendationNode[] }>,
    userMangaIds = new Set<number>(),
    excludeIds = new Set<string>(),
  ): Map<string, { score: number; basedOn: string[]; reason: string }> {
    const allRecs = new Map<string, { score: number; basedOn: string[]; reason: string }>();

    for (const { basedOnTitle, nodes } of recsBySource) {
      const recs = convertRecommendations(nodes, basedOnTitle, userMangaIds, excludeIds);
      for (const rec of recs) {
        const existing = allRecs.get(rec.externalId);
        if (existing) {
          const mergedBasedOn = [...new Set([...existing.basedOn, ...rec.basedOn])];
          const boostedScore = Math.min(existing.score + 0.05, 1.0);
          allRecs.set(rec.externalId, {
            score: Math.round(boostedScore * 100) / 100,
            basedOn: mergedBasedOn,
            reason:
              mergedBasedOn.length > 1
                ? `Recommended based on ${mergedBasedOn.join(", ")}`
                : existing.reason,
          });
        } else {
          allRecs.set(rec.externalId, {
            score: rec.score,
            basedOn: rec.basedOn,
            reason: rec.reason,
          });
        }
      }
    }

    return allRecs;
  }

  it("boosts score by 0.05 for duplicate recommendation", () => {
    // Same manga recommended from two different seeds
    // rating=80, averageScore=80 → communityScore=0.8, avgScore=0.8
    // score = round((0.8*0.6 + 0.8*0.4)*100)/100 = 0.8
    const node = makeNode({ id: 1, rating: 80, averageScore: 80 });
    const merged = mergeRecommendations([
      { basedOnTitle: "Berserk", nodes: [node] },
      { basedOnTitle: "Vagabond", nodes: [node] },
    ]);

    const rec = merged.get("1");
    expect(rec).toBeDefined();
    // 0.8 + 0.05 = 0.85
    expect(rec?.score).toBe(0.85);
  });

  it("clamps boosted score at 1.0", () => {
    // rating=100, averageScore=100 → score = 1.0
    const node = makeNode({ id: 1, rating: 100, averageScore: 100 });
    const merged = mergeRecommendations([
      { basedOnTitle: "Seed A", nodes: [node] },
      { basedOnTitle: "Seed B", nodes: [node] },
    ]);

    const rec = merged.get("1");
    expect(rec).toBeDefined();
    // 1.0 + 0.05 → clamped to 1.0
    expect(rec?.score).toBe(1.0);
  });

  it("merges and deduplicates basedOn arrays", () => {
    const node = makeNode({ id: 1, rating: 50 });
    const merged = mergeRecommendations([
      { basedOnTitle: "Berserk", nodes: [node] },
      { basedOnTitle: "Vagabond", nodes: [node] },
    ]);

    const rec = merged.get("1");
    expect(rec).toBeDefined();
    expect(rec?.basedOn).toEqual(["Berserk", "Vagabond"]);
  });

  it("updates reason text with multiple sources", () => {
    const node = makeNode({ id: 1, rating: 50 });
    const merged = mergeRecommendations([
      { basedOnTitle: "Berserk", nodes: [node] },
      { basedOnTitle: "Vagabond", nodes: [node] },
    ]);

    const rec = merged.get("1");
    expect(rec).toBeDefined();
    expect(rec?.reason).toBe("Recommended based on Berserk, Vagabond");
  });

  it("applies two boosts for triple-recommended manga", () => {
    // rating=60, averageScore=70 → communityScore=0.6, avgScore=0.7
    // score = round((0.6*0.6 + 0.7*0.4)*100)/100 = round((0.36+0.28)*100)/100 = 0.64
    const node = makeNode({ id: 1, rating: 60, averageScore: 70 });
    const merged = mergeRecommendations([
      { basedOnTitle: "Seed A", nodes: [node] },
      { basedOnTitle: "Seed B", nodes: [node] },
      { basedOnTitle: "Seed C", nodes: [node] },
    ]);

    const rec = merged.get("1");
    expect(rec).toBeDefined();
    // 0.64 + 0.05 + 0.05 = 0.74
    expect(rec?.score).toBe(0.74);
    expect(rec?.basedOn).toEqual(["Seed A", "Seed B", "Seed C"]);
    expect(rec?.reason).toBe("Recommended based on Seed A, Seed B, Seed C");
  });

  it("does not boost same basedOn title appearing twice", () => {
    const node = makeNode({ id: 1, rating: 50, averageScore: 50 });
    const merged = mergeRecommendations([
      { basedOnTitle: "Berserk", nodes: [node] },
      { basedOnTitle: "Berserk", nodes: [node] },
    ]);

    const rec = merged.get("1");
    expect(rec).toBeDefined();
    // basedOn is deduplicated
    expect(rec?.basedOn).toEqual(["Berserk"]);
    // Score is still boosted (different source instances)
    // 0.5 + 0.05 = 0.55
    expect(rec?.score).toBe(0.55);
    // Single basedOn means reason stays as original
    expect(rec?.reason).toBe("Recommended because you liked Berserk");
  });
});
