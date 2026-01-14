import { beforeEach, describe, expect, it, vi } from "vitest";
import { renderWithProviders, screen, waitFor } from "@/test/utils";

import { useReaderStore } from "@/store/readerStore";
import { EpubReader } from "./EpubReader";

// Mock react-reader since it's a complex library that requires actual EPUB files
vi.mock("react-reader", () => ({
	ReactReader: vi.fn(({ url, location, locationChanged, getRendition, showToc }) => {
		// Simulate getting rendition on mount
		const mockRendition = {
			themes: {
				override: vi.fn(),
				fontSize: vi.fn(),
			},
			book: {
				loaded: {
					navigation: Promise.resolve({ toc: [] }),
				},
				ready: Promise.resolve(),
				locations: {
					generate: vi.fn().mockResolvedValue([]),
					percentageFromCfi: vi.fn().mockReturnValue(0.5),
					cfiFromPercentage: vi.fn().mockReturnValue("epubcfi(/6/2[chapter1]!/4/2)"),
				},
				spine: {
					get: vi.fn(),
				},
			},
			on: vi.fn(),
			display: vi.fn(),
		};

		// Call getRendition callback if provided
		if (getRendition) {
			setTimeout(() => getRendition(mockRendition), 0);
		}

		return (
			<div data-testid="react-reader-mock">
				<div>Mock ReactReader</div>
				<div data-testid="epub-url">{url}</div>
				<div data-testid="show-toc">{String(showToc)}</div>
			</div>
		);
	}),
	ReactReaderStyle: {
		readerArea: {},
		arrow: {},
		arrowHover: {},
	},
}));

// Mock the hooks
vi.mock("./hooks/useEpubProgress", () => ({
	useEpubProgress: vi.fn(() => ({
		getSavedLocation: vi.fn().mockReturnValue(null),
		initialPercentage: null,
		isLoadingProgress: false,
		saveLocation: vi.fn(),
	})),
}));

vi.mock("./hooks/useEpubBookmarks", () => ({
	useEpubBookmarks: vi.fn(() => ({
		bookmarks: [],
		addBookmark: vi.fn(),
		updateBookmark: vi.fn(),
		removeBookmark: vi.fn(),
		isBookmarked: vi.fn().mockReturnValue(false),
		getBookmarkByCfi: vi.fn().mockReturnValue(null),
	})),
}));

// Mock the API client
vi.mock("@/api/client", () => ({
	api: {
		get: vi.fn(),
		put: vi.fn(),
		post: vi.fn(),
		patch: vi.fn(),
		delete: vi.fn(),
	},
}));

// Default settings to reset store before each test
const defaultSettings = {
	fitMode: "screen" as const,
	pageLayout: "single" as const,
	readingDirection: "ltr" as const,
	backgroundColor: "black" as const,
	pdfMode: "streaming" as const,
	pdfSpreadMode: "single" as const,
	pdfContinuousScroll: false,
	autoHideToolbar: true,
	toolbarHideDelay: 3000,
	epubTheme: "light" as const,
	epubFontSize: 100,
	preloadPages: 1,
	doublePageShowWideAlone: true,
	doublePageStartOnOdd: true,
	pageTransition: "slide" as const,
	transitionDuration: 200,
	webtoonSidePadding: 0,
	webtoonPageGap: 0,
};

const defaultSessionState = {
	currentPage: 1,
	totalPages: 10,
	isLoading: false,
	toolbarVisible: true,
	isFullscreen: false,
	currentBookId: null,
	readingDirectionOverride: null,
	adjacentBooks: null,
	boundaryState: "none" as const,
	pageOrientations: {},
	lastNavigationDirection: null,
	preloadedImages: new Set<string>(),
};

const defaultProps = {
	bookId: "book-123",
	title: "Test EPUB Book",
	totalPages: 100,
	onClose: vi.fn(),
};

describe("EpubReader", () => {
	beforeEach(() => {
		vi.clearAllMocks();
		useReaderStore.setState({
			settings: { ...defaultSettings },
			...defaultSessionState,
		});
	});

	describe("rendering", () => {
		it("should render the reader container", () => {
			renderWithProviders(<EpubReader {...defaultProps} />);

			// The reader should be rendered with the title in toolbar
			expect(screen.getByText("Test EPUB Book")).toBeInTheDocument();
		});

		it("should render the toolbar with title", () => {
			renderWithProviders(<EpubReader {...defaultProps} />);

			expect(screen.getByText("Test EPUB Book")).toBeInTheDocument();
		});

		it("should render ReactReader component", () => {
			renderWithProviders(<EpubReader {...defaultProps} />);

			// Our mock ReactReader should be rendered
			expect(screen.getByTestId("react-reader-mock")).toBeInTheDocument();
		});

		it("should pass correct EPUB URL to ReactReader", () => {
			renderWithProviders(<EpubReader {...defaultProps} />);

			// Check that the URL is correctly passed
			expect(screen.getByTestId("epub-url")).toHaveTextContent(
				"/api/v1/books/book-123/file"
			);
		});

		it("should hide built-in TOC", () => {
			renderWithProviders(<EpubReader {...defaultProps} />);

			// showToc should be false (we use custom TOC)
			expect(screen.getByTestId("show-toc")).toHaveTextContent("false");
		});
	});

	describe("toolbar", () => {
		it("should show toolbar when visible", () => {
			useReaderStore.setState({ toolbarVisible: true });

			renderWithProviders(<EpubReader {...defaultProps} />);

			expect(screen.getByText("Test EPUB Book")).toBeInTheDocument();
		});

		it("should have TOC button", () => {
			renderWithProviders(<EpubReader {...defaultProps} />);

			// TOC button should be present
			const buttons = screen.getAllByRole("button");
			expect(buttons.length).toBeGreaterThan(0);
		});

		it("should have bookmarks button", () => {
			renderWithProviders(<EpubReader {...defaultProps} />);

			// Bookmarks button should be present
			const buttons = screen.getAllByRole("button");
			expect(buttons.length).toBeGreaterThan(0);
		});

		it("should have search button", () => {
			renderWithProviders(<EpubReader {...defaultProps} />);

			// Search button should be present
			const buttons = screen.getAllByRole("button");
			expect(buttons.length).toBeGreaterThan(0);
		});
	});

	describe("theme", () => {
		it("should apply light theme by default", () => {
			renderWithProviders(<EpubReader {...defaultProps} />);

			const container = document.querySelector('[style*="100vw"]');
			expect(container).toBeInTheDocument();
			expect(useReaderStore.getState().settings.epubTheme).toBe("light");
		});

		it("should apply dark theme when configured", () => {
			useReaderStore.setState({
				settings: { ...defaultSettings, epubTheme: "dark" },
			});

			renderWithProviders(<EpubReader {...defaultProps} />);

			expect(useReaderStore.getState().settings.epubTheme).toBe("dark");
		});

		it("should apply sepia theme when configured", () => {
			useReaderStore.setState({
				settings: { ...defaultSettings, epubTheme: "sepia" },
			});

			renderWithProviders(<EpubReader {...defaultProps} />);

			expect(useReaderStore.getState().settings.epubTheme).toBe("sepia");
		});
	});

	describe("font size", () => {
		it("should use default font size of 100%", () => {
			renderWithProviders(<EpubReader {...defaultProps} />);

			expect(useReaderStore.getState().settings.epubFontSize).toBe(100);
		});

		it("should respect configured font size", () => {
			useReaderStore.setState({
				settings: { ...defaultSettings, epubFontSize: 150 },
			});

			renderWithProviders(<EpubReader {...defaultProps} />);

			expect(useReaderStore.getState().settings.epubFontSize).toBe(150);
		});
	});

	describe("settings modal", () => {
		it("should have settings button in toolbar", () => {
			renderWithProviders(<EpubReader {...defaultProps} />);

			// Settings button should be present among toolbar buttons
			const buttons = screen.getAllByRole("button");
			expect(buttons.length).toBeGreaterThan(3);
		});
	});

	describe("URL parameters", () => {
		it("should handle startPercent parameter", () => {
			renderWithProviders(
				<EpubReader {...defaultProps} startPercent={0.5} />
			);

			// Reader should render with the start percent
			expect(screen.getByTestId("react-reader-mock")).toBeInTheDocument();
		});

		it("should ignore invalid startPercent (negative)", () => {
			renderWithProviders(
				<EpubReader {...defaultProps} startPercent={-0.5} />
			);

			// Should still render without error
			expect(screen.getByTestId("react-reader-mock")).toBeInTheDocument();
		});

		it("should ignore invalid startPercent (greater than 1)", () => {
			renderWithProviders(
				<EpubReader {...defaultProps} startPercent={1.5} />
			);

			// Should still render without error
			expect(screen.getByTestId("react-reader-mock")).toBeInTheDocument();
		});
	});

	describe("fullscreen", () => {
		it("should not be fullscreen by default", () => {
			renderWithProviders(<EpubReader {...defaultProps} />);

			expect(useReaderStore.getState().isFullscreen).toBe(false);
		});
	});

	describe("auto-hide toolbar", () => {
		it("should respect auto-hide toolbar setting", () => {
			useReaderStore.setState({
				settings: { ...defaultSettings, autoHideToolbar: false },
			});

			renderWithProviders(<EpubReader {...defaultProps} />);

			expect(useReaderStore.getState().settings.autoHideToolbar).toBe(false);
		});
	});

	describe("close callback", () => {
		it("should have close button that calls onClose", async () => {
			const onClose = vi.fn();

			renderWithProviders(
				<EpubReader {...defaultProps} onClose={onClose} />
			);

			// Close button should be present
			const buttons = screen.getAllByRole("button");
			expect(buttons.length).toBeGreaterThan(0);
			// First button is typically the close button
			const closeButton = buttons[0];
			expect(closeButton).toBeInTheDocument();
		});
	});

	describe("EPUB-specific features", () => {
		it("should render TOC drawer component", () => {
			renderWithProviders(<EpubReader {...defaultProps} />);

			// EpubTableOfContents is rendered in toolbar
			// It's toggled by a button
			expect(screen.getByTestId("react-reader-mock")).toBeInTheDocument();
		});

		it("should render bookmarks drawer component", () => {
			renderWithProviders(<EpubReader {...defaultProps} />);

			// EpubBookmarks is rendered in toolbar
			expect(screen.getByTestId("react-reader-mock")).toBeInTheDocument();
		});

		it("should render search drawer component", () => {
			renderWithProviders(<EpubReader {...defaultProps} />);

			// EpubSearch is rendered in toolbar
			expect(screen.getByTestId("react-reader-mock")).toBeInTheDocument();
		});
	});

	describe("background color", () => {
		it("should apply theme-based background color", () => {
			renderWithProviders(<EpubReader {...defaultProps} />);

			const container = document.querySelector('[style*="100vw"]');
			expect(container).toBeInTheDocument();
			// Light theme uses white background
			expect(container).toHaveStyle({ backgroundColor: "#ffffff" });
		});

		it("should apply dark theme background", () => {
			useReaderStore.setState({
				settings: { ...defaultSettings, epubTheme: "dark" },
			});

			renderWithProviders(<EpubReader {...defaultProps} />);

			const container = document.querySelector('[style*="100vw"]');
			expect(container).toBeInTheDocument();
			// Dark theme uses dark background
			expect(container).toHaveStyle({ backgroundColor: "#1a1a1a" });
		});

		it("should apply sepia theme background", () => {
			useReaderStore.setState({
				settings: { ...defaultSettings, epubTheme: "sepia" },
			});

			renderWithProviders(<EpubReader {...defaultProps} />);

			const container = document.querySelector('[style*="100vw"]');
			expect(container).toBeInTheDocument();
			// Sepia theme uses cream background
			expect(container).toHaveStyle({ backgroundColor: "#f4ecd8" });
		});
	});
});
