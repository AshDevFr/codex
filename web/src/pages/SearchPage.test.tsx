import { screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { booksApi } from "@/api/books";
import { seriesApi } from "@/api/series";
import { useBulkSelectionStore } from "@/store/bulkSelectionStore";
import { renderWithProviders } from "@/test/utils";
import { SearchPage } from "./SearchPage";

vi.mock("@/api/books", () => ({
  booksApi: {
    search: vi.fn().mockResolvedValue({
      data: [],
      total: 0,
      page: 1,
      pageSize: 50,
      totalPages: 0,
    }),
  },
}));

vi.mock("@/api/series", () => ({
  seriesApi: {
    search: vi.fn().mockResolvedValue({
      data: [],
      total: 0,
      page: 1,
      pageSize: 50,
      totalPages: 0,
    }),
  },
}));

vi.mock("@/api/filterPresets", () => ({
  filterPresetsApi: {
    list: vi.fn().mockResolvedValue([]),
    create: vi.fn(),
    delete: vi.fn(),
  },
}));

vi.mock("@/api/settings", () => ({
  settingsApi: {
    getPublicSettings: vi.fn().mockResolvedValue({}),
  },
}));

vi.mock("@/api/libraries", () => ({
  librariesApi: {
    getAll: vi.fn().mockResolvedValue([]),
  },
}));

vi.mock("@/hooks/useAppName", () => ({
  useAppName: () => "Codex",
  useBranding: () => ({ data: { applicationName: "Codex" } }),
  brandingQueryKey: ["settings", "branding"],
  DEFAULT_APP_NAME: "Codex",
}));

// Stub MediaCard but surface the selection props so wiring can be asserted.
// Clicking the card invokes onSelect, exercising the store wiring.
vi.mock("@/components/library/MediaCard", () => ({
  MediaCard: ({
    type,
    data,
    index,
    onSelect,
    isSelected,
    isSelectionMode,
    canBeSelected,
  }: {
    type: "series" | "book";
    data: { id: string };
    index?: number;
    onSelect?: (id: string, shiftKey: boolean, index?: number) => void;
    isSelected?: boolean;
    isSelectionMode?: boolean;
    canBeSelected?: boolean;
  }) => (
    <button
      type="button"
      data-testid="media-card"
      data-card-type={type}
      data-selected={isSelected ? "true" : "false"}
      data-selection-mode={isSelectionMode ? "true" : "false"}
      data-can-select={canBeSelected ? "true" : "false"}
      onClick={(e) => onSelect?.(data.id, e.shiftKey, index)}
    >
      {data.id}
    </button>
  ),
}));

// The real toolbar pulls in permissions, plugin, and membership hooks; its
// behavior is covered by BulkSelectionToolbar.test.tsx. Here we only need to
// confirm SearchPage mounts a toolbar wired to the same selection store.
vi.mock("@/components/library/BulkSelectionToolbar", async () => {
  const { useBulkSelectionStore: useStore } = await import(
    "@/store/bulkSelectionStore"
  );
  return {
    BulkSelectionToolbar: () => {
      const count = useStore((s) => s.selectedIds.size);
      return count > 0 ? (
        <div data-testid="bulk-toolbar">{count} selected</div>
      ) : null;
    },
  };
});

describe("SearchPage", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    // The selection store is a global singleton; reset it between tests.
    useBulkSelectionStore.getState().clearSelection();
    useBulkSelectionStore.getState().setPageItems(null);
  });

  it("renders the search heading", () => {
    renderWithProviders(<SearchPage />, { initialEntries: ["/search"] });
    expect(screen.getByRole("heading", { name: "Search" })).toBeInTheDocument();
  });

  it("sets the document title to the search query", async () => {
    renderWithProviders(<SearchPage />, {
      initialEntries: ["/search?q=batman"],
    });
    await waitFor(() => {
      expect(document.title).toContain("Search: batman");
    });
  });

  it("shows both Series and Books tabs", () => {
    renderWithProviders(<SearchPage />, { initialEntries: ["/search"] });
    expect(screen.getByRole("tab", { name: /series/i })).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: /books/i })).toBeInTheDocument();
  });

  it("renders the filter builder", () => {
    renderWithProviders(<SearchPage />, { initialEntries: ["/search"] });
    expect(screen.getByText(/no filters yet/i)).toBeInTheDocument();
  });

  it("decodes a condition from the URL and renders the corresponding filter row", () => {
    // base64url-encoded `{"name":{"operator":"contains","value":"punch"}}`
    const c =
      "eyJuYW1lIjp7Im9wZXJhdG9yIjoiY29udGFpbnMiLCJ2YWx1ZSI6InB1bmNoIn19";
    renderWithProviders(<SearchPage />, {
      initialEntries: [`/search?q=punch&c=${c}`],
    });
    // The "no filters yet" message should NOT appear when a condition is present.
    expect(screen.queryByText(/no filters yet/i)).not.toBeInTheDocument();
  });

  it("shows the idle empty state and skips fetching when no query or filter is set", async () => {
    renderWithProviders(<SearchPage />, { initialEntries: ["/search"] });

    expect(
      await screen.findByText(/nothing to search yet/i),
    ).toBeInTheDocument();
    expect(seriesApi.search).not.toHaveBeenCalled();
    expect(booksApi.search).not.toHaveBeenCalled();
  });

  it("fetches results when only a query is provided", async () => {
    renderWithProviders(<SearchPage />, {
      initialEntries: ["/search?q=batman"],
    });

    await waitFor(() => {
      expect(seriesApi.search).toHaveBeenCalled();
    });
    expect(
      screen.queryByText(/nothing to search yet/i),
    ).not.toBeInTheDocument();
  });

  it("only fetches after the user submits the search form", async () => {
    const user = userEvent.setup();
    renderWithProviders(<SearchPage />, { initialEntries: ["/search"] });

    const input = screen.getByRole("textbox", { name: /search query/i });
    await user.type(input, "batman");

    // Typing alone must not trigger a fetch.
    expect(seriesApi.search).not.toHaveBeenCalled();

    await user.click(screen.getByRole("button", { name: /^search$/i }));

    await waitFor(() => {
      expect(seriesApi.search).toHaveBeenCalled();
    });
  });

  it("fetches results when only a configured filter is provided", async () => {
    // base64url-encoded `{"title":{"operator":"contains","value":"punch"}}`
    const c =
      "eyJ0aXRsZSI6eyJvcGVyYXRvciI6ImNvbnRhaW5zIiwidmFsdWUiOiJwdW5jaCJ9fQ";
    renderWithProviders(<SearchPage />, {
      initialEntries: [`/search?c=${c}`],
    });

    await waitFor(() => {
      expect(seriesApi.search).toHaveBeenCalled();
    });
    expect(
      screen.queryByText(/nothing to search yet/i),
    ).not.toBeInTheDocument();
  });

  it("sends the condition through to both tabs' queries", async () => {
    // base64url-encoded `{"title":{"operator":"contains","value":"space"}}`.
    // Both tabs should receive the filter so the inactive-tab badge
    // reflects the same condition the user is searching with.
    const c =
      "eyJ0aXRsZSI6eyJvcGVyYXRvciI6ImNvbnRhaW5zIiwidmFsdWUiOiJzcGFjZSJ9fQ";
    renderWithProviders(<SearchPage />, {
      initialEntries: [`/search?c=${c}`],
    });

    await waitFor(() => {
      expect(seriesApi.search).toHaveBeenCalled();
      expect(booksApi.search).toHaveBeenCalled();
    });

    const expectedLeaf = {
      title: { operator: "contains", value: "space" },
    };
    const seriesCall = vi.mocked(seriesApi.search).mock.calls[0];
    expect(seriesCall[1].condition).toEqual(expectedLeaf);
    const booksCall = vi.mocked(booksApi.search).mock.calls[0];
    expect(booksCall[1].condition).toEqual(expectedLeaf);
  });

  describe("bulk selection", () => {
    const seriesResult = (ids: string[]) => ({
      data: ids.map((id) => ({ id })),
      total: ids.length,
      page: 1,
      pageSize: 50,
      totalPages: 1,
    });

    it("passes selection props to result cards and registers active-tab page items", async () => {
      vi.mocked(seriesApi.search).mockResolvedValue(
        seriesResult(["s1", "s2"]) as never,
      );
      vi.mocked(booksApi.search).mockResolvedValue(
        seriesResult(["b1"]) as never,
      );

      renderWithProviders(<SearchPage />, {
        initialEntries: ["/search?q=batman"],
      });

      const cards = await screen.findAllByTestId("media-card");
      const seriesCards = cards.filter(
        (c) => c.getAttribute("data-card-type") === "series",
      );
      expect(seriesCards).toHaveLength(2);
      // No selection yet: not in selection mode, series are selectable.
      expect(seriesCards[0]).toHaveAttribute("data-selection-mode", "false");
      expect(seriesCards[0]).toHaveAttribute("data-can-select", "true");

      // Only the active (series) tab registers page items.
      const pageItems = useBulkSelectionStore.getState().pageItems;
      expect(pageItems).toEqual({ ids: ["s1", "s2"], type: "series" });
    });

    it("toggles store selection when a result card is clicked", async () => {
      const user = userEvent.setup();
      vi.mocked(seriesApi.search).mockResolvedValue(
        seriesResult(["s1", "s2"]) as never,
      );

      renderWithProviders(<SearchPage />, {
        initialEntries: ["/search?q=batman"],
      });

      const cards = await screen.findAllByTestId("media-card");
      const seriesCards = cards.filter(
        (c) => c.getAttribute("data-card-type") === "series",
      );
      await user.click(seriesCards[0]);

      const state = useBulkSelectionStore.getState();
      expect(state.selectedIds.has("s1")).toBe(true);
      expect(state.selectionType).toBe("series");
      expect(state.isSelectionMode).toBe(true);

      // The mounted toolbar reflects the live selection count.
      expect(await screen.findByTestId("bulk-toolbar")).toHaveTextContent(
        "1 selected",
      );
    });

    it("selects a contiguous range on shift-click", async () => {
      const user = userEvent.setup();
      vi.mocked(seriesApi.search).mockResolvedValue(
        seriesResult(["s1", "s2", "s3"]) as never,
      );

      renderWithProviders(<SearchPage />, {
        initialEntries: ["/search?q=batman"],
      });

      const cards = await screen.findAllByTestId("media-card");
      const seriesCards = cards.filter(
        (c) => c.getAttribute("data-card-type") === "series",
      );

      // Anchor on the first card, then shift-click the third.
      await user.click(seriesCards[0]);
      await user.keyboard("{Shift>}");
      await user.click(seriesCards[2]);
      await user.keyboard("{/Shift}");

      const state = useBulkSelectionStore.getState();
      expect([...state.selectedIds].sort()).toEqual(["s1", "s2", "s3"]);
    });

    it("clears the selection when switching tabs", async () => {
      const user = userEvent.setup();
      vi.mocked(seriesApi.search).mockResolvedValue(
        seriesResult(["s1"]) as never,
      );

      renderWithProviders(<SearchPage />, {
        initialEntries: ["/search?q=batman"],
      });

      const cards = await screen.findAllByTestId("media-card");
      await user.click(
        cards.filter((c) => c.getAttribute("data-card-type") === "series")[0],
      );
      expect(useBulkSelectionStore.getState().selectedIds.size).toBe(1);

      await user.click(screen.getByRole("tab", { name: /books/i }));

      const state = useBulkSelectionStore.getState();
      expect(state.selectedIds.size).toBe(0);
      expect(state.selectionType).toBeNull();
      expect(state.isSelectionMode).toBe(false);
    });
  });
});
