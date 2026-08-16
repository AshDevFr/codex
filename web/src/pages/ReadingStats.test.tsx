import { screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { HttpResponse, http } from "msw";
import { setupServer } from "msw/node";
import { afterAll, afterEach, beforeAll, describe, expect, it } from "vitest";
import { useReadingStatsPreferencesStore } from "@/store/readingStatsPreferencesStore";
import { renderWithProviders } from "@/test/utils";
import { ReadingStats } from "./ReadingStats";

const MINUTE = 60_000;

/** Granularities the page asked for, newest last. */
let requestedGranularities: (string | null)[] = [];
/** Ranking keys the page asked for, newest last. */
let requestedSorts: (string | null)[] = [];

function today(): string {
  return new Date().toISOString().slice(0, 10);
}

const server = setupServer(
  http.get("*/api/v1/settings/branding", () =>
    HttpResponse.json({ applicationName: "Codex" }),
  ),
  http.get("*/reading-stats", ({ request }) => {
    const url = new URL(request.url);
    requestedGranularities.push(url.searchParams.get("granularity"));
    requestedSorts.push(url.searchParams.get("sort"));

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
  useReadingStatsPreferencesStore.setState({ metric: "time" });
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
