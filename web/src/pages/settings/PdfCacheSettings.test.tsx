import { screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import * as pdfCacheApi from "@/api/pdfCache";
import { renderWithProviders } from "@/test/utils";
import { PdfCacheSettings } from "./PdfCacheSettings";

// Mock the PDF cache API
vi.mock("@/api/pdfCache", () => ({
  pdfCacheApi: {
    getStats: vi.fn(),
    triggerCleanup: vi.fn(),
    clearCache: vi.fn(),
  },
}));

// Default mock stats
const defaultStats: pdfCacheApi.PdfCacheStatsDto = {
  totalFiles: 1500,
  totalSizeBytes: 157_286_400,
  totalSizeHuman: "150.0 MB",
  bookCount: 45,
  oldestFileAgeDays: 15,
  cacheDir: "/data/cache",
  cacheEnabled: true,
};

describe("PdfCacheSettings", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    // Default mock implementation
    vi.mocked(pdfCacheApi.pdfCacheApi.getStats).mockResolvedValue(defaultStats);
  });

  it("should render page title", async () => {
    renderWithProviders(<PdfCacheSettings />);

    await waitFor(() => {
      expect(screen.getByText("PDF Page Cache")).toBeInTheDocument();
    });
  });

  it("should show loading state initially", () => {
    renderWithProviders(<PdfCacheSettings />);

    // Loading state shows a loader
    expect(screen.getByText("Loading cache statistics...")).toBeInTheDocument();
  });

  it("should display cache statistics labels after loading", async () => {
    renderWithProviders(<PdfCacheSettings />);

    await waitFor(() => {
      // Stats card labels
      expect(screen.getByText("Cached Pages")).toBeInTheDocument();
      expect(screen.getByText("Cache Size")).toBeInTheDocument();
      expect(screen.getByText("Books Cached")).toBeInTheDocument();
      expect(screen.getByText("Oldest Page")).toBeInTheDocument();
    });
  });

  it("should display cache size value", async () => {
    renderWithProviders(<PdfCacheSettings />);

    await waitFor(() => {
      expect(screen.getByText("150.0 MB")).toBeInTheDocument();
    });
  });

  it("should display cache directory", async () => {
    renderWithProviders(<PdfCacheSettings />);

    await waitFor(() => {
      expect(screen.getByText("/data/cache")).toBeInTheDocument();
    });
  });

  it("should show info alert about PDF cache", async () => {
    renderWithProviders(<PdfCacheSettings />);

    await waitFor(() => {
      expect(screen.getByText("About PDF Page Cache")).toBeInTheDocument();
    });
  });

  it("should show cleanup buttons when cache has files", async () => {
    renderWithProviders(<PdfCacheSettings />);

    await waitFor(() => {
      expect(
        screen.getByRole("button", { name: /cleanup old/i }),
      ).toBeInTheDocument();
      expect(
        screen.getByRole("button", { name: /clear all/i }),
      ).toBeInTheDocument();
    });
  });

  it("should show refresh button", async () => {
    renderWithProviders(<PdfCacheSettings />);

    await waitFor(() => {
      expect(
        screen.getByRole("button", { name: /refresh/i }),
      ).toBeInTheDocument();
    });
  });

  it("should not show cleanup buttons when cache is empty", async () => {
    vi.mocked(pdfCacheApi.pdfCacheApi.getStats).mockResolvedValue({
      ...defaultStats,
      totalFiles: 0,
      totalSizeBytes: 0,
      totalSizeHuman: "0 B",
      bookCount: 0,
    });

    renderWithProviders(<PdfCacheSettings />);

    await waitFor(() => {
      expect(screen.getByText("Cache empty")).toBeInTheDocument();
    });

    expect(
      screen.queryByRole("button", { name: /cleanup old/i }),
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: /clear all/i }),
    ).not.toBeInTheDocument();
  });

  it("should show warning when cache is disabled", async () => {
    vi.mocked(pdfCacheApi.pdfCacheApi.getStats).mockResolvedValue({
      ...defaultStats,
      cacheEnabled: false,
    });

    renderWithProviders(<PdfCacheSettings />);

    await waitFor(() => {
      expect(
        screen.getByText(/PDF page caching is currently disabled/i),
      ).toBeInTheDocument();
    });
  });

  it("should display status badge with pages cached text", async () => {
    renderWithProviders(<PdfCacheSettings />);

    await waitFor(() => {
      // Use regex to match the badge text which includes locale-formatted number
      expect(screen.getByText(/pages cached/i)).toBeInTheDocument();
    });
  });
});
