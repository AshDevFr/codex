import { describe, expect, it } from "vitest";
import type {
  ReadingByDeviceDto,
  ReadingBySeriesDto,
} from "@/api/readingStats";
import { renderWithProviders, screen } from "@/test/utils";
import {
  ActivityCalendar,
  busiestBucketCaption,
  DeviceBreakdown,
  ProvenanceLegend,
  StatTile,
  TopSeries,
} from "./ReadingStatsPanels";
import { buildCalendar } from "./readingStatsFormat";

const MINUTE = 60_000;
const HOUR = 60 * MINUTE;

function duration(measuredMs: number, inferredMs = 0) {
  return { measuredMs, inferredMs, totalMs: measuredMs + inferredMs };
}

describe("StatTile", () => {
  it("shows the headline figure and its label", () => {
    renderWithProviders(<StatTile label="Time read" value="3h 20m" />);

    expect(screen.getByText("Time read")).toBeInTheDocument();
    expect(screen.getByText("3h 20m")).toBeInTheDocument();
  });

  it("shows the measured portion when there is one", () => {
    renderWithProviders(
      <StatTile label="Time read" value="4h" hint="3h measured" />,
    );

    expect(screen.getByText("3h measured")).toBeInTheDocument();
  });
});

describe("ProvenanceLegend", () => {
  /// Identity must never rest on colour alone, so both halves are named.
  it("names both kinds of time", () => {
    renderWithProviders(<ProvenanceLegend inferredMs={45 * MINUTE} />);

    expect(screen.getByText("Measured")).toBeInTheDocument();
    expect(screen.getByText(/Estimated \(45m\)/)).toBeInTheDocument();
  });
});

describe("ActivityCalendar", () => {
  it("says so plainly when nothing was read", () => {
    renderWithProviders(<ActivityCalendar days={[]} />);

    expect(
      screen.getByText("No reading recorded in this period."),
    ).toBeInTheDocument();
  });

  it("draws a cell for every day in the window", () => {
    const days = buildCalendar(
      [],
      new Date("2026-06-01T00:00:00Z"),
      new Date("2026-06-07T00:00:00Z"),
    );

    const { container } = renderWithProviders(<ActivityCalendar days={days} />);

    expect(container.querySelectorAll("rect")).toHaveLength(7);
  });

  it("labels the grid for screen readers", () => {
    const days = buildCalendar(
      [],
      new Date("2026-06-01T00:00:00Z"),
      new Date("2026-06-03T00:00:00Z"),
    );

    renderWithProviders(<ActivityCalendar days={days} />);

    expect(
      screen.getByRole("img", { name: /Daily reading activity, 3 days/ }),
    ).toBeInTheDocument();
  });
});

describe("TopSeries", () => {
  const series: ReadingBySeriesDto[] = [
    {
      seriesId: "11111111-1111-1111-1111-111111111111",
      seriesName: "Berserk",
      duration: duration(2 * HOUR),
      pagesRead: 120,
      sessions: 4,
      books: 2,
    },
    {
      seriesId: "22222222-2222-2222-2222-222222222222",
      seriesName: "Vinland Saga",
      duration: duration(30 * MINUTE),
      pagesRead: 40,
      sessions: 1,
      books: 1,
    },
  ];

  it("lists series with their time and page counts", () => {
    renderWithProviders(<TopSeries series={series} />);

    expect(screen.getByText("Berserk")).toBeInTheDocument();
    expect(screen.getByText("2h")).toBeInTheDocument();
    expect(screen.getByText("120 pages across 2 books")).toBeInTheDocument();
  });

  /// "1 books" reads as a bug even when the number is right.
  it("says book rather than books for a single volume", () => {
    renderWithProviders(<TopSeries series={series} />);

    expect(screen.getByText("40 pages across 1 book")).toBeInTheDocument();
  });

  it("says so plainly when nothing was read", () => {
    renderWithProviders(<TopSeries series={[]} />);

    expect(
      screen.getByText("No series read in this period."),
    ).toBeInTheDocument();
  });
});

describe("DeviceBreakdown", () => {
  const devices: ReadingByDeviceDto[] = [
    {
      deviceId: "browser-1",
      deviceName: "Codex Web (Mac)",
      duration: duration(90 * MINUTE),
      pagesRead: 60,
      sessions: 3,
      lastReadAt: "2026-06-10T12:00:00Z",
    },
    {
      deviceId: "apikey:abc",
      deviceName: null,
      duration: duration(0, 20 * MINUTE),
      pagesRead: 10,
      sessions: 1,
      lastReadAt: "2026-06-09T12:00:00Z",
    },
  ];

  it("prefers a friendly device name", () => {
    renderWithProviders(<DeviceBreakdown devices={devices} />);

    expect(screen.getByText("Codex Web (Mac)")).toBeInTheDocument();
  });

  /// A device with no friendly name still has to be identifiable.
  it("falls back to the raw id when there is no name", () => {
    renderWithProviders(<DeviceBreakdown devices={devices} />);

    expect(screen.getByText("apikey:abc")).toBeInTheDocument();
  });

  it("says sitting rather than sittings for one", () => {
    renderWithProviders(<DeviceBreakdown devices={devices} />);

    expect(screen.getByText(/1 sitting, last read/)).toBeInTheDocument();
  });
});

describe("busiestBucketCaption", () => {
  it("names the busiest bucket", () => {
    const caption = busiestBucketCaption([
      { bucket: "2026-06-01", totalMs: 30 * MINUTE },
      { bucket: "2026-06-02", totalMs: 2 * HOUR },
    ]);

    expect(caption).toBe("Busiest: 2026-06-02, 2h");
  });

  it("says nothing when there is nothing to report", () => {
    expect(busiestBucketCaption([])).toBeNull();
    expect(
      busiestBucketCaption([{ bucket: "2026-06-01", totalMs: 0 }]),
    ).toBeNull();
  });
});
