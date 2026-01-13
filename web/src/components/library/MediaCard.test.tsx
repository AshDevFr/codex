import { createBook } from "@/mocks/data/factories";
import { renderWithProviders, screen } from "@/test/utils";
import { describe, expect, it, vi } from "vitest";
import { MediaCard } from "./MediaCard";

// Mock the API modules
vi.mock("@/api/books", () => ({
	booksApi: {
		analyze: vi.fn(),
		markAsRead: vi.fn(),
		markAsUnread: vi.fn(),
	},
}));

vi.mock("@/api/series", () => ({
	seriesApi: {
		analyze: vi.fn(),
		analyzeUnanalyzed: vi.fn(),
		markAsRead: vi.fn(),
		markAsUnread: vi.fn(),
	},
}));

describe("MediaCard", () => {
	describe("book display", () => {
		it("should display cover image for non-deleted book", () => {
			const book = createBook({ deleted: false });

			renderWithProviders(<MediaCard type="book" data={book} />);

			// Should have an image element with the thumbnail URL
			const image = screen.getByRole("img");
			expect(image).toBeInTheDocument();
			expect(image).toHaveAttribute(
				"src",
				expect.stringContaining(`/api/v1/books/${book.id}/thumbnail`),
			);
		});

		it("should display deleted placeholder for deleted book", () => {
			const book = createBook({ deleted: true });

			renderWithProviders(<MediaCard type="book" data={book} />);

			// Should show "Deleted" text instead of cover image
			expect(screen.getByText("Deleted")).toBeInTheDocument();

			// Should not have an image element for the thumbnail
			const images = screen.queryAllByRole("img");
			// There should be no images (deleted placeholder uses icon, not img)
			expect(images).toHaveLength(0);
		});

		it("should display book title", () => {
			const book = createBook({ title: "Test Book Title", number: 5 });

			renderWithProviders(<MediaCard type="book" data={book} />);

			expect(screen.getByText("5 - Test Book Title")).toBeInTheDocument();
		});

		it("should display series name for book", () => {
			const book = createBook({ seriesName: "Test Series Name" });

			renderWithProviders(<MediaCard type="book" data={book} />);

			expect(screen.getByText("Test Series Name")).toBeInTheDocument();
		});

		it("should display page count and file format", () => {
			const book = createBook({
				pageCount: 42,
				fileFormat: "cbz",
			});

			renderWithProviders(<MediaCard type="book" data={book} />);

			expect(screen.getByText("42 pages")).toBeInTheDocument();
			expect(screen.getByText("CBZ")).toBeInTheDocument();
		});
	});
});
