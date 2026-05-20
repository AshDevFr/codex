import { screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { renderWithProviders, userEvent } from "@/test/utils";
import { ListPresetControls } from "./ListPresetControls";

vi.mock("@/api/filterPresets", () => ({
  filterPresetsApi: {
    list: vi.fn(),
    create: vi.fn(),
    delete: vi.fn(),
    update: vi.fn(),
  },
}));

const { filterPresetsApi } = await import("@/api/filterPresets");
const listMock = filterPresetsApi.list as ReturnType<typeof vi.fn>;
const createMock = filterPresetsApi.create as ReturnType<typeof vi.fn>;
const deleteMock = filterPresetsApi.delete as ReturnType<typeof vi.fn>;

const samplePreset = {
  id: "preset-1",
  userId: "user-1",
  libraryId: "lib-1",
  name: "Manga unread",
  scope: "list" as const,
  target: "books" as const,
  condition: {},
  query: null,
  sort: null,
  createdAt: "2026-05-19T00:00:00Z",
  updatedAt: "2026-05-19T00:00:00Z",
};

const globalPreset = {
  ...samplePreset,
  id: "preset-2",
  libraryId: null,
  name: "Reading list",
};

describe("ListPresetControls", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    listMock.mockResolvedValue([]);
  });

  it("disables save when no filter chips are active", () => {
    renderWithProviders(
      <ListPresetControls
        target="books"
        libraryId="lib-1"
        currentCondition={undefined}
        hasActiveFilters={false}
        onApply={vi.fn()}
      />,
    );

    const saveBtn = screen.getByRole("button", { name: /save preset/i });
    expect(saveBtn).toBeDisabled();
  });

  it("renders the empty-state copy when the user has no presets", async () => {
    renderWithProviders(
      <ListPresetControls
        target="books"
        libraryId="lib-1"
        currentCondition={undefined}
        hasActiveFilters={false}
        onApply={vi.fn()}
      />,
    );

    await waitFor(() => {
      expect(
        screen.getByText(/no saved presets for this page yet/i),
      ).toBeInTheDocument();
    });
  });

  it("filters out presets that belong to a different library", async () => {
    const otherLibraryPreset = {
      ...samplePreset,
      id: "preset-3",
      libraryId: "lib-other",
      name: "Library X preset",
    };
    listMock.mockResolvedValue([
      samplePreset,
      otherLibraryPreset,
      globalPreset,
    ]);

    renderWithProviders(
      <ListPresetControls
        target="books"
        libraryId="lib-1"
        currentCondition={{ genre: { operator: "is", value: "Action" } }}
        hasActiveFilters
        onApply={vi.fn()}
      />,
    );

    const trigger = await screen.findByRole("button", {
      name: /apply preset/i,
    });
    await userEvent.click(trigger);

    // The menu items contain the preset names as visible text.
    expect(await screen.findByText("Manga unread")).toBeInTheDocument();
    expect(screen.getByText("Reading list")).toBeInTheDocument();
    expect(screen.queryByText("Library X preset")).not.toBeInTheDocument();
  });

  it("calls onApply when a preset is selected", async () => {
    listMock.mockResolvedValue([samplePreset]);
    const onApply = vi.fn();

    renderWithProviders(
      <ListPresetControls
        target="books"
        libraryId="lib-1"
        currentCondition={{ genre: { operator: "is", value: "Action" } }}
        hasActiveFilters
        onApply={onApply}
      />,
    );

    const trigger = await screen.findByRole("button", {
      name: /apply preset/i,
    });
    await userEvent.click(trigger);

    const item = await screen.findByText("Manga unread");
    await userEvent.click(item);

    expect(onApply).toHaveBeenCalledWith(
      expect.objectContaining({ id: "preset-1" }),
    );
  });

  it("posts a save request through filterPresetsApi.create", async () => {
    listMock.mockResolvedValue([]);
    createMock.mockResolvedValue({ ...samplePreset });

    renderWithProviders(
      <ListPresetControls
        target="books"
        libraryId="lib-1"
        currentCondition={{ genre: { operator: "is", value: "Action" } }}
        hasActiveFilters
        onApply={vi.fn()}
      />,
    );

    await userEvent.click(screen.getByRole("button", { name: /save preset/i }));

    const nameInput = await screen.findByLabelText(/Preset name/i);
    await userEvent.type(nameInput, "Test preset");

    await userEvent.click(screen.getByRole("button", { name: "Save" }));

    await waitFor(() => expect(createMock).toHaveBeenCalledTimes(1));
    expect(createMock).toHaveBeenCalledWith(
      expect.objectContaining({
        name: "Test preset",
        scope: "list",
        target: "books",
        libraryId: "lib-1",
        condition: { genre: { operator: "is", value: "Action" } },
      }),
    );
  });

  it("hides the library/global toggle when browsing all libraries", async () => {
    renderWithProviders(
      <ListPresetControls
        target="series"
        libraryId={null}
        currentCondition={{ genre: { operator: "is", value: "Action" } }}
        hasActiveFilters
        onApply={vi.fn()}
      />,
    );

    await userEvent.click(screen.getByRole("button", { name: /save preset/i }));
    expect(await screen.findByLabelText(/Preset name/i)).toBeInTheDocument();
    expect(screen.queryByText(/Availability/)).not.toBeInTheDocument();
  });

  it("allows deleting a preset from the menu", async () => {
    listMock.mockResolvedValue([samplePreset]);
    deleteMock.mockResolvedValue(undefined);
    const confirmSpy = vi
      .spyOn(window, "confirm")
      .mockImplementation(() => true);

    renderWithProviders(
      <ListPresetControls
        target="books"
        libraryId="lib-1"
        currentCondition={{ genre: { operator: "is", value: "Action" } }}
        hasActiveFilters
        onApply={vi.fn()}
      />,
    );

    const trigger = await screen.findByRole("button", {
      name: /apply preset/i,
    });
    await userEvent.click(trigger);

    // ActionIcon is rendered as `component="span"` inside the menu item to
    // avoid nested buttons; look it up by its accessible label instead.
    const delBtn = await screen.findByLabelText(/delete preset Manga unread/i);
    await userEvent.click(delBtn);

    await waitFor(() => expect(deleteMock).toHaveBeenCalledWith("preset-1"));
    confirmSpy.mockRestore();
  });
});
