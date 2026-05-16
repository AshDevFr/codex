import { beforeEach, describe, expect, it, vi } from "vitest";
import { useReaderStore } from "@/store/readerStore";
import { fireEvent, renderWithProviders, screen, waitFor } from "@/test/utils";
import { ReaderToolbar } from "./ReaderToolbar";

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

describe("ReaderToolbar", () => {
  const defaultProps = {
    title: "Test Book",
    visible: true,
    onClose: vi.fn(),
    onOpenSettings: vi.fn(),
  };

  beforeEach(() => {
    vi.clearAllMocks();
    // Most tests run in desktop mode; mobile tests opt in via
    // forceMobileViewport().
    forceDesktopViewport();
    // Reset store to default state
    useReaderStore.setState({
      settings: {
        fitMode: "screen",
        pageLayout: "single",
        readingDirection: "ltr",
        backgroundColor: "black",
        pdfMode: "streaming",
        pdfSpreadMode: "single",
        pdfContinuousScroll: false,
        autoHideToolbar: true,
        toolbarHideDelay: 3000,
        epubTheme: "light",
        epubFontSize: 100,
        epubFontFamily: "default",
        epubLineHeight: 150,
        epubMargin: 10,
        preloadPages: 1,
        doublePageShowWideAlone: true,
        doublePageStartOnOdd: true,
        pageTransition: "slide",
        transitionDuration: 200,
        webtoonSidePadding: 0,
        webtoonPageGap: 0,
        autoAdvanceToNextBook: false,
      },
      currentPage: 5,
      totalPages: 10,
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
    });
  });

  it("should render book title", () => {
    renderWithProviders(<ReaderToolbar {...defaultProps} />);

    expect(screen.getByText("Test Book")).toBeInTheDocument();
  });

  it("should display current page and total pages", () => {
    renderWithProviders(<ReaderToolbar {...defaultProps} />);

    expect(screen.getByText("5 / 10")).toBeInTheDocument();
  });

  it("should call onClose when close button is clicked", () => {
    renderWithProviders(<ReaderToolbar {...defaultProps} />);

    // Close button is the first button (X icon)
    const buttons = screen.getAllByRole("button");
    fireEvent.click(buttons[0]); // First button is close

    expect(defaultProps.onClose).toHaveBeenCalledTimes(1);
  });

  it("should call onOpenSettings when settings button is clicked", () => {
    renderWithProviders(<ReaderToolbar {...defaultProps} />);

    // Settings button is the last button
    const buttons = screen.getAllByRole("button");
    fireEvent.click(buttons[buttons.length - 1]); // Last button is settings

    expect(defaultProps.onOpenSettings).toHaveBeenCalledTimes(1);
  });

  it("should navigate to next page when forward button is clicked", () => {
    renderWithProviders(<ReaderToolbar {...defaultProps} />);

    // Navigation buttons are in the center group
    const buttons = screen.getAllByRole("button");
    // In LTR: [close, prev, next, fit, fullscreen, settings]
    // Index 2 is the forward button
    fireEvent.click(buttons[2]);

    expect(useReaderStore.getState().currentPage).toBe(6);
  });

  it("should navigate to previous page when backward button is clicked", () => {
    renderWithProviders(<ReaderToolbar {...defaultProps} />);

    const buttons = screen.getAllByRole("button");
    // Index 1 is the backward button
    fireEvent.click(buttons[1]);

    expect(useReaderStore.getState().currentPage).toBe(4);
  });

  it("should disable backward button on first page in LTR", () => {
    useReaderStore.setState({ currentPage: 1 });
    renderWithProviders(<ReaderToolbar {...defaultProps} />);

    const buttons = screen.getAllByRole("button");
    // In LTR mode, first page means backward (prev) is disabled
    expect(buttons[1]).toBeDisabled();
  });

  it("should disable forward button on last page in LTR", () => {
    useReaderStore.setState({ currentPage: 10 });
    renderWithProviders(<ReaderToolbar {...defaultProps} />);

    const buttons = screen.getAllByRole("button");
    // In LTR mode, last page means forward (next) is disabled
    expect(buttons[2]).toBeDisabled();
  });

  it("should toggle fullscreen when fullscreen button is clicked", () => {
    renderWithProviders(<ReaderToolbar {...defaultProps} />);

    const buttons = screen.getAllByRole("button");
    // Fullscreen button is index 4
    fireEvent.click(buttons[4]);

    expect(useReaderStore.getState().isFullscreen).toBe(true);
  });

  it("should cycle fit mode when fit mode button is clicked", () => {
    renderWithProviders(<ReaderToolbar {...defaultProps} />);

    const buttons = screen.getAllByRole("button");
    // Fit mode button is index 3
    fireEvent.click(buttons[3]);

    expect(useReaderStore.getState().settings.fitMode).toBe("width");
  });

  it("should not render when visible is false", () => {
    renderWithProviders(<ReaderToolbar {...defaultProps} visible={false} />);

    // The toolbar should be hidden via Transition
    expect(screen.queryByText("Test Book")).not.toBeInTheDocument();
  });

  it("should display progress percentage", () => {
    renderWithProviders(<ReaderToolbar {...defaultProps} />);

    // 5/10 = 50%
    expect(screen.getByText("50%")).toBeInTheDocument();
  });

  describe("RTL reading direction", () => {
    beforeEach(() => {
      useReaderStore.setState({
        settings: {
          ...useReaderStore.getState().settings,
          readingDirection: "rtl",
        },
      });
    });

    it("should render with RTL direction buttons", () => {
      renderWithProviders(<ReaderToolbar {...defaultProps} />);

      // In RTL mode, the buttons should still be rendered
      const buttons = screen.getAllByRole("button");
      expect(buttons.length).toBeGreaterThan(0);
    });

    it("should navigate correctly in RTL mode", () => {
      renderWithProviders(<ReaderToolbar {...defaultProps} />);

      const buttons = screen.getAllByRole("button");

      // In RTL mode, the backward button (index 1) should call nextPage
      fireEvent.click(buttons[1]);
      expect(useReaderStore.getState().currentPage).toBe(6);

      // Reset page
      useReaderStore.setState({ currentPage: 5 });

      // In RTL mode, the forward button (index 2) should call prevPage
      fireEvent.click(buttons[2]);
      expect(useReaderStore.getState().currentPage).toBe(4);
    });

    it("should use scaleX(-1) with unchanged slider value in RTL mode", () => {
      // currentPage=5, totalPages=10
      // The slider value stays as currentPage (scaleX(-1) handles the visual flip)
      // Only onChange inverts the value so clicks map to the correct page
      renderWithProviders(<ReaderToolbar {...defaultProps} />);

      const slider = screen.getByRole("slider");
      expect(slider).toHaveAttribute("aria-valuenow", "5");
      expect(slider).toHaveAttribute("aria-valuemin", "1");
      expect(slider).toHaveAttribute("aria-valuemax", "10");
    });
  });

  describe("mobile (phone) viewport", () => {
    beforeEach(() => {
      forceMobileViewport();
    });

    it("hides the inline slider on phones", () => {
      // On phones the bottom slider row is dropped from the toolbar — the
      // MobileReaderBottomBar takes over. The inline page-counter ("5 / 10")
      // is also moved out of the top bar to keep it within 390px viewports.
      renderWithProviders(<ReaderToolbar {...defaultProps} />);

      expect(screen.queryByRole("slider")).not.toBeInTheDocument();
      expect(screen.queryByText("5 / 10")).not.toBeInTheDocument();
    });

    it("renders close, title, settings, and a single overflow trigger", () => {
      renderWithProviders(<ReaderToolbar {...defaultProps} />);

      expect(screen.getByLabelText("Close reader")).toBeInTheDocument();
      expect(screen.getByText("Test Book")).toBeInTheDocument();
      expect(screen.getByLabelText("Reader settings")).toBeInTheDocument();
      expect(screen.getByLabelText("More reader options")).toBeInTheDocument();
    });

    it("opens the overflow menu and exposes fit-mode + fullscreen", async () => {
      renderWithProviders(<ReaderToolbar {...defaultProps} />);

      fireEvent.click(screen.getByLabelText("More reader options"));

      await waitFor(() => {
        expect(screen.getByText(/Fit:/)).toBeInTheDocument();
      });
      expect(
        screen.getByText(/Fullscreen|Exit fullscreen/),
      ).toBeInTheDocument();
    });

    it("cycles the fit mode from the overflow menu", async () => {
      renderWithProviders(<ReaderToolbar {...defaultProps} />);

      fireEvent.click(screen.getByLabelText("More reader options"));
      await waitFor(() => {
        expect(screen.getByText(/Fit:/)).toBeInTheDocument();
      });
      fireEvent.click(screen.getByText(/Fit:/));

      expect(useReaderStore.getState().settings.fitMode).toBe("width");
    });

    it("toggles fullscreen from the overflow menu", async () => {
      renderWithProviders(<ReaderToolbar {...defaultProps} />);

      fireEvent.click(screen.getByLabelText("More reader options"));
      await waitFor(() => {
        expect(screen.getByText(/Fullscreen/)).toBeInTheDocument();
      });
      fireEvent.click(screen.getByText(/Fullscreen/));

      expect(useReaderStore.getState().isFullscreen).toBe(true);
    });

    it("calls onPrevBook from the overflow menu when provided", async () => {
      const onPrevBook = vi.fn();
      renderWithProviders(
        <ReaderToolbar
          {...defaultProps}
          onPrevBook={onPrevBook}
          prevBook={{ title: "Vol. 1" }}
        />,
      );

      fireEvent.click(screen.getByLabelText("More reader options"));
      await waitFor(() => {
        expect(screen.getByText(/Previous: Vol\. 1/)).toBeInTheDocument();
      });
      fireEvent.click(screen.getByText(/Previous: Vol\. 1/));

      expect(onPrevBook).toHaveBeenCalledTimes(1);
    });

    it("renders custom mobileMenuItems in the overflow menu", async () => {
      renderWithProviders(
        <ReaderToolbar
          {...defaultProps}
          mobileMenuItems={
            <button type="button" data-testid="custom-mobile-action">
              EPUB action
            </button>
          }
        />,
      );

      fireEvent.click(screen.getByLabelText("More reader options"));
      await waitFor(() => {
        expect(screen.getByTestId("custom-mobile-action")).toBeInTheDocument();
      });
    });

    it("keeps leftActions mounted (display:none) so portaled drawers survive", () => {
      const leftMarker = (
        <div data-testid="left-actions-marker">left actions</div>
      );
      renderWithProviders(
        <ReaderToolbar {...defaultProps} leftActions={leftMarker} />,
      );

      // The element is in the DOM tree but visually hidden by display:none on
      // its wrapper. The important contract: it's NOT unmounted, so any
      // portaled drawer body inside leftActions keeps responding to parent
      // `opened` state when triggered from the mobile overflow menu.
      expect(screen.getByTestId("left-actions-marker")).toBeInTheDocument();
    });
  });
});
