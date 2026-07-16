import { beforeEach, describe, expect, it, vi } from "vitest";
import type { ExportFieldCatalogResponse } from "@/api/seriesExports";
import { renderWithProviders, screen, userEvent, waitFor } from "@/test/utils";
import { SeriesExportsSettings } from "./SeriesExportsSettings";

vi.mock("@/hooks/useTaskProgress", () => ({
  useTaskProgress: () => ({ activeTasks: [] }),
}));

vi.mock("@/api/libraries", () => ({
  librariesApi: {
    getAll: vi.fn(async () => [
      { id: "lib-1", name: "Comics", path: "/comics" },
    ]),
  },
}));

vi.mock("@/api/seriesExports", () => ({
  seriesExportsApi: {
    list: vi.fn(async () => []),
    getFieldCatalog: vi.fn(async () => mockCatalog),
    create: vi.fn(),
    delete: vi.fn(),
    download: vi.fn(),
  },
}));

function field(key: string, label: string, multiValue = false) {
  return {
    key,
    label,
    multiValue,
    userSpecific: false,
    isAnchor: false,
  };
}

const mockCatalog: ExportFieldCatalogResponse = {
  fields: [
    { ...field("series_name", "Series Name"), isAnchor: true },
    field("title", "Title"),
    field("genres", "Genres", true),
    field("collections", "Collections", true),
  ],
  bookFields: [
    { ...field("book_name", "Book Name"), isAnchor: true },
    field("title", "Title"),
    field("read_lists", "Read Lists", true),
  ],
  presets: {
    llmSelect: ["title", "genres", "collections"],
    llmSelectBooks: ["title", "read_lists"],
  },
};

async function openCreateModal(user: ReturnType<typeof userEvent.setup>) {
  renderWithProviders(<SeriesExportsSettings />);
  const createButton = await screen.findByRole("button", {
    name: /new export/i,
  });
  await user.click(createButton);
  await waitFor(() => {
    expect(screen.getByText("Series Fields")).toBeInTheDocument();
  });
}

describe("SeriesExportsSettings create modal", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("shows the collections field checkbox for series exports", async () => {
    const user = userEvent.setup();
    await openCreateModal(user);

    expect(
      screen.getByRole("checkbox", { name: "Collections" }),
    ).toBeInTheDocument();
  });

  it("shows the read lists field checkbox for book exports", async () => {
    const user = userEvent.setup();
    await openCreateModal(user);

    await user.click(screen.getByRole("radio", { name: "Books" }));

    expect(
      await screen.findByRole("checkbox", { name: "Read Lists" }),
    ).toBeInTheDocument();
  });

  it("renders catalog fields missing from every hardcoded group", async () => {
    // If the server catalog grows a field the UI groups don't know about,
    // it must still be selectable rather than silently dropped ("Select all"
    // and "LLM Select" would otherwise select it invisibly).
    mockCatalog.fields.push(field("brand_new_field", "Brand New Field"));
    const user = userEvent.setup();
    await openCreateModal(user);

    expect(
      screen.getByRole("checkbox", { name: "Brand New Field" }),
    ).toBeInTheDocument();
    mockCatalog.fields.pop();
  });
});
