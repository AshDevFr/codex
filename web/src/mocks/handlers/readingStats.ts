/**
 * MSW handlers for the reading statistics API.
 *
 * The data is shaped like a real library that predates session tracking, since
 * that is the case the dashboard has to survive and the one that is awkward to
 * reproduce by hand:
 *
 * - an older era backfilled from `read_progress`, carrying a timestamp and a
 *   book but no duration and no pages, attributed to the `legacy` device
 * - one bulk import day in that era, an order of magnitude above any real day,
 *   which is what a mark-everything-read looks like in the calendar
 * - a recent era with genuine measured time from the web reader and an iPad,
 *   plus a KOReader device whose time is reconstructed rather than measured
 *
 * Consequences worth seeing in the UI: series and formats that contributed no
 * time drop out of their panels, the `legacy` device drops out with them, and
 * the "N of M sittings reported no time" caveat appears under the tiles.
 *
 * Windows are honoured, so the range control changes the answer, and a window
 * that predates all of this reading returns nothing at all.
 */

import { delay, HttpResponse, http } from "msw";
import type { components } from "@/types/api.generated";
import { mockSeries } from "../data/store";

type ReadingStatsResponse = components["schemas"]["ReadingStatsResponse"];
type ReadingPeriodDto = components["schemas"]["ReadingPeriodDto"];
type ReadingBySeriesDto = components["schemas"]["ReadingBySeriesDto"];
type ReadingByDeviceDto = components["schemas"]["ReadingByDeviceDto"];
type ReadingByFormatDto = components["schemas"]["ReadingByFormatDto"];

const MINUTE = 60_000;

/** A day's reading, attributed to one series, device and format. */
interface MockReadingDay {
  date: string;
  measuredMs: number;
  inferredMs: number;
  pagesRead: number;
  sessions: number;
  books: number;
  seriesIndex: number;
  deviceId: string;
  format: string;
}

/**
 * Deterministic pseudo-randomness.
 *
 * The dataset is generated once at module load, and a reload that reshuffled it
 * would make "did my change do that?" unanswerable by eye.
 */
function makeRandom(seed: number): () => number {
  let state = seed;
  return () => {
    state = (state * 1_664_525 + 1_013_904_223) % 4_294_967_296;
    return state / 4_294_967_296;
  };
}

const DEVICES: Record<string, string | null> = {
  "browser-mac": "Codex Web (Mac)",
  "ipad-air": "Codex Reader (iPad)",
  koreader: "KOReader (Kobo Libra)",
  // No friendly name: the backfill invents this id, and the UI is expected to
  // render it as "Before time tracking" rather than as a device.
  legacy: null,
};

function isoDay(daysAgo: number): string {
  const date = new Date();
  date.setUTCHours(0, 0, 0, 0);
  date.setUTCDate(date.getUTCDate() - daysAgo);
  return date.toISOString().slice(0, 10);
}

/** 400 days of history, silent days omitted exactly as the API omits them. */
function generateHistory(): MockReadingDay[] {
  const random = makeRandom(20260816);
  const days: MockReadingDay[] = [];
  const pick = <T>(items: T[]): T => items[Math.floor(random() * items.length)];

  // The legacy era: books finished, nothing measured.
  for (let daysAgo = 400; daysAgo > 45; daysAgo -= 1) {
    if (random() > 0.55) continue;
    days.push({
      date: isoDay(daysAgo),
      measuredMs: 0,
      inferredMs: 0,
      pagesRead: 0,
      sessions: 1 + Math.floor(random() * 11),
      books: 1 + Math.floor(random() * 4),
      seriesIndex: Math.floor(random() * 24),
      deviceId: "legacy",
      format: pick(["cbz", "cbr", "epub", "pdf"]),
    });
  }

  // The day a shelf was marked read in bulk. Deliberately an order of magnitude
  // above everything else: a heat scale relative to the maximum collapses every
  // genuine day to the faintest step against it.
  days.push({
    date: isoDay(300),
    measuredMs: 0,
    inferredMs: 0,
    pagesRead: 0,
    sessions: 412,
    books: 412,
    seriesIndex: 3,
    deviceId: "legacy",
    format: "cbz",
  });

  // The measured era.
  for (let daysAgo = 45; daysAgo >= 0; daysAgo -= 1) {
    if (random() > 0.62) continue;
    const deviceId = pick([
      "browser-mac",
      "browser-mac",
      "ipad-air",
      "koreader",
    ]);
    const minutes = 15 + Math.floor(random() * 105);
    // KOReader cannot report its own reading time; the server reconstructs it.
    const inferred = deviceId === "koreader";
    days.push({
      date: isoDay(daysAgo),
      measuredMs: inferred ? 0 : minutes * MINUTE,
      inferredMs: inferred ? minutes * MINUTE : 0,
      pagesRead: 18 + Math.floor(random() * 90),
      sessions: 1 + Math.floor(random() * 4),
      books: 1 + Math.floor(random() * 2),
      seriesIndex: Math.floor(random() * 5),
      deviceId,
      format: inferred ? "epub" : pick(["cbz", "cbz", "cbr"]),
    });
  }

  return days.sort((a, b) => a.date.localeCompare(b.date));
}

const HISTORY = generateHistory();

/** The bucket key a day belongs to at the requested granularity. */
function bucketKey(date: string, granularity: string): string {
  if (granularity === "month") return `${date.slice(0, 7)}-01`;
  if (granularity !== "week") return date;

  const day = new Date(`${date}T00:00:00Z`);
  day.setUTCDate(day.getUTCDate() - ((day.getUTCDay() + 6) % 7));
  return day.toISOString().slice(0, 10);
}

function durationOf(days: MockReadingDay[]) {
  const measuredMs = days.reduce((sum, d) => sum + d.measuredMs, 0);
  const inferredMs = days.reduce((sum, d) => sum + d.inferredMs, 0);
  return { measuredMs, inferredMs, totalMs: measuredMs + inferredMs };
}

function groupBy<K>(
  days: MockReadingDay[],
  key: (day: MockReadingDay) => K,
): Map<K, MockReadingDay[]> {
  const groups = new Map<K, MockReadingDay[]>();
  for (const day of days) {
    const group = groups.get(key(day));
    if (group) group.push(day);
    else groups.set(key(day), [day]);
  }
  return groups;
}

/** Time read descending, which is how the API ranks every breakdown. */
function byTimeDesc(a: { duration: { totalMs: number } }, b: typeof a): number {
  return b.duration.totalMs - a.duration.totalMs;
}

export const readingStatsHandlers = [
  http.get("*/api/v1/reading-stats", async ({ request }) => {
    await delay(150);

    const url = new URL(request.url);
    const from = url.searchParams.get("from");
    const to = url.searchParams.get("to");
    const granularity = url.searchParams.get("granularity") ?? "day";
    const seriesLimit = Number(url.searchParams.get("seriesLimit") ?? 8);

    const fromDay = from ? from.slice(0, 10) : "0000-01-01";
    const toDay = to ? to.slice(0, 10) : "9999-12-31";
    const inWindow = HISTORY.filter(
      (day) => day.date >= fromDay && day.date <= toDay,
    );

    const periods: ReadingPeriodDto[] = [
      ...groupBy(inWindow, (day) => bucketKey(day.date, granularity)),
    ]
      .map(([bucket, days]) => ({
        bucket,
        duration: durationOf(days),
        pagesRead: days.reduce((sum, d) => sum + d.pagesRead, 0),
        sessions: days.reduce((sum, d) => sum + d.sessions, 0),
      }))
      .sort((a, b) => a.bucket.localeCompare(b.bucket));

    const series: ReadingBySeriesDto[] = [
      ...groupBy(inWindow, (day) => day.seriesIndex),
    ]
      .map(([seriesIndex, days]) => {
        const source = mockSeries[seriesIndex % mockSeries.length];
        return {
          seriesId: source.id,
          seriesName: source.title,
          duration: durationOf(days),
          pagesRead: days.reduce((sum, d) => sum + d.pagesRead, 0),
          sessions: days.reduce((sum, d) => sum + d.sessions, 0),
          books: days.reduce((sum, d) => sum + d.books, 0),
        };
      })
      .sort(byTimeDesc)
      .slice(0, seriesLimit);

    const devices: ReadingByDeviceDto[] = [
      ...groupBy(inWindow, (day) => day.deviceId),
    ]
      .map(([deviceId, days]) => ({
        deviceId,
        deviceName: DEVICES[deviceId] ?? null,
        duration: durationOf(days),
        pagesRead: days.reduce((sum, d) => sum + d.pagesRead, 0),
        sessions: days.reduce((sum, d) => sum + d.sessions, 0),
        lastReadAt: `${days[days.length - 1].date}T20:14:00Z`,
      }))
      .sort(byTimeDesc);

    const formats: ReadingByFormatDto[] = [
      ...groupBy(inWindow, (day) => day.format),
    ]
      .map(([format, days]) => ({
        format,
        duration: durationOf(days),
        pagesRead: days.reduce((sum, d) => sum + d.pagesRead, 0),
        sessions: days.reduce((sum, d) => sum + d.sessions, 0),
        books: days.reduce((sum, d) => sum + d.books, 0),
      }))
      .sort(byTimeDesc);

    const silent = inWindow.filter((d) => d.measuredMs + d.inferredMs === 0);

    const response: ReadingStatsResponse = {
      from: from ?? `${fromDay}T00:00:00Z`,
      to: to ?? `${toDay}T23:59:59Z`,
      granularity: granularity as ReadingStatsResponse["granularity"],
      summary: {
        duration: durationOf(inWindow),
        pagesRead: inWindow.reduce((sum, d) => sum + d.pagesRead, 0),
        sessions: inWindow.reduce((sum, d) => sum + d.sessions, 0),
        books: inWindow.reduce((sum, d) => sum + d.books, 0),
        sessionsWithoutDuration: silent.reduce((sum, d) => sum + d.sessions, 0),
      },
      periods,
      devices,
      series,
      formats,
    };

    return HttpResponse.json(response);
  }),
];
