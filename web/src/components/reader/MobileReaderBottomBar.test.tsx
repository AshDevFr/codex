import { beforeEach, describe, expect, it, vi } from "vitest";
import { useReaderStore } from "@/store/readerStore";
import { fireEvent, renderWithProviders, screen, waitFor } from "@/test/utils";
import { MobileReaderBottomBar } from "./MobileReaderBottomBar";

/**
 * Force the phone breakpoint by reporting `matches: true` for max-width
 * media queries. The shared test setup mocks matchMedia to always return
 * `matches: false`, which is the desktop default. The MobileReaderBottomBar
 * self-gates on `useMediaQuery("(max-width: 30.0625em)")` so without this
 * override it would render nothing.
 */
function forceMobileViewport() {
  Object.defineProperty(window, "matchMedia", {
    writable: true,
    configurable: true,
    value: vi.fn().mockImplementation((query: string) => ({
      matches: query.includes("max-width"),
      media: query,
      onchange: null,
      addListener: vi.fn(),
      removeListener: vi.fn(),
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
      dispatchEvent: vi.fn(),
    })),
  });
}

function forceDesktopViewport() {
  Object.defineProperty(window, "matchMedia", {
    writable: true,
    configurable: true,
    value: vi.fn().mockImplementation((query: string) => ({
      matches: false,
      media: query,
      onchange: null,
      addListener: vi.fn(),
      removeListener: vi.fn(),
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
      dispatchEvent: vi.fn(),
    })),
  });
}

const DEFAULT_SETTINGS = {
  fitMode: "screen" as const,
  webtoonFitMode: "width" as const,
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
  epubSpread: "auto" as const,
  preloadPages: 1,
  doublePageShowWideAlone: true,
  doublePageStartOnOdd: true,
  pageTransition: "slide" as const,
  transitionDuration: 200,
  webtoonSidePadding: 0,
  webtoonPageGap: 0,
  autoAdvanceToNextBook: false,
};

function resetStore(overrides: Record<string, unknown> = {}) {
  useReaderStore.setState({
    settings: DEFAULT_SETTINGS,
    currentPage: 5,
    totalPages: 20,
    isLoading: false,
    toolbarVisible: true,
    isFullscreen: false,
    currentBookId: "book-123",
    readingDirectionOverride: null,
    adjacentBooks: null,
    boundaryState: "none",
    pageOrientations: {},
    lastNavigationDirection: null,
    preloadedImages: new Set<string>(),
    ...overrides,
  });
}

describe("MobileReaderBottomBar", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetStore();
  });

  describe("desktop viewport", () => {
    beforeEach(() => {
      forceDesktopViewport();
    });

    it("renders nothing above the xs breakpoint", () => {
      renderWithProviders(<MobileReaderBottomBar visible={true} />);

      // Top-bar slider is the only one in desktop ReaderToolbar; this
      // component should bail out entirely so it doesn't duplicate it.
      expect(screen.queryByRole("slider")).not.toBeInTheDocument();
      expect(screen.queryByText("5 / 20")).not.toBeInTheDocument();
    });
  });

  describe("phone viewport", () => {
    beforeEach(() => {
      forceMobileViewport();
    });

    it("renders the page counter and slider", () => {
      renderWithProviders(<MobileReaderBottomBar visible={true} />);

      expect(screen.getByText("5 / 20")).toBeInTheDocument();
      expect(screen.getByRole("slider")).toBeInTheDocument();
    });

    it("renders nothing when totalPages is 0", () => {
      resetStore({ totalPages: 0, currentPage: 0 });
      renderWithProviders(<MobileReaderBottomBar visible={true} />);

      expect(screen.queryByRole("slider")).not.toBeInTheDocument();
    });

    it("calls the provided onNextPage when right chevron is tapped", () => {
      const onNextPage = vi.fn();
      renderWithProviders(
        <MobileReaderBottomBar visible={true} onNextPage={onNextPage} />,
      );

      fireEvent.click(screen.getByLabelText("Next page"));

      expect(onNextPage).toHaveBeenCalledTimes(1);
    });

    it("calls the provided onPrevPage when left chevron is tapped", () => {
      const onPrevPage = vi.fn();
      renderWithProviders(
        <MobileReaderBottomBar visible={true} onPrevPage={onPrevPage} />,
      );

      fireEvent.click(screen.getByLabelText("Previous page"));

      expect(onPrevPage).toHaveBeenCalledTimes(1);
    });

    it("falls back to the store actions when no handlers are provided", () => {
      renderWithProviders(<MobileReaderBottomBar visible={true} />);

      fireEvent.click(screen.getByLabelText("Next page"));

      // Store's nextPage clamps at totalPages, so currentPage 5 → 6.
      expect(useReaderStore.getState().currentPage).toBe(6);
    });

    it("disables the prev chevron on page 1", () => {
      resetStore({ currentPage: 1 });
      renderWithProviders(<MobileReaderBottomBar visible={true} />);

      expect(screen.getByLabelText("Previous page")).toBeDisabled();
    });

    it("disables the next chevron on the last page", () => {
      resetStore({ currentPage: 20 });
      renderWithProviders(<MobileReaderBottomBar visible={true} />);

      expect(screen.getByLabelText("Next page")).toBeDisabled();
    });

    it("swaps prev/next semantics in RTL reading mode", () => {
      resetStore({
        settings: { ...DEFAULT_SETTINGS, readingDirection: "rtl" },
      });
      const onNextPage = vi.fn();
      const onPrevPage = vi.fn();
      renderWithProviders(
        <MobileReaderBottomBar
          visible={true}
          onNextPage={onNextPage}
          onPrevPage={onPrevPage}
        />,
      );

      // In RTL the visual "previous page" chevron is on the right, so the
      // left chevron should advance to the next page.
      fireEvent.click(screen.getByLabelText("Next page"));
      expect(onNextPage).toHaveBeenCalledTimes(1);

      fireEvent.click(screen.getByLabelText("Previous page"));
      expect(onPrevPage).toHaveBeenCalledTimes(1);
    });

    it("opens the jump-to-page modal when the page counter is tapped", async () => {
      renderWithProviders(<MobileReaderBottomBar visible={true} />);

      fireEvent.click(screen.getByLabelText("Jump to page"));

      await waitFor(() => {
        // Modal renders a heading with the title "Go to page".
        expect(
          screen.getByRole("dialog", { name: /go to page/i }),
        ).toBeInTheDocument();
      });
    });

    it("jumps to the page entered in the modal when Go is pressed", async () => {
      renderWithProviders(<MobileReaderBottomBar visible={true} />);

      fireEvent.click(screen.getByLabelText("Jump to page"));

      await waitFor(() => {
        expect(
          screen.getByRole("dialog", { name: /go to page/i }),
        ).toBeInTheDocument();
      });

      const input = screen.getByRole("textbox");
      fireEvent.change(input, { target: { value: "12" } });
      fireEvent.click(screen.getByRole("button", { name: "Go" }));

      expect(useReaderStore.getState().currentPage).toBe(12);
    });

    describe("EPUB chapter variant (U2)", () => {
      it("renders a chapter pill instead of the page-counter slider", () => {
        // EPUB doesn't drive the reader store's currentPage/totalPages.
        resetStore({ totalPages: 0, currentPage: 0 });
        renderWithProviders(
          <MobileReaderBottomBar
            visible={true}
            epubChapter={{ currentIndex: 3, total: 12, onTap: vi.fn() }}
          />,
        );

        expect(screen.getByText("Ch 3 / 12")).toBeInTheDocument();
        // No slider in EPUB layout (pagination is reflowable).
        expect(screen.queryByRole("slider")).not.toBeInTheDocument();
        // No page-jump button either.
        expect(screen.queryByLabelText("Jump to page")).not.toBeInTheDocument();
      });

      it("opens the TOC drawer when the chapter pill is tapped", () => {
        resetStore({ totalPages: 0, currentPage: 0 });
        const onTap = vi.fn();
        renderWithProviders(
          <MobileReaderBottomBar
            visible={true}
            epubChapter={{ currentIndex: 1, total: 10, onTap }}
          />,
        );

        fireEvent.click(screen.getByLabelText("Open table of contents"));

        expect(onTap).toHaveBeenCalledTimes(1);
      });

      it("still wires prev/next chevrons in EPUB layout", () => {
        resetStore({ totalPages: 0, currentPage: 0 });
        const onPrevPage = vi.fn();
        const onNextPage = vi.fn();
        renderWithProviders(
          <MobileReaderBottomBar
            visible={true}
            onPrevPage={onPrevPage}
            onNextPage={onNextPage}
            epubChapter={{ currentIndex: 2, total: 5, onTap: vi.fn() }}
          />,
        );

        fireEvent.click(screen.getByLabelText("Previous page"));
        fireEvent.click(screen.getByLabelText("Next page"));

        expect(onPrevPage).toHaveBeenCalledTimes(1);
        expect(onNextPage).toHaveBeenCalledTimes(1);
      });
    });

    it("clamps the jump value to the valid page range", async () => {
      renderWithProviders(<MobileReaderBottomBar visible={true} />);

      fireEvent.click(screen.getByLabelText("Jump to page"));
      await waitFor(() => {
        expect(
          screen.getByRole("dialog", { name: /go to page/i }),
        ).toBeInTheDocument();
      });

      const input = screen.getByRole("textbox");
      // Try to jump way past the end of the book.
      fireEvent.change(input, { target: { value: "999" } });
      fireEvent.click(screen.getByRole("button", { name: "Go" }));

      expect(useReaderStore.getState().currentPage).toBe(20);
    });
  });
});
