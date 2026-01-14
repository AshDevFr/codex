import { act, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { renderWithProviders } from "@/test/utils";

import { EpubSearch, type SearchResult } from "./EpubSearch";

describe("EpubSearch", () => {
	const defaultProps = {
		opened: false,
		onToggle: vi.fn(),
		onSearch: vi.fn().mockResolvedValue([]),
		onNavigate: vi.fn(),
	};

	beforeEach(() => {
		vi.useFakeTimers({ shouldAdvanceTime: true });
	});

	afterEach(() => {
		vi.useRealTimers();
	});

	describe("search toggle button", () => {
		it("should render search button", () => {
			renderWithProviders(<EpubSearch {...defaultProps} />);

			expect(screen.getByLabelText("Search")).toBeInTheDocument();
		});

		it("should call onToggle when clicking search button", async () => {
			const user = userEvent.setup();
			const onToggle = vi.fn();

			renderWithProviders(<EpubSearch {...defaultProps} onToggle={onToggle} />);

			await user.click(screen.getByLabelText("Search"));

			expect(onToggle).toHaveBeenCalledTimes(1);
		});
	});

	describe("search drawer", () => {
		it("should show drawer when opened", () => {
			renderWithProviders(<EpubSearch {...defaultProps} opened={true} />);

			expect(screen.getByRole("dialog")).toBeInTheDocument();
			expect(
				screen.getByPlaceholderText("Search in book..."),
			).toBeInTheDocument();
		});

		it("should show initial prompt when no search performed", () => {
			renderWithProviders(<EpubSearch {...defaultProps} opened={true} />);

			expect(
				screen.getByText("Enter a search term to find text in the book"),
			).toBeInTheDocument();
		});

		it("should autofocus search input when opened", () => {
			renderWithProviders(<EpubSearch {...defaultProps} opened={true} />);

			expect(screen.getByPlaceholderText("Search in book...")).toHaveFocus();
		});
	});

	describe("search functionality", () => {
		it("should call onSearch with debounce when typing", async () => {
			const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
			const onSearch = vi.fn().mockResolvedValue([]);

			renderWithProviders(
				<EpubSearch {...defaultProps} opened={true} onSearch={onSearch} />,
			);

			await user.type(screen.getByPlaceholderText("Search in book..."), "test");

			// Advance timers to trigger debounce (300ms)
			await act(async () => {
				vi.advanceTimersByTime(400);
			});

			expect(onSearch).toHaveBeenCalledWith("test");
		});

		it("should show loading state while searching", async () => {
			const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
			// Create a promise that won't resolve immediately
			let resolveSearch: (value: SearchResult[]) => void;
			const searchPromise = new Promise<SearchResult[]>((resolve) => {
				resolveSearch = resolve;
			});
			const onSearch = vi.fn().mockReturnValue(searchPromise);

			renderWithProviders(
				<EpubSearch {...defaultProps} opened={true} onSearch={onSearch} />,
			);

			await user.type(screen.getByPlaceholderText("Search in book..."), "test");

			// Advance timers to trigger debounce
			await act(async () => {
				vi.advanceTimersByTime(400);
			});

			expect(onSearch).toHaveBeenCalled();

			// Should show loader - Mantine Loader has class mantine-Loader-root
			expect(
				document.querySelector(".mantine-Loader-root"),
			).toBeInTheDocument();

			// Resolve the search
			await act(async () => {
				resolveSearch?.([]);
			});

			// Loader should disappear
			await waitFor(() => {
				expect(
					document.querySelector(".mantine-Loader-root"),
				).not.toBeInTheDocument();
			});
		});

		it("should show no results message when search returns empty", async () => {
			const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
			const onSearch = vi.fn().mockResolvedValue([]);

			renderWithProviders(
				<EpubSearch {...defaultProps} opened={true} onSearch={onSearch} />,
			);

			await user.type(screen.getByPlaceholderText("Search in book..."), "xyz");

			await act(async () => {
				vi.advanceTimersByTime(400);
			});

			await waitFor(() => {
				expect(
					screen.getByText(/No results found for "xyz"/),
				).toBeInTheDocument();
			});
		});

		it("should display search results", async () => {
			const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
			const results: SearchResult[] = [
				{
					cfi: "epubcfi(/6/4!/4/2/1:0)",
					excerpt: "This is a test excerpt with the search term",
					chapter: "Chapter 1",
				},
				{
					cfi: "epubcfi(/6/4!/4/2/2:0)",
					excerpt: "Another result with different content",
					chapter: "Chapter 2",
				},
			];
			let resolveSearch: (value: SearchResult[]) => void;
			const searchPromise = new Promise<SearchResult[]>((resolve) => {
				resolveSearch = resolve;
			});
			const onSearch = vi.fn().mockReturnValue(searchPromise);

			renderWithProviders(
				<EpubSearch {...defaultProps} opened={true} onSearch={onSearch} />,
			);

			await user.type(screen.getByPlaceholderText("Search in book..."), "test");

			// Advance timers to trigger debounce
			await act(async () => {
				vi.advanceTimersByTime(400);
			});

			expect(onSearch).toHaveBeenCalledWith("test");

			// Resolve the search with results
			await act(async () => {
				resolveSearch(results);
			});

			await waitFor(() => {
				expect(screen.getByText("2 results found")).toBeInTheDocument();
			});

			expect(screen.getByText("Chapter 1")).toBeInTheDocument();
			expect(screen.getByText("Chapter 2")).toBeInTheDocument();
		});

		it("should show singular 'result' for one result", async () => {
			const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
			const results: SearchResult[] = [
				{
					cfi: "epubcfi(/6/4!/4/2/1:0)",
					excerpt: "Single result",
				},
			];
			let resolveSearch: (value: SearchResult[]) => void;
			const searchPromise = new Promise<SearchResult[]>((resolve) => {
				resolveSearch = resolve;
			});
			const onSearch = vi.fn().mockReturnValue(searchPromise);

			renderWithProviders(
				<EpubSearch {...defaultProps} opened={true} onSearch={onSearch} />,
			);

			await user.type(screen.getByPlaceholderText("Search in book..."), "test");

			// Advance timers to trigger debounce
			await act(async () => {
				vi.advanceTimersByTime(400);
			});

			// Resolve the search with results
			await act(async () => {
				resolveSearch(results);
			});

			await waitFor(() => {
				expect(screen.getByText("1 result found")).toBeInTheDocument();
			});
		});
	});

	describe("search result navigation", () => {
		it("should call onNavigate when clicking a result", async () => {
			const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
			const onNavigate = vi.fn();
			const results: SearchResult[] = [
				{
					cfi: "epubcfi(/6/4!/4/2/1:0)",
					excerpt: "Click me to navigate",
					chapter: "Chapter 1",
				},
			];
			let resolveSearch: (value: SearchResult[]) => void;
			const searchPromise = new Promise<SearchResult[]>((resolve) => {
				resolveSearch = resolve;
			});
			const onSearch = vi.fn().mockReturnValue(searchPromise);

			renderWithProviders(
				<EpubSearch
					{...defaultProps}
					opened={true}
					onSearch={onSearch}
					onNavigate={onNavigate}
				/>,
			);

			await user.type(screen.getByPlaceholderText("Search in book..."), "test");

			// Advance timers to trigger debounce
			await act(async () => {
				vi.advanceTimersByTime(400);
			});

			// Resolve the search with results
			await act(async () => {
				resolveSearch(results);
			});

			await waitFor(() => {
				expect(screen.getByText("Chapter 1")).toBeInTheDocument();
			});

			await user.click(screen.getByText(/Click me to navigate/));

			expect(onNavigate).toHaveBeenCalledWith("epubcfi(/6/4!/4/2/1:0)");
		});

		it("should call onToggle to close drawer after navigation", async () => {
			const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
			const onToggle = vi.fn();
			const results: SearchResult[] = [
				{
					cfi: "epubcfi(/6/4!/4/2/1:0)",
					excerpt: "Test result",
				},
			];
			let resolveSearch: (value: SearchResult[]) => void;
			const searchPromise = new Promise<SearchResult[]>((resolve) => {
				resolveSearch = resolve;
			});
			const onSearch = vi.fn().mockReturnValue(searchPromise);

			renderWithProviders(
				<EpubSearch
					{...defaultProps}
					opened={true}
					onSearch={onSearch}
					onToggle={onToggle}
				/>,
			);

			await user.type(screen.getByPlaceholderText("Search in book..."), "test");

			// Advance timers to trigger debounce
			await act(async () => {
				vi.advanceTimersByTime(400);
			});

			expect(onSearch).toHaveBeenCalledWith("test");

			// Resolve the search with results
			await act(async () => {
				resolveSearch(results);
			});

			await waitFor(() => {
				expect(screen.getByText("1 result found")).toBeInTheDocument();
			});

			// Click on the search result box (highlighted text spans multiple elements)
			const resultBox = document.querySelector('[style*="cursor: pointer"]');
			expect(resultBox).toBeInTheDocument();
			if (resultBox) await user.click(resultBox);

			expect(onToggle).toHaveBeenCalled();
		});
	});

	describe("clear search", () => {
		it("should show clear button when there is query text", async () => {
			const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });

			renderWithProviders(<EpubSearch {...defaultProps} opened={true} />);

			const input = screen.getByPlaceholderText("Search in book...");
			await user.type(input, "test");

			expect(screen.getByLabelText("Clear search")).toBeInTheDocument();
		});

		it("should clear query when clicking clear button", async () => {
			const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });

			renderWithProviders(<EpubSearch {...defaultProps} opened={true} />);

			const input = screen.getByPlaceholderText("Search in book...");
			await user.type(input, "test");
			await user.click(screen.getByLabelText("Clear search"));

			expect(input).toHaveValue("");
		});

		it("should reset results when clearing query", async () => {
			const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
			const results: SearchResult[] = [
				{
					cfi: "epubcfi(/6/4!/4/2/1:0)",
					excerpt: "Test result",
				},
			];
			let resolveSearch: (value: SearchResult[]) => void;
			const searchPromise = new Promise<SearchResult[]>((resolve) => {
				resolveSearch = resolve;
			});
			const onSearch = vi.fn().mockReturnValue(searchPromise);

			renderWithProviders(
				<EpubSearch {...defaultProps} opened={true} onSearch={onSearch} />,
			);

			await user.type(screen.getByPlaceholderText("Search in book..."), "test");

			// Advance timers to trigger debounce
			await act(async () => {
				vi.advanceTimersByTime(400);
			});

			// Resolve the search with results
			await act(async () => {
				resolveSearch(results);
			});

			await waitFor(() => {
				expect(screen.getByText("1 result found")).toBeInTheDocument();
			});

			await user.click(screen.getByLabelText("Clear search"));

			expect(
				screen.getByText("Enter a search term to find text in the book"),
			).toBeInTheDocument();
		});
	});

	describe("search highlighting", () => {
		it("should highlight search term in results", async () => {
			const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
			const results: SearchResult[] = [
				{
					cfi: "epubcfi(/6/4!/4/2/1:0)",
					excerpt: "This contains the word test in the middle",
				},
			];
			let resolveSearch: (value: SearchResult[]) => void;
			const searchPromise = new Promise<SearchResult[]>((resolve) => {
				resolveSearch = resolve;
			});
			const onSearch = vi.fn().mockReturnValue(searchPromise);

			renderWithProviders(
				<EpubSearch {...defaultProps} opened={true} onSearch={onSearch} />,
			);

			await user.type(screen.getByPlaceholderText("Search in book..."), "test");

			// Advance timers to trigger debounce
			await act(async () => {
				vi.advanceTimersByTime(400);
			});

			// Resolve the search with results
			await act(async () => {
				resolveSearch(results);
			});

			await waitFor(() => {
				const mark = screen.getByText("test", { selector: "mark" });
				expect(mark).toBeInTheDocument();
			});
		});
	});

	describe("results without chapter", () => {
		it("should display results without chapter info", async () => {
			const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
			const results: SearchResult[] = [
				{
					cfi: "epubcfi(/6/4!/4/2/1:0)",
					excerpt: "Result without chapter",
					// No chapter property
				},
			];
			let resolveSearch: (value: SearchResult[]) => void;
			const searchPromise = new Promise<SearchResult[]>((resolve) => {
				resolveSearch = resolve;
			});
			const onSearch = vi.fn().mockReturnValue(searchPromise);

			renderWithProviders(
				<EpubSearch {...defaultProps} opened={true} onSearch={onSearch} />,
			);

			await user.type(screen.getByPlaceholderText("Search in book..."), "test");

			// Advance timers to trigger debounce
			await act(async () => {
				vi.advanceTimersByTime(400);
			});

			// Resolve the search with results
			await act(async () => {
				resolveSearch(results);
			});

			await waitFor(() => {
				expect(screen.getByText(/Result without chapter/)).toBeInTheDocument();
			});
		});
	});
});
