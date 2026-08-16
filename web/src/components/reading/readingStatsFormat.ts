/**
 * Formatting and shaping helpers for the reading dashboard.
 *
 * Kept out of the components so the awkward parts (duration wording, filling
 * the gaps the API deliberately leaves in its time series) are testable without
 * rendering anything.
 */

import type { ReadingPeriodDto } from "@/api/readingStats";
import type { ReadingMetric } from "@/store/readingStatsPreferencesStore";

/**
 * Human duration, at the precision a reader actually cares about.
 *
 * Minutes below an hour, hours and minutes above it, and never seconds: nobody
 * reads for 3 minutes and 42 seconds in any sense worth reporting.
 */
export function formatDuration(ms: number): string {
  if (ms <= 0) return "0m";

  const totalMinutes = Math.round(ms / 60_000);
  if (totalMinutes < 60) return `${totalMinutes}m`;

  const hours = Math.floor(totalMinutes / 60);
  const minutes = totalMinutes % 60;
  return minutes === 0 ? `${hours}h` : `${hours}h ${minutes}m`;
}

/** Compact duration for axis and cell labels, where space is scarce. */
export function formatDurationShort(ms: number): string {
  if (ms <= 0) return "0";
  const hours = ms / 3_600_000;
  if (hours >= 1) return `${Math.round(hours)}h`;
  return `${Math.round(ms / 60_000)}m`;
}

export interface CalendarDay {
  /** ISO date, `YYYY-MM-DD`. */
  date: string;
  totalMs: number;
  measuredMs: number;
  inferredMs: number;
  pagesRead: number;
  booksFinished: number;
}

/** What a row is worth under the metric currently being drawn. */
export function metricValue(
  row: { totalMs: number; pagesRead: number; booksFinished: number },
  metric: ReadingMetric,
): number {
  if (metric === "pages") return row.pagesRead;
  if (metric === "booksFinished") return row.booksFinished;
  return row.totalMs;
}

/** How a metric's value reads in a tooltip or a caption. */
export function formatMetric(value: number, metric: ReadingMetric): string {
  if (metric === "time") return formatDuration(value);
  if (metric === "pages") return `${value.toLocaleString()} pages`;
  return `${value.toLocaleString()} ${value === 1 ? "book" : "books"}`;
}

/**
 * Compact form of the same, for captions where space is scarce.
 *
 * Counts keep their noun: a bare "421" in a caption says nothing about whether
 * it counts pages, books or minutes.
 */
export function formatMetricShort(
  value: number,
  metric: ReadingMetric,
): string {
  if (metric === "time") return formatDurationShort(value);
  if (metric === "pages") return `${value.toLocaleString()} pages`;
  return `${value.toLocaleString()} ${value === 1 ? "book" : "books"}`;
}

/**
 * Expand the API's sparse series into every day of the window.
 *
 * The endpoint omits days with no reading, which is right for the wire but
 * wrong for a calendar: a heatmap has to draw the silent days too, or the grid
 * has holes where the quiet weeks were.
 */
export function buildCalendar(
  periods: ReadingPeriodDto[],
  from: Date,
  to: Date,
): CalendarDay[] {
  const byDate = new Map(periods.map((p) => [p.bucket, p]));
  const days: CalendarDay[] = [];

  // Iterate in UTC. Buckets are UTC dates, so walking in local time would drift
  // a day whenever the viewer is behind UTC.
  const cursor = new Date(
    Date.UTC(from.getUTCFullYear(), from.getUTCMonth(), from.getUTCDate()),
  );
  const end = Date.UTC(to.getUTCFullYear(), to.getUTCMonth(), to.getUTCDate());

  while (cursor.getTime() <= end) {
    const date = cursor.toISOString().slice(0, 10);
    const period = byDate.get(date);
    days.push({
      date,
      totalMs: period?.duration.totalMs ?? 0,
      measuredMs: period?.duration.measuredMs ?? 0,
      inferredMs: period?.duration.inferredMs ?? 0,
      pagesRead: period?.pagesRead ?? 0,
      booksFinished: period?.booksFinished ?? 0,
    });
    cursor.setUTCDate(cursor.getUTCDate() + 1);
  }

  return days;
}

/** The Monday that starts the week containing an ISO date, in UTC. */
function mondayOf(iso: string): string {
  const date = new Date(`${iso}T00:00:00Z`);
  date.setUTCDate(date.getUTCDate() - ((date.getUTCDay() + 6) % 7));
  return date.toISOString().slice(0, 10);
}

/**
 * Collapse daily buckets into Monday-start weeks.
 *
 * The calendar needs daily grain and the period chart wants weeks once the
 * window is a year long, so the page asks the API for days and aggregates the
 * chart here. Asking twice at two granularities would leave the two panels able
 * to disagree about the same window.
 *
 * Weeks are keyed by their Monday, which is how the server keys them too, so a
 * bucket label means the same thing whether it was built here or there.
 */
export function rollUpIntoWeeks(
  periods: ReadingPeriodDto[],
): ReadingPeriodDto[] {
  const byWeek = new Map<string, ReadingPeriodDto>();

  for (const period of periods) {
    const bucket = mondayOf(period.bucket);
    const week = byWeek.get(bucket);

    if (!week) {
      // Copied, not aliased: the caller still holds these day objects.
      byWeek.set(bucket, {
        bucket,
        duration: { ...period.duration },
        pagesRead: period.pagesRead,
        sessions: period.sessions,
        booksFinished: period.booksFinished,
      });
      continue;
    }

    week.duration.measuredMs += period.duration.measuredMs;
    week.duration.inferredMs += period.duration.inferredMs;
    week.duration.totalMs += period.duration.totalMs;
    week.pagesRead += period.pagesRead;
    week.sessions += period.sessions;
    week.booksFinished += period.booksFinished;
  }

  return [...byWeek.values()].sort((a, b) => a.bucket.localeCompare(b.bucket));
}

/**
 * Below this many days with reading, quantiles describe nothing.
 *
 * Four active days cannot be divided into five populated steps: the boundaries
 * land on the same handful of values and the calendar shows fewer colours than
 * its own legend advertises.
 */
const MIN_QUANTILE_SAMPLE = 8;

/**
 * The four boundaries between the five heat steps.
 *
 * Quantiles over the days that had reading, not fractions of the busiest day.
 * A library marked read in bulk has one day holding an order of magnitude more
 * than any genuine day; measured against that maximum, every real day falls in
 * the faintest step and the calendar reads as uniformly empty. Quantiles are
 * indifferent to how extreme the outlier is, because they count days rather
 * than measure distance.
 *
 * Under {@link MIN_QUANTILE_SAMPLE} active days the scale falls back to
 * fractions of the maximum, which stays stable on a handful of points.
 */
export function heatThresholds(values: number[]): number[] {
  const read = values.filter((value) => value > 0).sort((a, b) => a - b);
  if (read.length === 0) return [0, 0, 0, 0];

  const max = read[read.length - 1];
  if (read.length < MIN_QUANTILE_SAMPLE) {
    return [0.2, 0.4, 0.6, 0.8].map((fraction) => fraction * max);
  }

  return [0.2, 0.4, 0.6, 0.8].map(
    (quantile) => read[Math.floor(quantile * (read.length - 1))],
  );
}

/**
 * Which of five ramp steps a value belongs in, or 0 for "nothing read".
 *
 * Zero has its own step so a silent day is visibly different from a quiet one
 * rather than merely paler.
 */
export function heatLevel(
  value: number,
  thresholds: number[],
): 0 | 1 | 2 | 3 | 4 | 5 {
  if (value <= 0) return 0;

  const step = thresholds.filter((threshold) => value > threshold).length;
  return (step + 1) as 1 | 2 | 3 | 4 | 5;
}

/** Group days into calendar weeks, each column starting on Monday. */
export function groupIntoWeeks(days: CalendarDay[]): (CalendarDay | null)[][] {
  if (days.length === 0) return [];

  const weeks: (CalendarDay | null)[][] = [];
  let current: (CalendarDay | null)[] = [];

  // Monday is index 0, matching the server's week bucketing.
  const weekdayIndex = (iso: string) =>
    (new Date(`${iso}T00:00:00Z`).getUTCDay() + 6) % 7;

  // Pad the first column so the first day sits on its real weekday.
  for (let i = 0; i < weekdayIndex(days[0].date); i += 1) {
    current.push(null);
  }

  for (const day of days) {
    current.push(day);
    if (current.length === 7) {
      weeks.push(current);
      current = [];
    }
  }
  if (current.length > 0) {
    while (current.length < 7) current.push(null);
    weeks.push(current);
  }

  return weeks;
}

/** A readable date for tooltips. */
export function formatDayLabel(iso: string): string {
  return new Date(`${iso}T00:00:00Z`).toLocaleDateString(undefined, {
    weekday: "short",
    day: "numeric",
    month: "short",
    year: "numeric",
    timeZone: "UTC",
  });
}
