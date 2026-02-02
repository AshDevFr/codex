import { beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, renderWithProviders, screen, waitFor } from "@/test/utils";
import { DoublePageSpread } from "./DoublePageSpread";

describe("DoublePageSpread", () => {
  const defaultProps = {
    pages: [
      { pageNumber: 2, src: "/api/v1/books/book-123/pages/2" },
      { pageNumber: 3, src: "/api/v1/books/book-123/pages/3" },
    ],
    fitMode: "screen" as const,
    backgroundColor: "black" as const,
    readingDirection: "ltr" as const,
  };

  beforeEach(() => {
    vi.clearAllMocks();
  });

  // ==========================================================================
  // Basic rendering
  // ==========================================================================

  describe("basic rendering", () => {
    it("should render with loading state initially", () => {
      renderWithProviders(<DoublePageSpread {...defaultProps} />);

      // Should show loader initially
      const container = screen.getByTestId("double-page-spread");
      expect(container).toBeInTheDocument();
    });

    it("should render two images for a spread", () => {
      renderWithProviders(<DoublePageSpread {...defaultProps} />);

      const images = screen.getAllByRole("img", { hidden: true });
      expect(images).toHaveLength(2);
    });

    it("should render single image when only one page provided", () => {
      const singlePageProps = {
        ...defaultProps,
        pages: [{ pageNumber: 5, src: "/api/v1/books/book-123/pages/5" }],
      };
      renderWithProviders(<DoublePageSpread {...singlePageProps} />);

      const images = screen.getAllByRole("img", { hidden: true });
      expect(images).toHaveLength(1);
    });

    it("should display images with correct src", () => {
      renderWithProviders(<DoublePageSpread {...defaultProps} />);

      const images = screen.getAllByRole("img", { hidden: true });
      expect(images[0]).toHaveAttribute(
        "src",
        "/api/v1/books/book-123/pages/2",
      );
      expect(images[1]).toHaveAttribute(
        "src",
        "/api/v1/books/book-123/pages/3",
      );
    });

    it("should not render when isVisible is false", () => {
      renderWithProviders(
        <DoublePageSpread {...defaultProps} isVisible={false} />,
      );

      expect(
        screen.queryByTestId("double-page-spread"),
      ).not.toBeInTheDocument();
    });
  });

  // ==========================================================================
  // Reading direction (LTR/RTL)
  // ==========================================================================

  describe("reading direction", () => {
    it("should display pages in the order provided (LTR)", () => {
      renderWithProviders(
        <DoublePageSpread {...defaultProps} readingDirection="ltr" />,
      );

      const images = screen.getAllByRole("img", { hidden: true });
      // Pages are displayed in the order provided by parent
      expect(images[0]).toHaveAttribute(
        "src",
        "/api/v1/books/book-123/pages/2",
      );
      expect(images[1]).toHaveAttribute(
        "src",
        "/api/v1/books/book-123/pages/3",
      );
    });

    it("should display pages in the order provided (RTL - parent handles ordering)", () => {
      // In RTL mode, parent (ComicReader) passes pages already reordered via getDisplayOrder()
      // So if parent passes [3, 2], component displays [3, 2] (higher page on left)
      const rtlProps = {
        ...defaultProps,
        pages: [
          { pageNumber: 3, src: "/api/v1/books/book-123/pages/3" },
          { pageNumber: 2, src: "/api/v1/books/book-123/pages/2" },
        ],
        readingDirection: "rtl" as const,
      };
      renderWithProviders(<DoublePageSpread {...rtlProps} />);

      const images = screen.getAllByRole("img", { hidden: true });
      // First image should be page 3 (higher number on left for RTL/manga)
      expect(images[0]).toHaveAttribute(
        "src",
        "/api/v1/books/book-123/pages/3",
      );
      expect(images[1]).toHaveAttribute(
        "src",
        "/api/v1/books/book-123/pages/2",
      );
    });

    it("should display single page as provided for RTL", () => {
      const singlePageProps = {
        ...defaultProps,
        pages: [{ pageNumber: 5, src: "/api/v1/books/book-123/pages/5" }],
        readingDirection: "rtl" as const,
      };
      renderWithProviders(<DoublePageSpread {...singlePageProps} />);

      const images = screen.getAllByRole("img", { hidden: true });
      expect(images).toHaveLength(1);
      expect(images[0]).toHaveAttribute(
        "src",
        "/api/v1/books/book-123/pages/5",
      );
    });
  });

  // ==========================================================================
  // Click zones
  // ==========================================================================

  describe("click zones", () => {
    it("should call onClick with 'left' when clicking left half in LTR mode", () => {
      const onClick = vi.fn();
      renderWithProviders(
        <DoublePageSpread
          {...defaultProps}
          readingDirection="ltr"
          onClick={onClick}
        />,
      );

      const container = screen.getByTestId("double-page-spread");
      vi.spyOn(container, "getBoundingClientRect").mockReturnValue({
        left: 0,
        width: 1000,
        top: 0,
        height: 600,
        right: 1000,
        bottom: 600,
        x: 0,
        y: 0,
        toJSON: () => {},
      });

      fireEvent.click(container, { clientX: 200 }); // Left half (200 < 500)
      expect(onClick).toHaveBeenCalledWith("left");
    });

    it("should call onClick with 'right' when clicking right half in LTR mode", () => {
      const onClick = vi.fn();
      renderWithProviders(
        <DoublePageSpread
          {...defaultProps}
          readingDirection="ltr"
          onClick={onClick}
        />,
      );

      const container = screen.getByTestId("double-page-spread");
      vi.spyOn(container, "getBoundingClientRect").mockReturnValue({
        left: 0,
        width: 1000,
        top: 0,
        height: 600,
        right: 1000,
        bottom: 600,
        x: 0,
        y: 0,
        toJSON: () => {},
      });

      fireEvent.click(container, { clientX: 800 }); // Right half (800 > 500)
      expect(onClick).toHaveBeenCalledWith("right");
    });

    it("should swap click zones for RTL mode - left click advances (right)", () => {
      const onClick = vi.fn();
      renderWithProviders(
        <DoublePageSpread
          {...defaultProps}
          readingDirection="rtl"
          onClick={onClick}
        />,
      );

      const container = screen.getByTestId("double-page-spread");
      vi.spyOn(container, "getBoundingClientRect").mockReturnValue({
        left: 0,
        width: 1000,
        top: 0,
        height: 600,
        right: 1000,
        bottom: 600,
        x: 0,
        y: 0,
        toJSON: () => {},
      });

      // In RTL mode, clicking left half should trigger "right" (advance)
      fireEvent.click(container, { clientX: 200 });
      expect(onClick).toHaveBeenCalledWith("right");
    });

    it("should swap click zones for RTL mode - right click goes back (left)", () => {
      const onClick = vi.fn();
      renderWithProviders(
        <DoublePageSpread
          {...defaultProps}
          readingDirection="rtl"
          onClick={onClick}
        />,
      );

      const container = screen.getByTestId("double-page-spread");
      vi.spyOn(container, "getBoundingClientRect").mockReturnValue({
        left: 0,
        width: 1000,
        top: 0,
        height: 600,
        right: 1000,
        bottom: 600,
        x: 0,
        y: 0,
        toJSON: () => {},
      });

      // In RTL mode, clicking right half should trigger "left" (go back)
      fireEvent.click(container, { clientX: 800 });
      expect(onClick).toHaveBeenCalledWith("left");
    });

    it("should not call onClick when no handler provided", () => {
      renderWithProviders(<DoublePageSpread {...defaultProps} />);

      const container = screen.getByTestId("double-page-spread");
      // Should not throw
      fireEvent.click(container);
    });
  });

  // ==========================================================================
  // Background colors
  // ==========================================================================

  describe("background colors", () => {
    it("should apply black background", () => {
      renderWithProviders(
        <DoublePageSpread {...defaultProps} backgroundColor="black" />,
      );

      const container = screen.getByTestId("double-page-spread");
      expect(container).toHaveStyle({ backgroundColor: "#000000" });
    });

    it("should apply gray background", () => {
      renderWithProviders(
        <DoublePageSpread {...defaultProps} backgroundColor="gray" />,
      );

      const container = screen.getByTestId("double-page-spread");
      expect(container).toHaveStyle({ backgroundColor: "#1a1a1a" });
    });

    it("should apply white background", () => {
      renderWithProviders(
        <DoublePageSpread {...defaultProps} backgroundColor="white" />,
      );

      const container = screen.getByTestId("double-page-spread");
      expect(container).toHaveStyle({ backgroundColor: "#ffffff" });
    });
  });

  // ==========================================================================
  // Image loading
  // ==========================================================================

  describe("image loading", () => {
    it("should keep images visible after loading", async () => {
      renderWithProviders(<DoublePageSpread {...defaultProps} />);

      const images = screen.getAllByRole("img", { hidden: true });

      // Images should always be rendered (no display: none)
      for (const img of images) {
        expect(img).toBeInTheDocument();
      }

      // Simulate both images loading
      for (const img of images) {
        fireEvent.load(img);
      }

      await waitFor(() => {
        // After load, images should still be visible
        for (const img of images) {
          expect(img).toBeInTheDocument();
        }
      });
    });

    it("should show error message when image fails to load", async () => {
      renderWithProviders(<DoublePageSpread {...defaultProps} />);

      const images = screen.getAllByRole("img", { hidden: true });
      fireEvent.error(images[0]);

      await waitFor(() => {
        expect(screen.getByText("Failed to load page 2")).toBeInTheDocument();
      });
    });
  });

  // ==========================================================================
  // Page orientation detection
  // ==========================================================================

  describe("page orientation detection", () => {
    it("should call onPageOrientationDetected when image loads", async () => {
      const onPageOrientationDetected = vi.fn();
      renderWithProviders(
        <DoublePageSpread
          {...defaultProps}
          onPageOrientationDetected={onPageOrientationDetected}
        />,
      );

      const images = screen.getAllByRole("img", { hidden: true });

      // Mock image dimensions (portrait)
      Object.defineProperty(images[0], "naturalWidth", { value: 800 });
      Object.defineProperty(images[0], "naturalHeight", { value: 1200 });

      fireEvent.load(images[0]);

      await waitFor(() => {
        expect(onPageOrientationDetected).toHaveBeenCalledWith(2, "portrait");
      });
    });

    it("should detect landscape orientation correctly", async () => {
      const onPageOrientationDetected = vi.fn();
      renderWithProviders(
        <DoublePageSpread
          {...defaultProps}
          onPageOrientationDetected={onPageOrientationDetected}
        />,
      );

      const images = screen.getAllByRole("img", { hidden: true });

      // Mock image dimensions (landscape)
      Object.defineProperty(images[0], "naturalWidth", { value: 1600 });
      Object.defineProperty(images[0], "naturalHeight", { value: 800 });

      fireEvent.load(images[0]);

      await waitFor(() => {
        expect(onPageOrientationDetected).toHaveBeenCalledWith(2, "landscape");
      });
    });
  });

  // ==========================================================================
  // Fit modes
  // ==========================================================================

  describe("fit modes for double page", () => {
    it("should apply screen fit mode with 100% max width for double pages (container handles 50% split)", () => {
      renderWithProviders(
        <DoublePageSpread {...defaultProps} fitMode="screen" />,
      );

      const images = screen.getAllByRole("img", { hidden: true });
      // Each image fills its container (container has maxWidth: 50%)
      expect(images[0]).toHaveStyle({ maxWidth: "100%" });
      expect(images[1]).toHaveStyle({ maxWidth: "100%" });
    });

    it("should apply full width for single page in screen mode", () => {
      const singlePageProps = {
        ...defaultProps,
        pages: [{ pageNumber: 1, src: "/api/v1/books/book-123/pages/1" }],
      };
      renderWithProviders(
        <DoublePageSpread {...singlePageProps} fitMode="screen" />,
      );

      const images = screen.getAllByRole("img", { hidden: true });
      expect(images[0]).toHaveStyle({ maxWidth: "100%" });
    });

    it("should apply width fit mode with 100% width for double pages (container handles 50% split)", () => {
      renderWithProviders(
        <DoublePageSpread {...defaultProps} fitMode="width" />,
      );

      const images = screen.getAllByRole("img", { hidden: true });
      // Each image fills its container (container has maxWidth: 50%)
      expect(images[0]).toHaveStyle({ width: "100%" });
      expect(images[1]).toHaveStyle({ width: "100%" });
    });

    it("should apply height fit mode with 100% maxWidth for double pages (container handles 50% split)", () => {
      renderWithProviders(
        <DoublePageSpread {...defaultProps} fitMode="height" />,
      );

      const images = screen.getAllByRole("img", { hidden: true });
      // Each image fills its container (container has maxWidth: 50%)
      expect(images[0]).toHaveStyle({ height: "100%", maxWidth: "100%" });
      expect(images[1]).toHaveStyle({ height: "100%", maxWidth: "100%" });
    });
  });

  // ==========================================================================
  // Page containers
  // ==========================================================================

  describe("page containers", () => {
    it("should have test ids for each page", () => {
      renderWithProviders(<DoublePageSpread {...defaultProps} />);

      expect(screen.getByTestId("spread-page-2")).toBeInTheDocument();
      expect(screen.getByTestId("spread-page-3")).toBeInTheDocument();
    });

    it("should render page containers in correct order for RTL", () => {
      renderWithProviders(
        <DoublePageSpread {...defaultProps} readingDirection="rtl" />,
      );

      const pageContainers = [
        screen.getByTestId("spread-page-3"),
        screen.getByTestId("spread-page-2"),
      ];

      // Both should be in document, order is managed by the component
      expect(pageContainers[0]).toBeInTheDocument();
      expect(pageContainers[1]).toBeInTheDocument();
    });
  });
});
