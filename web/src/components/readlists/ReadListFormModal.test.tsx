import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { readListsApi } from "@/api/readlists";
import { renderWithProviders, screen, waitFor } from "@/test/utils";
import { ReadListFormModal } from "./ReadListFormModal";

vi.mock("@/api/readlists", () => ({
  readListsApi: {
    create: vi.fn().mockResolvedValue({
      id: "r1",
      name: "Civil War",
      summary: "Crossover",
      ordered: true,
      bookCount: 0,
      createdAt: "2026-06-15T00:00:00Z",
      updatedAt: "2026-06-15T00:00:00Z",
    }),
    update: vi.fn(),
  },
}));

describe("ReadListFormModal", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("creates a read list with name, summary, and ordered flag", async () => {
    const user = userEvent.setup();
    const onCreated = vi.fn();
    renderWithProviders(
      <ReadListFormModal opened onClose={vi.fn()} onCreated={onCreated} />,
    );

    await user.type(screen.getByPlaceholderText("e.g. Civil War"), "Civil War");
    await user.type(
      screen.getByPlaceholderText("Optional description"),
      "Crossover",
    );
    await user.click(screen.getByRole("button", { name: /create/i }));

    await waitFor(() =>
      expect(readListsApi.create).toHaveBeenCalledWith({
        name: "Civil War",
        summary: "Crossover",
        ordered: true,
      }),
    );
    await waitFor(() => expect(onCreated).toHaveBeenCalled());
  });
});
