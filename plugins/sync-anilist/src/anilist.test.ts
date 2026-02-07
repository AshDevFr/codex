import { describe, expect, it } from "vitest";
import {
  anilistStatusToSync,
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
