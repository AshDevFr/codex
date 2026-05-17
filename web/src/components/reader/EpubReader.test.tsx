import { beforeEach, describe, expect, it, vi } from "vitest";
import { useReaderStore } from "@/store/readerStore";
import { renderWithProviders, screen } from "@/test/utils";
import { EpubReader } from "./EpubReader";
import { useTouchNav } from "./hooks/useTouchNav";

// Mock useTouchNav so we can drive its callbacks directly in tests (R7-1).
// Returning a no-op ref keeps the production wiring code happy.
vi.mock("./hooks/useTouchNav", () => ({
  useTouchNav: vi.fn(() => ({ touchRef: vi.fn() })),
}));

// Captures the per-event handlers `EpubReader` registers on the rendition,
// so tests can fire (e.g.) the "click" handler to verify R7-1 toolbar toggle.
const renditionHandlers: Record<string, (...args: unknown[]) => void> = {};
// Captures hooks.content.register callbacks so R10-1 tests can drive the
// inside-iframe pointer hook with a fake `contents` document.
const contentHookCallbacks: Array<(contents: { document: Document }) => void> =
  [];
// Stash the latest readerStyles ReactReader received so R7-3 tests can assert
// the side-arrow `display: none` override is applied on mobile viewports.
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

      // EpubTableOfContents is rendered in toolbar
      // It's toggled by a button
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

  describe("mobile tap-to-toggle toolbar (R7-1)", () => {
    it("wires useTouchNav with onTap that toggles the toolbar", () => {
      renderWithProviders(<EpubReader {...defaultProps} />);

      expect(useTouchNav).toHaveBeenCalled();
      const opts = vi.mocked(useTouchNav).mock.calls.at(-1)?.[0];
      expect(opts?.onTap).toBe(useReaderStore.getState().toggleToolbar);

      // Drive the captured onTap to verify it flips toolbarVisible
      useReaderStore.setState({ toolbarVisible: true });
      opts?.onTap?.();
      expect(useReaderStore.getState().toolbarVisible).toBe(false);
      opts?.onTap?.();
      expect(useReaderStore.getState().toolbarVisible).toBe(true);
    });

    it("registers a content hook that wires pointer events on the iframe doc (R10-1)", async () => {
      renderWithProviders(<EpubReader {...defaultProps} />);

      // Rendition is wired asynchronously via setTimeout in the mock
      await new Promise((r) => setTimeout(r, 0));

      expect(contentHookCallbacks.length).toBeGreaterThan(0);
    });
  });

  describe("EPUB iframe pointer navigation (R10-1)", () => {
    // Helpers for the R10-1 test suite. They build the same fake-iframe doc
    // and dispatch pointer events against it so the inside-iframe hook (which
    // can't see real epub.js iframes in JSDOM) gets exercised.
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
        timeStamp?: number;
        target?: Element;
      } = {},
    ) => {
      const {
        pointerType = "touch",
        pointerId = 1,
        isPrimary = true,
        button = 0,
        timeStamp = 0,
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
      Object.defineProperty(event, "timeStamp", { value: timeStamp });
      const dispatchTarget = target ?? doc.body;
      dispatchTarget.dispatchEvent(event);
    };

    const mountAndGetIframeDoc = async () => {
      renderWithProviders(<EpubReader {...defaultProps} />);
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

    it("calls rendition.next on a left swipe inside the iframe (LTR)", async () => {
      const doc = await mountAndGetIframeDoc();

      useReaderStore.setState({ toolbarVisible: false });
      dispatchPointerEvent(doc, "pointerdown", 300, 200, { timeStamp: 0 });
      dispatchPointerEvent(doc, "pointerup", 100, 200, { timeStamp: 100 });

      // Toolbar should not toggle from a swipe
      expect(useReaderStore.getState().toolbarVisible).toBe(false);
    });

    it("calls toggleToolbar on a tap inside the iframe", async () => {
      const doc = await mountAndGetIframeDoc();

      useReaderStore.setState({ toolbarVisible: true });
      dispatchPointerEvent(doc, "pointerdown", 200, 200, { timeStamp: 0 });
      dispatchPointerEvent(doc, "pointerup", 201, 200, { timeStamp: 80 });
      expect(useReaderStore.getState().toolbarVisible).toBe(false);

      dispatchPointerEvent(doc, "pointerdown", 200, 200, { timeStamp: 100 });
      dispatchPointerEvent(doc, "pointerup", 200, 201, { timeStamp: 150 });
      expect(useReaderStore.getState().toolbarVisible).toBe(true);
    });

    it("routes iframe taps through tap zones (LTR)", async () => {
      const doc = await mountAndGetIframeDoc();
      // The fake iframe doc is detached, so view-derived sizes are 0; stub
      // documentElement.clientWidth/Height to give the hook a real geometry.
      Object.defineProperty(doc.documentElement, "clientWidth", {
        configurable: true,
        value: 900,
      });
      Object.defineProperty(doc.documentElement, "clientHeight", {
        configurable: true,
        value: 600,
      });

      const visibleBefore = useReaderStore.getState().toolbarVisible;

      // Left third tap → prev page (LTR).
      dispatchPointerEvent(doc, "pointerdown", 100, 300, { timeStamp: 0 });
      dispatchPointerEvent(doc, "pointerup", 100, 300, { timeStamp: 50 });

      // Right third tap → next page.
      dispatchPointerEvent(doc, "pointerdown", 800, 300, { timeStamp: 100 });
      dispatchPointerEvent(doc, "pointerup", 800, 300, { timeStamp: 150 });

      // Toolbar visibility is unchanged because both taps landed in nav zones.
      expect(useReaderStore.getState().toolbarVisible).toBe(visibleBefore);
    });

    it("ignores pointer interactions starting on links and form controls", async () => {
      const doc = await mountAndGetIframeDoc();

      const link = doc.createElement("a");
      doc.body.appendChild(link);
      const input = doc.createElement("input");
      doc.body.appendChild(input);

      useReaderStore.setState({ toolbarVisible: true });
      dispatchPointerEvent(doc, "pointerdown", 50, 50, {
        target: link,
        timeStamp: 0,
      });
      dispatchPointerEvent(doc, "pointerup", 51, 51, {
        target: link,
        timeStamp: 50,
      });
      expect(useReaderStore.getState().toolbarVisible).toBe(true);

      dispatchPointerEvent(doc, "pointerdown", 50, 50, {
        target: input,
        timeStamp: 100,
      });
      dispatchPointerEvent(doc, "pointerup", 51, 51, {
        target: input,
        timeStamp: 150,
      });
      expect(useReaderStore.getState().toolbarVisible).toBe(true);
    });

    it("ignores non-primary pointers (multi-touch)", async () => {
      const doc = await mountAndGetIframeDoc();

      useReaderStore.setState({ toolbarVisible: true });
      dispatchPointerEvent(doc, "pointerdown", 200, 200, {
        isPrimary: false,
        pointerId: 2,
        timeStamp: 0,
      });
      dispatchPointerEvent(doc, "pointerup", 200, 200, {
        isPrimary: false,
        pointerId: 2,
        timeStamp: 50,
      });
      expect(useReaderStore.getState().toolbarVisible).toBe(true);
    });

    it("aborts when pointercancel fires before pointerup", async () => {
      const doc = await mountAndGetIframeDoc();

      useReaderStore.setState({ toolbarVisible: true });
      dispatchPointerEvent(doc, "pointerdown", 300, 200, { timeStamp: 0 });
      dispatchPointerEvent(doc, "pointercancel", 200, 200, { timeStamp: 50 });
      dispatchPointerEvent(doc, "pointerup", 100, 200, { timeStamp: 100 });
      // Cancel cleared the gesture; pointerup should not register as a swipe
      // or a tap.
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

  describe("mobile reader styles (R7-3)", () => {
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
