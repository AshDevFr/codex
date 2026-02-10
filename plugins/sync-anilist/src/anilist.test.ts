import { afterEach, describe, expect, it, vi } from "vitest";
import {
  AniListClient,
  anilistStatusToSync,
  convertScoreFromAnilist,
  convertScoreToAnilist,
  fuzzyDateToIso,
  isoToFuzzyDate,
  syncStatusToAnilist,
} from "./anilist.js";

// =============================================================================
// Status Mapping Tests
// =============================================================================

describe("anilistStatusToSync", () => {
  it("maps CURRENT to reading", () => {
    expect(anilistStatusToSync("CURRENT")).toBe("reading");
  });

  it("maps REPEATING to reading", () => {
    expect(anilistStatusToSync("REPEATING")).toBe("reading");
  });

  it("maps COMPLETED to completed", () => {
    expect(anilistStatusToSync("COMPLETED")).toBe("completed");
  });

  it("maps PAUSED to on_hold", () => {
    expect(anilistStatusToSync("PAUSED")).toBe("on_hold");
  });

  it("maps DROPPED to dropped", () => {
    expect(anilistStatusToSync("DROPPED")).toBe("dropped");
  });

  it("maps PLANNING to plan_to_read", () => {
    expect(anilistStatusToSync("PLANNING")).toBe("plan_to_read");
  });

  it("maps unknown status to reading", () => {
    expect(anilistStatusToSync("UNKNOWN")).toBe("reading");
  });
});

describe("syncStatusToAnilist", () => {
  it("maps reading to CURRENT", () => {
    expect(syncStatusToAnilist("reading")).toBe("CURRENT");
  });

  it("maps completed to COMPLETED", () => {
    expect(syncStatusToAnilist("completed")).toBe("COMPLETED");
  });

  it("maps on_hold to PAUSED", () => {
    expect(syncStatusToAnilist("on_hold")).toBe("PAUSED");
  });

  it("maps dropped to DROPPED", () => {
    expect(syncStatusToAnilist("dropped")).toBe("DROPPED");
  });

  it("maps plan_to_read to PLANNING", () => {
    expect(syncStatusToAnilist("plan_to_read")).toBe("PLANNING");
  });

  it("maps unknown status to CURRENT", () => {
    expect(syncStatusToAnilist("unknown")).toBe("CURRENT");
  });
});

// =============================================================================
// Date Conversion Tests
// =============================================================================

describe("fuzzyDateToIso", () => {
  it("converts full date", () => {
    expect(fuzzyDateToIso({ year: 2026, month: 2, day: 6 })).toBe("2026-02-06T00:00:00Z");
  });

  it("converts year and month only", () => {
    expect(fuzzyDateToIso({ year: 2026, month: 3 })).toBe("2026-03-01T00:00:00Z");
  });

  it("converts year only", () => {
    expect(fuzzyDateToIso({ year: 2025 })).toBe("2025-01-01T00:00:00Z");
  });

  it("returns undefined for null date", () => {
    expect(fuzzyDateToIso(null)).toBeUndefined();
  });

  it("returns undefined for undefined date", () => {
    expect(fuzzyDateToIso(undefined)).toBeUndefined();
  });

  it("returns undefined when year is null", () => {
    expect(fuzzyDateToIso({ year: null })).toBeUndefined();
  });

  it("pads month and day", () => {
    expect(fuzzyDateToIso({ year: 2026, month: 1, day: 5 })).toBe("2026-01-05T00:00:00Z");
  });
});

describe("isoToFuzzyDate", () => {
  it("converts ISO date string", () => {
    const result = isoToFuzzyDate("2026-02-06T00:00:00Z");
    expect(result).toEqual({ year: 2026, month: 2, day: 6 });
  });

  it("converts ISO datetime", () => {
    const result = isoToFuzzyDate("2025-12-25T14:30:00Z");
    expect(result).toEqual({ year: 2025, month: 12, day: 25 });
  });

  it("returns undefined for undefined input", () => {
    expect(isoToFuzzyDate(undefined)).toBeUndefined();
  });

  it("returns undefined for empty string", () => {
    expect(isoToFuzzyDate("")).toBeUndefined();
  });

  it("returns undefined for invalid date", () => {
    expect(isoToFuzzyDate("not-a-date")).toBeUndefined();
  });
});

// =============================================================================
// Roundtrip Tests
// =============================================================================

describe("status roundtrip", () => {
  const statuses = [
    { anilist: "CURRENT", sync: "reading" },
    { anilist: "COMPLETED", sync: "completed" },
    { anilist: "PAUSED", sync: "on_hold" },
    { anilist: "DROPPED", sync: "dropped" },
    { anilist: "PLANNING", sync: "plan_to_read" },
  ] as const;

  for (const { anilist, sync } of statuses) {
    it(`roundtrips ${anilist} -> ${sync} -> ${anilist}`, () => {
      const codexStatus = anilistStatusToSync(anilist);
      expect(codexStatus).toBe(sync);
      const backToAnilist = syncStatusToAnilist(codexStatus);
      expect(backToAnilist).toBe(anilist);
    });
  }
});

describe("date roundtrip", () => {
  it("roundtrips a full date", () => {
    const original = { year: 2026, month: 6, day: 15 };
    const iso = fuzzyDateToIso(original);
    const result = isoToFuzzyDate(iso);
    expect(result).toEqual(original);
  });
});

// =============================================================================
// Score Conversion Tests (1-100 Codex scale <-> AniList formats)
// =============================================================================

describe("convertScoreToAnilist (1-100 input)", () => {
  it("POINT_100: pass-through", () => {
    expect(convertScoreToAnilist(85, "POINT_100")).toBe(85);
    expect(convertScoreToAnilist(100, "POINT_100")).toBe(100);
    expect(convertScoreToAnilist(1, "POINT_100")).toBe(1);
  });

  it("POINT_10_DECIMAL: divides by 10", () => {
    expect(convertScoreToAnilist(85, "POINT_10_DECIMAL")).toBe(8.5);
    expect(convertScoreToAnilist(100, "POINT_10_DECIMAL")).toBe(10);
    expect(convertScoreToAnilist(10, "POINT_10_DECIMAL")).toBe(1);
  });

  it("POINT_10: rounds to nearest integer after dividing", () => {
    expect(convertScoreToAnilist(85, "POINT_10")).toBe(9);
    expect(convertScoreToAnilist(84, "POINT_10")).toBe(8);
    expect(convertScoreToAnilist(100, "POINT_10")).toBe(10);
    expect(convertScoreToAnilist(10, "POINT_10")).toBe(1);
  });

  it("POINT_5: maps to 1-5 scale", () => {
    expect(convertScoreToAnilist(100, "POINT_5")).toBe(5);
    expect(convertScoreToAnilist(80, "POINT_5")).toBe(4);
    expect(convertScoreToAnilist(50, "POINT_5")).toBe(3);
    expect(convertScoreToAnilist(20, "POINT_5")).toBe(1);
  });

  it("POINT_3: maps to 1/2/3 based on thresholds", () => {
    expect(convertScoreToAnilist(90, "POINT_3")).toBe(3);
    expect(convertScoreToAnilist(70, "POINT_3")).toBe(3);
    expect(convertScoreToAnilist(69, "POINT_3")).toBe(2);
    expect(convertScoreToAnilist(40, "POINT_3")).toBe(2);
    expect(convertScoreToAnilist(39, "POINT_3")).toBe(1);
    expect(convertScoreToAnilist(1, "POINT_3")).toBe(1);
  });

  it("unknown format: defaults to POINT_10 behavior", () => {
    expect(convertScoreToAnilist(80, "UNKNOWN")).toBe(8);
  });
});

describe("convertScoreFromAnilist (to 1-100 output)", () => {
  it("POINT_100: pass-through", () => {
    expect(convertScoreFromAnilist(85, "POINT_100")).toBe(85);
    expect(convertScoreFromAnilist(100, "POINT_100")).toBe(100);
    expect(convertScoreFromAnilist(1, "POINT_100")).toBe(1);
  });

  it("POINT_10_DECIMAL: multiplies by 10", () => {
    expect(convertScoreFromAnilist(8.5, "POINT_10_DECIMAL")).toBe(85);
    expect(convertScoreFromAnilist(10, "POINT_10_DECIMAL")).toBe(100);
    expect(convertScoreFromAnilist(1, "POINT_10_DECIMAL")).toBe(10);
  });

  it("POINT_10: multiplies by 10", () => {
    expect(convertScoreFromAnilist(8, "POINT_10")).toBe(80);
    expect(convertScoreFromAnilist(10, "POINT_10")).toBe(100);
    expect(convertScoreFromAnilist(1, "POINT_10")).toBe(10);
  });

  it("POINT_5: multiplies by 20", () => {
    expect(convertScoreFromAnilist(5, "POINT_5")).toBe(100);
    expect(convertScoreFromAnilist(4, "POINT_5")).toBe(80);
    expect(convertScoreFromAnilist(1, "POINT_5")).toBe(20);
  });

  it("POINT_3: multiplies by ~33.3", () => {
    expect(convertScoreFromAnilist(3, "POINT_3")).toBe(100);
    expect(convertScoreFromAnilist(2, "POINT_3")).toBe(67);
    expect(convertScoreFromAnilist(1, "POINT_3")).toBe(33);
  });

  it("unknown format: defaults to POINT_10 behavior", () => {
    expect(convertScoreFromAnilist(7, "UNKNOWN")).toBe(70);
  });
});

describe("score roundtrip", () => {
  it("POINT_100 roundtrips exactly", () => {
    const codex = 85;
    const anilist = convertScoreToAnilist(codex, "POINT_100");
    expect(convertScoreFromAnilist(anilist, "POINT_100")).toBe(codex);
  });

  it("POINT_10_DECIMAL roundtrips exactly", () => {
    const codex = 85;
    const anilist = convertScoreToAnilist(codex, "POINT_10_DECIMAL");
    expect(convertScoreFromAnilist(anilist, "POINT_10_DECIMAL")).toBe(codex);
  });

  it("POINT_10 roundtrips within ±5", () => {
    // 85 -> 9 -> 90 (lossy due to rounding)
    const codex = 80;
    const anilist = convertScoreToAnilist(codex, "POINT_10");
    expect(convertScoreFromAnilist(anilist, "POINT_10")).toBe(80);
  });

  it("POINT_5 roundtrips within ±10", () => {
    const codex = 80;
    const anilist = convertScoreToAnilist(codex, "POINT_5");
    expect(convertScoreFromAnilist(anilist, "POINT_5")).toBe(80);
  });
});

// =============================================================================
// AniListClient Fetch Behavior Tests
// =============================================================================

describe("AniListClient fetch behavior", () => {
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

    const client = new AniListClient("test-token");
    await client.getViewer();

    expect(fetchSpy).toHaveBeenCalledOnce();
    const callArgs = fetchSpy.mock.calls[0];
    const init = callArgs[1] as RequestInit;
    expect(init.signal).toBeDefined();
  });

  it("wraps timeout errors with descriptive message", async () => {
    const timeoutError = new DOMException(
      "The operation was aborted due to timeout",
      "TimeoutError",
    );
    vi.spyOn(globalThis, "fetch").mockRejectedValue(timeoutError);

    const client = new AniListClient("test-token");
    await expect(client.getViewer()).rejects.toThrow(
      "AniList API request timed out after 30 seconds",
    );
  });

  it("re-throws non-timeout fetch errors as-is", async () => {
    vi.spyOn(globalThis, "fetch").mockRejectedValue(new Error("Network failure"));

    const client = new AniListClient("test-token");
    await expect(client.getViewer()).rejects.toThrow("Network failure");
  });

  it("retries once on 429 then succeeds", async () => {
    const fetchSpy = vi
      .spyOn(globalThis, "fetch")
      .mockResolvedValueOnce(
        new Response("", {
          status: 429,
          headers: { "Retry-After": "0" },
        }),
      )
      .mockResolvedValueOnce(
        new Response(
          JSON.stringify({
            data: {
              Viewer: {
                id: 1,
                name: "test",
                avatar: {},
                siteUrl: "",
                options: { displayAdultContent: false },
                mediaListOptions: { scoreFormat: "POINT_10" },
              },
            },
          }),
          { status: 200, headers: { "Content-Type": "application/json" } },
        ),
      );

    const client = new AniListClient("test-token");
    const viewer = await client.getViewer();

    expect(viewer.id).toBe(1);
    expect(fetchSpy).toHaveBeenCalledTimes(2);
  });

  it("throws RateLimitError after retry exhausted on 429", async () => {
    vi.spyOn(globalThis, "fetch").mockResolvedValue(
      new Response("", {
        status: 429,
        headers: { "Retry-After": "0" },
      }),
    );

    const client = new AniListClient("test-token");
    await expect(client.getViewer()).rejects.toThrow("AniList rate limit exceeded");
  });
});

// =============================================================================
// searchManga Tests
// =============================================================================

describe("AniListClient.searchManga", () => {
  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("returns search result when found", async () => {
    vi.spyOn(globalThis, "fetch").mockResolvedValueOnce(
      new Response(
        JSON.stringify({
          data: {
            Media: { id: 42, title: { romaji: "Berserk", english: "Berserk" } },
          },
        }),
        { status: 200, headers: { "Content-Type": "application/json" } },
      ),
    );

    const client = new AniListClient("test-token");
    const result = await client.searchManga("Berserk");

    expect(result).not.toBeNull();
    expect(result?.id).toBe(42);
    expect(result?.title.english).toBe("Berserk");
  });

  it("returns null when Media is null", async () => {
    vi.spyOn(globalThis, "fetch").mockResolvedValueOnce(
      new Response(JSON.stringify({ data: { Media: null } }), {
        status: 200,
        headers: { "Content-Type": "application/json" },
      }),
    );

    const client = new AniListClient("test-token");
    const result = await client.searchManga("Nonexistent Manga");

    expect(result).toBeNull();
  });

  it("returns null on API error (swallows exceptions)", async () => {
    vi.spyOn(globalThis, "fetch").mockResolvedValueOnce(
      new Response(JSON.stringify({ errors: [{ message: "Not found" }] }), {
        status: 200,
        headers: { "Content-Type": "application/json" },
      }),
    );

    const client = new AniListClient("test-token");
    const result = await client.searchManga("Error Manga");

    expect(result).toBeNull();
  });

  it("returns null on network error", async () => {
    vi.spyOn(globalThis, "fetch").mockRejectedValueOnce(new Error("Network failure"));

    const client = new AniListClient("test-token");
    const result = await client.searchManga("Network Manga");

    expect(result).toBeNull();
  });
});
