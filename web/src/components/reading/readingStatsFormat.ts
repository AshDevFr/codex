/**
 * Formatting and shaping helpers for the reading dashboard.
 *
 * Kept out of the components so the awkward parts (duration wording, filling
 * the gaps the API deliberately leaves in its time series) are testable without
 * rendering anything.
 */

import type { ReadingCoverage, ReadingPeriodDto } from "@/api/readingStats";
import type {
  ReadingMetric,
  ReadingRange,
} from "@/store/readingStatsPreferencesStore";
import { DEFAULT_READING_RANGE } from "@/store/readingStatsPreferencesStore";

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
  return rollUpBy(periods, mondayOf);
}

/** Sum daily buckets into whatever coarser bucket `keyOf` names. */
function rollUpBy(
  periods: ReadingPeriodDto[],
  keyOf: (iso: string) => string,
): ReadingPeriodDto[] {
  const buckets = new Map<string, ReadingPeriodDto>();

  for (const period of periods) {
    const bucket = keyOf(period.bucket);
    const existing = buckets.get(bucket);

    if (!existing) {
      // Copied, not aliased: the caller still holds these day objects.
      buckets.set(bucket, {
        bucket,
        duration: { ...period.duration },
        pagesRead: period.pagesRead,
        sessions: period.sessions,
        booksFinished: period.booksFinished,
      });
      continue;
    }

    existing.duration.measuredMs += period.duration.measuredMs;
    existing.duration.inferredMs += period.duration.inferredMs;
    existing.duration.totalMs += period.duration.totalMs;
    existing.pagesRead += period.pagesRead;
    existing.sessions += period.sessions;
    existing.booksFinished += period.booksFinished;
  }

  return [...buckets.values()].sort((a, b) => a.bucket.localeCompare(b.bucket));
}

/** The first of the month containing an ISO date. */
function firstOfMonth(iso: string): string {
  return `${iso.slice(0, 7)}-01`;
}

/**
 * Collapse daily buckets into calendar months.
 *
 * The twin of {@link rollUpIntoWeeks}, for the all-time range where even weekly
 * bars run to hundreds of columns. Keyed by the first of the month, which is
 * how the server keys months too.
 */
export function rollUpIntoMonths(
  periods: ReadingPeriodDto[],
): ReadingPeriodDto[] {
  return rollUpBy(periods, firstOfMonth);
}

/**
 * The window a range covers, and how wide the period chart's bars should be.
 *
 * Pure, so the awkward parts (a calendar year is the whole year; all-time has
 * to start somewhere real) are testable without rendering or fetching.
 *
 * The bucket width is a drawing decision only. The request is always daily,
 * because the calendar needs every day and one request has to serve both.
 */
export function windowFor(
  range: ReadingRange,
  coverage: ReadingCoverage,
  now: Date,
): { from: Date; to: Date; bars: "day" | "week" | "month" } {
  const endOfToday = new Date(now);
  endOfToday.setUTCHours(23, 59, 59, 999);

  if (range.kind === "all") {
    // Nothing read yet collapses to today rather than asking for every date
    // since the epoch.
    const first = coverage.firstReadAt ? new Date(coverage.firstReadAt) : now;
    const from = new Date(first);
    from.setUTCHours(0, 0, 0, 0);
    return { from, to: endOfToday, bars: "month" };
  }

  if (range.kind === "year") {
    // The whole calendar year, not the year so far: a part-year grid changes
    // shape as the year goes on, and the empty cells at the end are honest.
    return {
      from: new Date(Date.UTC(range.year, 0, 1, 0, 0, 0, 0)),
      to: new Date(Date.UTC(range.year, 11, 31, 23, 59, 59, 999)),
      bars: "week",
    };
  }

  const from = new Date(endOfToday);
  from.setUTCDate(from.getUTCDate() - (range.days - 1));
  from.setUTCHours(0, 0, 0, 0);
  return { from, to: endOfToday, bars: range.days > 90 ? "week" : "day" };
}

/**
 * Every year the reader could ask for, newest first.
 *
 * Years they read nothing in are included: the reader knows those years
 * happened, and a gap in the list reads as a bug rather than as silence.
 */
export function yearsCovered(coverage: ReadingCoverage, now: Date): number[] {
  if (!coverage.firstReadAt) return [];

  const first = new Date(coverage.firstReadAt).getUTCFullYear();
  const last = now.getUTCFullYear();
  const years: number[] = [];
  for (let year = last; year >= first; year -= 1) years.push(year);
  return years;
}

/**
 * The range to actually show, given what was restored from storage.
 *
 * A stored year can outlive its data, or be restored under a different account
 * entirely. The store cannot know which years are real, so the decision lives
 * here where the available years are known, and stays a pure function.
 */
export function resolveRange(
  stored: ReadingRange,
  availableYears: number[],
): ReadingRange {
  if (stored.kind === "year" && !availableYears.includes(stored.year)) {
    return DEFAULT_READING_RANGE;
  }
  return stored;
}

/**
 * Split days into calendar years, newest first.
 *
 * All-time draws one grid per year rather than one grid spanning a decade,
 * which would be unreadable at any cell size that still fits on a screen.
 */
export function groupIntoYears(
  days: CalendarDay[],
): { year: number; days: CalendarDay[] }[] {
  const byYear = new Map<number, CalendarDay[]>();

  for (const day of days) {
    const year = Number(day.date.slice(0, 4));
    const existing = byYear.get(year);
    if (existing) existing.push(day);
    else byYear.set(year, [day]);
  }

  return [...byYear.entries()]
    .map(([year, yearDays]) => ({ year, days: yearDays }))
    .sort((a, b) => b.year - a.year);
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
