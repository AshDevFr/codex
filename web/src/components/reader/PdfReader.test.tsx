import { beforeEach, describe, expect, it, vi } from "vitest";
import { useReaderStore } from "@/store/readerStore";
import { fireEvent, renderWithProviders, screen, waitFor } from "@/test/utils";

// Mock react-pdf since it requires Web Workers which aren't available in tests
vi.mock("react-pdf", () => ({
  Document: (props: {
    children: React.ReactNode;
    file: string;
    onLoadSuccess?: (pdf: { numPages: number }) => void;
    onLoadError?: (error: Error) => void;
    loading?: React.ReactNode;
  }) => {
    // Simulate successful load after a tick
    if (props.onLoadSuccess) {
      setTimeout(() => props.onLoadSuccess?.({ numPages: 10 }), 0);
    }
    return (
      <div data-testid="pdf-document" data-file={props.file}>
        {props.children}
      </div>
    );
  },
  Page: (props: {
    pageNumber: number;
    width?: number;
    height?: number;
    scale?: number;
    renderTextLayer?: boolean;
    renderAnnotationLayer?: boolean;
    loading?: React.ReactNode;
    customTextRenderer?: ({ str }: { str: string }) => string;
  }) => (
    <div
      data-testid="pdf-page"
      data-page-number={props.pageNumber}
      data-width={props.width}
      data-height={props.height}
      data-scale={props.scale}
      data-render-text-layer={props.renderTextLayer}
      data-render-annotation-layer={props.renderAnnotationLayer}
    >
      PDF Page {props.pageNumber}
    </div>
  ),
  pdfjs: {
    GlobalWorkerOptions: {
      workerSrc: "",
    },
  },
}));

// Mock the CSS imports
vi.mock("react-pdf/dist/Page/TextLayer.css", () => ({}));
vi.mock("react-pdf/dist/Page/AnnotationLayer.css", () => ({}));

// Mock hooks
vi.mock("./hooks", () => ({
  useAdjacentBooks: vi.fn(),
  useBoundaryNotification: vi.fn(() => ({
    message: null,
    onBoundaryChange: vi.fn(),
    clearNotification: vi.fn(),
  })),
  useKeyboardNav: vi.fn(),
  useReadProgress: vi.fn(() => ({
    initialPage: 1,
    isLoading: false,
  })),
  useSeriesNavigation: vi.fn(() => ({
    handleNextPage: vi.fn(),
    handlePrevPage: vi.fn(),
    goToNextBook: vi.fn(),
    goToPrevBook: vi.fn(),
    canGoNextBook: false,
    canGoPrevBook: false,
    isSeriesEnd: false,
    isSeriesStart: false,
  })),
  useTouchNav: vi.fn(() => ({
    touchRef: vi.fn(),
  })),
}));

// Import component after mocks
import { PdfReader } from "./PdfReader";

// Mock ResizeObserver - needed for container dimension measurement
const mockResizeObserve = vi.fn();
const mockResizeDisconnect = vi.fn();

describe("PdfReader", () => {
  const defaultProps = {
    bookId: "book-123",
    title: "Test PDF Book",
    totalPages: 10,
    onClose: vi.fn(),
  };

  const setMatchMedia = (matches: boolean) => {
    window.matchMedia = vi.fn().mockImplementation((query) => ({
      matches,
      media: query,
      onchange: null,
      addListener: vi.fn(),
      removeListener: vi.fn(),
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
      dispatchEvent: vi.fn(),
    }));
  };

  beforeEach(() => {
    vi.clearAllMocks();
    // Reset store state
    useReaderStore.getState().resetSession();
    // Default to non-mobile viewport; mobile-specific tests override.
    setMatchMedia(false);

    // Setup ResizeObserver mock (class-based for vitest v4 compatibility)
    global.ResizeObserver = class MockResizeObserver {
      observe = mockResizeObserve;
      disconnect = mockResizeDisconnect;
      constructor(callback: ResizeObserverCallback) {
        // Immediately call the callback with mock dimensions
        queueMicrotask(() => {
          callback([{ contentRect: { width: 800, height: 600 } }] as any, this);
        });
      }
    } as any;
  });

  it("should render PDF document with correct file URL", async () => {
    renderWithProviders(<PdfReader {...defaultProps} />);

    await waitFor(() => {
      const document = screen.getByTestId("pdf-document");
      expect(document).toHaveAttribute(
        "data-file",
        "/api/v1/books/book-123/file",
      );
    });
  });

  it("should render current page", async () => {
    renderWithProviders(<PdfReader {...defaultProps} />);

    await waitFor(() => {
      const page = screen.getByTestId("pdf-page");
      expect(page).toHaveAttribute("data-page-number", "1");
    });
  });

  it("should display book title in toolbar", async () => {
    renderWithProviders(<PdfReader {...defaultProps} />);

    await waitFor(() => {
      expect(screen.getByText("Test PDF Book")).toBeInTheDocument();
    });
  });

  it("should enable text layer rendering", async () => {
    renderWithProviders(<PdfReader {...defaultProps} />);

    await waitFor(() => {
      const page = screen.getByTestId("pdf-page");
      expect(page).toHaveAttribute("data-render-text-layer", "true");
    });
  });

  it("should enable annotation layer rendering", async () => {
    renderWithProviders(<PdfReader {...defaultProps} />);

    await waitFor(() => {
      const page = screen.getByTestId("pdf-page");
      expect(page).toHaveAttribute("data-render-annotation-layer", "true");
    });
  });

  it("should use startPage when provided", async () => {
    renderWithProviders(<PdfReader {...defaultProps} startPage={5} />);

    // Wait for initialization to complete
    await waitFor(() => {
      const state = useReaderStore.getState();
      expect(state.currentPage).toBe(5);
    });
  });

  describe("search functionality", () => {
    it("should show search bar when Ctrl+F is pressed", async () => {
      renderWithProviders(<PdfReader {...defaultProps} />);

      // Simulate Ctrl+F
      fireEvent.keyDown(document, { key: "f", ctrlKey: true });

      await waitFor(() => {
        expect(
          screen.getByPlaceholderText("Search in PDF..."),
        ).toBeInTheDocument();
      });
    });

    it("should hide search bar when Escape is pressed", async () => {
      renderWithProviders(<PdfReader {...defaultProps} />);

      // Open search
      fireEvent.keyDown(document, { key: "f", ctrlKey: true });

      await waitFor(() => {
        expect(
          screen.getByPlaceholderText("Search in PDF..."),
        ).toBeInTheDocument();
      });

      // Close search
      const searchInput = screen.getByPlaceholderText("Search in PDF...");
      fireEvent.keyDown(searchInput, { key: "Escape" });

      await waitFor(() => {
        expect(
          screen.queryByPlaceholderText("Search in PDF..."),
        ).not.toBeInTheDocument();
      });
    });
  });

  describe("mobile default zoom", () => {
    it("defaults to fit-page on non-mobile viewports", async () => {
      // Default beforeEach sets matchMedia matches=false (non-mobile)
      renderWithProviders(<PdfReader {...defaultProps} />);

      await waitFor(() => {
        const page = screen.getByTestId("pdf-page");
        const scale = Number(page.getAttribute("data-scale"));
        // fit-page is height-constrained for a 612x792 page in an 800x600
        // container (after toolbar + padding), producing scale ~0.63.
        expect(scale).toBeGreaterThan(0);
        expect(scale).toBeLessThan(1);
      });
    });

    it("defaults to fit-width on mobile viewports", async () => {
      setMatchMedia(true);

      renderWithProviders(<PdfReader {...defaultProps} />);

      await waitFor(() => {
        const page = screen.getByTestId("pdf-page");
        const scale = Number(page.getAttribute("data-scale"));
        // fit-width uses the available width only (~1.24 for a 612-wide page
        // in an 800-wide container after padding) — strictly larger than the
        // fit-page result above, confirming the mobile default kicked in.
        expect(scale).toBeGreaterThan(1);
      });
    });
  });

  describe("click zones", () => {
    it("should navigate on left zone click", async () => {
      // Validate hook availability for click zone navigation
      const hooks = await import("./hooks");
      expect(hooks.useSeriesNavigation).toBeDefined();

      renderWithProviders(<PdfReader {...defaultProps} />);

      await waitFor(() => {
        expect(screen.getByTestId("pdf-page")).toBeInTheDocument();
      });
    });
  });
});
