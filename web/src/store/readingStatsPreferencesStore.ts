import { create } from "zustand";
import { devtools, persist } from "zustand/middleware";
import type { ReadingStatsSort } from "@/api/readingStats";

/**
 * Which measure the dashboard draws.
 *
 * Time is the richest measure and the default, but it is silent about any
 * reading that predates session tracking: those rows were backfilled with no
 * duration and no page count. Books finished is the only measure that means the
 * same thing on both sides of that cutover, so a reader whose history is mostly
 * older can still see it.
 */
export type ReadingMetric = "time" | "pages" | "booksFinished";

export const READING_METRICS: { value: ReadingMetric; label: string }[] = [
  { value: "time", label: "Time" },
  { value: "pages", label: "Pages" },
  { value: "booksFinished", label: "Books finished" },
];

/**
 * The ranking key the API needs for a given metric.
 *
 * The server decides which rows survive the series limit, so the metric the UI
 * draws and the key the query ranks by must not drift apart.
 */
export function sortForMetric(metric: ReadingMetric): ReadingStatsSort {
  if (metric === "pages") return "pages";
  if (metric === "booksFinished") return "completions";
  return "time";
}

/**
 * Which slice of history the dashboard shows.
 *
 * A tagged union rather than a number of days, because a calendar year is not
 * a rolling window and all-time has no fixed length. Relative ranges count back
 * from today; a year is that whole calendar year.
 */
export type ReadingRange =
  | { kind: "relative"; days: 30 | 90 | 365 }
  | { kind: "year"; year: number }
  | { kind: "all" };

export const DEFAULT_READING_RANGE: ReadingRange = {
  kind: "relative",
  days: 90,
};

export const RELATIVE_RANGES: { days: 30 | 90 | 365; label: string }[] = [
  { days: 30, label: "30 days" },
  { days: 90, label: "90 days" },
  { days: 365, label: "1 year" },
];

/** A stable key for a range, for query keys and control state. */
export function rangeKey(range: ReadingRange): string {
  if (range.kind === "all") return "all";
  if (range.kind === "year") return `year-${range.year}`;
  return `days-${range.days}`;
}

export interface ReadingStatsPreferencesState {
  metric: ReadingMetric;
  setMetric: (metric: ReadingMetric) => void;
  range: ReadingRange;
  setRange: (range: ReadingRange) => void;
}

export const useReadingStatsPreferencesStore =
  create<ReadingStatsPreferencesState>()(
    devtools(
      persist(
        (set) => ({
          metric: "time",
          setMetric: (metric) => set({ metric }),
          range: DEFAULT_READING_RANGE,
          setRange: (range) => set({ range }),
        }),
        { name: "reading-stats-preferences-storage" },
      ),
      { name: "ReadingStatsPreferences", enabled: import.meta.env.DEV },
    ),
  );
