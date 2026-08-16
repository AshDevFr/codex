import { screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { HttpResponse, http } from "msw";
import { setupServer } from "msw/node";
import { afterAll, afterEach, beforeAll, describe, expect, it } from "vitest";
import { renderWithProviders } from "@/test/utils";
import { ReadingStats } from "./ReadingStats";

const MINUTE = 60_000;

/** Granularities the page asked for, newest last. */
let requestedGranularities: (string | null)[] = [];

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
