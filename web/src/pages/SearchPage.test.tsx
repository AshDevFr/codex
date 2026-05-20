import { screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
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

vi.mock("@/components/library/MediaCard", () => ({
  MediaCard: () => <div data-testid="media-card" />,
}));

describe("SearchPage", () => {
  beforeEach(() => {
    vi.clearAllMocks();
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
});
