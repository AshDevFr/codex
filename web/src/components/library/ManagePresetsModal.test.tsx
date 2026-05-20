import { screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { renderWithProviders, userEvent } from "@/test/utils";
import { ManagePresetsModal } from "./ManagePresetsModal";

vi.mock("@/api/filterPresets", () => ({
  filterPresetsApi: {
    list: vi.fn(),
    update: vi.fn(),
    delete: vi.fn(),
  },
}));

const { filterPresetsApi } = await import("@/api/filterPresets");
const listMock = filterPresetsApi.list as ReturnType<typeof vi.fn>;
const updateMock = filterPresetsApi.update as ReturnType<typeof vi.fn>;
const deleteMock = filterPresetsApi.delete as ReturnType<typeof vi.fn>;

const listPreset = {
  id: "p-list",
  userId: "u1",
  libraryId: "lib-1",
  name: "List page preset",
  scope: "list" as const,
  target: "series" as const,
  condition: { genre: { operator: "is", value: "Action" } },
  query: null,
  sort: null,
  createdAt: "2026-05-19T00:00:00Z",
  updatedAt: "2026-05-19T00:00:00Z",
};

const searchPreset = {
  ...listPreset,
  id: "p-search",
  scope: "search" as const,
  name: "Search preset",
  query: "one punch",
};

describe("ManagePresetsModal", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    // Mantine Tabs mounts both panels: drive the mock by target so each tab
    // only sees its own presets and queries can match a single instance.
    listMock.mockImplementation(({ target }: { target?: string } = {}) => {
      if (target === "series")
        return Promise.resolve([listPreset, searchPreset]);
      return Promise.resolve([]);
    });
  });

  it("renders nothing when closed", () => {
    renderWithProviders(
      <ManagePresetsModal opened={false} onClose={vi.fn()} />,
    );
    expect(
      screen.queryByText(/Manage filter presets/i),
    ).not.toBeInTheDocument();
  });

  it("shows empty state when no presets exist", async () => {
    listMock.mockResolvedValue([]);
    renderWithProviders(<ManagePresetsModal opened onClose={vi.fn()} />);
    expect(
      await screen.findByText(/you haven't saved any series presets yet/i),
    ).toBeInTheDocument();
  });

  it("groups presets by scope", async () => {
    renderWithProviders(<ManagePresetsModal opened onClose={vi.fn()} />);

    expect(await screen.findByText(/List page preset/)).toBeInTheDocument();
    expect(screen.getByText(/Search preset/)).toBeInTheDocument();
    expect(screen.getByText(/List pages/)).toBeInTheDocument();
    expect(screen.getByText(/Advanced search/)).toBeInTheDocument();
  });

  it("deletes a preset after confirm", async () => {
    deleteMock.mockResolvedValue(undefined);
    const confirmSpy = vi
      .spyOn(window, "confirm")
      .mockImplementation(() => true);

    renderWithProviders(<ManagePresetsModal opened onClose={vi.fn()} />);

    await screen.findByText("List page preset");
    const deleteBtn = screen.getByLabelText(/Delete List page preset/i);
    await userEvent.click(deleteBtn);

    await waitFor(() => expect(deleteMock).toHaveBeenCalledWith("p-list"));
    confirmSpy.mockRestore();
  });

  it("renames a preset", async () => {
    updateMock.mockResolvedValue({ ...listPreset, name: "Renamed" });

    renderWithProviders(<ManagePresetsModal opened onClose={vi.fn()} />);

    await screen.findByText("List page preset");
    await userEvent.click(screen.getByLabelText(/Rename List page preset/i));

    const input = screen.getByDisplayValue("List page preset");
    await userEvent.clear(input);
    await userEvent.type(input, "Renamed");
    await userEvent.click(screen.getByRole("button", { name: "Save" }));

    await waitFor(() => expect(updateMock).toHaveBeenCalledTimes(1));
    expect(updateMock).toHaveBeenCalledWith(
      "p-list",
      expect.objectContaining({ name: "Renamed" }),
    );
  });
});
