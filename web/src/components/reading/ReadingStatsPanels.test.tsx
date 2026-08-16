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
  FormatBreakdown,
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

  /// Silent days are drawn too, so the grid has no holes where the quiet days
  /// were. One day of reading is enough to make the window worth drawing.
  it("draws a cell for every day in the window", () => {
    const days = buildCalendar(
      [
        {
          bucket: "2026-06-03",
          duration: duration(45 * MINUTE),
          pagesRead: 20,
          sessions: 1,
        },
      ],
      new Date("2026-06-01T00:00:00Z"),
      new Date("2026-06-07T00:00:00Z"),
    );

    const { container } = renderWithProviders(<ActivityCalendar days={days} />);

    expect(container.querySelectorAll("rect")).toHaveLength(7);
  });

  /// A filled window always has weeks, so "did the API return nothing" is the
  /// wrong question: a year the reader did not read in produces 365 zeroes and
  /// a uniformly empty grid that looks exactly like a bug.
  it("says so plainly when every day in the window is zero", () => {
    const days = buildCalendar(
      [],
      new Date("2026-01-01T00:00:00Z"),
      new Date("2026-12-31T00:00:00Z"),
    );

    const { container } = renderWithProviders(<ActivityCalendar days={days} />);

    expect(
      screen.getByText("No reading recorded in this period."),
    ).toBeInTheDocument();
    expect(container.querySelectorAll("rect")).toHaveLength(0);
  });

  it("labels the grid for screen readers", () => {
    const days = buildCalendar(
      [
        {
          bucket: "2026-06-02",
          duration: duration(45 * MINUTE),
          pagesRead: 20,
          sessions: 1,
        },
      ],
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
      booksFinished: 2,
    },
    {
      seriesId: "22222222-2222-2222-2222-222222222222",
      seriesName: "Vinland Saga",
      duration: duration(30 * MINUTE),
      pagesRead: 40,
      sessions: 1,
      books: 1,
      booksFinished: 0,
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

  it("links each series to its detail page", () => {
    renderWithProviders(<TopSeries series={series} />);

    expect(screen.getByRole("link", { name: "Berserk" })).toHaveAttribute(
      "href",
      "/series/11111111-1111-1111-1111-111111111111",
    );
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

/// Rows that contributed no time are noise: before this, a library imported
/// from before time tracking filled every panel with "0m" rows.
describe("empty rows", () => {
  it("leaves series that contributed no time out of the ranking", () => {
    renderWithProviders(
      <TopSeries
        series={[
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
            seriesName: "Imported Series",
            duration: duration(0),
            pagesRead: 0,
            sessions: 6,
            books: 6,
          },
        ]}
      />,
    );

    expect(screen.getByText("Berserk")).toBeInTheDocument();
    expect(screen.queryByText("Imported Series")).not.toBeInTheDocument();
  });

  it("says so plainly when every series is empty", () => {
    renderWithProviders(
      <TopSeries
        series={[
          {
            seriesId: "22222222-2222-2222-2222-222222222222",
            seriesName: "Imported Series",
            duration: duration(0),
            pagesRead: 0,
            sessions: 6,
            books: 6,
          },
        ]}
      />,
    );

    expect(
      screen.getByText("No series read in this period."),
    ).toBeInTheDocument();
  });

  it("leaves devices that reported no time out", () => {
    renderWithProviders(
      <DeviceBreakdown
        devices={[
          {
            deviceId: "browser-1",
            deviceName: "Codex Web (Mac)",
            duration: duration(90 * MINUTE),
            pagesRead: 60,
            sessions: 3,
            lastReadAt: "2026-06-10T12:00:00Z",
          },
          {
            deviceId: "legacy",
            deviceName: null,
            duration: duration(0),
            pagesRead: 0,
            sessions: 3656,
            lastReadAt: "2026-08-16T12:00:00Z",
          },
        ]}
      />,
    );

    expect(screen.getByText("Codex Web (Mac)")).toBeInTheDocument();
    expect(screen.queryByText(/3656 sittings/)).not.toBeInTheDocument();
  });

  it("leaves formats that reported no time out", () => {
    renderWithProviders(
      <FormatBreakdown
        formats={[
          {
            format: "cbz",
            duration: duration(90 * MINUTE),
            pagesRead: 60,
            sessions: 3,
            books: 2,
          },
          {
            format: "pdf",
            duration: duration(0),
            pagesRead: 0,
            sessions: 0,
            books: 0,
          },
        ]}
      />,
    );

    expect(screen.getByText("cbz")).toBeInTheDocument();
    expect(screen.queryByText("pdf")).not.toBeInTheDocument();
    expect(screen.getByText("1h 30m")).toBeInTheDocument();
  });

  it("renders no format panel at all when every format is empty", () => {
    renderWithProviders(
      <FormatBreakdown
        formats={[
          {
            format: "pdf",
            duration: duration(0),
            pagesRead: 0,
            sessions: 0,
            books: 0,
          },
        ]}
      />,
    );

    expect(screen.queryByText("pdf")).not.toBeInTheDocument();
    expect(screen.queryByText("0m")).not.toBeInTheDocument();
  });
});

/// The whole point of the metric toggle: a row silent under one measure can be
/// the reader's entire history under another.
describe("the active metric", () => {
  const legacy: ReadingByDeviceDto = {
    deviceId: "legacy",
    deviceName: null,
    duration: duration(0),
    pagesRead: 0,
    sessions: 3656,
    booksFinished: 412,
    lastReadAt: "2026-08-16T12:00:00Z",
  };

  it("hides a row with no time when showing time", () => {
    renderWithProviders(<DeviceBreakdown devices={[legacy]} metric="time" />);

    expect(
      screen.getByText("No devices recorded in this period."),
    ).toBeInTheDocument();
  });

  it("shows that same row when showing books finished", () => {
    renderWithProviders(
      <DeviceBreakdown devices={[legacy]} metric="booksFinished" />,
    );

    expect(screen.getByText("Before time tracking")).toBeInTheDocument();
    expect(screen.getByText("412 books")).toBeInTheDocument();
  });

  it("counts a series by pages when showing pages", () => {
    renderWithProviders(
      <TopSeries
        series={[
          {
            seriesId: "11111111-1111-1111-1111-111111111111",
            seriesName: "Berserk",
            duration: duration(2 * HOUR),
            pagesRead: 420,
            sessions: 4,
            books: 2,
            booksFinished: 1,
          },
        ]}
        metric="pages"
      />,
    );

    expect(screen.getByText("420 pages")).toBeInTheDocument();
    expect(screen.queryByText("2h")).not.toBeInTheDocument();
  });

  /// The calendar has to follow too, or the panels disagree about what they
  /// are measuring while sitting on the same screen.
  it("colours the calendar by the metric", () => {
    const days = buildCalendar(
      [
        {
          bucket: "2026-06-02",
          duration: duration(0),
          pagesRead: 0,
          sessions: 9,
          booksFinished: 3,
        },
      ],
      new Date("2026-06-01T00:00:00Z"),
      new Date("2026-06-03T00:00:00Z"),
    );

    const { container } = renderWithProviders(
      <ActivityCalendar days={days} metric="booksFinished" />,
    );

    const lit = [...container.querySelectorAll("rect")].filter(
      (cell) => cell.getAttribute("fill") !== "var(--heat-0)",
    );
    expect(lit).toHaveLength(1);
  });
});

/// `legacy` is a device id the backfill invents for reading that predates
/// session tracking. Showing it raw presents an implementation detail as a
/// piece of hardware the reader owns.
describe("the legacy device", () => {
  it("is named for what it is", () => {
    renderWithProviders(
      <DeviceBreakdown
        devices={[
          {
            deviceId: "legacy",
            deviceName: null,
            duration: duration(30 * MINUTE),
            pagesRead: 10,
            sessions: 12,
            lastReadAt: "2026-06-10T12:00:00Z",
          },
        ]}
      />,
    );

    expect(screen.getByText("Before time tracking")).toBeInTheDocument();
    expect(screen.queryByText("legacy")).not.toBeInTheDocument();
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
