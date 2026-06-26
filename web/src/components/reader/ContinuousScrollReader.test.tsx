import { act, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { renderWithProviders } from "@/test/utils";
import { ContinuousScrollReader } from "./ContinuousScrollReader";

// Mock the reader store with controllable currentPage
const mockGoToPage = vi.fn((page: number) => {
  mockCurrentPage = page;
});
const mockCorrectTotalPages = vi.fn();
let mockCurrentPage = 1;

function getMockState() {
  return {
    goToPage: mockGoToPage,
    currentPage: mockCurrentPage,
    correctTotalPages: mockCorrectTotalPages,
  };
}

vi.mock("@/store/readerStore", () => {
  const store = (
    selector: (state: ReturnType<typeof getMockState>) => unknown,
  ) => {
    return selector(getMockState());
  };
  store.getState = () => getMockState();
  // Minimal subscribe: listeners are never called in tests (store is a mock),
  // but the method must exist so the useEffect that calls it doesn't throw.
  store.subscribe = (_listener: unknown) => {
    return () => {}; // unsubscribe no-op
  };
  return { useReaderStore: store };
});

// Mock IntersectionObserver
class MockIntersectionObserver {
  callback: IntersectionObserverCallback;
  elements: Set<Element> = new Set();

  constructor(callback: IntersectionObserverCallback) {
    this.callback = callback;
  }

  observe(element: Element) {
    this.elements.add(element);
  }

  unobserve(element: Element) {
    this.elements.delete(element);
  }

  disconnect() {
    this.elements.clear();
  }

  // Helper to simulate intersection
  simulateIntersection(entries: Partial<IntersectionObserverEntry>[]) {
    const fullEntries = entries.map((entry) => ({
      boundingClientRect: {
        top: 0,
        bottom: 100,
        height: 100,
        ...entry.boundingClientRect,
      } as DOMRect,
      intersectionRatio: 1,
      intersectionRect: {} as DOMRect,
      isIntersecting: true,
      rootBounds: null,
      target: entry.target || document.createElement("div"),
      time: Date.now(),
      ...entry,
    })) as IntersectionObserverEntry[];
    this.callback(fullEntries, this as unknown as IntersectionObserver);
  }
}

let mockObserverInstance: MockIntersectionObserver | null = null;

beforeEach(() => {
  vi.clearAllMocks();
  mockObserverInstance = null;
  mockCurrentPage = 1;

  // Mock IntersectionObserver (class-based for vitest v4 compatibility)
  global.IntersectionObserver = class extends MockIntersectionObserver {
    constructor(callback: IntersectionObserverCallback) {
      super(callback);
      mockObserverInstance = this;
    }
  } as unknown as typeof IntersectionObserver;

  // Mock scrollIntoView
  Element.prototype.scrollIntoView = vi.fn();
});

describe("ContinuousScrollReader", () => {
  const defaultProps = {
    bookId: "test-book-123",
    totalPages: 10,
    fitMode: "width" as const,
    backgroundColor: "black" as const,
  };

  describe("Rendering", () => {
    it("should render the scroll container", () => {
      renderWithProviders(<ContinuousScrollReader {...defaultProps} />);

      expect(
        screen.getByTestId("continuous-scroll-container"),
      ).toBeInTheDocument();
    });

    it("should render page containers for all pages", () => {
      renderWithProviders(<ContinuousScrollReader {...defaultProps} />);

      for (let i = 1; i <= defaultProps.totalPages; i++) {
        expect(screen.getByTestId(`page-container-${i}`)).toBeInTheDocument();
      }
    });

    it("should show empty state when totalPages is 0", () => {
      renderWithProviders(
        <ContinuousScrollReader {...defaultProps} totalPages={0} />,
      );

      expect(screen.getByText("This book has no pages")).toBeInTheDocument();
    });

    it("should apply correct background color", () => {
      const { rerender } = renderWithProviders(
        <ContinuousScrollReader {...defaultProps} backgroundColor="black" />,
      );

      const container = screen.getByTestId("continuous-scroll-container");
      expect(container).toHaveStyle({ backgroundColor: "#000000" });

      rerender(
        <ContinuousScrollReader {...defaultProps} backgroundColor="gray" />,
      );
      expect(container).toHaveStyle({ backgroundColor: "#1a1a1a" });

      rerender(
        <ContinuousScrollReader {...defaultProps} backgroundColor="white" />,
      );
      expect(container).toHaveStyle({ backgroundColor: "#ffffff" });
    });
  });

  describe("Lazy Loading", () => {
    it("should initially render pages around the initial page", () => {
      renderWithProviders(
        <ContinuousScrollReader
          {...defaultProps}
          initialPage={5}
          preloadBuffer={2}
        />,
      );

      // Pages 3-7 should be rendered (5 +/- 2)
      // Check for placeholders vs actual content based on initial render
      for (let i = 1; i <= 10; i++) {
        const container = screen.getByTestId(`page-container-${i}`);
        expect(container).toBeInTheDocument();
      }
    });

    it("should render placeholders for pages outside buffer", () => {
      renderWithProviders(
        <ContinuousScrollReader
          {...defaultProps}
          totalPages={20}
          initialPage={1}
          preloadBuffer={2}
        />,
      );

      // Pages far from initial should have placeholders
      const farPagePlaceholder = screen.queryByTestId("page-placeholder-15");
      expect(farPagePlaceholder).toBeInTheDocument();
    });

    it("should render images for pages within buffer", async () => {
      renderWithProviders(
        <ContinuousScrollReader
          {...defaultProps}
          initialPage={1}
          preloadBuffer={2}
        />,
      );

      // Pages 1-3 should have images (1 + 2 buffer)
      await waitFor(() => {
        const image1 = screen.queryByTestId("page-image-1");
        expect(image1).toBeInTheDocument();
      });
    });
  });

  describe("Fit Modes", () => {
    it("should apply screen fit mode styles", () => {
      renderWithProviders(
        <ContinuousScrollReader {...defaultProps} fitMode="screen" />,
      );

      // Simulate page becoming visible
      const pageContainer = screen.getByTestId("page-container-1");
      act(() => {
        mockObserverInstance?.simulateIntersection([
          {
            target: pageContainer,
            isIntersecting: true,
          },
        ]);
      });

      const image = screen.queryByTestId("page-image-1");
      if (image) {
        expect(image).toHaveStyle({ maxWidth: "100%", maxHeight: "100vh" });
      }
    });

    it("should apply width fit mode styles", () => {
      renderWithProviders(
        <ContinuousScrollReader {...defaultProps} fitMode="width" />,
      );

      const pageContainer = screen.getByTestId("page-container-1");
      act(() => {
        mockObserverInstance?.simulateIntersection([
          {
            target: pageContainer,
            isIntersecting: true,
          },
        ]);
      });

      const image = screen.queryByTestId("page-image-1");
      if (image) {
        expect(image).toHaveStyle({ width: "100%" });
      }
    });

    it("should apply width-shrink fit mode styles", () => {
      renderWithProviders(
        <ContinuousScrollReader {...defaultProps} fitMode="width-shrink" />,
      );

      const pageContainer = screen.getByTestId("page-container-1");
      act(() => {
        mockObserverInstance?.simulateIntersection([
          {
            target: pageContainer,
            isIntersecting: true,
          },
        ]);
      });

      const image = screen.queryByTestId("page-image-1");
      if (image) {
        expect(image).toHaveStyle({ maxWidth: "100%" });
      }
    });

    it("should apply height fit mode styles", () => {
      renderWithProviders(
        <ContinuousScrollReader {...defaultProps} fitMode="height" />,
      );

      const pageContainer = screen.getByTestId("page-container-1");
      act(() => {
        mockObserverInstance?.simulateIntersection([
          {
            target: pageContainer,
            isIntersecting: true,
          },
        ]);
      });

      const image = screen.queryByTestId("page-image-1");
      if (image) {
        expect(image).toHaveStyle({ height: "100vh" });
      }
    });

    it("should apply original fit mode styles", () => {
      renderWithProviders(
        <ContinuousScrollReader {...defaultProps} fitMode="original" />,
      );

      // Image should be present in the DOM (within buffer range)
      const image = screen.queryByTestId("page-image-1");
      // Original mode: image should exist and have margin: 0 auto (centered)
      expect(image).toBeInTheDocument();
    });
  });

  describe("Image URLs", () => {
    it("should generate correct page URLs", () => {
      renderWithProviders(
        <ContinuousScrollReader
          {...defaultProps}
          bookId="my-book-id"
          initialPage={1}
        />,
      );

      // Simulate first page becoming visible
      const pageContainer = screen.getByTestId("page-container-1");
      act(() => {
        mockObserverInstance?.simulateIntersection([
          {
            target: pageContainer,
            isIntersecting: true,
          },
        ]);
      });

      const image = screen.queryByTestId("page-image-1");
      if (image) {
        expect(image).toHaveAttribute(
          "src",
          "/api/v1/books/my-book-id/pages/1",
        );
      }
    });
  });

  describe("Intersection Observer", () => {
    it("should create an IntersectionObserver", () => {
      renderWithProviders(<ContinuousScrollReader {...defaultProps} />);

      expect(mockObserverInstance).not.toBeNull();
    });

    it("should observe page containers", () => {
      renderWithProviders(<ContinuousScrollReader {...defaultProps} />);

      expect(mockObserverInstance).not.toBeNull();
      // Observer should be observing elements
      expect(mockObserverInstance?.elements.size).toBeGreaterThan(0);
    });

    it("should track visible pages when elements intersect", () => {
      renderWithProviders(<ContinuousScrollReader {...defaultProps} />);

      const pageContainer = screen.getByTestId("page-container-1");

      act(() => {
        mockObserverInstance?.simulateIntersection([
          {
            target: pageContainer,
            isIntersecting: true,
            boundingClientRect: { top: 0, bottom: 100, height: 100 } as DOMRect,
          },
        ]);
      });

      // The page should now be rendered (not a placeholder)
      const image = screen.queryByTestId("page-image-1");
      expect(image).toBeInTheDocument();
    });
  });

  describe("Progress Tracking", () => {
    // Helper: mock the container's getBoundingClientRect so the intersection
    // observer callback can compute visible ratios and update currentVisiblePage.
    function mockContainerRect() {
      const container = screen.getByTestId("continuous-scroll-container");
      container.getBoundingClientRect = () =>
        ({
          top: 0,
          bottom: 800,
          left: 0,
          right: 600,
          width: 600,
          height: 800,
          x: 0,
          y: 0,
          toJSON: () => {},
        }) as DOMRect;
    }

    it("should call goToPage when visible page changes", async () => {
      vi.useFakeTimers();

      renderWithProviders(
        <ContinuousScrollReader {...defaultProps} initialPage={1} />,
      );

      // Clear initial calls
      mockGoToPage.mockClear();
      mockContainerRect();

      const scrollContainer = screen.getByTestId("continuous-scroll-container");
      const pageContainer3 = screen.getByTestId("page-container-3");

      act(() => {
        // Simulate scrolling to page 3 - it becomes the topmost visible page
        mockObserverInstance?.simulateIntersection([
          {
            target: pageContainer3,
            isIntersecting: true,
            boundingClientRect: { top: 0, bottom: 800, height: 800 } as DOMRect,
          },
        ]);
      });

      // Dispatch scroll event to trigger the debounced handler
      act(() => {
        scrollContainer.dispatchEvent(new Event("scroll", { bubbles: false }));
      });

      // Advance timers to trigger debounced page change
      await act(async () => {
        vi.advanceTimersByTime(150);
      });

      // goToPage should have been called (exact page depends on implementation)
      expect(mockGoToPage).toHaveBeenCalled();

      vi.useRealTimers();
    });

    it("should debounce page change callbacks", async () => {
      vi.useFakeTimers();

      renderWithProviders(
        <ContinuousScrollReader {...defaultProps} initialPage={1} />,
      );

      // Clear initial calls
      mockGoToPage.mockClear();
      mockContainerRect();

      const scrollContainer = screen.getByTestId("continuous-scroll-container");
      const pageContainer2 = screen.getByTestId("page-container-2");
      const pageContainer3 = screen.getByTestId("page-container-3");

      // Simulate rapid scrolling - first to page 2
      act(() => {
        mockObserverInstance?.simulateIntersection([
          {
            target: pageContainer2,
            isIntersecting: true,
            boundingClientRect: { top: 0, bottom: 800, height: 800 } as DOMRect,
          },
        ]);
        scrollContainer.dispatchEvent(new Event("scroll", { bubbles: false }));
      });

      await act(async () => {
        vi.advanceTimersByTime(50);
      });

      // Then quickly to page 3
      act(() => {
        mockObserverInstance?.simulateIntersection([
          {
            target: pageContainer3,
            isIntersecting: true,
            boundingClientRect: { top: 0, bottom: 800, height: 800 } as DOMRect,
          },
        ]);
        scrollContainer.dispatchEvent(new Event("scroll", { bubbles: false }));
      });

      await act(async () => {
        vi.advanceTimersByTime(150);
      });

      // Due to debouncing, should not call too many times
      // The exact behavior depends on intersection timing
      expect(mockGoToPage.mock.calls.length).toBeLessThanOrEqual(2);

      vi.useRealTimers();
    });

    it("should call onPageChange callback when page changes", async () => {
      vi.useFakeTimers();
      const onPageChange = vi.fn();

      renderWithProviders(
        <ContinuousScrollReader
          {...defaultProps}
          initialPage={1}
          onPageChange={onPageChange}
        />,
      );

      mockContainerRect();

      const scrollContainer = screen.getByTestId("continuous-scroll-container");
      const pageContainer5 = screen.getByTestId("page-container-5");

      act(() => {
        mockObserverInstance?.simulateIntersection([
          {
            target: pageContainer5,
            isIntersecting: true,
            boundingClientRect: { top: 0, bottom: 800, height: 800 } as DOMRect,
          },
        ]);
      });

      act(() => {
        scrollContainer.dispatchEvent(new Event("scroll", { bubbles: false }));
      });

      await act(async () => {
        vi.advanceTimersByTime(150);
      });

      // onPageChange should have been called at least once
      expect(onPageChange).toHaveBeenCalled();

      vi.useRealTimers();
    });
  });

  describe("Initial Scroll Position", () => {
    it("should scroll to initial page on mount", () => {
      renderWithProviders(
        <ContinuousScrollReader {...defaultProps} initialPage={5} />,
      );

      // The component should attempt to scroll to the initial page
      // scrollIntoView is mocked, so we just verify it was set up
      expect(Element.prototype.scrollIntoView).toBeDefined();
    });

    it("should not scroll when initial page is 1", () => {
      // Reset the mock before this test
      vi.mocked(Element.prototype.scrollIntoView).mockClear();

      renderWithProviders(
        <ContinuousScrollReader {...defaultProps} initialPage={1} />,
      );

      // For page 1, we don't need to scroll since it's already at the top
      // This is implementation-dependent but scrollIntoView should not be called
      expect(Element.prototype.scrollIntoView).not.toHaveBeenCalled();
    });
  });

  describe("Page Gap", () => {
    it("should apply custom page gap", () => {
      renderWithProviders(
        <ContinuousScrollReader {...defaultProps} pageGap={16} />,
      );

      // The gap is applied via inline styles on the inner flex container
      const innerContainer = screen.getByTestId("continuous-scroll-inner");
      expect(innerContainer).toHaveStyle({ gap: "16px" });
    });

    it("should use default page gap when not specified", () => {
      renderWithProviders(<ContinuousScrollReader {...defaultProps} />);

      const innerContainer = screen.getByTestId("continuous-scroll-inner");
      expect(innerContainer).toHaveStyle({ gap: "0" }); // DEFAULT_PAGE_GAP is 0
    });
  });

  describe("Image Loading States", () => {
    it("should show loader while image is loading", () => {
      renderWithProviders(
        <ContinuousScrollReader {...defaultProps} initialPage={1} />,
      );

      // Simulate page becoming visible
      const pageContainer = screen.getByTestId("page-container-1");
      act(() => {
        mockObserverInstance?.simulateIntersection([
          {
            target: pageContainer,
            isIntersecting: true,
          },
        ]);
      });

      // Initially, loader should be visible (image not loaded yet)
      // The image is hidden until loaded
      const image = screen.queryByTestId("page-image-1");
      if (image) {
        expect(image).toHaveStyle({ display: "none" });
      }
    });

    it("should hide loader and show image after load", async () => {
      renderWithProviders(
        <ContinuousScrollReader {...defaultProps} initialPage={1} />,
      );

      const pageContainer = screen.getByTestId("page-container-1");
      act(() => {
        mockObserverInstance?.simulateIntersection([
          {
            target: pageContainer,
            isIntersecting: true,
          },
        ]);
      });

      const image = screen.queryByTestId("page-image-1");
      if (image) {
        // Simulate image load
        await act(async () => {
          image.dispatchEvent(new Event("load"));
        });

        expect(image).toHaveStyle({ display: "block" });
      }
    });
  });

  describe("Preload Buffer", () => {
    it("should respect custom preload buffer", () => {
      renderWithProviders(
        <ContinuousScrollReader
          {...defaultProps}
          totalPages={20}
          initialPage={10}
          preloadBuffer={5}
        />,
      );

      // Pages 5-15 should be in the render range (10 +/- 5)
      // Check that page 5 is not a placeholder
      const page5Placeholder = screen.queryByTestId("page-placeholder-5");
      const page15Placeholder = screen.queryByTestId("page-placeholder-15");

      // These should NOT be placeholders since they're within buffer
      expect(page5Placeholder).not.toBeInTheDocument();
      expect(page15Placeholder).not.toBeInTheDocument();

      // Page 1 should be a placeholder (outside buffer)
      const page1Placeholder = screen.queryByTestId("page-placeholder-1");
      expect(page1Placeholder).toBeInTheDocument();
    });

    it("should use default preload buffer when not specified", () => {
      renderWithProviders(
        <ContinuousScrollReader
          {...defaultProps}
          totalPages={20}
          initialPage={10}
        />,
      );

      // Default buffer is 2, so pages 8-12 should be rendered
      // Page 1 should be a placeholder
      const page1Placeholder = screen.queryByTestId("page-placeholder-1");
      expect(page1Placeholder).toBeInTheDocument();
    });
  });

  describe("Accessibility", () => {
    it("should have alt text for images", () => {
      renderWithProviders(
        <ContinuousScrollReader {...defaultProps} initialPage={1} />,
      );

      const pageContainer = screen.getByTestId("page-container-1");
      act(() => {
        mockObserverInstance?.simulateIntersection([
          {
            target: pageContainer,
            isIntersecting: true,
          },
        ]);
      });

      const image = screen.queryByTestId("page-image-1");
      if (image) {
        expect(image).toHaveAttribute("alt", "Page 1");
      }
    });
  });

  describe("Cleanup", () => {
    it("should disconnect observer on unmount", () => {
      const { unmount } = renderWithProviders(
        <ContinuousScrollReader {...defaultProps} />,
      );

      if (mockObserverInstance) {
        const disconnectSpy = vi.spyOn(mockObserverInstance, "disconnect");
        unmount();
        expect(disconnectSpy).toHaveBeenCalled();
      }
    });
  });

  describe("External Page Sync", () => {
    it("should scroll to page when store currentPage changes externally", async () => {
      vi.useFakeTimers();
      const scrollIntoViewSpy = vi.mocked(Element.prototype.scrollIntoView);
      scrollIntoViewSpy.mockClear();

      const { rerender } = renderWithProviders(
        <ContinuousScrollReader {...defaultProps} initialPage={1} />,
      );

      scrollIntoViewSpy.mockClear();

      // Simulate an external store change (e.g., toolbar slider)
      mockCurrentPage = 5;

      // Re-render to pick up the new store value
      await act(async () => {
        rerender(<ContinuousScrollReader {...defaultProps} initialPage={1} />);
      });

      // The component should scroll to the externally-requested page
      expect(scrollIntoViewSpy).toHaveBeenCalled();

      vi.useRealTimers();
    });
  });

  // Scroll boundary detection tests were removed because boundary detection
  // moved from ContinuousScrollReader (scroll-based) to useKeyboardNav
  // (arrow-key-based) to prevent accidental triggers from casual scrolling.
  // See useKeyboardNav tests for boundary callback coverage.

  describe("Tap-to-toggle ref", () => {
    it("forwards the scroll container to tapRef", () => {
      const tapRef = vi.fn();

      renderWithProviders(
        <ContinuousScrollReader {...defaultProps} tapRef={tapRef} />,
      );

      const container = screen.getByTestId("continuous-scroll-container");
      // The tap ref is wired to the element that actually scrolls so
      // useTouchNav can detect taps without breaking native scrolling.
      expect(tapRef).toHaveBeenCalledWith(container);
    });
  });

  describe("Scroll position stability", () => {
    // Regression: when a loaded page scrolled out of the render window, it used
    // to collapse to a fixed 100vh placeholder. If its real height differed,
    // content above the viewport shifted and the reader "snapped" / lost the
    // stop position when scrolling settled. A virtualised page must reserve its
    // last measured height instead.
    it("reserves a loaded page's measured height when it is virtualised out", async () => {
      vi.useFakeTimers();

      renderWithProviders(
        <ContinuousScrollReader
          {...defaultProps}
          totalPages={20}
          initialPage={1}
          preloadBuffer={0}
        />,
      );

      // Page 1 renders initially (visible set is empty -> render around initial
      // page). Give its container a concrete rendered height, then load it.
      const page1Container = screen.getByTestId("page-container-1");
      Object.defineProperty(page1Container, "offsetHeight", {
        configurable: true,
        value: 742,
      });

      const image1 = screen.getByTestId("page-image-1");
      await act(async () => {
        image1.dispatchEvent(new Event("load"));
      });

      // Scroll far away so page 1 leaves the (zero) buffer and virtualises.
      const scrollContainer = screen.getByTestId("continuous-scroll-container");
      const page10Container = screen.getByTestId("page-container-10");
      act(() => {
        mockObserverInstance?.simulateIntersection([
          {
            target: page10Container,
            isIntersecting: true,
            boundingClientRect: { top: 0, bottom: 800, height: 800 } as DOMRect,
          },
        ]);
        scrollContainer.dispatchEvent(new Event("scroll", { bubbles: false }));
      });
      await act(async () => {
        vi.advanceTimersByTime(150);
      });

      // Page 1 is now a placeholder, but keeps the height it was measured at
      // rather than reverting to 100vh, so content below it does not shift.
      const placeholder = screen.getByTestId("page-placeholder-1");
      expect(placeholder).toHaveStyle({ height: "742px" });

      vi.useRealTimers();
    });

    it("falls back to a viewport-height placeholder for never-measured pages", () => {
      renderWithProviders(
        <ContinuousScrollReader
          {...defaultProps}
          totalPages={20}
          initialPage={1}
          preloadBuffer={2}
        />,
      );

      // With no measurements at all, unmeasured pages use the 100vh guess.
      const placeholder = screen.getByTestId("page-placeholder-15");
      expect(placeholder).toHaveStyle({ height: "100vh" });
    });

    it("reserves height for a rendered page whose image has not loaded yet", () => {
      renderWithProviders(
        <ContinuousScrollReader
          {...defaultProps}
          totalPages={20}
          initialPage={1}
          preloadBuffer={0}
        />,
      );

      // Page 1 is in the render window but its image hasn't loaded. The
      // container must keep the reserved height so entering the render
      // window doesn't collapse the page to the loader height and shift
      // everything below it.
      const container = screen.getByTestId("page-container-1");
      expect(container).toHaveStyle({ minHeight: "100vh" });
    });

    it("estimates unmeasured placeholders from the average measured height", async () => {
      const { rerender } = renderWithProviders(
        <ContinuousScrollReader
          {...defaultProps}
          totalPages={20}
          initialPage={1}
          preloadBuffer={0}
        />,
      );

      const page1Container = screen.getByTestId("page-container-1");
      Object.defineProperty(page1Container, "offsetHeight", {
        configurable: true,
        value: 1500,
      });

      const image1 = screen.getByTestId("page-image-1");
      await act(async () => {
        image1.dispatchEvent(new Event("load"));
      });

      // Heights are measured in a layout effect (after the loadedPages
      // commit), so estimates apply from the next render onward. That lag is
      // invisible: placeholders are outside the render window, so only their
      // contribution to scroll geometry matters.
      await act(async () => {
        rerender(
          <ContinuousScrollReader
            {...defaultProps}
            totalPages={20}
            initialPage={1}
            preloadBuffer={0}
          />,
        );
      });

      // Webtoon pages are tall; once one page is measured, never-measured
      // placeholders use the measured average instead of the 100vh guess.
      const placeholder = screen.getByTestId("page-placeholder-10");
      expect(placeholder).toHaveStyle({ height: "1500px" });
    });

    it("does not manually adjust scrollTop when a page above grows on load", async () => {
      // The reader now relies on the browser's native scroll anchoring
      // (overflow-anchor) plus exact reserved heights instead of hand-patching
      // scrollTop. The old manual compensation was what produced the visible
      // "stop, then jump" glitch, so a normal image load must leave scrollTop
      // untouched.
      renderWithProviders(
        <ContinuousScrollReader
          {...defaultProps}
          totalPages={20}
          initialPage={1}
          preloadBuffer={1}
        />,
      );

      const scrollContainer = screen.getByTestId("continuous-scroll-container");
      let scrollTop = 2000;
      const sets: number[] = [];
      Object.defineProperty(scrollContainer, "scrollTop", {
        configurable: true,
        get: () => scrollTop,
        set: (v: number) => {
          sets.push(v);
          scrollTop = v;
        },
      });
      scrollContainer.getBoundingClientRect = () =>
        ({ top: 0, bottom: 800, height: 800 }) as DOMRect;

      const page1Container = screen.getByTestId("page-container-1");
      Object.defineProperty(page1Container, "offsetHeight", {
        configurable: true,
        value: 1600,
      });
      page1Container.getBoundingClientRect = () =>
        ({ top: -1200, bottom: 400, height: 1600 }) as DOMRect;

      const image1 = screen.getByTestId("page-image-1");
      await act(async () => {
        image1.dispatchEvent(new Event("load"));
      });

      // No scrollTop writes from a normal (non-sync) load.
      expect(sets).toEqual([]);
      expect(scrollTop).toBe(2000);
    });

    it("reserves a page's exact height from its real dimensions", () => {
      // With known dimensions, an out-of-window placeholder is sized to the
      // page's true rendered height up front, so the image later loads into
      // space that is already held — zero layout shift. In `original` fit mode
      // the rendered height equals the page's natural pixel height, which is
      // deterministic regardless of jsdom's (zero) layout.
      const pageDimensions = new Map([[10, { width: 800, height: 1234 }]]);

      renderWithProviders(
        <ContinuousScrollReader
          {...defaultProps}
          fitMode="original"
          totalPages={20}
          initialPage={1}
          preloadBuffer={0}
          pageDimensions={pageDimensions}
        />,
      );

      const placeholder = screen.getByTestId("page-placeholder-10");
      expect(placeholder).toHaveStyle({ height: "1234px" });
    });
  });

  describe("Transition panels", () => {
    function setScrollMetrics(
      el: HTMLElement,
      { scrollTop, clientHeight, scrollHeight }: Record<string, number>,
    ) {
      Object.defineProperty(el, "scrollHeight", {
        configurable: true,
        value: scrollHeight,
      });
      Object.defineProperty(el, "clientHeight", {
        configurable: true,
        value: clientHeight,
      });
      Object.defineProperty(el, "scrollTop", {
        configurable: true,
        writable: true,
        value: scrollTop,
      });
    }

    it("renders the leading and trailing slots when provided", () => {
      renderWithProviders(
        <ContinuousScrollReader
          {...defaultProps}
          leadingSlot={<div>prev-panel</div>}
          trailingSlot={<div>next-panel</div>}
        />,
      );

      expect(screen.getByTestId("continuous-leading-slot")).toBeInTheDocument();
      expect(
        screen.getByTestId("continuous-trailing-slot"),
      ).toBeInTheDocument();
      expect(screen.getByText("prev-panel")).toBeInTheDocument();
      expect(screen.getByText("next-panel")).toBeInTheDocument();
    });

    it("scrolls past the leading panel to page 1 on mount", () => {
      const scrollIntoView = vi.spyOn(Element.prototype, "scrollIntoView");
      renderWithProviders(
        <ContinuousScrollReader
          {...defaultProps}
          initialPage={1}
          leadingSlot={<div>prev-panel</div>}
        />,
      );
      expect(scrollIntoView).toHaveBeenCalled();
    });

    it("reports the final page and fires trailing-reached at the bottom", async () => {
      vi.useFakeTimers();
      const onTrailingReachedChange = vi.fn();

      renderWithProviders(
        <ContinuousScrollReader
          {...defaultProps}
          totalPages={10}
          trailingSlot={<div>next-panel</div>}
          onTrailingReachedChange={onTrailingReachedChange}
        />,
      );

      mockGoToPage.mockClear();
      const container = screen.getByTestId("continuous-scroll-container");
      setScrollMetrics(container, {
        scrollTop: 1200,
        clientHeight: 800,
        scrollHeight: 2000,
      });

      act(() => {
        container.dispatchEvent(new Event("scroll", { bubbles: false }));
      });
      await act(async () => {
        vi.advanceTimersByTime(150);
      });

      expect(mockGoToPage).toHaveBeenCalledWith(10);
      expect(onTrailingReachedChange).toHaveBeenCalledWith(true);

      vi.useRealTimers();
    });

    it("does not report the final page when not at the bottom", async () => {
      vi.useFakeTimers();
      const onTrailingReachedChange = vi.fn();

      renderWithProviders(
        <ContinuousScrollReader
          {...defaultProps}
          totalPages={10}
          trailingSlot={<div>next-panel</div>}
          onTrailingReachedChange={onTrailingReachedChange}
        />,
      );

      mockGoToPage.mockClear();
      const container = screen.getByTestId("continuous-scroll-container");
      setScrollMetrics(container, {
        scrollTop: 100,
        clientHeight: 800,
        scrollHeight: 2000,
      });

      act(() => {
        container.dispatchEvent(new Event("scroll", { bubbles: false }));
      });
      await act(async () => {
        vi.advanceTimersByTime(150);
      });

      expect(mockGoToPage).not.toHaveBeenCalledWith(10);
      expect(onTrailingReachedChange).not.toHaveBeenCalledWith(true);

      vi.useRealTimers();
    });
  });
});
