import { describe, expect, it } from "vitest";
import type { ReadingPeriodDto } from "@/api/readingStats";
import {
  buildCalendar,
  formatDuration,
  formatDurationShort,
  groupIntoWeeks,
  heatLevel,
  heatThresholds,
  resolveRange,
  rollUpIntoMonths,
  rollUpIntoWeeks,
  windowFor,
  yearsCovered,
} from "./readingStatsFormat";

const MINUTE = 60_000;
const HOUR = 60 * MINUTE;

function period(bucket: string, minutes: number): ReadingPeriodDto {
  return {
    bucket,
    duration: {
      measuredMs: minutes * MINUTE,
      inferredMs: 0,
      totalMs: minutes * MINUTE,
    },
    pagesRead: 10,
    sessions: 1,
    booksFinished: 1,
  };
}

describe("formatDuration", () => {
  it("reports minutes below an hour", () => {
    expect(formatDuration(25 * MINUTE)).toBe("25m");
  });

  it("reports hours and minutes above an hour", () => {
    expect(formatDuration(2 * HOUR + 15 * MINUTE)).toBe("2h 15m");
  });

  it("drops a zero minute component", () => {
    expect(formatDuration(3 * HOUR)).toBe("3h");
  });

  /// Nobody reads for "0h 0m". Zero is zero.
  it("reports nothing read as a plain zero", () => {
    expect(formatDuration(0)).toBe("0m");
    expect(formatDuration(-5)).toBe("0m");
  });

  it("has a compact form for tight labels", () => {
    expect(formatDurationShort(90 * MINUTE)).toBe("2h");
    expect(formatDurationShort(20 * MINUTE)).toBe("20m");
  });
});

describe("buildCalendar", () => {
  /// The API omits silent days; a calendar has to draw them anyway or the
  /// grid has holes where the quiet weeks were.
  it("fills days the API left out", () => {
    const days = buildCalendar(
      [period("2026-06-01", 30), period("2026-06-04", 45)],
      new Date("2026-06-01T00:00:00Z"),
      new Date("2026-06-05T00:00:00Z"),
    );

    expect(days).toHaveLength(5);
    expect(days.map((d) => d.date)).toEqual([
      "2026-06-01",
      "2026-06-02",
      "2026-06-03",
      "2026-06-04",
      "2026-06-05",
    ]);
    expect(days[1].totalMs).toBe(0);
    expect(days[3].totalMs).toBe(45 * MINUTE);
  });

  it("carries the provenance split through", () => {
    const days = buildCalendar(
      [
        {
          bucket: "2026-06-01",
          duration: {
            measuredMs: 10 * MINUTE,
            inferredMs: 5 * MINUTE,
            totalMs: 15 * MINUTE,
          },
          pagesRead: 3,
          sessions: 1,
        },
      ],
      new Date("2026-06-01T00:00:00Z"),
      new Date("2026-06-01T00:00:00Z"),
    );

    expect(days[0].measuredMs).toBe(10 * MINUTE);
    expect(days[0].inferredMs).toBe(5 * MINUTE);
  });

  /// Buckets are UTC dates, so the walk has to be in UTC or a viewer behind
  /// UTC sees the whole calendar shifted by a day.
  it("walks in UTC regardless of the viewer's timezone", () => {
    const days = buildCalendar(
      [],
      new Date("2026-06-01T23:30:00Z"),
      new Date("2026-06-02T00:30:00Z"),
    );

    expect(days.map((d) => d.date)).toEqual(["2026-06-01", "2026-06-02"]);
  });

  it("handles a window with no reading at all", () => {
    const days = buildCalendar(
      [],
      new Date("2026-06-01T00:00:00Z"),
      new Date("2026-06-03T00:00:00Z"),
    );

    expect(days).toHaveLength(3);
    expect(days.every((d) => d.totalMs === 0)).toBe(true);
  });
});

describe("heatLevel", () => {
  /// Relative to the days actually read, not to an absolute scale: otherwise a
  /// twenty-minutes-a-day reader gets an entirely blank calendar.
  it("scales against the days that were read", () => {
    const short = heatThresholds([2, 6, 10, 14, 20].map((m) => m * MINUTE));
    expect(heatLevel(20 * MINUTE, short)).toBe(5);
    expect(heatLevel(2 * MINUTE, short)).toBe(1);

    const long = heatThresholds([36 * MINUTE, 2 * HOUR, 4 * HOUR, 6 * HOUR]);
    expect(heatLevel(6 * HOUR, long)).toBe(5);
    expect(heatLevel(36 * MINUTE, long)).toBe(1);
  });

  it("gives no reading its own empty step", () => {
    expect(heatLevel(0, heatThresholds([5 * HOUR]))).toBe(0);
  });

  it("has a defined answer when nothing was read at all", () => {
    expect(heatLevel(0, heatThresholds([]))).toBe(0);
    expect(heatLevel(10, heatThresholds([]))).toBe(5);
  });
});

describe("heatThresholds", () => {
  /// The case that motivated quantiles: a bulk import day holds an order of
  /// magnitude more than any real day. Scaled against the maximum, every
  /// genuine day lands in the faintest step and the calendar reads as flat.
  it("keeps ordinary days distributed under a bulk-import outlier", () => {
    const ordinary = Array.from({ length: 40 }, (_, i) => i + 1);
    const thresholds = heatThresholds([...ordinary, 1000]);

    const levels = new Set(ordinary.map((v) => heatLevel(v, thresholds)));
    expect(levels.size).toBeGreaterThan(3);
    expect(heatLevel(1000, thresholds)).toBe(5);
  });

  /// Under a handful of active days there is no distribution to take quantiles
  /// of, so the scale stays proportional to the busiest day.
  it("falls back to fractions of the maximum on a small sample", () => {
    const thresholds = heatThresholds([10, 100]);

    expect(thresholds).toEqual([20, 40, 60, 80]);
    expect(heatLevel(10, thresholds)).toBe(1);
    expect(heatLevel(100, thresholds)).toBe(5);
  });

  it("ignores days with no reading when building the scale", () => {
    expect(heatThresholds([0, 0, 10, 100])).toEqual(heatThresholds([10, 100]));
  });

  /// Every day identical is not an error, and must not report a spread that
  /// isn't there.
  it("gives uniform days a uniform level", () => {
    const thresholds = heatThresholds(Array.from({ length: 30 }, () => 42));

    expect(heatLevel(42, thresholds)).toBe(1);
  });
});

describe("rollUpIntoWeeks", () => {
  /// 2026-06-01 is a Monday, so the whole of that week keys to it.
  it("sums days into the week's Monday", () => {
    const weeks = rollUpIntoWeeks([
      period("2026-06-01", 30),
      period("2026-06-03", 45),
      period("2026-06-07", 15),
    ]);

    expect(weeks).toHaveLength(1);
    expect(weeks[0].bucket).toBe("2026-06-01");
    expect(weeks[0].duration.totalMs).toBe(90 * MINUTE);
    expect(weeks[0].pagesRead).toBe(30);
    expect(weeks[0].sessions).toBe(3);
  });

  /// A Sunday belongs to the week that started six days earlier, not to the
  /// Monday that follows it.
  it("puts Sunday at the end of its own week", () => {
    const weeks = rollUpIntoWeeks([
      period("2026-06-07", 10),
      period("2026-06-08", 20),
    ]);

    expect(weeks.map((w) => w.bucket)).toEqual(["2026-06-01", "2026-06-08"]);
  });

  it("keeps the provenance split separable", () => {
    const weeks = rollUpIntoWeeks([
      {
        bucket: "2026-06-01",
        duration: {
          measuredMs: 10 * MINUTE,
          inferredMs: 5 * MINUTE,
          totalMs: 15 * MINUTE,
        },
        pagesRead: 3,
        sessions: 1,
      },
      {
        bucket: "2026-06-02",
        duration: {
          measuredMs: 20 * MINUTE,
          inferredMs: 0,
          totalMs: 20 * MINUTE,
        },
        pagesRead: 4,
        sessions: 2,
      },
    ]);

    expect(weeks[0].duration.measuredMs).toBe(30 * MINUTE);
    expect(weeks[0].duration.inferredMs).toBe(5 * MINUTE);
    expect(weeks[0].duration.totalMs).toBe(35 * MINUTE);
  });

  it("returns weeks in date order whatever order the days arrive in", () => {
    const weeks = rollUpIntoWeeks([
      period("2026-06-15", 10),
      period("2026-06-01", 10),
      period("2026-06-08", 10),
    ]);

    expect(weeks.map((w) => w.bucket)).toEqual([
      "2026-06-01",
      "2026-06-08",
      "2026-06-15",
    ]);
  });

  it("does not mutate the days it was given", () => {
    const days = [period("2026-06-01", 30), period("2026-06-02", 30)];
    rollUpIntoWeeks(days);

    expect(days[0].duration.totalMs).toBe(30 * MINUTE);
    expect(days[0].pagesRead).toBe(10);
  });

  it("has nothing to roll up when nothing was read", () => {
    expect(rollUpIntoWeeks([])).toEqual([]);
  });
});

describe("rollUpIntoMonths", () => {
  it("sums days into the first of their month", () => {
    const months = rollUpIntoMonths([
      period("2026-06-01", 30),
      period("2026-06-30", 45),
      period("2026-07-02", 15),
    ]);

    expect(months.map((m) => m.bucket)).toEqual(["2026-06-01", "2026-07-01"]);
    expect(months[0].duration.totalMs).toBe(75 * MINUTE);
    expect(months[1].duration.totalMs).toBe(15 * MINUTE);
  });

  it("keeps months in date order across a year boundary", () => {
    const months = rollUpIntoMonths([
      period("2026-01-05", 10),
      period("2025-12-31", 10),
    ]);

    expect(months.map((m) => m.bucket)).toEqual(["2025-12-01", "2026-01-01"]);
  });

  it("does not mutate the days it was given", () => {
    const days = [period("2026-06-01", 30), period("2026-06-02", 30)];
    rollUpIntoMonths(days);

    expect(days[0].duration.totalMs).toBe(30 * MINUTE);
  });
});

describe("windowFor", () => {
  const now = new Date("2026-08-16T14:30:00Z");
  const coverage = { firstReadAt: "2024-03-04T18:22:11Z", lastReadAt: null };

  it("counts a relative range back from today, inclusive", () => {
    const { from, to, bars } = windowFor(
      { kind: "relative", days: 30 },
      coverage,
      now,
    );

    expect(from.toISOString().slice(0, 10)).toBe("2026-07-18");
    expect(to.toISOString().slice(0, 10)).toBe("2026-08-16");
    expect(bars).toBe("day");
  });

  /// A year of daily bars is unreadable, so the long ranges widen the bucket.
  /// The request stays daily either way; this is a drawing decision.
  it("widens the bucket as the window grows", () => {
    expect(windowFor({ kind: "relative", days: 365 }, coverage, now).bars).toBe(
      "week",
    );
    expect(windowFor({ kind: "year", year: 2025 }, coverage, now).bars).toBe(
      "week",
    );
    expect(windowFor({ kind: "all" }, coverage, now).bars).toBe("month");
  });

  /// A calendar year is the whole year, not the year so far: the grid keeps its
  /// shape and the empty days at the end read as "not yet" rather than absent.
  it("covers a whole calendar year", () => {
    const { from, to } = windowFor({ kind: "year", year: 2025 }, coverage, now);

    expect(from.toISOString().slice(0, 10)).toBe("2025-01-01");
    expect(to.toISOString().slice(0, 10)).toBe("2025-12-31");
  });

  it("starts all-time at the reader's first recorded reading", () => {
    const { from, to } = windowFor({ kind: "all" }, coverage, now);

    expect(from.toISOString().slice(0, 10)).toBe("2024-03-04");
    expect(to.toISOString().slice(0, 10)).toBe("2026-08-16");
  });

  /// Nothing read yet is not an error: all-time collapses to today rather than
  /// asking the API for every date since the epoch.
  it("handles all-time for a reader who has never read", () => {
    const { from, to } = windowFor(
      { kind: "all" },
      { firstReadAt: null, lastReadAt: null },
      now,
    );

    expect(from.toISOString().slice(0, 10)).toBe("2026-08-16");
    expect(to.toISOString().slice(0, 10)).toBe("2026-08-16");
  });
});

describe("resolveRange", () => {
  /// A stored year can outlive its data, or be restored under a different
  /// account entirely. The store cannot know which years are real, so the page
  /// decides here rather than trusting what it read back.
  it("falls back when the stored year has no data", () => {
    expect(resolveRange({ kind: "year", year: 2019 }, [2025, 2026])).toEqual({
      kind: "relative",
      days: 90,
    });
  });

  it("keeps a stored year that still exists", () => {
    expect(resolveRange({ kind: "year", year: 2025 }, [2025, 2026])).toEqual({
      kind: "year",
      year: 2025,
    });
  });

  it("leaves relative and all-time ranges alone", () => {
    expect(resolveRange({ kind: "relative", days: 30 }, [])).toEqual({
      kind: "relative",
      days: 30,
    });
    expect(resolveRange({ kind: "all" }, [])).toEqual({ kind: "all" });
  });
});

describe("yearsCovered", () => {
  it("lists every year from the first reading to now, newest first", () => {
    const years = yearsCovered(
      { firstReadAt: "2024-03-04T18:22:11Z", lastReadAt: null },
      new Date("2026-08-16T00:00:00Z"),
    );

    expect(years).toEqual([2026, 2025, 2024]);
  });

  /// Years with no reading in the middle are still offered: the reader knows
  /// they exist, and a gap in the list looks like a bug.
  it("offers nothing when the reader has never read", () => {
    expect(
      yearsCovered(
        { firstReadAt: null, lastReadAt: null },
        new Date("2026-08-16T00:00:00Z"),
      ),
    ).toEqual([]);
  });
});

describe("groupIntoWeeks", () => {
  /// 2026-06-03 is a Wednesday, so its column needs two blanks above it for
  /// the day to land on the right weekday row.
  it("pads the first column so days land on their real weekday", () => {
    const days = buildCalendar(
      [],
      new Date("2026-06-03T00:00:00Z"),
      new Date("2026-06-07T00:00:00Z"),
    );

    const weeks = groupIntoWeeks(days);
    expect(weeks).toHaveLength(1);
    expect(weeks[0][0]).toBeNull();
    expect(weeks[0][1]).toBeNull();
    expect(weeks[0][2]?.date).toBe("2026-06-03");
    expect(weeks[0][6]?.date).toBe("2026-06-07");
  });

  it("pads the final column so every week is seven rows", () => {
    const days = buildCalendar(
      [],
      new Date("2026-06-01T00:00:00Z"),
      new Date("2026-06-09T00:00:00Z"),
    );

    const weeks = groupIntoWeeks(days);
    expect(weeks).toHaveLength(2);
    expect(weeks.every((w) => w.length === 7)).toBe(true);
  });

  it("returns nothing for an empty window", () => {
    expect(groupIntoWeeks([])).toEqual([]);
  });
});
