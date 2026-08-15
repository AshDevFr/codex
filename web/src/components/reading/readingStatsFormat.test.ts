import { describe, expect, it } from "vitest";
import type { ReadingPeriodDto } from "@/api/readingStats";
import {
  buildCalendar,
  formatDuration,
  formatDurationShort,
  groupIntoWeeks,
  heatLevel,
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
  /// Relative to the busiest day, not to an absolute scale: otherwise a
  /// twenty-minutes-a-day reader gets an entirely blank calendar.
  it("scales against the busiest day", () => {
    expect(heatLevel(20 * MINUTE, 20 * MINUTE)).toBe(5);
    expect(heatLevel(2 * MINUTE, 20 * MINUTE)).toBe(1);

    expect(heatLevel(6 * HOUR, 6 * HOUR)).toBe(5);
    expect(heatLevel(36 * MINUTE, 6 * HOUR)).toBe(1);
  });

  it("gives no reading its own empty step", () => {
    expect(heatLevel(0, 5 * HOUR)).toBe(0);
  });

  it("does not divide by a zero maximum", () => {
    expect(heatLevel(0, 0)).toBe(0);
    expect(heatLevel(10, 0)).toBe(0);
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
