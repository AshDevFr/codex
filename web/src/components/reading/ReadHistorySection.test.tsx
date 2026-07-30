import { beforeEach, describe, expect, it, vi } from "vitest";
import { readProgressApi } from "@/api/readProgress";
import { renderWithProviders, screen, userEvent, waitFor } from "@/test/utils";
import { ReadHistorySection } from "./ReadHistorySection";

vi.mock("@/api/readProgress", () => ({
  readProgressApi: {
    getBookHistory: vi.fn(),
    getSeriesHistory: vi.fn(),
    clearBookHistory: vi.fn(),
    clearSeriesHistory: vi.fn(),
  },
}));

const BOOK_ID = "bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb";
const SERIES_ID = "ssssssss-ssss-ssss-ssss-ssssssssssss";

function history(readCount: number, dates: [string, string][] = []) {
  return {
    readCount,
    lastCompletedAt: dates[0]?.[1] ?? null,
    entries: dates.map(([startedAt, completedAt]) => ({
      startedAt,
      completedAt,
    })),
  };
}

describe("ReadHistorySection", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(readProgressApi.clearBookHistory).mockResolvedValue(undefined);
    vi.mocked(readProgressApi.clearSeriesHistory).mockResolvedValue(undefined);
  });

  // A reader who has never finished the book has no history to reason about, so
  // an empty section would be noise on every unread book.
  it("renders nothing when nothing has been finished", async () => {
    vi.mocked(readProgressApi.getBookHistory).mockResolvedValue(history(0));

    renderWithProviders(<ReadHistorySection scope="book" id={BOOK_ID} />);

    await waitFor(() =>
      expect(readProgressApi.getBookHistory).toHaveBeenCalled(),
    );
    expect(screen.queryByText(/you've finished this/i)).not.toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: /clear read history/i }),
    ).not.toBeInTheDocument();
  });

  it("summarizes a single completion in the singular", async () => {
    vi.mocked(readProgressApi.getBookHistory).mockResolvedValue(
      history(1, [["2025-03-01T00:00:00Z", "2025-03-05T00:00:00Z"]]),
    );

    renderWithProviders(<ReadHistorySection scope="book" id={BOOK_ID} />);

    expect(
      await screen.findByText(/you've finished this book once/i),
    ).toBeVisible();
  });

  it("summarizes repeat reads with a count and the last date", async () => {
    vi.mocked(readProgressApi.getBookHistory).mockResolvedValue(
      history(2, [
        ["2025-03-01T00:00:00Z", "2025-03-05T00:00:00Z"],
        ["2024-01-01T00:00:00Z", "2024-01-08T00:00:00Z"],
      ]),
    );

    renderWithProviders(<ReadHistorySection scope="book" id={BOOK_ID} />);

    expect(
      await screen.findByText(/you've finished this book 2 times, last on/i),
    ).toBeVisible();
  });

  // The dated list is collapsed by default so the section stays one line.
  it("reveals the dated entries when expanded", async () => {
    const user = userEvent.setup();
    vi.mocked(readProgressApi.getBookHistory).mockResolvedValue(
      history(2, [
        ["2025-03-01T00:00:00Z", "2025-03-05T00:00:00Z"],
        ["2024-01-01T00:00:00Z", "2024-01-08T00:00:00Z"],
      ]),
    );

    renderWithProviders(<ReadHistorySection scope="book" id={BOOK_ID} />);
    await screen.findByText(/you've finished this book/i);

    await user.click(
      screen.getByRole("button", { name: /show read history/i }),
    );

    // Both passes are listed, with their start dates.
    await waitFor(() =>
      expect(screen.getAllByText(/started/i)).toHaveLength(2),
    );
    expect(
      screen.getByRole("button", { name: /hide read history/i }),
    ).toBeInTheDocument();
  });

  it("uses series wording for a series", async () => {
    vi.mocked(readProgressApi.getSeriesHistory).mockResolvedValue(
      history(1, [["2025-03-01T00:00:00Z", "2025-03-05T00:00:00Z"]]),
    );

    renderWithProviders(<ReadHistorySection scope="series" id={SERIES_ID} />);

    expect(
      await screen.findByText(/you've finished this series once/i),
    ).toBeVisible();
    expect(readProgressApi.getSeriesHistory).toHaveBeenCalledWith(SERIES_ID);
    expect(readProgressApi.getBookHistory).not.toHaveBeenCalled();
  });

  // The one thing a user will be nervous about before pressing the button.
  it("promises the reset leaves current progress alone", async () => {
    const user = userEvent.setup();
    vi.mocked(readProgressApi.getBookHistory).mockResolvedValue(
      history(1, [["2025-03-01T00:00:00Z", "2025-03-05T00:00:00Z"]]),
    );

    renderWithProviders(<ReadHistorySection scope="book" id={BOOK_ID} />);
    await screen.findByText(/you've finished this book/i);

    await user.click(
      screen.getByRole("button", { name: /clear read history/i }),
    );

    expect(
      await screen.findByText(/current reading progress is not affected/i),
    ).toBeInTheDocument();
  });

  it("clears a book's history on confirmation", async () => {
    const user = userEvent.setup();
    vi.mocked(readProgressApi.getBookHistory).mockResolvedValue(
      history(1, [["2025-03-01T00:00:00Z", "2025-03-05T00:00:00Z"]]),
    );

    renderWithProviders(<ReadHistorySection scope="book" id={BOOK_ID} />);
    await screen.findByText(/you've finished this book/i);

    await user.click(
      screen.getByRole("button", { name: /clear read history/i }),
    );
    await user.click(
      await screen.findByRole("button", {
        name: /^clear history$/i,
        hidden: true,
      }),
    );

    await waitFor(() =>
      expect(readProgressApi.clearBookHistory).toHaveBeenCalledWith(BOOK_ID),
    );
    expect(readProgressApi.clearSeriesHistory).not.toHaveBeenCalled();
  });

  it("clears a series' history on confirmation", async () => {
    const user = userEvent.setup();
    vi.mocked(readProgressApi.getSeriesHistory).mockResolvedValue(
      history(1, [["2025-03-01T00:00:00Z", "2025-03-05T00:00:00Z"]]),
    );

    renderWithProviders(<ReadHistorySection scope="series" id={SERIES_ID} />);
    await screen.findByText(/you've finished this series/i);

    await user.click(
      screen.getByRole("button", { name: /clear read history/i }),
    );
    await user.click(
      await screen.findByRole("button", {
        name: /^clear history$/i,
        hidden: true,
      }),
    );

    await waitFor(() =>
      expect(readProgressApi.clearSeriesHistory).toHaveBeenCalledWith(
        SERIES_ID,
      ),
    );
    expect(readProgressApi.clearBookHistory).not.toHaveBeenCalled();
  });

  it("does not clear anything when the dialog is cancelled", async () => {
    const user = userEvent.setup();
    vi.mocked(readProgressApi.getBookHistory).mockResolvedValue(
      history(1, [["2025-03-01T00:00:00Z", "2025-03-05T00:00:00Z"]]),
    );

    renderWithProviders(<ReadHistorySection scope="book" id={BOOK_ID} />);
    await screen.findByText(/you've finished this book/i);

    await user.click(
      screen.getByRole("button", { name: /clear read history/i }),
    );
    await user.click(
      await screen.findByRole("button", { name: /cancel/i, hidden: true }),
    );

    expect(readProgressApi.clearBookHistory).not.toHaveBeenCalled();
  });

  // The API returns UTC; the section must not print a raw ISO string.
  it("formats dates for the viewer rather than showing raw ISO", async () => {
    vi.mocked(readProgressApi.getBookHistory).mockResolvedValue(
      history(1, [["2025-03-01T00:00:00Z", "2025-03-05T00:00:00Z"]]),
    );

    renderWithProviders(<ReadHistorySection scope="book" id={BOOK_ID} />);
    await screen.findByText(/you've finished this book/i);

    expect(screen.queryByText(/2025-03-05T00:00:00Z/)).not.toBeInTheDocument();
  });
});
