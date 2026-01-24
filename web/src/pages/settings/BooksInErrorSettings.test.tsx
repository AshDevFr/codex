import { screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { BooksWithErrorsResponse } from "@/api/books";
import { booksApi } from "@/api/books";
import { renderWithProviders } from "@/test/utils";
import { BooksInErrorSettings } from "./BooksInErrorSettings";

// Mock the books API
vi.mock("@/api/books", () => ({
	booksApi: {
		getBooksWithErrors: vi.fn(),
		retryBookErrors: vi.fn(),
		retryAllErrors: vi.fn(),
	},
}));

// Mock useTaskProgress hook
vi.mock("@/hooks/useTaskProgress", () => ({
	useTaskProgress: () => ({
		activeTasks: [],
	}),
}));

// Helper to create a mock book with all required properties
const createMockBook = (overrides: {
	id: string;
	title: string;
	seriesId: string;
	seriesName: string;
	fileFormat: string;
	pageCount: number;
}) => ({
	...overrides,
	deleted: false,
	fileHash: `hash-${overrides.id}`,
	filePath: `/path/to/${overrides.title}.${overrides.fileFormat}`,
	fileSize: 1024 * 1024 * 10, // 10 MB
	libraryId: "library-1",
	libraryName: "Test Library",
	createdAt: "2024-01-01T00:00:00Z",
	updatedAt: "2024-01-01T00:00:00Z",
});

// Default mock data with errors
const mockErrorsData: BooksWithErrorsResponse = {
	totalBooksWithErrors: 3,
	totalPages: 1,
	page: 0,
	pageSize: 100,
	errorCounts: {
		parser: 2,
		thumbnail: 1,
	},
	groups: [
		{
			errorType: "parser",
			label: "Parser Error",
			count: 2,
			books: [
				{
					book: createMockBook({
						id: "book-1",
						title: "Test Book 1",
						seriesId: "series-1",
						seriesName: "Test Series",
						fileFormat: "cbz",
						pageCount: 100,
					}),
					errors: [
						{
							errorType: "parser",
							message: "Failed to parse archive",
							occurredAt: "2024-01-01T00:00:00Z",
						},
					],
				},
				{
					book: createMockBook({
						id: "book-2",
						title: "Test Book 2",
						seriesId: "series-1",
						seriesName: "Test Series",
						fileFormat: "cbr",
						pageCount: 50,
					}),
					errors: [
						{
							errorType: "parser",
							message: "Invalid RAR format",
							occurredAt: "2024-01-01T00:00:00Z",
						},
					],
				},
			],
		},
		{
			errorType: "thumbnail",
			label: "Thumbnail Error",
			count: 1,
			books: [
				{
					book: createMockBook({
						id: "book-3",
						title: "Test Book 3",
						seriesId: "series-2",
						seriesName: "Another Series",
						fileFormat: "pdf",
						pageCount: 200,
					}),
					errors: [
						{
							errorType: "thumbnail",
							message: "Failed to generate thumbnail",
							occurredAt: "2024-01-01T00:00:00Z",
						},
					],
				},
			],
		},
	],
};

// Empty state mock data
const emptyErrorsData: BooksWithErrorsResponse = {
	totalBooksWithErrors: 0,
	totalPages: 0,
	page: 0,
	pageSize: 100,
	errorCounts: {},
	groups: [],
};

describe("BooksInErrorSettings", () => {
	beforeEach(() => {
		vi.clearAllMocks();
		// Default mock implementation
		vi.mocked(booksApi.getBooksWithErrors).mockResolvedValue(mockErrorsData);
		vi.mocked(booksApi.retryBookErrors).mockResolvedValue({
			tasksEnqueued: 1,
			message: "1 task enqueued",
		});
		vi.mocked(booksApi.retryAllErrors).mockResolvedValue({
			tasksEnqueued: 3,
			message: "3 tasks enqueued",
		});
	});

	it("should render page title", async () => {
		renderWithProviders(<BooksInErrorSettings />);

		await waitFor(() => {
			expect(screen.getByText("Books in Error")).toBeInTheDocument();
		});
	});

	it("should show loading state initially", () => {
		renderWithProviders(<BooksInErrorSettings />);

		expect(
			screen.getByText("Loading books with errors..."),
		).toBeInTheDocument();
	});

	it("should display info alert after loading", async () => {
		renderWithProviders(<BooksInErrorSettings />);

		await waitFor(() => {
			expect(screen.getByText("About Book Errors")).toBeInTheDocument();
		});
	});

	it("should display total errors stat card", async () => {
		renderWithProviders(<BooksInErrorSettings />);

		await waitFor(() => {
			expect(screen.getByText("Total Errors")).toBeInTheDocument();
			expect(screen.getByText("3")).toBeInTheDocument();
		});
	});

	it("should display error groups in accordion", async () => {
		renderWithProviders(<BooksInErrorSettings />);

		await waitFor(() => {
			// Multiple "Parser Error" elements may appear (stat card, accordion, badges)
			expect(screen.getAllByText("Parser Error").length).toBeGreaterThan(0);
			expect(screen.getAllByText("Thumbnail Error").length).toBeGreaterThan(0);
		});
	});

	it("should display book titles in error groups", async () => {
		renderWithProviders(<BooksInErrorSettings />);

		await waitFor(() => {
			expect(screen.getByText("Test Book 1")).toBeInTheDocument();
			expect(screen.getByText("Test Book 2")).toBeInTheDocument();
		});
	});

	it("should display error messages", async () => {
		renderWithProviders(<BooksInErrorSettings />);

		await waitFor(() => {
			expect(screen.getByText("Failed to parse archive")).toBeInTheDocument();
			expect(screen.getByText("Invalid RAR format")).toBeInTheDocument();
		});
	});

	it("should show Retry All button when there are errors", async () => {
		renderWithProviders(<BooksInErrorSettings />);

		await waitFor(() => {
			expect(
				screen.getByRole("button", { name: /retry all \(3\)/i }),
			).toBeInTheDocument();
		});
	});

	it("should show Refresh button", async () => {
		renderWithProviders(<BooksInErrorSettings />);

		await waitFor(() => {
			expect(
				screen.getByRole("button", { name: /refresh/i }),
			).toBeInTheDocument();
		});
	});

	it("should show empty state when no errors", async () => {
		vi.mocked(booksApi.getBooksWithErrors).mockResolvedValue(emptyErrorsData);

		renderWithProviders(<BooksInErrorSettings />);

		await waitFor(() => {
			expect(screen.getByText("No Books in Error")).toBeInTheDocument();
			expect(
				screen.getByText(/all books have been processed successfully/i),
			).toBeInTheDocument();
		});
	});

	it("should not show Retry All button when no errors", async () => {
		vi.mocked(booksApi.getBooksWithErrors).mockResolvedValue(emptyErrorsData);

		renderWithProviders(<BooksInErrorSettings />);

		await waitFor(() => {
			expect(screen.getByText("No Books in Error")).toBeInTheDocument();
		});

		expect(
			screen.queryByRole("button", { name: /retry all/i }),
		).not.toBeInTheDocument();
	});

	it("should show individual retry buttons for each book", async () => {
		renderWithProviders(<BooksInErrorSettings />);

		await waitFor(() => {
			// There should be multiple Retry buttons (one for each book card)
			const retryButtons = screen.getAllByRole("button", { name: /^retry$/i });
			expect(retryButtons.length).toBeGreaterThan(0);
		});
	});

	it("should call retryAllErrors when Retry All is clicked", async () => {
		const user = userEvent.setup();
		renderWithProviders(<BooksInErrorSettings />);

		await waitFor(() => {
			expect(
				screen.getByRole("button", { name: /retry all \(3\)/i }),
			).toBeInTheDocument();
		});

		await user.click(screen.getByRole("button", { name: /retry all \(3\)/i }));

		await waitFor(() => {
			expect(booksApi.retryAllErrors).toHaveBeenCalledWith({});
		});
	});

	it("should display error type badges with correct labels", async () => {
		renderWithProviders(<BooksInErrorSettings />);

		await waitFor(() => {
			// Check for error type badges
			const parserBadges = screen.getAllByText("Parser Error");
			expect(parserBadges.length).toBeGreaterThan(0);
		});
	});

	it("should display book file format", async () => {
		renderWithProviders(<BooksInErrorSettings />);

		await waitFor(() => {
			expect(screen.getByText(/CBZ/i)).toBeInTheDocument();
			expect(screen.getByText(/CBR/i)).toBeInTheDocument();
		});
	});

	it("should display series name for books", async () => {
		renderWithProviders(<BooksInErrorSettings />);

		await waitFor(() => {
			expect(screen.getAllByText("Test Series").length).toBeGreaterThan(0);
		});
	});

	it("should handle single error type only", async () => {
		const singleTypeData: BooksWithErrorsResponse = {
			totalBooksWithErrors: 2,
			totalPages: 1,
			page: 0,
			pageSize: 100,
			errorCounts: {
				parser: 2,
			},
			groups: [
				{
					errorType: "parser",
					label: "Parser Error",
					count: 2,
					books: [
						{
							book: createMockBook({
								id: "book-1",
								title: "Test Book 1",
								seriesId: "series-1",
								seriesName: "Test Series",
								fileFormat: "cbz",
								pageCount: 100,
							}),
							errors: [
								{
									errorType: "parser",
									message: "Failed to parse archive",
									occurredAt: "2024-01-01T00:00:00Z",
								},
							],
						},
						{
							book: createMockBook({
								id: "book-2",
								title: "Test Book 2",
								seriesId: "series-1",
								seriesName: "Test Series",
								fileFormat: "cbr",
								pageCount: 50,
							}),
							errors: [
								{
									errorType: "parser",
									message: "Invalid RAR format",
									occurredAt: "2024-01-01T00:00:00Z",
								},
							],
						},
					],
				},
			],
		};

		vi.mocked(booksApi.getBooksWithErrors).mockResolvedValue(singleTypeData);

		renderWithProviders(<BooksInErrorSettings />);

		await waitFor(() => {
			// Should display only parser error group
			expect(screen.getAllByText("Parser Error").length).toBeGreaterThan(0);
			// Should not display other error types
			expect(screen.queryByText("Thumbnail Error")).not.toBeInTheDocument();
		});

		// Verify stats show the correct total errors count
		await waitFor(() => {
			// The number "2" appears in multiple places (count in badge, stat card, etc.)
			// We verify by checking the total errors stat card exists
			expect(screen.getAllByText("2").length).toBeGreaterThan(0);
		});
	});

	it("should handle long error messages with truncation", async () => {
		const longMessage =
			"This is a very long error message that should be truncated in the UI display. ".repeat(
				10,
			);
		const longMessageData: BooksWithErrorsResponse = {
			totalBooksWithErrors: 1,
			totalPages: 1,
			page: 0,
			pageSize: 100,
			errorCounts: {
				parser: 1,
			},
			groups: [
				{
					errorType: "parser",
					label: "Parser Error",
					count: 1,
					books: [
						{
							book: createMockBook({
								id: "book-1",
								title: "Test Book 1",
								seriesId: "series-1",
								seriesName: "Test Series",
								fileFormat: "cbz",
								pageCount: 100,
							}),
							errors: [
								{
									errorType: "parser",
									message: longMessage,
									occurredAt: "2024-01-01T00:00:00Z",
								},
							],
						},
					],
				},
			],
		};

		vi.mocked(booksApi.getBooksWithErrors).mockResolvedValue(longMessageData);

		renderWithProviders(<BooksInErrorSettings />);

		await waitFor(() => {
			// The error message should be in the document (even if truncated visually)
			expect(screen.getByText("Test Book 1")).toBeInTheDocument();
		});

		// The message should be rendered (CSS handles truncation with lineClamp)
		await waitFor(() => {
			const errorText = screen.getByText((content) =>
				content.includes("This is a very long error message"),
			);
			expect(errorText).toBeInTheDocument();
		});
	});

	it("should handle multiple errors per book", async () => {
		const multipleErrorsData: BooksWithErrorsResponse = {
			totalBooksWithErrors: 1,
			totalPages: 1,
			page: 0,
			pageSize: 100,
			errorCounts: {
				parser: 1,
				thumbnail: 1,
			},
			groups: [
				{
					errorType: "parser",
					label: "Parser Error",
					count: 1,
					books: [
						{
							book: createMockBook({
								id: "book-1",
								title: "Test Book 1",
								seriesId: "series-1",
								seriesName: "Test Series",
								fileFormat: "cbz",
								pageCount: 100,
							}),
							errors: [
								{
									errorType: "parser",
									message: "Failed to parse archive",
									occurredAt: "2024-01-01T00:00:00Z",
								},
							],
						},
					],
				},
				{
					errorType: "thumbnail",
					label: "Thumbnail Error",
					count: 1,
					books: [
						{
							book: createMockBook({
								id: "book-1",
								title: "Test Book 1",
								seriesId: "series-1",
								seriesName: "Test Series",
								fileFormat: "cbz",
								pageCount: 100,
							}),
							errors: [
								{
									errorType: "thumbnail",
									message: "Failed to generate thumbnail",
									occurredAt: "2024-01-01T00:00:00Z",
								},
							],
						},
					],
				},
			],
		};

		vi.mocked(booksApi.getBooksWithErrors).mockResolvedValue(
			multipleErrorsData,
		);

		renderWithProviders(<BooksInErrorSettings />);

		await waitFor(() => {
			// Should show total errors header
			expect(screen.getByText("Total Errors")).toBeInTheDocument();
			// The number "1" appears in multiple places (stat card, badge counts, etc.)
			expect(screen.getAllByText("1").length).toBeGreaterThan(0);
		});

		// Should have both parser and thumbnail groups
		await waitFor(() => {
			expect(screen.getAllByText("Parser Error").length).toBeGreaterThan(0);
			expect(screen.getAllByText("Thumbnail Error").length).toBeGreaterThan(0);
		});
	});

	it("should call retryBookErrors when individual retry is clicked", async () => {
		const user = userEvent.setup();
		renderWithProviders(<BooksInErrorSettings />);

		await waitFor(() => {
			expect(screen.getByText("Test Book 1")).toBeInTheDocument();
		});

		// Click the first individual retry button
		const retryButtons = screen.getAllByRole("button", { name: /^retry$/i });
		await user.click(retryButtons[0]);

		await waitFor(() => {
			expect(booksApi.retryBookErrors).toHaveBeenCalledWith(
				"book-1",
				undefined,
			);
		});
	});

	it("should handle refresh button click", async () => {
		const user = userEvent.setup();
		renderWithProviders(<BooksInErrorSettings />);

		await waitFor(() => {
			expect(
				screen.getByRole("button", { name: /refresh/i }),
			).toBeInTheDocument();
		});

		await user.click(screen.getByRole("button", { name: /refresh/i }));

		// Should call the API again (initial + refresh)
		await waitFor(() => {
			expect(booksApi.getBooksWithErrors).toHaveBeenCalledTimes(2);
		});
	});
});
