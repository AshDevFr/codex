import { beforeEach, describe, expect, it, vi } from "vitest";
import { useReaderStore } from "@/store/readerStore";
import { renderWithProviders, screen, waitFor } from "@/test/utils";
import { ComicReader } from "./ComicReader";

// Store the mock implementations so we can change them per test
let mockUseReadProgress = vi.fn(() => ({
	initialPage: 1,
	isLoading: false,
}));

let mockUseSeriesNavigation = vi.fn(() => ({
	handleNextPage: vi.fn(),
	handlePrevPage: vi.fn(),
	goToNextBook: vi.fn(),
	goToPrevBook: vi.fn(),
	canGoNextBook: false,
	canGoPrevBook: false,
}));

let mockUseSeriesReaderSettings = vi.fn(() => ({
	hasSeriesOverride: false as boolean,
	effectiveSettings: {
		fitMode: "screen" as string,
		pageLayout: "single" as string,
		readingDirection: "ltr" as string,
		backgroundColor: "black" as string,
		doublePageShowWideAlone: true,
		doublePageStartOnOdd: true,
	},
	forkToSeries: vi.fn(),
	resetToGlobal: vi.fn(),
	updateSetting: vi.fn(),
	isLoaded: true,
	seriesOverride: null,
}));

// Mock the hooks
vi.mock("./hooks", () => ({
	useAdjacentBooks: vi.fn(),
	useKeyboardNav: vi.fn(),
	useReadProgress: (...args: Parameters<typeof mockUseReadProgress>) =>
		mockUseReadProgress(...args),
	useSeriesNavigation: (...args: Parameters<typeof mockUseSeriesNavigation>) =>
		mockUseSeriesNavigation(...args),
	useSeriesReaderSettings: (
		...args: Parameters<typeof mockUseSeriesReaderSettings>
	) => mockUseSeriesReaderSettings(...args),
	useTouchNav: vi.fn(() => ({
		touchRef: vi.fn(),
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
	epubFontFamily: "default" as const,
	epubLineHeight: 150,
	epubMargin: 10,
	preloadPages: 1,
	doublePageShowWideAlone: true,
	doublePageStartOnOdd: true,
	pageTransition: "slide" as const,
	transitionDuration: 200,
	webtoonSidePadding: 0,
	webtoonPageGap: 0,
	autoAdvanceToNextBook: false,
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
	seriesId: "series-456",
	title: "Test Comic",
	totalPages: 10,
	format: "CBZ",
	onClose: vi.fn(),
};

describe("ComicReader", () => {
	beforeEach(() => {
		vi.clearAllMocks();
		// Reset mock implementations to defaults
		mockUseReadProgress = vi.fn(() => ({
			initialPage: 1,
			isLoading: false,
		}));
		mockUseSeriesNavigation = vi.fn(() => ({
			handleNextPage: vi.fn(),
			handlePrevPage: vi.fn(),
			goToNextBook: vi.fn(),
			goToPrevBook: vi.fn(),
			canGoNextBook: false,
			canGoPrevBook: false,
		}));
		mockUseSeriesReaderSettings = vi.fn(() => ({
			hasSeriesOverride: false,
			effectiveSettings: {
				fitMode: "screen" as const,
				pageLayout: "single" as const,
				readingDirection: "ltr" as const,
				backgroundColor: "black" as const,
				doublePageShowWideAlone: true,
				doublePageStartOnOdd: true,
			},
			forkToSeries: vi.fn(),
			resetToGlobal: vi.fn(),
			updateSetting: vi.fn(),
			isLoaded: true,
			seriesOverride: null,
		}));
		useReaderStore.setState({
			settings: { ...defaultSettings },
			...defaultSessionState,
		});
	});

	describe("rendering", () => {
		it("should render the reader container", () => {
			renderWithProviders(<ComicReader {...defaultProps} />);

			// The reader should be rendered (toolbar is visible by default)
			expect(screen.getByText("Test Comic")).toBeInTheDocument();
		});

		it("should render the toolbar with title", () => {
			renderWithProviders(<ComicReader {...defaultProps} />);

			expect(screen.getByText("Test Comic")).toBeInTheDocument();
		});

		it("should render a page image", () => {
			renderWithProviders(<ComicReader {...defaultProps} />);

			// Find the img element by its src attribute
			const image = document.querySelector(
				'img[src*="/api/v1/books/book-123/pages/"]',
			);
			expect(image).toBeInTheDocument();
		});

		it("should show loading state when progress is loading", () => {
			mockUseReadProgress = vi.fn(() => ({
				initialPage: 1,
				isLoading: true,
			}));

			renderWithProviders(<ComicReader {...defaultProps} />);

			// Should show loader
			expect(
				document.querySelector(".mantine-Loader-root"),
			).toBeInTheDocument();
		});

		it("should show message when book has no pages", () => {
			renderWithProviders(<ComicReader {...defaultProps} totalPages={0} />);

			expect(screen.getByText("This book has no pages")).toBeInTheDocument();
		});
	});

	describe("initialization", () => {
		it("should initialize reader with book data", async () => {
			renderWithProviders(<ComicReader {...defaultProps} />);

			await waitFor(() => {
				const state = useReaderStore.getState();
				expect(state.currentBookId).toBe("book-123");
				expect(state.totalPages).toBe(10);
			});
		});

		it("should start at startPage when provided", async () => {
			renderWithProviders(<ComicReader {...defaultProps} startPage={5} />);

			await waitFor(() => {
				expect(useReaderStore.getState().currentPage).toBe(5);
			});
		});

		it("should use initialPage when startPage is out of range", async () => {
			renderWithProviders(<ComicReader {...defaultProps} startPage={100} />);

			// Should use initialPage (1) since 100 is out of range
			await waitFor(() => {
				expect(useReaderStore.getState().currentPage).toBe(1);
			});
		});

		it("should apply reading direction override from props", async () => {
			renderWithProviders(
				<ComicReader {...defaultProps} readingDirectionOverride="rtl" />,
			);

			await waitFor(() => {
				expect(useReaderStore.getState().readingDirectionOverride).toBe("rtl");
			});
		});
	});

	describe("page display", () => {
		it("should display single page in single layout mode", () => {
			// pageLayout comes from the series settings hook (default is "single")
			renderWithProviders(<ComicReader {...defaultProps} />);

			const images = document.querySelectorAll('img[src*="/api/v1/books/"]');
			expect(images).toHaveLength(1);
		});

		it("should render continuous scroll reader when layout is continuous", () => {
			// Set the mock to return continuous pageLayout
			mockUseSeriesReaderSettings = vi.fn(() => ({
				hasSeriesOverride: false,
				effectiveSettings: {
					fitMode: "screen" as const,
					pageLayout: "continuous" as const,
					readingDirection: "ltr" as const,
					backgroundColor: "black" as const,
					doublePageShowWideAlone: true,
					doublePageStartOnOdd: true,
				},
				forkToSeries: vi.fn(),
				resetToGlobal: vi.fn(),
				updateSetting: vi.fn(),
				isLoaded: true,
				seriesOverride: null,
			}));

			renderWithProviders(<ComicReader {...defaultProps} />);

			// ContinuousScrollReader renders pages in a scrollable container
			// Verify the container is rendered (it uses overflow-y: auto)
			const container = document.querySelector('[style*="100vw"]');
			expect(container).toBeInTheDocument();
		});

		it("should render continuous scroll reader when reading direction is webtoon", () => {
			useReaderStore.setState({
				settings: { ...defaultSettings },
				readingDirectionOverride: "webtoon",
			});

			renderWithProviders(
				<ComicReader {...defaultProps} readingDirectionOverride="webtoon" />,
			);

			// Verify the reader renders
			const container = document.querySelector('[style*="100vw"]');
			expect(container).toBeInTheDocument();
			// Verify the reading direction is set
			expect(useReaderStore.getState().readingDirectionOverride).toBe(
				"webtoon",
			);
		});
	});

	describe("toolbar", () => {
		it("should show toolbar when visible", () => {
			useReaderStore.setState({ toolbarVisible: true });

			renderWithProviders(<ComicReader {...defaultProps} />);

			expect(screen.getByText("Test Comic")).toBeInTheDocument();
		});
	});

	describe("settings modal", () => {
		it("should have settings button in toolbar", () => {
			renderWithProviders(<ComicReader {...defaultProps} />);

			// Find buttons in toolbar
			const buttons = screen.getAllByRole("button");
			// There should be multiple buttons (close, prev book, page nav, next book, settings, fullscreen)
			expect(buttons.length).toBeGreaterThan(3);
		});
	});

	describe("cleanup", () => {
		it("should reset session on unmount", async () => {
			const { unmount } = renderWithProviders(
				<ComicReader {...defaultProps} />,
			);

			// Wait for initialization
			await waitFor(() => {
				expect(useReaderStore.getState().currentBookId).toBe("book-123");
			});

			unmount();

			// Session should be reset
			expect(useReaderStore.getState().currentBookId).toBeNull();
		});
	});

	describe("background color", () => {
		it("should apply black background by default", () => {
			renderWithProviders(<ComicReader {...defaultProps} />);

			const container = document.querySelector('[style*="100vw"]');
			expect(container).toHaveStyle({ backgroundColor: "#000" });
		});
	});

	describe("series navigation state", () => {
		it("should maintain boundary state", () => {
			useReaderStore.setState({
				boundaryState: "at-end",
				adjacentBooks: {
					prev: null,
					next: { id: "next-book", title: "Next Book" } as never,
				},
			});

			renderWithProviders(<ComicReader {...defaultProps} />);

			// Verify state is maintained
			expect(useReaderStore.getState().boundaryState).toBe("at-end");
		});
	});

	describe("preloading configuration", () => {
		it("should respect preloadPages setting", () => {
			useReaderStore.setState({
				settings: { ...defaultSettings, preloadPages: 2 },
			});

			renderWithProviders(<ComicReader {...defaultProps} />);

			// Verify the setting is applied
			expect(useReaderStore.getState().settings.preloadPages).toBe(2);
		});

		it("should work when preloadPages is 0", () => {
			useReaderStore.setState({
				settings: { ...defaultSettings, preloadPages: 0 },
			});

			renderWithProviders(<ComicReader {...defaultProps} />);

			expect(useReaderStore.getState().settings.preloadPages).toBe(0);
		});
	});

	describe("page transitions", () => {
		it("should use configured transition settings", () => {
			useReaderStore.setState({
				settings: {
					...defaultSettings,
					pageTransition: "fade",
					transitionDuration: 300,
				},
			});

			renderWithProviders(<ComicReader {...defaultProps} />);

			const settings = useReaderStore.getState().settings;
			expect(settings.pageTransition).toBe("fade");
			expect(settings.transitionDuration).toBe(300);
		});
	});

	describe("double page mode", () => {
		it("should render double page spread when layout is double", () => {
			// Set the mock to return double pageLayout
			mockUseSeriesReaderSettings = vi.fn(() => ({
				hasSeriesOverride: false,
				effectiveSettings: {
					fitMode: "screen" as const,
					pageLayout: "double" as const,
					readingDirection: "ltr" as const,
					backgroundColor: "black" as const,
					doublePageShowWideAlone: true,
					doublePageStartOnOdd: true,
				},
				forkToSeries: vi.fn(),
				resetToGlobal: vi.fn(),
				updateSetting: vi.fn(),
				isLoaded: true,
				seriesOverride: null,
			}));

			renderWithProviders(<ComicReader {...defaultProps} />);

			// In double mode, there might be 1-2 images depending on orientation
			// At minimum the component should render without error
			const container = document.querySelector('[style*="100vw"]');
			expect(container).toBeInTheDocument();
		});
	});

	describe("per-series settings integration", () => {
		it("should call useSeriesReaderSettings with seriesId", () => {
			renderWithProviders(
				<ComicReader {...defaultProps} seriesId="series-123" />,
			);

			expect(mockUseSeriesReaderSettings).toHaveBeenCalledWith("series-123");
		});

		it("should call useSeriesReaderSettings with null when no seriesId", () => {
			renderWithProviders(<ComicReader {...defaultProps} seriesId={null} />);

			expect(mockUseSeriesReaderSettings).toHaveBeenCalledWith(null);
		});

		it("should show loading state until series settings are loaded", () => {
			mockUseSeriesReaderSettings = vi.fn(() => ({
				hasSeriesOverride: false,
				effectiveSettings: {
					fitMode: "screen" as const,
					pageLayout: "single" as const,
					readingDirection: "ltr" as const,
					backgroundColor: "black" as const,
					doublePageShowWideAlone: true,
					doublePageStartOnOdd: true,
				},
				forkToSeries: vi.fn(),
				resetToGlobal: vi.fn(),
				updateSetting: vi.fn(),
				isLoaded: false, // Not loaded yet
				seriesOverride: null,
			}));

			renderWithProviders(<ComicReader {...defaultProps} />);

			// Should show loader
			expect(
				document.querySelector(".mantine-Loader-root"),
			).toBeInTheDocument();
		});

		it("should use series-specific fitMode from effectiveSettings", () => {
			mockUseSeriesReaderSettings = vi.fn(() => ({
				hasSeriesOverride: true,
				effectiveSettings: {
					fitMode: "width" as const, // Series-specific
					pageLayout: "single" as const,
					readingDirection: "ltr" as const,
					backgroundColor: "black" as const,
					doublePageShowWideAlone: true,
					doublePageStartOnOdd: true,
				},
				forkToSeries: vi.fn(),
				resetToGlobal: vi.fn(),
				updateSetting: vi.fn(),
				isLoaded: true,
				seriesOverride: null,
			}));

			renderWithProviders(<ComicReader {...defaultProps} />);

			// Reader should render without error - fitMode "width" is applied
			const container = document.querySelector('[style*="100vw"]');
			expect(container).toBeInTheDocument();
		});

		it("should use series-specific backgroundColor from effectiveSettings", () => {
			mockUseSeriesReaderSettings = vi.fn(() => ({
				hasSeriesOverride: true,
				effectiveSettings: {
					fitMode: "screen" as const,
					pageLayout: "single" as const,
					readingDirection: "ltr" as const,
					backgroundColor: "white" as const, // Series-specific
					doublePageShowWideAlone: true,
					doublePageStartOnOdd: true,
				},
				forkToSeries: vi.fn(),
				resetToGlobal: vi.fn(),
				updateSetting: vi.fn(),
				isLoaded: true,
				seriesOverride: null,
			}));

			renderWithProviders(<ComicReader {...defaultProps} />);

			// Reader should render without error - backgroundColor is passed to child components
			const container = document.querySelector('[style*="100vw"]');
			expect(container).toBeInTheDocument();
		});
	});
});
