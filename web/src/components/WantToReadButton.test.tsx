import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { wantToReadApi } from "@/api/wantToRead";
import { renderWithProviders, screen, waitFor } from "@/test/utils";
import { WantToReadButton } from "./WantToReadButton";

vi.mock("@/api/wantToRead", () => ({
  wantToReadApi: {
    addSeries: vi.fn().mockResolvedValue({}),
    addBook: vi.fn().mockResolvedValue({}),
    removeSeries: vi.fn().mockResolvedValue(undefined),
    removeBook: vi.fn().mockResolvedValue(undefined),
  },
}));

describe("WantToReadButton", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("adds a series when it isn't in the queue yet", async () => {
    const user = userEvent.setup();
    renderWithProviders(
      <WantToReadButton itemType="series" id="series-1" wantToRead={false} />,
    );

    await user.click(
      screen.getByRole("button", { name: /add to want to read/i }),
    );

    await waitFor(() =>
      expect(wantToReadApi.addSeries).toHaveBeenCalledWith("series-1"),
    );
    expect(wantToReadApi.removeSeries).not.toHaveBeenCalled();
  });

  it("removes a book when it is already in the queue", async () => {
    const user = userEvent.setup();
    renderWithProviders(
      <WantToReadButton itemType="book" id="book-1" wantToRead={true} />,
    );

    await user.click(
      screen.getByRole("button", { name: /remove from want to read/i }),
    );

    await waitFor(() =>
      expect(wantToReadApi.removeBook).toHaveBeenCalledWith("book-1"),
    );
    expect(wantToReadApi.addBook).not.toHaveBeenCalled();
  });
});
