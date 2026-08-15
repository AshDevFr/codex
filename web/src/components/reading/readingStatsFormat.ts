/**
 * Formatting and shaping helpers for the reading dashboard.
 *
 * Kept out of the components so the awkward parts (duration wording, filling
 * the gaps the API deliberately leaves in its time series) are testable without
 * rendering anything.
 */

import type { ReadingPeriodDto } from "@/api/readingStats";

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
    });
    cursor.setUTCDate(cursor.getUTCDate() + 1);
  }

  return days;
}

/**
 * Which of five ramp steps a day belongs in, or 0 for "no reading".
 *
 * Thresholds are relative to the busiest day rather than absolute, so the
 * calendar is legible for someone reading twenty minutes a day and for someone
 * reading six hours. An absolute scale would flatten one of them to nothing.
 */
export function heatLevel(
  totalMs: number,
  maxMs: number,
): 0 | 1 | 2 | 3 | 4 | 5 {
  if (totalMs <= 0) return 0;
  if (maxMs <= 0) return 0;

  const ratio = totalMs / maxMs;
  if (ratio <= 0.2) return 1;
  if (ratio <= 0.4) return 2;
  if (ratio <= 0.6) return 3;
  if (ratio <= 0.8) return 4;
  return 5;
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
