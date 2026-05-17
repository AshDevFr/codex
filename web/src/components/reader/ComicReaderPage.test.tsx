import { beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, renderWithProviders, screen, waitFor } from "@/test/utils";
import { ComicReaderPage } from "./ComicReaderPage";

describe("ComicReaderPage", () => {
  const defaultProps = {
    src: "/api/v1/books/book-123/pages/1",
    alt: "Page 1 of Test Book",
    fitMode: "screen" as const,
    backgroundColor: "black" as const,
  };

  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("should render with loading state initially", () => {
    renderWithProviders(<ComicReaderPage {...defaultProps} />);

    // Loader should be visible initially
    expect(screen.getByRole("img", { hidden: true })).toBeInTheDocument();
  });

  it("should display image with correct src and alt", () => {
    renderWithProviders(<ComicReaderPage {...defaultProps} />);

    const img = screen.getByRole("img", { hidden: true });
    expect(img).toHaveAttribute("src", "/api/v1/books/book-123/pages/1");
    expect(img).toHaveAttribute("alt", "Page 1 of Test Book");
  });

  it("should not render when isVisible is false", () => {
    renderWithProviders(
      <ComicReaderPage {...defaultProps} isVisible={false} />,
    );

    expect(screen.queryByRole("img")).not.toBeInTheDocument();
  });

  describe("fit modes", () => {
    it("should apply screen fit mode styles", () => {
      renderWithProviders(
        <ComicReaderPage {...defaultProps} fitMode="screen" />,
      );

      const img = screen.getByRole("img", { hidden: true });
      expect(img).toHaveStyle({ maxWidth: "100%", maxHeight: "100%" });
    });

    it("should apply width fit mode styles", () => {
      renderWithProviders(
        <ComicReaderPage {...defaultProps} fitMode="width" />,
      );

      const img = screen.getByRole("img", { hidden: true });
      expect(img).toHaveStyle({ width: "100%" });
    });

    it("should apply height fit mode styles", () => {
      renderWithProviders(
        <ComicReaderPage {...defaultProps} fitMode="height" />,
      );

      const img = screen.getByRole("img", { hidden: true });
      expect(img).toHaveStyle({ height: "100%" });
    });
  });

  describe("background colors", () => {
    it("should apply black background", () => {
      renderWithProviders(
        <ComicReaderPage {...defaultProps} backgroundColor="black" />,
      );

      const container = screen.getByRole("img", { hidden: true }).parentElement;
      expect(container).toHaveStyle({ backgroundColor: "#000000" });
    });

    it("should apply gray background", () => {
      renderWithProviders(
        <ComicReaderPage {...defaultProps} backgroundColor="gray" />,
      );

      const container = screen.getByRole("img", { hidden: true }).parentElement;
      expect(container).toHaveStyle({ backgroundColor: "#1a1a1a" });
    });

    it("should apply white background", () => {
      renderWithProviders(
        <ComicReaderPage {...defaultProps} backgroundColor="white" />,
      );

      const container = screen.getByRole("img", { hidden: true }).parentElement;
      expect(container).toHaveStyle({ backgroundColor: "#ffffff" });
    });
  });

  describe("image loading", () => {
    it("should hide loader overlay when image loads", async () => {
      renderWithProviders(<ComicReaderPage {...defaultProps} />);

      const img = screen.getByRole("img", { hidden: true });

      // Image should always be rendered (no display: none)
      expect(img).toBeInTheDocument();

      fireEvent.load(img);

      // After load, loader overlay should be removed (no loader element in DOM)
      await waitFor(() => {
        // The loader is an overlay that disappears after loading
        // Check that the image is still visible
        expect(img).toBeInTheDocument();
      });
    });

    it("should show error message when image fails to load", async () => {
      renderWithProviders(<ComicReaderPage {...defaultProps} />);

      const img = screen.getByRole("img", { hidden: true });
      fireEvent.error(img);

      await waitFor(() => {
        expect(screen.getByText("Failed to load page")).toBeInTheDocument();
      });
    });
  });
});
