import { screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { booksApi } from "@/api/books";
import { seriesApi } from "@/api/series";
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
});
