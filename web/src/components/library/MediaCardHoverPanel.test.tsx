import { describe, expect, it } from "vitest";
import { createBook, createSeries } from "@/mocks/data/factories";
import { renderWithProviders, screen } from "@/test/utils";
import { MediaCardHoverPanel } from "./MediaCardHoverPanel";

describe("MediaCardHoverPanel", () => {
  describe("series", () => {
    it("shows the title, a volumes/chapters count line, and the summary", () => {
      const series = createSeries({
        title: "Berserk",
        summary: "Guts, a lone mercenary, wanders a brutal medieval world.",
        bookCount: 41,
        localMaxVolume: 41,
        localMaxChapter: 364.5,
      });

      renderWithProviders(
        <MediaCardHoverPanel type="series" title="Berserk" data={series} />,
      );

      expect(screen.getByText("Berserk")).toBeInTheDocument();
      expect(screen.getByText("41 vol · 364.5 ch")).toBeInTheDocument();
      expect(screen.getByText(/Guts, a lone mercenary/)).toBeInTheDocument();
    });

    it("omits the description block when the summary is empty or a placeholder", () => {
      const series = createSeries({
        title: "No Summary",
        summary: "-",
        bookCount: 3,
        localMaxVolume: null,
        localMaxChapter: null,
      });

      renderWithProviders(
        <MediaCardHoverPanel type="series" title="No Summary" data={series} />,
      );

      expect(screen.getByText("3 books")).toBeInTheDocument();
      // The placeholder "-" must not be rendered as a description.
      expect(screen.queryByText("-")).not.toBeInTheDocument();
    });
  });

  describe("book", () => {
    it("shows the title, series name, and a metadata line", () => {
      const book = createBook({
        title: "Volume 3",
        seriesName: "Chainsaw Man",
        volume: 3,
        chapter: null,
        pageCount: 192,
        fileFormat: "cbz",
      });

      renderWithProviders(
        <MediaCardHoverPanel type="book" title="3 - Volume 3" data={book} />,
      );

      expect(screen.getByText("3 - Volume 3")).toBeInTheDocument();
      expect(screen.getByText("Chainsaw Man")).toBeInTheDocument();
      expect(screen.getByText(/Vol 3/)).toBeInTheDocument();
      expect(screen.getByText(/192 pages/)).toBeInTheDocument();
      expect(screen.getByText(/CBZ/)).toBeInTheDocument();
    });

    it("shows the book summary when present", () => {
      const book = createBook({
        title: "Standalone",
        seriesName: "Some Series",
        summary: "A self-contained story with its own arc.",
      });

      renderWithProviders(
        <MediaCardHoverPanel type="book" title="Standalone" data={book} />,
      );

      expect(
        screen.getByText("A self-contained story with its own arc."),
      ).toBeInTheDocument();
    });

    it("omits the description block when the book has no summary", () => {
      const book = createBook({
        title: "No Blurb",
        seriesName: "Some Series",
        summary: null,
      });

      renderWithProviders(
        <MediaCardHoverPanel type="book" title="No Blurb" data={book} />,
      );

      expect(screen.getByText("No Blurb")).toBeInTheDocument();
      expect(screen.queryByText(/self-contained/)).not.toBeInTheDocument();
    });
  });
});
