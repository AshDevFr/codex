import { act, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import * as pdfCacheApi from "@/api/pdfCache";
import { renderWithProviders } from "@/test/utils";
import { PdfCacheSettings } from "./PdfCacheSettings";

// Mock the PDF cache API
vi.mock("@/api/pdfCache", () => ({
  pdfCacheApi: {
    getStats: vi.fn(),
    getHandleStats: vi.fn(),
    triggerCleanup: vi.fn(),
    clearPageCache: vi.fn(),
    clearHandleCache: vi.fn(),
    evictBookHandle: vi.fn(),
  },
}));

// Default mock stats - combined page + handle cache.
const defaultStats: pdfCacheApi.PdfCacheStatsDto = {
  pages: {
    totalFiles: 1500,
    totalSizeBytes: 157_286_400,
    totalSizeHuman: "150.0 MB",
    bookCount: 45,
    oldestFileAgeDays: 15,
    cacheDir: "/data/cache",
    cacheEnabled: true,
  },
  handles: {
    enabled: true,
    capacity: 256,
    idleTtlSeconds: 900,
    currentSize: 3,
    hits: 120,
    misses: 18,
    opens: 18,
    evictions: 1,
    idleEvictions: 0,
    entries: [
      {
        bookId: "11111111-1111-1111-1111-111111111111",
        filePath: "/library/book-a.pdf",
        ageSeconds: 600,
        idleSeconds: 5,
        renderCount: 30,
      },
      {
        bookId: "22222222-2222-2222-2222-222222222222",
        filePath: "/library/book-b.pdf",
        ageSeconds: 120,
        idleSeconds: 60,
        renderCount: 12,
      },
      {
        bookId: "33333333-3333-3333-3333-333333333333",
        filePath: "/library/book-c.pdf",
        ageSeconds: 30,
        idleSeconds: 30,
        renderCount: 4,
      },
    ],
  },
};

describe("PdfCacheSettings", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    // Default mock implementation
    vi.mocked(pdfCacheApi.pdfCacheApi.getStats).mockResolvedValue(defaultStats);
  });

  it("renders the renamed top-level title", async () => {
    renderWithProviders(<PdfCacheSettings />);

    await waitFor(() => {
      expect(
        screen.getByRole("heading", { level: 1, name: "PDF Cache" }),
      ).toBeInTheDocument();
    });
  });

  it("renders the shape-matched skeleton after the 150ms gate", () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    try {
      vi.mocked(pdfCacheApi.pdfCacheApi.getStats).mockReturnValueOnce(
        new Promise(() => {}),
      );
      const { container } = renderWithProviders(<PdfCacheSettings />);

      // Pre-gate: skeleton tiles suppressed.
      expect(container.querySelector(".mantine-Skeleton-root")).toBeNull();

      act(() => {
        vi.advanceTimersByTime(200);
      });

      expect(container.querySelector(".mantine-Skeleton-root")).not.toBeNull();
    } finally {
      vi.useRealTimers();
    }
  });

  it("renders both section headings", async () => {
    renderWithProviders(<PdfCacheSettings />);

    await waitFor(() => {
      expect(
        screen.getByRole("heading", { name: /rendered pages \(on disk\)/i }),
      ).toBeInTheDocument();
      expect(
        screen.getByRole("heading", { name: /open documents \(in memory\)/i }),
      ).toBeInTheDocument();
    });
  });

  it("displays page cache statistics", async () => {
    renderWithProviders(<PdfCacheSettings />);

    await waitFor(() => {
      expect(screen.getByText("Cached Pages")).toBeInTheDocument();
      expect(screen.getByText("Cache Size")).toBeInTheDocument();
      expect(screen.getByText("Books Cached")).toBeInTheDocument();
      expect(screen.getByText("Oldest Page")).toBeInTheDocument();
      expect(screen.getByText("150.0 MB")).toBeInTheDocument();
      expect(screen.getByText("/data/cache")).toBeInTheDocument();
    });
  });

  it("displays handle cache statistics", async () => {
    renderWithProviders(<PdfCacheSettings />);

    await waitFor(() => {
      expect(screen.getByText("Open Handles")).toBeInTheDocument();
      expect(screen.getByText("Cache Hits")).toBeInTheDocument();
      expect(screen.getByText("Opens")).toBeInTheDocument();
      expect(screen.getByText("Evictions")).toBeInTheDocument();
    });

    // Should list each resident document.
    expect(screen.getByText(/book-a\.pdf/)).toBeInTheDocument();
    expect(screen.getByText(/book-b\.pdf/)).toBeInTheDocument();
    expect(screen.getByText(/book-c\.pdf/)).toBeInTheDocument();
  });

  it("shows page cleanup buttons when cache has files", async () => {
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

  it("shows the close-all-handles button when handles are resident", async () => {
    renderWithProviders(<PdfCacheSettings />);

    await waitFor(() => {
      expect(
        screen.getByRole("button", { name: /close all/i }),
      ).toBeInTheDocument();
    });
  });

  it("shows refresh button", async () => {
    renderWithProviders(<PdfCacheSettings />);

    await waitFor(() => {
      expect(
        screen.getByRole("button", { name: /refresh/i }),
      ).toBeInTheDocument();
    });
  });

  it("does not show cleanup buttons when both caches are empty", async () => {
    vi.mocked(pdfCacheApi.pdfCacheApi.getStats).mockResolvedValue({
      pages: {
        ...defaultStats.pages,
        totalFiles: 0,
        totalSizeBytes: 0,
        totalSizeHuman: "0 B",
        bookCount: 0,
        oldestFileAgeDays: null as unknown as undefined,
      },
      handles: {
        ...defaultStats.handles,
        currentSize: 0,
        entries: [],
      },
    });

    renderWithProviders(<PdfCacheSettings />);

    await waitFor(() => {
      expect(screen.getByText("Cache empty")).toBeInTheDocument();
      expect(screen.getByText("No open handles")).toBeInTheDocument();
    });

    expect(
      screen.queryByRole("button", { name: /cleanup old/i }),
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: /clear all/i }),
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: /close all/i }),
    ).not.toBeInTheDocument();
  });

  it("shows warning when page cache is disabled", async () => {
    vi.mocked(pdfCacheApi.pdfCacheApi.getStats).mockResolvedValue({
      ...defaultStats,
      pages: { ...defaultStats.pages, cacheEnabled: false },
    });

    renderWithProviders(<PdfCacheSettings />);

    await waitFor(() => {
      expect(
        screen.getByText(/PDF page caching is currently disabled/i),
      ).toBeInTheDocument();
    });
  });

  it("shows warning when handle cache is disabled", async () => {
    vi.mocked(pdfCacheApi.pdfCacheApi.getStats).mockResolvedValue({
      ...defaultStats,
      handles: { ...defaultStats.handles, enabled: false },
    });

    renderWithProviders(<PdfCacheSettings />);

    await waitFor(() => {
      expect(
        screen.getByText(/Handle caching is currently disabled/i),
      ).toBeInTheDocument();
    });
  });

  it("clears the handle cache when confirmed", async () => {
    const user = userEvent.setup();
    vi.mocked(pdfCacheApi.pdfCacheApi.clearHandleCache).mockResolvedValue({
      handlesClosed: 3,
    });

    renderWithProviders(<PdfCacheSettings />);

    await waitFor(() => {
      expect(
        screen.getByRole("button", { name: /close all/i }),
      ).toBeInTheDocument();
    });

    await user.click(screen.getByRole("button", { name: /close all/i }));

    await waitFor(() => {
      expect(
        screen.getByRole("button", { name: /close all handles/i }),
      ).toBeInTheDocument();
    });

    await user.click(
      screen.getByRole("button", { name: /close all handles/i }),
    );

    await waitFor(() => {
      expect(pdfCacheApi.pdfCacheApi.clearHandleCache).toHaveBeenCalledTimes(1);
    });
  });

  it("evicts a single book handle from the table", async () => {
    const user = userEvent.setup();
    vi.mocked(pdfCacheApi.pdfCacheApi.evictBookHandle).mockResolvedValue({
      handlesClosed: 1,
    });

    renderWithProviders(<PdfCacheSettings />);

    await waitFor(() => {
      expect(screen.getAllByRole("button", { name: /close$/i })).toHaveLength(
        3,
      );
    });

    await user.click(screen.getAllByRole("button", { name: /close$/i })[0]);

    await waitFor(() => {
      expect(pdfCacheApi.pdfCacheApi.evictBookHandle).toHaveBeenCalledWith(
        "11111111-1111-1111-1111-111111111111",
      );
    });
  });
});
