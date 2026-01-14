import { describe, expect, it, vi } from "vitest";
import { createBook, createReadProgress } from "@/mocks/data/factories";
import { renderWithProviders, screen, userEvent } from "@/test/utils";
import { MediaCard } from "./MediaCard";

const mockNavigate = vi.fn();

// Mock react-router-dom
vi.mock("react-router-dom", async () => {
	const actual = await vi.importActual("react-router-dom");
	return {
		...actual,
		useNavigate: () => mockNavigate,
	};
});

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

	describe("hover overlay and progress", () => {
		beforeEach(() => {
			mockNavigate.mockClear();
		});

		it("should display read button overlay for non-deleted books", () => {
			const book = createBook({ deleted: false });

			renderWithProviders(<MediaCard type="book" data={book} />);

			// Read button should be present (even if hidden by CSS)
			const readButton = screen.getByRole("button", { name: "Read book" });
			expect(readButton).toBeInTheDocument();
		});

		it("should not display read button overlay for deleted books", () => {
			const book = createBook({ deleted: true });

			renderWithProviders(<MediaCard type="book" data={book} />);

			// Read button should not be present for deleted books
			expect(
				screen.queryByRole("button", { name: "Read book" }),
			).not.toBeInTheDocument();
		});

		it("should navigate to reader on read button click", async () => {
			const user = userEvent.setup();
			const book = createBook({ id: "book-123", deleted: false });

			renderWithProviders(<MediaCard type="book" data={book} />);

			const readButton = screen.getByRole("button", { name: "Read book" });
			await user.click(readButton);

			expect(mockNavigate).toHaveBeenCalledWith("/reader/book-123?page=1");
		});

		it("should navigate to reader with current page when book has progress", async () => {
			const user = userEvent.setup();
			const progress = createReadProgress({
				current_page: 15,
				completed: false,
			});
			const book = createBook({
				id: "book-456",
				deleted: false,
				readProgress: progress,
			});

			renderWithProviders(<MediaCard type="book" data={book} />);

			const readButton = screen.getByRole("button", { name: "Read book" });
			await user.click(readButton);

			expect(mockNavigate).toHaveBeenCalledWith("/reader/book-456?page=15");
		});

		it("should display progress bar for books with in-progress reading", () => {
			const progress = createReadProgress({
				current_page: 10,
				completed: false,
			});
			const book = createBook({ pageCount: 50, readProgress: progress });

			renderWithProviders(<MediaCard type="book" data={book} />);

			// Progress bar should be present
			const progressBar = document.querySelector('[role="progressbar"]');
			expect(progressBar).toBeInTheDocument();
			expect(progressBar).toHaveAttribute("aria-valuenow", "20"); // 10/50 = 20%
		});

		it("should not display progress bar for completed books", () => {
			const progress = createReadProgress({
				current_page: 50,
				completed: true,
			});
			const book = createBook({ pageCount: 50, readProgress: progress });

			renderWithProviders(<MediaCard type="book" data={book} />);

			// Progress bar should not be present for completed books
			const progressBar = document.querySelector('[role="progressbar"]');
			expect(progressBar).not.toBeInTheDocument();
		});

		it("should not display progress bar for books without progress", () => {
			const book = createBook({ readProgress: null });

			renderWithProviders(<MediaCard type="book" data={book} />);

			// Progress bar should not be present
			const progressBar = document.querySelector('[role="progressbar"]');
			expect(progressBar).not.toBeInTheDocument();
		});
	});
});
