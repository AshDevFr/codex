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

export interface ReadingStatsPreferencesState {
  metric: ReadingMetric;
  setMetric: (metric: ReadingMetric) => void;
}

export const useReadingStatsPreferencesStore =
  create<ReadingStatsPreferencesState>()(
    devtools(
      persist(
        (set) => ({
          metric: "time",
          setMetric: (metric) => set({ metric }),
        }),
        { name: "reading-stats-preferences-storage" },
      ),
      { name: "ReadingStatsPreferences", enabled: import.meta.env.DEV },
    ),
  );
