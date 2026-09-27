import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { readingStatsApi } from "@/api/readingStats";
import { renderWithProviders, screen, waitFor } from "@/test/utils";
import { RemovedHistoryPurge } from "./RemovedHistoryPurge";

vi.mock("@/api/readingStats", () => ({
  readingStatsApi: {
    orphaned: vi.fn(),
    purgeOrphaned: vi.fn(),
  },
}));

const HOUR = 60 * 60_000;

describe("RemovedHistoryPurge", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  /// The dashboard row is windowed and a purge is not, so the dialog has to
  /// state the unwindowed totals it fetched, not the row's.
  it("states what will be deleted across all dates before asking", async () => {
    const user = userEvent.setup();
    vi.mocked(readingStatsApi.orphaned).mockResolvedValue({
      duration: { measuredMs: 2 * HOUR, inferredMs: 0, totalMs: 2 * HOUR },
      pagesRead: 80,
      sessions: 5,
      completions: 2,
    });
    renderWithProviders(<RemovedHistoryPurge />);

    await user.click(screen.getByRole("button", { name: "Delete" }));

    expect(
      await screen.findByText(
        /permanently deletes 5 sittings \(2h of reading\)/,
      ),
    ).toBeInTheDocument();
    expect(screen.getByText(/2 finished reads/)).toBeInTheDocument();
    expect(screen.getByText(/across all dates/)).toBeInTheDocument();
    expect(readingStatsApi.purgeOrphaned).not.toHaveBeenCalled();
  });

  it("deletes only after the second confirmation", async () => {
    const user = userEvent.setup();
    vi.mocked(readingStatsApi.orphaned).mockResolvedValue({
      duration: { measuredMs: HOUR, inferredMs: 0, totalMs: HOUR },
      pagesRead: 10,
      sessions: 1,
      completions: 0,
    });
    vi.mocked(readingStatsApi.purgeOrphaned).mockResolvedValue({
      sessionsRemoved: 1,
      completionsRemoved: 0,
    });
    renderWithProviders(<RemovedHistoryPurge />);

    await user.click(screen.getByRole("button", { name: "Delete" }));
    const confirm = await screen.findByRole("button", {
      name: "Delete permanently",
    });
    await waitFor(() => expect(confirm).toBeEnabled());
    await user.click(confirm);

    await waitFor(() =>
      expect(readingStatsApi.purgeOrphaned).toHaveBeenCalledTimes(1),
    );
  });

  it("cannot delete when there is nothing left to delete", async () => {
    const user = userEvent.setup();
    vi.mocked(readingStatsApi.orphaned).mockResolvedValue({
      duration: { measuredMs: 0, inferredMs: 0, totalMs: 0 },
      pagesRead: 0,
      sessions: 0,
      completions: 0,
    });
    renderWithProviders(<RemovedHistoryPurge />);

    await user.click(screen.getByRole("button", { name: "Delete" }));

    expect(
      await screen.findByText("There is no removed history left to delete."),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Delete permanently" }),
    ).toBeDisabled();
  });
});
