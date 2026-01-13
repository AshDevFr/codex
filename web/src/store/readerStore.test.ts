import { beforeEach, describe, expect, it } from "vitest";
import {
	selectAdjacentBooks,
	selectBoundaryState,
	selectEffectiveReadingDirection,
	selectHasNextBook,
	selectHasPrevBook,
	selectIsFirstPage,
	selectIsLastPage,
	selectLastNavigationDirection,
	selectPageTransition,
	selectProgressPercent,
	selectTransitionDuration,
	useReaderStore,
} from "./readerStore";

describe("readerStore", () => {
	beforeEach(() => {
		// Reset store to default state before each test
		useReaderStore.setState({
			settings: {
				fitMode: "screen",
				pageLayout: "single",
				readingDirection: "ltr",
				backgroundColor: "black",
				pdfMode: "streaming",
				autoHideToolbar: true,
				toolbarHideDelay: 3000,
				epubTheme: "light",
				epubFontSize: 100,
				epubFontFamily: "default",
				epubLineHeight: 140,
				epubMargin: 10,
				preloadPages: 1,
				doublePageShowWideAlone: true,
				doublePageStartOnOdd: true,
				pageTransition: "slide",
				transitionDuration: 200,
				webtoonSidePadding: 0,
				webtoonPageGap: 0,
			},
			currentPage: 1,
			totalPages: 0,
			isLoading: false,
			toolbarVisible: true,
			isFullscreen: false,
			currentBookId: null,
			readingDirectionOverride: null,
			adjacentBooks: null,
			boundaryState: "none",
			pageOrientations: {},
			lastNavigationDirection: null,
		});
		localStorage.clear();
	});

	describe("initializeReader", () => {
		it("should initialize reader with book data", () => {
			const { initializeReader } = useReaderStore.getState();

			initializeReader("book-123", 100, 50);

			const state = useReaderStore.getState();
			expect(state.currentBookId).toBe("book-123");
			expect(state.totalPages).toBe(100);
			expect(state.currentPage).toBe(50);
			expect(state.isLoading).toBe(false);
			expect(state.toolbarVisible).toBe(true);
		});

		it("should clamp start page to valid range", () => {
			const { initializeReader } = useReaderStore.getState();

			// Start page greater than total
			initializeReader("book-123", 100, 150);
			expect(useReaderStore.getState().currentPage).toBe(100);

			// Start page less than 1
			initializeReader("book-123", 100, 0);
			expect(useReaderStore.getState().currentPage).toBe(1);
		});

		it("should default to page 1 if no start page provided", () => {
			const { initializeReader } = useReaderStore.getState();

			initializeReader("book-123", 100);

			expect(useReaderStore.getState().currentPage).toBe(1);
		});
	});

	describe("navigation", () => {
		beforeEach(() => {
			useReaderStore.getState().initializeReader("book-123", 10, 5);
		});

		it("should go to next page", () => {
			const { nextPage } = useReaderStore.getState();

			nextPage();

			expect(useReaderStore.getState().currentPage).toBe(6);
		});

		it("should not go past last page", () => {
			useReaderStore.setState({ currentPage: 10 });
			const { nextPage } = useReaderStore.getState();

			nextPage();

			expect(useReaderStore.getState().currentPage).toBe(10);
		});

		it("should go to previous page", () => {
			const { prevPage } = useReaderStore.getState();

			prevPage();

			expect(useReaderStore.getState().currentPage).toBe(4);
		});

		it("should not go before first page", () => {
			useReaderStore.setState({ currentPage: 1 });
			const { prevPage } = useReaderStore.getState();

			prevPage();

			expect(useReaderStore.getState().currentPage).toBe(1);
		});

		it("should go to first page", () => {
			const { firstPage } = useReaderStore.getState();

			firstPage();

			expect(useReaderStore.getState().currentPage).toBe(1);
		});

		it("should go to last page", () => {
			const { lastPage } = useReaderStore.getState();

			lastPage();

			expect(useReaderStore.getState().currentPage).toBe(10);
		});

		it("should set specific page within range", () => {
			const { setPage } = useReaderStore.getState();

			setPage(7);

			expect(useReaderStore.getState().currentPage).toBe(7);
		});

		it("should not set page outside range", () => {
			const { setPage } = useReaderStore.getState();

			setPage(15);
			expect(useReaderStore.getState().currentPage).toBe(5);

			setPage(0);
			expect(useReaderStore.getState().currentPage).toBe(5);
		});
	});

	describe("settings", () => {
		it("should set fit mode", () => {
			const { setFitMode } = useReaderStore.getState();

			setFitMode("width");

			expect(useReaderStore.getState().settings.fitMode).toBe("width");
		});

		it("should cycle fit modes", () => {
			const { cycleFitMode } = useReaderStore.getState();

			// Start at "screen"
			expect(useReaderStore.getState().settings.fitMode).toBe("screen");

			cycleFitMode();
			expect(useReaderStore.getState().settings.fitMode).toBe("width");

			cycleFitMode();
			expect(useReaderStore.getState().settings.fitMode).toBe("width-shrink");

			cycleFitMode();
			expect(useReaderStore.getState().settings.fitMode).toBe("height");

			cycleFitMode();
			expect(useReaderStore.getState().settings.fitMode).toBe("original");

			cycleFitMode();
			expect(useReaderStore.getState().settings.fitMode).toBe("screen");
		});

		it("should set page layout", () => {
			const { setPageLayout } = useReaderStore.getState();

			setPageLayout("double");

			expect(useReaderStore.getState().settings.pageLayout).toBe("double");
		});

		it("should set reading direction", () => {
			const { setReadingDirection } = useReaderStore.getState();

			setReadingDirection("rtl");

			expect(useReaderStore.getState().settings.readingDirection).toBe("rtl");
		});

		it("should set background color", () => {
			const { setBackgroundColor } = useReaderStore.getState();

			setBackgroundColor("gray");

			expect(useReaderStore.getState().settings.backgroundColor).toBe("gray");
		});

		it("should cycle background colors", () => {
			const { cycleBackgroundColor } = useReaderStore.getState();

			// Start at "black"
			expect(useReaderStore.getState().settings.backgroundColor).toBe("black");

			cycleBackgroundColor();
			expect(useReaderStore.getState().settings.backgroundColor).toBe("gray");

			cycleBackgroundColor();
			expect(useReaderStore.getState().settings.backgroundColor).toBe("white");

			cycleBackgroundColor();
			expect(useReaderStore.getState().settings.backgroundColor).toBe("black");
		});

		it("should set PDF mode", () => {
			const { setPdfMode } = useReaderStore.getState();

			setPdfMode("native");

			expect(useReaderStore.getState().settings.pdfMode).toBe("native");
		});

		it("should set auto-hide toolbar", () => {
			const { setAutoHideToolbar } = useReaderStore.getState();

			setAutoHideToolbar(false);

			expect(useReaderStore.getState().settings.autoHideToolbar).toBe(false);
		});

		it("should set preload pages", () => {
			const { setPreloadPages } = useReaderStore.getState();

			setPreloadPages(3);

			expect(useReaderStore.getState().settings.preloadPages).toBe(3);
		});

		it("should clamp preload pages to minimum 0", () => {
			const { setPreloadPages } = useReaderStore.getState();

			setPreloadPages(-1);

			expect(useReaderStore.getState().settings.preloadPages).toBe(0);
		});

		it("should clamp preload pages to maximum 5", () => {
			const { setPreloadPages } = useReaderStore.getState();

			setPreloadPages(10);

			expect(useReaderStore.getState().settings.preloadPages).toBe(5);
		});
	});

	describe("UI state", () => {
		it("should toggle toolbar visibility", () => {
			const { toggleToolbar } = useReaderStore.getState();

			expect(useReaderStore.getState().toolbarVisible).toBe(true);

			toggleToolbar();
			expect(useReaderStore.getState().toolbarVisible).toBe(false);

			toggleToolbar();
			expect(useReaderStore.getState().toolbarVisible).toBe(true);
		});

		it("should set toolbar visibility", () => {
			const { setToolbarVisible } = useReaderStore.getState();

			setToolbarVisible(false);
			expect(useReaderStore.getState().toolbarVisible).toBe(false);

			setToolbarVisible(true);
			expect(useReaderStore.getState().toolbarVisible).toBe(true);
		});

		it("should toggle fullscreen", () => {
			const { toggleFullscreen } = useReaderStore.getState();

			expect(useReaderStore.getState().isFullscreen).toBe(false);

			toggleFullscreen();
			expect(useReaderStore.getState().isFullscreen).toBe(true);

			toggleFullscreen();
			expect(useReaderStore.getState().isFullscreen).toBe(false);
		});

		it("should set fullscreen", () => {
			const { setFullscreen } = useReaderStore.getState();

			setFullscreen(true);
			expect(useReaderStore.getState().isFullscreen).toBe(true);

			setFullscreen(false);
			expect(useReaderStore.getState().isFullscreen).toBe(false);
		});
	});

	describe("reading direction override", () => {
		it("should set reading direction override", () => {
			const { setReadingDirectionOverride } = useReaderStore.getState();

			setReadingDirectionOverride("rtl");

			expect(useReaderStore.getState().readingDirectionOverride).toBe("rtl");
		});

		it("should clear reading direction override", () => {
			useReaderStore.setState({ readingDirectionOverride: "rtl" });
			const { setReadingDirectionOverride } = useReaderStore.getState();

			setReadingDirectionOverride(null);

			expect(useReaderStore.getState().readingDirectionOverride).toBeNull();
		});
	});

	describe("resetSession", () => {
		it("should reset session state but keep settings", () => {
			// Set up some state
			useReaderStore.setState({
				currentPage: 50,
				totalPages: 100,
				isLoading: true,
				toolbarVisible: false,
				isFullscreen: true,
				currentBookId: "book-123",
				readingDirectionOverride: "rtl",
				adjacentBooks: {
					prev: { id: "book-0", title: "Prev", pageCount: 50 },
					next: { id: "book-2", title: "Next", pageCount: 100 },
				},
				boundaryState: "at-end",
				settings: {
					...useReaderStore.getState().settings,
					fitMode: "width",
				},
			});

			const { resetSession } = useReaderStore.getState();
			resetSession();

			const state = useReaderStore.getState();
			// Session state should be reset
			expect(state.currentPage).toBe(1);
			expect(state.totalPages).toBe(0);
			expect(state.isLoading).toBe(false);
			expect(state.toolbarVisible).toBe(true);
			expect(state.isFullscreen).toBe(false);
			expect(state.currentBookId).toBeNull();
			expect(state.readingDirectionOverride).toBeNull();
			expect(state.adjacentBooks).toBeNull();
			expect(state.boundaryState).toBe("none");
			// Settings should be preserved
			expect(state.settings.fitMode).toBe("width");
		});
	});

	describe("selectors", () => {
		describe("selectEffectiveReadingDirection", () => {
			it("should return override when set", () => {
				useReaderStore.setState({
					settings: {
						...useReaderStore.getState().settings,
						readingDirection: "ltr",
					},
					readingDirectionOverride: "rtl",
				});

				const result = selectEffectiveReadingDirection(useReaderStore.getState());

				expect(result).toBe("rtl");
			});

			it("should return default when no override", () => {
				useReaderStore.setState({
					settings: {
						...useReaderStore.getState().settings,
						readingDirection: "ltr",
					},
					readingDirectionOverride: null,
				});

				const result = selectEffectiveReadingDirection(useReaderStore.getState());

				expect(result).toBe("ltr");
			});
		});

		describe("selectProgressPercent", () => {
			it("should calculate progress percentage", () => {
				useReaderStore.setState({ currentPage: 25, totalPages: 100 });

				const result = selectProgressPercent(useReaderStore.getState());

				expect(result).toBe(25);
			});

			it("should return 0 when no pages", () => {
				useReaderStore.setState({ currentPage: 1, totalPages: 0 });

				const result = selectProgressPercent(useReaderStore.getState());

				expect(result).toBe(0);
			});

			it("should round to nearest integer", () => {
				useReaderStore.setState({ currentPage: 1, totalPages: 3 });

				const result = selectProgressPercent(useReaderStore.getState());

				expect(result).toBe(33); // 1/3 = 33.33... -> 33
			});
		});

		describe("selectIsFirstPage", () => {
			it("should return true when on first page", () => {
				useReaderStore.setState({ currentPage: 1 });

				expect(selectIsFirstPage(useReaderStore.getState())).toBe(true);
			});

			it("should return false when not on first page", () => {
				useReaderStore.setState({ currentPage: 5 });

				expect(selectIsFirstPage(useReaderStore.getState())).toBe(false);
			});
		});

		describe("selectIsLastPage", () => {
			it("should return true when on last page", () => {
				useReaderStore.setState({ currentPage: 10, totalPages: 10 });

				expect(selectIsLastPage(useReaderStore.getState())).toBe(true);
			});

			it("should return false when not on last page", () => {
				useReaderStore.setState({ currentPage: 5, totalPages: 10 });

				expect(selectIsLastPage(useReaderStore.getState())).toBe(false);
			});
		});

		describe("selectHasPrevBook", () => {
			it("should return false when no adjacent books", () => {
				expect(selectHasPrevBook(useReaderStore.getState())).toBe(false);
			});

			it("should return false when prev is null", () => {
				useReaderStore.setState({
					adjacentBooks: { prev: null, next: { id: "book-2", title: "Next", pageCount: 100 } },
				});

				expect(selectHasPrevBook(useReaderStore.getState())).toBe(false);
			});

			it("should return true when prev exists", () => {
				useReaderStore.setState({
					adjacentBooks: { prev: { id: "book-0", title: "Prev", pageCount: 50 }, next: null },
				});

				expect(selectHasPrevBook(useReaderStore.getState())).toBe(true);
			});
		});

		describe("selectHasNextBook", () => {
			it("should return false when no adjacent books", () => {
				expect(selectHasNextBook(useReaderStore.getState())).toBe(false);
			});

			it("should return false when next is null", () => {
				useReaderStore.setState({
					adjacentBooks: { prev: { id: "book-0", title: "Prev", pageCount: 50 }, next: null },
				});

				expect(selectHasNextBook(useReaderStore.getState())).toBe(false);
			});

			it("should return true when next exists", () => {
				useReaderStore.setState({
					adjacentBooks: { prev: null, next: { id: "book-2", title: "Next", pageCount: 100 } },
				});

				expect(selectHasNextBook(useReaderStore.getState())).toBe(true);
			});
		});

		describe("selectAdjacentBooks", () => {
			it("should return null when no adjacent books", () => {
				expect(selectAdjacentBooks(useReaderStore.getState())).toBeNull();
			});

			it("should return adjacent books when set", () => {
				const adjacentBooks = {
					prev: { id: "book-0", title: "Prev", pageCount: 50 },
					next: { id: "book-2", title: "Next", pageCount: 100 },
				};
				useReaderStore.setState({ adjacentBooks });

				expect(selectAdjacentBooks(useReaderStore.getState())).toEqual(adjacentBooks);
			});
		});

		describe("selectBoundaryState", () => {
			it("should return none by default", () => {
				expect(selectBoundaryState(useReaderStore.getState())).toBe("none");
			});

			it("should return at-start when set", () => {
				useReaderStore.setState({ boundaryState: "at-start" });

				expect(selectBoundaryState(useReaderStore.getState())).toBe("at-start");
			});

			it("should return at-end when set", () => {
				useReaderStore.setState({ boundaryState: "at-end" });

				expect(selectBoundaryState(useReaderStore.getState())).toBe("at-end");
			});
		});
	});

	describe("series navigation actions", () => {
		describe("setAdjacentBooks", () => {
			it("should set adjacent books", () => {
				const { setAdjacentBooks } = useReaderStore.getState();
				const adjacentBooks = {
					prev: { id: "book-0", title: "Prev", pageCount: 50 },
					next: { id: "book-2", title: "Next", pageCount: 100 },
				};

				setAdjacentBooks(adjacentBooks);

				expect(useReaderStore.getState().adjacentBooks).toEqual(adjacentBooks);
			});

			it("should set adjacent books to null", () => {
				useReaderStore.setState({
					adjacentBooks: {
						prev: { id: "book-0", title: "Prev", pageCount: 50 },
						next: null,
					},
				});
				const { setAdjacentBooks } = useReaderStore.getState();

				setAdjacentBooks(null);

				expect(useReaderStore.getState().adjacentBooks).toBeNull();
			});
		});

		describe("setBoundaryState", () => {
			it("should set boundary state to at-start", () => {
				const { setBoundaryState } = useReaderStore.getState();

				setBoundaryState("at-start");

				expect(useReaderStore.getState().boundaryState).toBe("at-start");
			});

			it("should set boundary state to at-end", () => {
				const { setBoundaryState } = useReaderStore.getState();

				setBoundaryState("at-end");

				expect(useReaderStore.getState().boundaryState).toBe("at-end");
			});
		});

		describe("clearBoundaryState", () => {
			it("should clear boundary state to none", () => {
				useReaderStore.setState({ boundaryState: "at-end" });
				const { clearBoundaryState } = useReaderStore.getState();

				clearBoundaryState();

				expect(useReaderStore.getState().boundaryState).toBe("none");
			});
		});
	});

	describe("double-page settings", () => {
		describe("setDoublePageShowWideAlone", () => {
			it("should set doublePageShowWideAlone to true", () => {
				const { setDoublePageShowWideAlone } = useReaderStore.getState();

				setDoublePageShowWideAlone(true);

				expect(
					useReaderStore.getState().settings.doublePageShowWideAlone,
				).toBe(true);
			});

			it("should set doublePageShowWideAlone to false", () => {
				const { setDoublePageShowWideAlone } = useReaderStore.getState();

				setDoublePageShowWideAlone(false);

				expect(
					useReaderStore.getState().settings.doublePageShowWideAlone,
				).toBe(false);
			});
		});

		describe("setDoublePageStartOnOdd", () => {
			it("should set doublePageStartOnOdd to true", () => {
				const { setDoublePageStartOnOdd } = useReaderStore.getState();

				setDoublePageStartOnOdd(true);

				expect(useReaderStore.getState().settings.doublePageStartOnOdd).toBe(
					true,
				);
			});

			it("should set doublePageStartOnOdd to false", () => {
				const { setDoublePageStartOnOdd } = useReaderStore.getState();

				setDoublePageStartOnOdd(false);

				expect(useReaderStore.getState().settings.doublePageStartOnOdd).toBe(
					false,
				);
			});
		});
	});

	describe("page orientation actions", () => {
		describe("setPageOrientation", () => {
			it("should set page orientation for a specific page", () => {
				const { setPageOrientation } = useReaderStore.getState();

				setPageOrientation(1, "portrait");

				expect(useReaderStore.getState().pageOrientations[1]).toBe("portrait");
			});

			it("should set multiple page orientations", () => {
				const { setPageOrientation } = useReaderStore.getState();

				setPageOrientation(1, "portrait");
				setPageOrientation(2, "landscape");
				setPageOrientation(3, "portrait");

				const state = useReaderStore.getState();
				expect(state.pageOrientations[1]).toBe("portrait");
				expect(state.pageOrientations[2]).toBe("landscape");
				expect(state.pageOrientations[3]).toBe("portrait");
			});

			it("should overwrite existing page orientation", () => {
				const { setPageOrientation } = useReaderStore.getState();

				setPageOrientation(1, "portrait");
				setPageOrientation(1, "landscape");

				expect(useReaderStore.getState().pageOrientations[1]).toBe("landscape");
			});
		});

		describe("clearPageOrientations", () => {
			it("should clear all page orientations", () => {
				const { setPageOrientation, clearPageOrientations } =
					useReaderStore.getState();

				setPageOrientation(1, "portrait");
				setPageOrientation(2, "landscape");
				clearPageOrientations();

				expect(useReaderStore.getState().pageOrientations).toEqual({});
			});
		});
	});

	describe("resetSession with double-page state", () => {
		it("should reset pageOrientations when resetting session", () => {
			const { setPageOrientation, resetSession } = useReaderStore.getState();

			setPageOrientation(1, "portrait");
			setPageOrientation(2, "landscape");
			resetSession();

			expect(useReaderStore.getState().pageOrientations).toEqual({});
		});

		it("should preserve double-page settings when resetting session", () => {
			useReaderStore.setState({
				settings: {
					...useReaderStore.getState().settings,
					doublePageShowWideAlone: false,
					doublePageStartOnOdd: false,
				},
			});

			const { resetSession } = useReaderStore.getState();
			resetSession();

			const state = useReaderStore.getState();
			expect(state.settings.doublePageShowWideAlone).toBe(false);
			expect(state.settings.doublePageStartOnOdd).toBe(false);
		});
	});

	describe("page transition settings", () => {
		describe("setPageTransition", () => {
			it("should set page transition to none", () => {
				const { setPageTransition } = useReaderStore.getState();

				setPageTransition("none");

				expect(useReaderStore.getState().settings.pageTransition).toBe("none");
			});

			it("should set page transition to fade", () => {
				const { setPageTransition } = useReaderStore.getState();

				setPageTransition("fade");

				expect(useReaderStore.getState().settings.pageTransition).toBe("fade");
			});

			it("should set page transition to slide", () => {
				const { setPageTransition } = useReaderStore.getState();

				setPageTransition("slide");

				expect(useReaderStore.getState().settings.pageTransition).toBe("slide");
			});
		});

		describe("setTransitionDuration", () => {
			it("should set transition duration", () => {
				const { setTransitionDuration } = useReaderStore.getState();

				setTransitionDuration(300);

				expect(useReaderStore.getState().settings.transitionDuration).toBe(300);
			});

			it("should clamp transition duration to minimum 50ms", () => {
				const { setTransitionDuration } = useReaderStore.getState();

				setTransitionDuration(10);

				expect(useReaderStore.getState().settings.transitionDuration).toBe(50);
			});

			it("should clamp transition duration to maximum 500ms", () => {
				const { setTransitionDuration } = useReaderStore.getState();

				setTransitionDuration(1000);

				expect(useReaderStore.getState().settings.transitionDuration).toBe(500);
			});
		});

		describe("setLastNavigationDirection", () => {
			it("should set last navigation direction to next", () => {
				const { setLastNavigationDirection } = useReaderStore.getState();

				setLastNavigationDirection("next");

				expect(useReaderStore.getState().lastNavigationDirection).toBe("next");
			});

			it("should set last navigation direction to prev", () => {
				const { setLastNavigationDirection } = useReaderStore.getState();

				setLastNavigationDirection("prev");

				expect(useReaderStore.getState().lastNavigationDirection).toBe("prev");
			});

			it("should set last navigation direction to null", () => {
				useReaderStore.setState({ lastNavigationDirection: "next" });
				const { setLastNavigationDirection } = useReaderStore.getState();

				setLastNavigationDirection(null);

				expect(useReaderStore.getState().lastNavigationDirection).toBeNull();
			});
		});
	});

	describe("page transition selectors", () => {
		describe("selectPageTransition", () => {
			it("should return page transition setting", () => {
				useReaderStore.setState({
					settings: {
						...useReaderStore.getState().settings,
						pageTransition: "slide",
					},
				});

				expect(selectPageTransition(useReaderStore.getState())).toBe("slide");
			});
		});

		describe("selectTransitionDuration", () => {
			it("should return transition duration setting", () => {
				useReaderStore.setState({
					settings: {
						...useReaderStore.getState().settings,
						transitionDuration: 350,
					},
				});

				expect(selectTransitionDuration(useReaderStore.getState())).toBe(350);
			});
		});

		describe("selectLastNavigationDirection", () => {
			it("should return null by default", () => {
				expect(selectLastNavigationDirection(useReaderStore.getState())).toBeNull();
			});

			it("should return next when set", () => {
				useReaderStore.setState({ lastNavigationDirection: "next" });

				expect(selectLastNavigationDirection(useReaderStore.getState())).toBe("next");
			});

			it("should return prev when set", () => {
				useReaderStore.setState({ lastNavigationDirection: "prev" });

				expect(selectLastNavigationDirection(useReaderStore.getState())).toBe("prev");
			});
		});
	});

	describe("resetSession with transition state", () => {
		it("should reset lastNavigationDirection when resetting session", () => {
			useReaderStore.setState({ lastNavigationDirection: "next" });
			const { resetSession } = useReaderStore.getState();

			resetSession();

			expect(useReaderStore.getState().lastNavigationDirection).toBeNull();
		});

		it("should preserve transition settings when resetting session", () => {
			useReaderStore.setState({
				settings: {
					...useReaderStore.getState().settings,
					pageTransition: "slide",
					transitionDuration: 300,
				},
			});

			const { resetSession } = useReaderStore.getState();
			resetSession();

			const state = useReaderStore.getState();
			expect(state.settings.pageTransition).toBe("slide");
			expect(state.settings.transitionDuration).toBe(300);
		});
	});

	describe("EPUB typography settings", () => {
		describe("setEpubFontFamily", () => {
			it("should set font family to default", () => {
				const { setEpubFontFamily } = useReaderStore.getState();

				setEpubFontFamily("default");

				expect(useReaderStore.getState().settings.epubFontFamily).toBe("default");
			});

			it("should set font family to serif", () => {
				const { setEpubFontFamily } = useReaderStore.getState();

				setEpubFontFamily("serif");

				expect(useReaderStore.getState().settings.epubFontFamily).toBe("serif");
			});

			it("should set font family to sans-serif", () => {
				const { setEpubFontFamily } = useReaderStore.getState();

				setEpubFontFamily("sans-serif");

				expect(useReaderStore.getState().settings.epubFontFamily).toBe("sans-serif");
			});

			it("should set font family to monospace", () => {
				const { setEpubFontFamily } = useReaderStore.getState();

				setEpubFontFamily("monospace");

				expect(useReaderStore.getState().settings.epubFontFamily).toBe("monospace");
			});

			it("should set font family to dyslexic", () => {
				const { setEpubFontFamily } = useReaderStore.getState();

				setEpubFontFamily("dyslexic");

				expect(useReaderStore.getState().settings.epubFontFamily).toBe("dyslexic");
			});
		});

		describe("setEpubLineHeight", () => {
			it("should set line height", () => {
				const { setEpubLineHeight } = useReaderStore.getState();

				setEpubLineHeight(160);

				expect(useReaderStore.getState().settings.epubLineHeight).toBe(160);
			});

			it("should clamp line height to minimum 100%", () => {
				const { setEpubLineHeight } = useReaderStore.getState();

				setEpubLineHeight(50);

				expect(useReaderStore.getState().settings.epubLineHeight).toBe(100);
			});

			it("should clamp line height to maximum 250%", () => {
				const { setEpubLineHeight } = useReaderStore.getState();

				setEpubLineHeight(300);

				expect(useReaderStore.getState().settings.epubLineHeight).toBe(250);
			});
		});

		describe("setEpubMargin", () => {
			it("should set margin", () => {
				const { setEpubMargin } = useReaderStore.getState();

				setEpubMargin(15);

				expect(useReaderStore.getState().settings.epubMargin).toBe(15);
			});

			it("should clamp margin to minimum 0%", () => {
				const { setEpubMargin } = useReaderStore.getState();

				setEpubMargin(-10);

				expect(useReaderStore.getState().settings.epubMargin).toBe(0);
			});

			it("should clamp margin to maximum 30%", () => {
				const { setEpubMargin } = useReaderStore.getState();

				setEpubMargin(50);

				expect(useReaderStore.getState().settings.epubMargin).toBe(30);
			});
		});

		describe("setEpubFontSize", () => {
			it("should set font size", () => {
				const { setEpubFontSize } = useReaderStore.getState();

				setEpubFontSize(120);

				expect(useReaderStore.getState().settings.epubFontSize).toBe(120);
			});

			it("should clamp font size to minimum 50%", () => {
				const { setEpubFontSize } = useReaderStore.getState();

				setEpubFontSize(30);

				expect(useReaderStore.getState().settings.epubFontSize).toBe(50);
			});

			it("should clamp font size to maximum 200%", () => {
				const { setEpubFontSize } = useReaderStore.getState();

				setEpubFontSize(250);

				expect(useReaderStore.getState().settings.epubFontSize).toBe(200);
			});
		});

		describe("setEpubTheme", () => {
			it("should set theme to light", () => {
				const { setEpubTheme } = useReaderStore.getState();

				setEpubTheme("light");

				expect(useReaderStore.getState().settings.epubTheme).toBe("light");
			});

			it("should set theme to sepia", () => {
				const { setEpubTheme } = useReaderStore.getState();

				setEpubTheme("sepia");

				expect(useReaderStore.getState().settings.epubTheme).toBe("sepia");
			});

			it("should set theme to dark", () => {
				const { setEpubTheme } = useReaderStore.getState();

				setEpubTheme("dark");

				expect(useReaderStore.getState().settings.epubTheme).toBe("dark");
			});

			it("should set theme to mint", () => {
				const { setEpubTheme } = useReaderStore.getState();

				setEpubTheme("mint");

				expect(useReaderStore.getState().settings.epubTheme).toBe("mint");
			});

			it("should set theme to slate", () => {
				const { setEpubTheme } = useReaderStore.getState();

				setEpubTheme("slate");

				expect(useReaderStore.getState().settings.epubTheme).toBe("slate");
			});
		});
	});

	describe("EPUB settings persistence", () => {
		it("should preserve EPUB typography settings when resetting session", () => {
			useReaderStore.setState({
				settings: {
					...useReaderStore.getState().settings,
					epubFontFamily: "serif",
					epubLineHeight: 180,
					epubMargin: 20,
					epubFontSize: 120,
					epubTheme: "dark",
				},
			});

			const { resetSession } = useReaderStore.getState();
			resetSession();

			const state = useReaderStore.getState();
			expect(state.settings.epubFontFamily).toBe("serif");
			expect(state.settings.epubLineHeight).toBe(180);
			expect(state.settings.epubMargin).toBe(20);
			expect(state.settings.epubFontSize).toBe(120);
			expect(state.settings.epubTheme).toBe("dark");
		});
	});
});
