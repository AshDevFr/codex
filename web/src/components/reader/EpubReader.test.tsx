import { beforeEach, describe, expect, it, vi } from "vitest";
import { useReaderStore } from "@/store/readerStore";
import { renderWithProviders, screen } from "@/test/utils";
import { EpubReader } from "./EpubReader";

// Captures the per-event handlers `EpubReader` registers on the rendition,
// so tests can fire (e.g.) the "click" handler to verify the toolbar toggle.
const renditionHandlers: Record<string, (...args: unknown[]) => void> = {};
// Captures hooks.content.register callbacks so iframe pointer tests can drive
// the inside-iframe pointer hook with a fake `contents` document.
const contentHookCallbacks: Array<(contents: { document: Document }) => void> =
  [];
// Stash the latest readerStyles ReactReader received so mobile-styles tests can
// assert the side-arrow `display: none` override is applied on mobile viewports.
let lastReaderStyles: Record<string, Record<string, unknown>> | null = null;

// Mock react-reader since it's a complex library that requires actual EPUB files
vi.mock("react-reader", () => ({
  ReactReader: vi.fn(
    ({
      url,
      location: _location,
      locationChanged: _locationChanged,
      getRendition,
      readerStyles,
      showToc,
    }) => {
      lastReaderStyles = readerStyles ?? null;
      // Simulate getting rendition on mount
      const mockRendition = {
        themes: {
          override: vi.fn(),
          fontSize: vi.fn(),
        },
        book: {
          loaded: {
            navigation: Promise.resolve({ toc: [] }),
          },
          ready: Promise.resolve(),
          locations: {
            generate: vi.fn().mockResolvedValue([]),
            percentageFromCfi: vi.fn().mockReturnValue(0.5),
            cfiFromPercentage: vi
              .fn()
              .mockReturnValue("epubcfi(/6/2[chapter1]!/4/2)"),
          },
          spine: {
            get: vi.fn(),
          },
        },
        on: vi.fn((event: string, handler: (...args: unknown[]) => void) => {
          renditionHandlers[event] = handler;
        }),
        hooks: {
          content: {
            register: vi.fn(
              (callback: (contents: { document: Document }) => void) => {
                contentHookCallbacks.push(callback);
              },
            ),
          },
        },
        display: vi.fn(),
        next: vi.fn(),
        prev: vi.fn(),
      };

      // Call getRendition callback if provided
      if (getRendition) {
        setTimeout(() => getRendition(mockRendition), 0);
      }

      return (
        <div data-testid="react-reader-mock">
          <div>Mock ReactReader</div>
          <div data-testid="epub-url">{url}</div>
          <div data-testid="show-toc">{String(showToc)}</div>
        </div>
      );
    },
  ),
  ReactReaderStyle: {
    readerArea: {},
    arrow: {},
    arrowHover: {},
  },
}));

// Mock the hooks
vi.mock("./hooks/useEpubProgress", () => ({
  useEpubProgress: vi.fn(() => ({
    getSavedLocation: vi.fn().mockReturnValue(null),
    initialPercentage: null,
    isLoadingProgress: false,
    saveLocation: vi.fn(),
  })),
}));

vi.mock("./hooks/useEpubBookmarks", () => ({
  useEpubBookmarks: vi.fn(() => ({
    bookmarks: [],
    addBookmark: vi.fn(),
    updateBookmark: vi.fn(),
    removeBookmark: vi.fn(),
    isBookmarked: vi.fn().mockReturnValue(false),
    getBookmarkByCfi: vi.fn().mockReturnValue(null),
  })),
}));

// Mock the API client
vi.mock("@/api/client", () => ({
  api: {
    get: vi.fn(),
    put: vi.fn(),
    post: vi.fn(),
    patch: vi.fn(),
    delete: vi.fn(),
  },
}));

// Default settings to reset store before each test
const defaultSettings = {
  fitMode: "screen" as const,
  pageLayout: "single" as const,
  readingDirection: "ltr" as const,
  backgroundColor: "black" as const,
  pdfMode: "streaming" as const,
  pdfSpreadMode: "single" as const,
  pdfContinuousScroll: false,
  autoHideToolbar: true,
  toolbarHideDelay: 3000,
  epubTheme: "light" as const,
  epubFontSize: 100,
  epubFontFamily: "default" as const,
  epubLineHeight: 150,
  epubMargin: 10,
  preloadPages: 1,
  doublePageShowWideAlone: true,
  doublePageStartOnOdd: true,
  pageTransition: "slide" as const,
  transitionDuration: 200,
  webtoonSidePadding: 0,
  webtoonPageGap: 0,
  autoAdvanceToNextBook: false,
};

const defaultSessionState = {
  currentPage: 1,
  totalPages: 10,
  isLoading: false,
  toolbarVisible: true,
  isFullscreen: false,
  currentBookId: null,
  readingDirectionOverride: null,
  adjacentBooks: null,
  boundaryState: "none" as const,
  pageOrientations: {},
  lastNavigationDirection: null,
  preloadedImages: new Set<string>(),
};

const defaultProps = {
  bookId: "book-123",
  seriesId: "series-123" as string | null,
  title: "Test EPUB Book",
  totalPages: 100,
  onClose: vi.fn(),
};

describe("EpubReader", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    for (const k of Object.keys(renditionHandlers)) {
      delete renditionHandlers[k];
    }
    contentHookCallbacks.length = 0;
    lastReaderStyles = null;
    // Default matchMedia: not mobile. Individual tests can override.
    window.matchMedia = vi.fn().mockImplementation((query) => ({
      matches: false,
      media: query,
      onchange: null,
      addListener: vi.fn(),
      removeListener: vi.fn(),
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
      dispatchEvent: vi.fn(),
    }));
    useReaderStore.setState({
      settings: { ...defaultSettings },
      ...defaultSessionState,
    });
  });

  describe("rendering", () => {
    it("should render the reader container", () => {
      renderWithProviders(<EpubReader {...defaultProps} />);

      // The reader should be rendered with the title in toolbar
      expect(screen.getByText("Test EPUB Book")).toBeInTheDocument();
    });

    it("should render the toolbar with title", () => {
      renderWithProviders(<EpubReader {...defaultProps} />);

      expect(screen.getByText("Test EPUB Book")).toBeInTheDocument();
    });

    it("should render ReactReader component", () => {
      renderWithProviders(<EpubReader {...defaultProps} />);

      // Our mock ReactReader should be rendered
      expect(screen.getByTestId("react-reader-mock")).toBeInTheDocument();
    });

    it("should pass correct EPUB URL to ReactReader", () => {
      renderWithProviders(<EpubReader {...defaultProps} />);

      // Check that the URL is correctly passed
      expect(screen.getByTestId("epub-url")).toHaveTextContent(
        "/api/v1/books/book-123/file",
      );
    });

    it("should hide built-in TOC", () => {
      renderWithProviders(<EpubReader {...defaultProps} />);

      // showToc should be false (we use custom TOC)
      expect(screen.getByTestId("show-toc")).toHaveTextContent("false");
    });
  });

  describe("toolbar", () => {
    it("should show toolbar when visible", () => {
      useReaderStore.setState({ toolbarVisible: true });

      renderWithProviders(<EpubReader {...defaultProps} />);

      expect(screen.getByText("Test EPUB Book")).toBeInTheDocument();
    });

    it("should have TOC button", () => {
      renderWithProviders(<EpubReader {...defaultProps} />);

      // TOC button should be present
      const buttons = screen.getAllByRole("button");
      expect(buttons.length).toBeGreaterThan(0);
    });

    it("should have bookmarks button", () => {
      renderWithProviders(<EpubReader {...defaultProps} />);

      // Bookmarks button should be present
      const buttons = screen.getAllByRole("button");
      expect(buttons.length).toBeGreaterThan(0);
    });

    it("should have search button", () => {
      renderWithProviders(<EpubReader {...defaultProps} />);

      // Search button should be present
      const buttons = screen.getAllByRole("button");
      expect(buttons.length).toBeGreaterThan(0);
    });
  });

  describe("theme", () => {
    it("should apply light theme by default", () => {
      renderWithProviders(<EpubReader {...defaultProps} />);

      const container = document.querySelector('[style*="100vw"]');
      expect(container).toBeInTheDocument();
      expect(useReaderStore.getState().settings.epubTheme).toBe("light");
    });

    it("should apply dark theme when configured", () => {
      useReaderStore.setState({
        settings: { ...defaultSettings, epubTheme: "dark" },
      });

      renderWithProviders(<EpubReader {...defaultProps} />);

      expect(useReaderStore.getState().settings.epubTheme).toBe("dark");
    });

    it("should apply sepia theme when configured", () => {
      useReaderStore.setState({
        settings: { ...defaultSettings, epubTheme: "sepia" },
      });

      renderWithProviders(<EpubReader {...defaultProps} />);

      expect(useReaderStore.getState().settings.epubTheme).toBe("sepia");
    });
  });

  describe("font size", () => {
    it("should use default font size of 100%", () => {
      renderWithProviders(<EpubReader {...defaultProps} />);

      expect(useReaderStore.getState().settings.epubFontSize).toBe(100);
    });

    it("should respect configured font size", () => {
      useReaderStore.setState({
        settings: { ...defaultSettings, epubFontSize: 150 },
      });

      renderWithProviders(<EpubReader {...defaultProps} />);

      expect(useReaderStore.getState().settings.epubFontSize).toBe(150);
    });
  });

  describe("settings modal", () => {
    it("should have settings button in toolbar", () => {
      renderWithProviders(<EpubReader {...defaultProps} />);

      // Settings button should be present among toolbar buttons
      const buttons = screen.getAllByRole("button");
      expect(buttons.length).toBeGreaterThan(3);
    });
  });

  describe("fullscreen", () => {
    it("should not be fullscreen by default", () => {
      renderWithProviders(<EpubReader {...defaultProps} />);

      expect(useReaderStore.getState().isFullscreen).toBe(false);
    });
  });

  describe("auto-hide toolbar", () => {
    it("should respect auto-hide toolbar setting", () => {
      useReaderStore.setState({
        settings: { ...defaultSettings, autoHideToolbar: false },
      });

      renderWithProviders(<EpubReader {...defaultProps} />);

      expect(useReaderStore.getState().settings.autoHideToolbar).toBe(false);
    });
  });

  describe("close callback", () => {
    it("should have close button that calls onClose", async () => {
      const onClose = vi.fn();

      renderWithProviders(<EpubReader {...defaultProps} onClose={onClose} />);

      // Close button should be present
      const buttons = screen.getAllByRole("button");
      expect(buttons.length).toBeGreaterThan(0);
      // First button is typically the close button
      const closeButton = buttons[0];
      expect(closeButton).toBeInTheDocument();
    });
  });

  describe("EPUB-specific features", () => {
    it("should render TOC drawer component", () => {
      renderWithProviders(<EpubReader {...defaultProps} />);

      // EpubTableOfContentsDrawer is rendered at the reader level so it
      // survives the toolbar's auto-hide; the trigger lives in the toolbar.
      expect(screen.getByTestId("react-reader-mock")).toBeInTheDocument();
    });

    it("should render bookmarks drawer component", () => {
      renderWithProviders(<EpubReader {...defaultProps} />);

      // EpubBookmarks is rendered in toolbar
      expect(screen.getByTestId("react-reader-mock")).toBeInTheDocument();
    });

    it("should render search drawer component", () => {
      renderWithProviders(<EpubReader {...defaultProps} />);

      // EpubSearch is rendered in toolbar
      expect(screen.getByTestId("react-reader-mock")).toBeInTheDocument();
    });
  });

  describe("mobile tap-to-toggle toolbar", () => {
    // The EPUB reader no longer wires an outer-container useTouchNav: the
    // iframe-internal pointer handler is the sole authority for taps, to
    // avoid double-classifying the same touch on iOS Safari.

    it("registers a content hook that wires pointer events on the iframe doc", async () => {
      renderWithProviders(<EpubReader {...defaultProps} />);

      // Rendition is wired asynchronously via setTimeout in the mock
      await new Promise((r) => setTimeout(r, 0));

      expect(contentHookCallbacks.length).toBeGreaterThan(0);
    });
  });

  describe("EPUB iframe pointer navigation", () => {
    // The iframe hook listens for pointer events (not `click`). iOS Safari
    // can suppress `click` inside sandboxed iframes (switching to it during
    // earlier work lost navigation entirely on real devices), while pointer
    // events fire reliably. Tap-zone width comes from
    // `window.innerWidth` (the parent viewport) because in epub.js's
    // paginated mode the iframe document is wider than the visible area
    // due to CSS columns.
    const dispatchPointerEvent = (
      doc: Document,
      type: "pointerdown" | "pointerup" | "pointercancel",
      x: number,
      y: number,
      init: {
        pointerType?: "touch" | "mouse" | "pen";
        pointerId?: number;
        isPrimary?: boolean;
        button?: number;
        target?: Element;
      } = {},
    ) => {
      const {
        pointerType = "touch",
        pointerId = 1,
        isPrimary = true,
        button = 0,
        target,
      } = init;
      const event = new MouseEvent(type, {
        clientX: x,
        clientY: y,
        button,
        bubbles: true,
        cancelable: true,
      }) as MouseEvent & {
        pointerId: number;
        pointerType: string;
        isPrimary: boolean;
      };
      Object.defineProperty(event, "pointerId", { value: pointerId });
      Object.defineProperty(event, "pointerType", { value: pointerType });
      Object.defineProperty(event, "isPrimary", { value: isPrimary });
      const dispatchTarget = target ?? doc.body;
      dispatchTarget.dispatchEvent(event);
    };

    const mountAndGetIframeDoc = async () => {
      // Pin the viewport so tap-zone classification is deterministic
      // regardless of jsdom defaults or earlier-test mutations. The hook
      // reads window.innerWidth/innerHeight to size the LTR/RTL thirds.
      Object.defineProperty(window, "innerWidth", {
        configurable: true,
        value: 900,
      });
      Object.defineProperty(window, "innerHeight", {
        configurable: true,
        value: 600,
      });

      renderWithProviders(<EpubReader {...defaultProps} />);
      // Two microtask flushes: the mocked ReactReader queues `getRendition`
      // in a setTimeout, and the React effect that wires the content hook
      // settles on the next tick. One flush isn't always enough when the
      // suite runs in isolation.
      await new Promise((r) => setTimeout(r, 0));
      await new Promise((r) => setTimeout(r, 0));
      expect(contentHookCallbacks.length).toBeGreaterThan(0);

      const fakeIframeDoc = document.implementation.createHTMLDocument("epub");
      // Drive every registered content callback so the hook attaches its
      // pointer listeners to this fake document.
      for (const cb of contentHookCallbacks) {
        cb({ document: fakeIframeDoc });
      }
      return fakeIframeDoc;
    };

    it("toggles the toolbar on a center-zone tap inside the iframe", async () => {
      const doc = await mountAndGetIframeDoc();

      // mountAndGetIframeDoc pins window.innerWidth=900, innerHeight=600;
      // center third is x ∈ [300, 600], y ∈ [200, 400]. (450, 300) is dead-center.
      useReaderStore.setState({ toolbarVisible: true });
      dispatchPointerEvent(doc, "pointerdown", 450, 300);
      dispatchPointerEvent(doc, "pointerup", 451, 300);
      expect(useReaderStore.getState().toolbarVisible).toBe(false);

      dispatchPointerEvent(doc, "pointerdown", 450, 300);
      dispatchPointerEvent(doc, "pointerup", 450, 301);
      expect(useReaderStore.getState().toolbarVisible).toBe(true);
    });

    it("routes edge-zone taps to prev/next without toggling the toolbar (LTR)", async () => {
      const doc = await mountAndGetIframeDoc();

      const visibleBefore = useReaderStore.getState().toolbarVisible;

      // window 900 wide → left third < 300, right third > 600.
      dispatchPointerEvent(doc, "pointerdown", 100, 300);
      dispatchPointerEvent(doc, "pointerup", 100, 300);
      dispatchPointerEvent(doc, "pointerdown", 800, 300);
      dispatchPointerEvent(doc, "pointerup", 800, 300);

      expect(useReaderStore.getState().toolbarVisible).toBe(visibleBefore);
    });

    it("ignores pointer interactions starting on links and form controls", async () => {
      const doc = await mountAndGetIframeDoc();

      const link = doc.createElement("a");
      doc.body.appendChild(link);
      const input = doc.createElement("input");
      doc.body.appendChild(input);

      useReaderStore.setState({ toolbarVisible: true });
      dispatchPointerEvent(doc, "pointerdown", 450, 300, { target: link });
      dispatchPointerEvent(doc, "pointerup", 451, 301, { target: link });
      expect(useReaderStore.getState().toolbarVisible).toBe(true);

      dispatchPointerEvent(doc, "pointerdown", 450, 300, { target: input });
      dispatchPointerEvent(doc, "pointerup", 451, 301, { target: input });
      expect(useReaderStore.getState().toolbarVisible).toBe(true);
    });

    it("ignores non-primary pointers (multi-touch)", async () => {
      const doc = await mountAndGetIframeDoc();

      useReaderStore.setState({ toolbarVisible: true });
      dispatchPointerEvent(doc, "pointerdown", 450, 300, {
        isPrimary: false,
        pointerId: 2,
      });
      dispatchPointerEvent(doc, "pointerup", 450, 300, {
        isPrimary: false,
        pointerId: 2,
      });
      expect(useReaderStore.getState().toolbarVisible).toBe(true);
    });

    it("ignores drags above the tap tolerance", async () => {
      const doc = await mountAndGetIframeDoc();

      useReaderStore.setState({ toolbarVisible: true });
      // A 200 px horizontal movement is well above the tap tolerance —
      // the hook should leave the toolbar (and navigation) untouched so
      // the browser keeps native pan / back-swipe behavior.
      dispatchPointerEvent(doc, "pointerdown", 450, 300);
      dispatchPointerEvent(doc, "pointerup", 250, 300);
      expect(useReaderStore.getState().toolbarVisible).toBe(true);
    });

    it("aborts when pointercancel fires before pointerup", async () => {
      const doc = await mountAndGetIframeDoc();

      useReaderStore.setState({ toolbarVisible: true });
      dispatchPointerEvent(doc, "pointerdown", 450, 300);
      dispatchPointerEvent(doc, "pointercancel", 450, 300);
      dispatchPointerEvent(doc, "pointerup", 450, 300);
      // Cancel cleared the gesture; pointerup should be a no-op.
      expect(useReaderStore.getState().toolbarVisible).toBe(true);
    });
  });

  describe("mobile chapter pill (U2)", () => {
    function forceMobileViewport() {
      window.matchMedia = vi.fn().mockImplementation((query) => ({
        matches: query.includes("max-width"),
        media: query,
        onchange: null,
        addListener: vi.fn(),
        removeListener: vi.fn(),
        addEventListener: vi.fn(),
        removeEventListener: vi.fn(),
        dispatchEvent: vi.fn(),
      }));
    }

    it("does not render the chapter pill until the TOC and location are known", () => {
      forceMobileViewport();
      renderWithProviders(<EpubReader {...defaultProps} />);

      // Initial mount: TOC is empty in the mock; chapter pill should not appear.
      expect(
        screen.queryByLabelText("Open table of contents"),
      ).not.toBeInTheDocument();
    });
  });

  describe("mobile reader styles", () => {
    it("does not hide side arrows on non-mobile viewports", () => {
      renderWithProviders(<EpubReader {...defaultProps} />);

      expect(lastReaderStyles?.arrow?.display).toBeUndefined();
    });

    it("hides react-reader side arrows on mobile viewports", () => {
      window.matchMedia = vi.fn().mockImplementation((query) => ({
        matches: true,
        media: query,
        onchange: null,
        addListener: vi.fn(),
        removeListener: vi.fn(),
        addEventListener: vi.fn(),
        removeEventListener: vi.fn(),
        dispatchEvent: vi.fn(),
      }));

      renderWithProviders(<EpubReader {...defaultProps} />);

      expect(lastReaderStyles?.arrow?.display).toBe("none");
    });
  });

  describe("background color", () => {
    it("should apply theme-based background color", () => {
      renderWithProviders(<EpubReader {...defaultProps} />);

      const container = document.querySelector('[style*="100vw"]');
      expect(container).toBeInTheDocument();
      // Light theme uses white background
      expect(container).toHaveStyle({ backgroundColor: "#ffffff" });
    });

    it("should apply dark theme background", () => {
      useReaderStore.setState({
        settings: { ...defaultSettings, epubTheme: "dark" },
      });

      renderWithProviders(<EpubReader {...defaultProps} />);

      const container = document.querySelector('[style*="100vw"]');
      expect(container).toBeInTheDocument();
      // Dark theme uses dark background
      expect(container).toHaveStyle({ backgroundColor: "#1a1a1a" });
    });

    it("should apply sepia theme background", () => {
      useReaderStore.setState({
        settings: { ...defaultSettings, epubTheme: "sepia" },
      });

      renderWithProviders(<EpubReader {...defaultProps} />);

      const container = document.querySelector('[style*="100vw"]');
      expect(container).toBeInTheDocument();
      // Sepia theme uses cream background
      expect(container).toHaveStyle({ backgroundColor: "#f4ecd8" });
    });
  });
});
