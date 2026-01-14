import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { renderWithProviders, screen, userEvent, waitFor } from "@/test/utils";
import { SearchInput } from "./SearchInput";

// Mock useSearch hook
vi.mock("@/hooks/useSearch", () => ({
	useSearch: vi.fn(),
}));

// Mock useNavigate
const mockNavigate = vi.fn();
vi.mock("react-router-dom", async () => {
	const actual = await vi.importActual("react-router-dom");
	return {
		...actual,
		useNavigate: () => mockNavigate,
	};
});

import { useSearch } from "@/hooks/useSearch";

describe("SearchInput", () => {
	beforeEach(() => {
		vi.clearAllMocks();

		// Override matchMedia to return matches: true (simulate desktop viewport)
		// This is needed for the visibleFrom="sm" prop to show the input
		window.matchMedia = vi.fn().mockImplementation((query: string) => ({
			matches: true,
			media: query,
			onchange: null,
			addListener: vi.fn(),
			removeListener: vi.fn(),
			addEventListener: vi.fn(),
			removeEventListener: vi.fn(),
			dispatchEvent: vi.fn(),
		}));

		// Default mock - no results
		vi.mocked(useSearch).mockReturnValue({
			results: { series: [], books: [] },
			isLoading: false,
			error: null,
		});
	});

	afterEach(() => {
		vi.restoreAllMocks();
	});

	it("should render the search input", () => {
		renderWithProviders(<SearchInput />);
		expect(screen.getByPlaceholderText("Search...")).toBeInTheDocument();
	});

	it("should show dropdown when typing 2+ characters with results", async () => {
		vi.mocked(useSearch).mockReturnValue({
			results: {
				series: [
					{
						id: "1",
						name: "Test Series",
						bookCount: 5,
						createdAt: "2024-01-01T00:00:00Z",
						libraryId: "lib-1",
						updatedAt: "2024-01-01T00:00:00Z",
					},
				],
				books: [],
			},
			isLoading: false,
			error: null,
		});

		const user = userEvent.setup();
		renderWithProviders(<SearchInput />);

		const input = screen.getByPlaceholderText("Search...");
		await user.type(input, "te");

		await waitFor(() => {
			expect(screen.getByText("Test Series")).toBeInTheDocument();
		});
	});

	it("should not show dropdown when typing less than 2 characters", async () => {
		vi.mocked(useSearch).mockReturnValue({
			results: {
				series: [
					{
						id: "1",
						name: "Test Series",
						bookCount: 5,
						createdAt: "2024-01-01T00:00:00Z",
						libraryId: "lib-1",
						updatedAt: "2024-01-01T00:00:00Z",
					},
				],
				books: [],
			},
			isLoading: false,
			error: null,
		});

		const user = userEvent.setup();
		renderWithProviders(<SearchInput />);

		const input = screen.getByPlaceholderText("Search...");
		await user.type(input, "t");

		expect(screen.queryByText("Test Series")).not.toBeInTheDocument();
	});

	describe("keyboard navigation", () => {
		const mockResults = {
			series: [
				{
					id: "s1",
					name: "Alpha Series",
					bookCount: 3,
					createdAt: "2024-01-01T00:00:00Z",
					libraryId: "lib-1",
					updatedAt: "2024-01-01T00:00:00Z",
				},
				{
					id: "s2",
					name: "Beta Series",
					bookCount: 5,
					createdAt: "2024-01-01T00:00:00Z",
					libraryId: "lib-1",
					updatedAt: "2024-01-01T00:00:00Z",
				},
			],
			books: [
				{
					id: "b1",
					title: "First Book",
					seriesName: "Gamma Series",
					seriesId: "s1",
					filePath: "/path/first.cbz",
					fileSize: 1000,
					fileHash: "hash1",
					fileFormat: "cbz",
					pageCount: 100,
					createdAt: "2024-01-01T00:00:00Z",
					updatedAt: "2024-01-01T00:00:00Z",
					deleted: false,
				},
				{
					id: "b2",
					title: "Second Book",
					seriesName: "Delta Series",
					seriesId: "s2",
					filePath: "/path/second.cbz",
					fileSize: 1000,
					fileHash: "hash2",
					fileFormat: "cbz",
					pageCount: 100,
					createdAt: "2024-01-01T00:00:00Z",
					updatedAt: "2024-01-01T00:00:00Z",
					deleted: false,
				},
			],
		};

		beforeEach(() => {
			vi.mocked(useSearch).mockReturnValue({
				results: mockResults,
				isLoading: false,
				error: null,
			});
		});

		it("should navigate to search page on Enter when no option selected", async () => {
			const user = userEvent.setup();
			renderWithProviders(<SearchInput />);

			const input = screen.getByPlaceholderText("Search...");
			await user.type(input, "test");
			await user.keyboard("{Enter}");

			expect(mockNavigate).toHaveBeenCalledWith("/search?q=test");
		});

		it("should navigate to item when clicking on series option", async () => {
			const user = userEvent.setup();
			renderWithProviders(<SearchInput />);

			const input = screen.getByPlaceholderText("Search...");
			await user.type(input, "test");

			await waitFor(() => {
				expect(screen.getByText("Alpha Series")).toBeInTheDocument();
			});

			// Click on series option
			await user.click(screen.getByText("Alpha Series"));

			expect(mockNavigate).toHaveBeenCalledWith("/series/s1");
		});

		it("should navigate to item when clicking on book option", async () => {
			const user = userEvent.setup();
			renderWithProviders(<SearchInput />);

			const input = screen.getByPlaceholderText("Search...");
			await user.type(input, "test");

			await waitFor(() => {
				expect(screen.getByText("First Book")).toBeInTheDocument();
			});

			// Click on book option
			await user.click(screen.getByText("First Book"));

			expect(mockNavigate).toHaveBeenCalledWith("/books/b1");
		});

		it("should clear query after clicking an option", async () => {
			const user = userEvent.setup();
			renderWithProviders(<SearchInput />);

			const input = screen.getByPlaceholderText(
				"Search...",
			) as HTMLInputElement;
			await user.type(input, "test");

			await waitFor(() => {
				expect(screen.getByText("Alpha Series")).toBeInTheDocument();
			});

			await user.click(screen.getByText("Alpha Series"));

			expect(input.value).toBe("");
		});

		it("should show both series and books in dropdown", async () => {
			const user = userEvent.setup();
			renderWithProviders(<SearchInput />);

			const input = screen.getByPlaceholderText("Search...");
			await user.type(input, "test");

			await waitFor(() => {
				// Check for group labels
				expect(screen.getByText("Series")).toBeInTheDocument();
				expect(screen.getByText("Books")).toBeInTheDocument();

				// Check for items
				expect(screen.getByText("Alpha Series")).toBeInTheDocument();
				expect(screen.getByText("Beta Series")).toBeInTheDocument();
				expect(screen.getByText("First Book")).toBeInTheDocument();
				expect(screen.getByText("Second Book")).toBeInTheDocument();
			});
		});

		it("should show loading state", async () => {
			vi.mocked(useSearch).mockReturnValue({
				results: { series: [], books: [] },
				isLoading: true,
				error: null,
			});

			const user = userEvent.setup();
			renderWithProviders(<SearchInput />);

			const input = screen.getByPlaceholderText("Search...");
			await user.type(input, "test");

			await waitFor(() => {
				expect(screen.getByText("Searching...")).toBeInTheDocument();
			});
		});

		it("should show no results message when empty", async () => {
			vi.mocked(useSearch).mockReturnValue({
				results: { series: [], books: [] },
				isLoading: false,
				error: null,
			});

			const user = userEvent.setup();
			renderWithProviders(<SearchInput />);

			const input = screen.getByPlaceholderText("Search...");
			await user.type(input, "test");

			await waitFor(() => {
				expect(screen.getByText("No results found")).toBeInTheDocument();
			});
		});
	});
});
