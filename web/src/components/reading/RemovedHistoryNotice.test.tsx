import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { type OrphanedHistory, readingStatsApi } from "@/api/readingStats";
import { renderWithProviders, screen, waitFor } from "@/test/utils";
import { RemovedHistoryNotice } from "./RemovedHistoryNotice";

vi.mock("@/api/readingStats", () => ({
  readingStatsApi: {
    orphaned: vi.fn(),
    purgeOrphaned: vi.fn(),
  },
}));

const HOUR = 60 * 60_000;

function totals(
  sessions: number,
  totalMs: number,
  completions: number,
): OrphanedHistory {
  return {
    duration: { measuredMs: totalMs, inferredMs: 0, totalMs },
    pagesRead: 0,
    sessions,
    completions,
  };
}

async function openDialog() {
  const user = userEvent.setup();
  await user.click(
    await screen.findByRole("button", {
      name: "Delete reading of removed books",
    }),
  );
  return user;
}

describe("RemovedHistoryNotice", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("renders nothing when no removed history exists", async () => {
    vi.mocked(readingStatsApi.orphaned).mockResolvedValue(totals(0, 0, 0));
    renderWithProviders(<RemovedHistoryNotice />);

    await waitFor(() => expect(readingStatsApi.orphaned).toHaveBeenCalled());
    expect(
      screen.queryByText("Reading of removed books"),
    ).not.toBeInTheDocument();
  });

  /// Not bound to the dashboard window or the series ranking: finished reads
  /// with no sittings at all would never produce a series row.
  it("offers the purge for completions that have no sittings", async () => {
    vi.mocked(readingStatsApi.orphaned).mockResolvedValue(totals(0, 0, 3));
    renderWithProviders(<RemovedHistoryNotice />);

    expect(
      await screen.findByText(/3 finished reads from books since deleted/),
    ).toBeInTheDocument();
  });

  it("states what will be deleted across all dates before asking", async () => {
    vi.mocked(readingStatsApi.orphaned).mockResolvedValue(
      totals(5, 2 * HOUR, 2),
    );
    renderWithProviders(<RemovedHistoryNotice />);
    await openDialog();

    expect(
      await screen.findByText(
        /permanently deletes 5 sittings \(2h of reading\) and 2 finished reads, across all dates/,
      ),
    ).toBeInTheDocument();
    expect(readingStatsApi.purgeOrphaned).not.toHaveBeenCalled();
  });

  it("re-reads the totals on open and deletes only after confirming", async () => {
    vi.mocked(readingStatsApi.orphaned).mockResolvedValue(totals(1, HOUR, 0));
    vi.mocked(readingStatsApi.purgeOrphaned).mockResolvedValue({
      sessionsRemoved: 1,
      completionsRemoved: 0,
    });
    renderWithProviders(<RemovedHistoryNotice />);
    const user = await openDialog();

    await waitFor(() =>
      expect(readingStatsApi.orphaned).toHaveBeenCalledTimes(2),
    );
    const confirm = await screen.findByRole("button", {
      name: "Delete permanently",
    });
    await waitFor(() => expect(confirm).toBeEnabled());
    await user.click(confirm);

    await waitFor(() =>
      expect(readingStatsApi.purgeOrphaned).toHaveBeenCalledTimes(1),
    );
  });

  it("will not confirm against totals it failed to refresh", async () => {
    vi.mocked(readingStatsApi.orphaned)
      .mockResolvedValueOnce(totals(4, HOUR, 0))
      .mockRejectedValue(new Error("offline"));
    renderWithProviders(<RemovedHistoryNotice />);
    await openDialog();

    expect(
      await screen.findByText(/Could not total the removed history/),
    ).toBeInTheDocument();
    expect(screen.queryByText(/permanently deletes/)).not.toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Delete permanently" }),
    ).toBeDisabled();
  });

  it("says so when the history is already gone by the time it opens", async () => {
    vi.mocked(readingStatsApi.orphaned)
      .mockResolvedValueOnce(totals(2, HOUR, 0))
      .mockResolvedValue(totals(0, 0, 0));
    renderWithProviders(<RemovedHistoryNotice />);
    await openDialog();

    expect(
      await screen.findByText("There is no removed history left to delete."),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Delete permanently" }),
    ).toBeDisabled();
  });
});
