import { screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { HttpResponse, http } from "msw";
import { setupServer } from "msw/node";
import { afterAll, afterEach, beforeAll, describe, expect, it } from "vitest";
import { useReadingStatsPreferencesStore } from "@/store/readingStatsPreferencesStore";
import { renderWithProviders } from "@/test/utils";
import { ReadingStats } from "./ReadingStats";

// The page cuts its window at the viewer's midnights and labels calendar days
// with the viewer's dates, so the process timezone is part of what these tests
// assert. Pin it west of UTC, where the original bug lived (evening reading
// showing up on tomorrow's calendar). Vitest gives each file its own process,
// so this leaks nowhere.
process.env.TZ = "America/Los_Angeles";

const MINUTE = 60_000;

/** Granularities the page asked for, newest last. */
let requestedGranularities: (string | null)[] = [];
/** Ranking keys the page asked for, newest last. */
let requestedSorts: (string | null)[] = [];
/** Windows the page asked for, newest last. */
let requestedWindows: { from: string | null; to: string | null }[] = [];

/** The viewer-local calendar date of an instant, `YYYY-MM-DD`. */
function localDate(d: Date): string {
  const month = String(d.getMonth() + 1).padStart(2, "0");
  const day = String(d.getDate()).padStart(2, "0");
  return `${d.getFullYear()}-${month}-${day}`;
}

// The local date, not the UTC one: buckets are the viewer's days now, and the
// UTC date is already tomorrow for a whole evening of every western day.
function today(): string {
  return localDate(new Date());
}

const server = setupServer(
  http.get("*/api/v1/settings/branding", () =>
    HttpResponse.json({ applicationName: "Codex" }),
  ),
  http.get("*/reading-stats/coverage", () =>
    HttpResponse.json({
      firstReadAt: "2024-03-04T18:22:11Z",
      lastReadAt: new Date().toISOString(),
    }),
  ),
  http.get("*/reading-stats", ({ request }) => {
    const url = new URL(request.url);
    requestedGranularities.push(url.searchParams.get("granularity"));
    requestedSorts.push(url.searchParams.get("sort"));
    requestedWindows.push({
      from: url.searchParams.get("from"),
      to: url.searchParams.get("to"),
    });

    return HttpResponse.json({
      from: url.searchParams.get("from"),
      to: url.searchParams.get("to"),
      granularity: url.searchParams.get("granularity") ?? "day",
      summary: {
        books: 3,
        duration: {
          measuredMs: 90 * MINUTE,
          inferredMs: 0,
          totalMs: 90 * MINUTE,
        },
        pagesRead: 120,
        sessions: 5,
        booksFinished: 2,
        sessionsWithoutDuration: 0,
        sessionsWithoutPages: 0,
      },
      // One day of reading, today, so the assertion holds whatever the window.
      periods: [
        {
          bucket: today(),
          duration: {
            measuredMs: 90 * MINUTE,
            inferredMs: 0,
            totalMs: 90 * MINUTE,
          },
          pagesRead: 120,
          sessions: 5,
          booksFinished: 2,
        },
      ],
      devices: [],
      series: [],
      formats: [],
    });
  }),
);

beforeAll(() => server.listen());
afterEach(() => {
  server.resetHandlers();
  requestedGranularities = [];
  requestedSorts = [];
  requestedWindows = [];
  useReadingStatsPreferencesStore.setState({
    metric: "time",
    range: { kind: "relative", days: 90 },
  });
});
afterAll(() => server.close());

/**
 * Calendar cells drawn in a non-empty step.
 *
 * Selected by the calendar's own label: the period chart is an `svg[role=img]`
 * too, and its bars are never `--heat-0`, so a looser selector counts them.
 */
function litCells(container: HTMLElement): Element[] {
  const calendar = container.querySelector(
    "svg[aria-label^='Daily reading activity']",
  );
  return [...(calendar?.querySelectorAll("rect") ?? [])].filter(
    (cell) => cell.getAttribute("fill") !== "var(--heat-0)",
  );
}

describe("ReadingStats", () => {
  it("colours the days that were read in", async () => {
    const { container } = renderWithProviders(<ReadingStats />);

    await screen.findByText("Daily activity");
    await waitFor(() => expect(litCells(container)).toHaveLength(1));
  });

  /// The server applies the series limit, so the ranking key has to travel
  /// with the request. Ranking a top-8 chosen by time would show the wrong
  /// series under any other metric.
  it("asks the API to rank by the metric on screen", async () => {
    const user = userEvent.setup();
    renderWithProviders(<ReadingStats />);

    await screen.findByText("Daily activity");
    expect(requestedSorts).toEqual(["time"]);

    await user.click(screen.getByRole("radio", { name: "Books finished" }));

    await waitFor(() =>
      expect(requestedSorts).toEqual(["time", "completions"]),
    );
  });

  /// Provenance is a property of measured time. Under another metric there is
  /// no estimate to disclose, so saying sittings reported no time answers a
  /// question the page is no longer asking.
  it("drops the missing-time caveat when not showing time", async () => {
    const user = userEvent.setup();
    server.use(
      http.get("*/reading-stats", () =>
        HttpResponse.json({
          from: null,
          to: null,
          granularity: "day",
          summary: {
            books: 3,
            duration: { measuredMs: 0, inferredMs: 0, totalMs: 0 },
            pagesRead: 0,
            sessions: 40,
            booksFinished: 12,
            sessionsWithoutDuration: 38,
            sessionsWithoutPages: 38,
          },
          periods: [
            {
              bucket: today(),
              duration: { measuredMs: 0, inferredMs: 0, totalMs: 0 },
              pagesRead: 0,
              sessions: 40,
              booksFinished: 12,
            },
          ],
          devices: [],
          series: [],
          formats: [],
        }),
      ),
    );

    renderWithProviders(<ReadingStats />);
    expect(
      await screen.findByText(/38 of 40 sittings reported no time/),
    ).toBeInTheDocument();

    await user.click(screen.getByRole("radio", { name: "Books finished" }));

    await waitFor(() =>
      expect(
        screen.queryByText(/38 of 40 sittings reported no time/),
      ).not.toBeInTheDocument(),
    );
  });

  /// Pages are as blind to a backfilled library as time is, so the page total
  /// needs the same caveat. Without it the Pages view presents a floor as if it
  /// were the whole record.
  it("discloses missing page counts when showing pages", async () => {
    const user = userEvent.setup();
    server.use(
      http.get("*/reading-stats", () =>
        HttpResponse.json({
          from: null,
          to: null,
          granularity: "day",
          summary: {
            books: 3,
            duration: { measuredMs: 0, inferredMs: 0, totalMs: 0 },
            pagesRead: 120,
            sessions: 40,
            booksFinished: 12,
            sessionsWithoutDuration: 38,
            sessionsWithoutPages: 36,
          },
          periods: [
            {
              bucket: today(),
              duration: { measuredMs: 0, inferredMs: 0, totalMs: 0 },
              pagesRead: 120,
              sessions: 40,
              booksFinished: 12,
            },
          ],
          devices: [],
          series: [],
          formats: [],
        }),
      ),
    );

    renderWithProviders(<ReadingStats />);

    // Silent under Time: that view discloses its own gap.
    expect(
      await screen.findByText(/38 of 40 sittings reported no time/),
    ).toBeInTheDocument();
    expect(
      screen.queryByText(/36 of 40 sittings reported no page count/),
    ).not.toBeInTheDocument();

    await user.click(screen.getByRole("radio", { name: "Pages" }));

    expect(
      await screen.findByText(/36 of 40 sittings reported no page count/),
    ).toBeInTheDocument();
    await waitFor(() =>
      expect(
        screen.queryByText(/38 of 40 sittings reported no time/),
      ).not.toBeInTheDocument(),
    );
  });

  /// A backfilled library has no time at all. Books finished is the one measure
  /// it can answer, so the calendar has to light up under it.
  it("draws a calendar under books finished where time is silent", async () => {
    const user = userEvent.setup();
    server.use(
      http.get("*/reading-stats", () =>
        HttpResponse.json({
          from: null,
          to: null,
          granularity: "day",
          summary: {
            books: 3,
            duration: { measuredMs: 0, inferredMs: 0, totalMs: 0 },
            pagesRead: 0,
            sessions: 40,
            booksFinished: 12,
            sessionsWithoutDuration: 40,
            sessionsWithoutPages: 40,
          },
          periods: [
            {
              bucket: today(),
              duration: { measuredMs: 0, inferredMs: 0, totalMs: 0 },
              pagesRead: 0,
              sessions: 40,
              booksFinished: 12,
            },
          ],
          devices: [],
          series: [],
          formats: [],
        }),
      ),
    );

    const { container } = renderWithProviders(<ReadingStats />);

    // Under time this history is invisible: no duration was ever recorded.
    await screen.findByText("Daily activity");
    await waitFor(() => expect(litCells(container)).toHaveLength(0));

    await user.click(screen.getByRole("radio", { name: "Books finished" }));

    await waitFor(() => expect(litCells(container)).toHaveLength(1));
  });

  /// A panel titled "Time per week" while drawing book counts is the same class
  /// of lie as the rest of this work: the heading has to name the measure.
  it("names the period chart after the metric it draws", async () => {
    const user = userEvent.setup();
    renderWithProviders(<ReadingStats />);

    await screen.findByText("Time per day");

    await user.click(screen.getByRole("radio", { name: "Books finished" }));

    expect(
      await screen.findByText("Books finished per day"),
    ).toBeInTheDocument();
    expect(screen.queryByText("Time per day")).not.toBeInTheDocument();
  });

  /// Years come from the coverage endpoint, which deliberately ignores the
  /// window: the range control cannot offer a year it does not know exists.
  it("offers a year for every year the reader has history in", async () => {
    renderWithProviders(<ReadingStats />);

    await screen.findByText("Daily activity");

    expect(
      screen.getByRole("button", { name: "All time" }),
    ).toBeInTheDocument();
    for (const year of ["2024", "2025", "2026"]) {
      expect(screen.getByRole("button", { name: year })).toBeInTheDocument();
    }
  });

  /// A calendar year is the whole year, not the year so far.
  it("asks for a whole calendar year when one is picked", async () => {
    const user = userEvent.setup();
    renderWithProviders(<ReadingStats />);

    await screen.findByText("Daily activity");
    await user.click(screen.getByRole("button", { name: "2025" }));

    await waitFor(() => {
      const window = requestedWindows[requestedWindows.length - 1];
      // The wire carries UTC instants; the year's edges are the viewer's
      // midnights, so it is the local reading of those instants that must
      // land on January 1st and December 31st.
      expect(localDate(new Date(window.from ?? ""))).toBe("2025-01-01");
      expect(localDate(new Date(window.to ?? ""))).toBe("2025-12-31");
    });
    expect(await screen.findByText("Time per week")).toBeInTheDocument();
  });

  /// All-time starts where the reader's history does, and spans years, so one
  /// grid per year rather than one grid ten years wide.
  it("draws a calendar per year over all time", async () => {
    const user = userEvent.setup();
    renderWithProviders(<ReadingStats />);

    await screen.findByText("Daily activity");
    await user.click(screen.getByRole("button", { name: "All time" }));

    await waitFor(() => {
      const window = requestedWindows[requestedWindows.length - 1];
      expect(window.from).toContain("2024-03-04");
    });

    expect(await screen.findByText("Time per month")).toBeInTheDocument();

    // One grid for the year that has reading; the two silent years say so
    // rather than drawing a full year of empty cells.
    await waitFor(() =>
      expect(
        screen.getAllByRole("img", { name: /Daily reading activity/ }),
      ).toHaveLength(1),
    );
    expect(
      screen.getAllByText("No reading recorded in this period."),
    ).toHaveLength(2);
  });

  /// All-time's window comes from the coverage request, which resolves after
  /// the first render. Keyed only by the range name, the query would keep the
  /// result it fetched for the placeholder window and show an empty dashboard
  /// forever. Only reproducible when all-time is the *restored* range.
  it("uses the real window when all time is restored from storage", async () => {
    useReadingStatsPreferencesStore.setState({ range: { kind: "all" } });

    renderWithProviders(<ReadingStats />);

    await screen.findByText("Daily activity");
    await waitFor(() => {
      const window = requestedWindows[requestedWindows.length - 1];
      expect(window.from).toContain("2024-03-04");
    });
  });

  /// A stored year can outlive its data, or arrive from another account. The
  /// page must not ask for a window the reader has no history in.
  it("falls back when the stored year is not on offer", async () => {
    useReadingStatsPreferencesStore.setState({
      range: { kind: "year", year: 2019 },
    });

    renderWithProviders(<ReadingStats />);

    await screen.findByText("Daily activity");
    await waitFor(() => {
      const window = requestedWindows[requestedWindows.length - 1];
      expect(window.from).not.toContain("2019");
    });
    expect(screen.getByText("Time per day")).toBeInTheDocument();
  });

  /// The calendar is daily whatever the period chart shows. Requesting weekly
  /// buckets for the long range left it with nothing to draw, so every day in
  /// the year rendered as "did not read".
  it("keeps the calendar populated on the one-year range", async () => {
    const user = userEvent.setup();
    const { container } = renderWithProviders(<ReadingStats />);

    await screen.findByText("Daily activity");
    await user.click(screen.getByText("1 year"));

    await screen.findByText("Time per week");
    await waitFor(() => expect(litCells(container)).toHaveLength(1));
    expect(requestedGranularities).toEqual(["day", "day"]);
  });
});
