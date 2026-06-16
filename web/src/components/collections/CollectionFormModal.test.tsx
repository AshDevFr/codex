import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { collectionsApi } from "@/api/collections";
import { renderWithProviders, screen, waitFor } from "@/test/utils";
import { CollectionFormModal } from "./CollectionFormModal";

vi.mock("@/api/collections", () => ({
  collectionsApi: {
    create: vi.fn().mockResolvedValue({
      id: "c1",
      name: "Batman",
      ordered: true,
      seriesCount: 0,
      createdAt: "2026-06-15T00:00:00Z",
      updatedAt: "2026-06-15T00:00:00Z",
    }),
    update: vi.fn(),
  },
}));

describe("CollectionFormModal", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("creates a collection with the entered name and ordered flag", async () => {
    const user = userEvent.setup();
    const onClose = vi.fn();
    const onCreated = vi.fn();
    renderWithProviders(
      <CollectionFormModal opened onClose={onClose} onCreated={onCreated} />,
    );

    await user.type(screen.getByPlaceholderText("e.g. Batman"), "Batman");
    await user.click(screen.getByRole("checkbox"));
    await user.click(screen.getByRole("button", { name: /create/i }));

    await waitFor(() =>
      expect(collectionsApi.create).toHaveBeenCalledWith({
        name: "Batman",
        ordered: true,
      }),
    );
    await waitFor(() => expect(onCreated).toHaveBeenCalled());
    expect(onClose).toHaveBeenCalled();
  });
});
