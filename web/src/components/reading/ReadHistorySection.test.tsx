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
    deleteBookHistoryEntry: vi.fn(),
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
    vi.mocked(readProgressApi.deleteBookHistoryEntry).mockResolvedValue(
      undefined,
    );
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

  // ==========================================================================
  // Removing a single entry
  // ==========================================================================

  /** History whose entries carry ids, as a book's do. */
  function bookHistory() {
    return {
      readCount: 2,
      lastCompletedAt: "2026-08-08T10:00:00Z",
      entries: [
        {
          id: "11111111-1111-1111-1111-111111111111",
          startedAt: "2026-08-07T10:00:00Z",
          completedAt: "2026-08-08T10:00:00Z",
        },
        {
          id: "22222222-2222-2222-2222-222222222222",
          startedAt: "2026-06-23T10:00:00Z",
          completedAt: "2026-06-24T10:00:00Z",
        },
      ],
    };
  }

  it("offers a way to remove each entry of a book's history", async () => {
    const user = userEvent.setup();
    vi.mocked(readProgressApi.getBookHistory).mockResolvedValue(bookHistory());

    renderWithProviders(<ReadHistorySection scope="book" id={BOOK_ID} />);

    await user.click(await screen.findByLabelText("Show read history"));

    const remove = await screen.findAllByLabelText(/^Remove the read-through/);
    expect(remove).toHaveLength(2);
  });

  it("removes the entry that was clicked", async () => {
    const user = userEvent.setup();
    vi.mocked(readProgressApi.getBookHistory).mockResolvedValue(bookHistory());

    renderWithProviders(<ReadHistorySection scope="book" id={BOOK_ID} />);

    await user.click(await screen.findByLabelText("Show read history"));
    const remove = await screen.findAllByLabelText(/^Remove the read-through/);
    await user.click(remove[1]);

    await waitFor(() =>
      expect(readProgressApi.deleteBookHistoryEntry).toHaveBeenCalledWith(
        BOOK_ID,
        "22222222-2222-2222-2222-222222222222",
      ),
    );
  });

  /// A series read-through is an aggregate over several books rather than a
  /// stored row, so the API sends no id and there is nothing single to delete.
  it("offers no per-entry removal on a series", async () => {
    const user = userEvent.setup();
    vi.mocked(readProgressApi.getSeriesHistory).mockResolvedValue(
      history(2, [
        ["2026-08-07T10:00:00Z", "2026-08-08T10:00:00Z"],
        ["2026-06-23T10:00:00Z", "2026-06-24T10:00:00Z"],
      ]),
    );

    renderWithProviders(<ReadHistorySection scope="series" id={SERIES_ID} />);

    await user.click(await screen.findByLabelText("Show read history"));
    // The date is split across elements; "(started …)" is one text node.
    await screen.findAllByText(/started/);

    expect(
      screen.queryByLabelText(/^Remove the read-through/),
    ).not.toBeInTheDocument();
  });
});
