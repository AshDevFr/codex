import { Box, Center, Loader } from "@mantine/core";
import type { Location, NavItem, Rendition } from "epubjs";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
	type IReactReaderStyle,
	ReactReader,
	ReactReaderStyle,
} from "react-reader";

import { useReaderStore } from "@/store/readerStore";

import { EpubBookmarks } from "./EpubBookmarks";
import { EpubReaderSettings } from "./EpubReaderSettings";
import { EpubSearch, type SearchResult } from "./EpubSearch";
import { EpubTableOfContents } from "./EpubTableOfContents";
import { useEpubBookmarks } from "./hooks/useEpubBookmarks";
import { useEpubProgress } from "./hooks/useEpubProgress";
import { ReaderToolbar } from "./ReaderToolbar";

// EPUB theme definitions
const EPUB_THEMES = {
	light: {
		body: {
			background: "#ffffff",
			color: "#1a1a1a",
		},
	},
	sepia: {
		body: {
			background: "#f4ecd8",
			color: "#5b4636",
		},
	},
	dark: {
		body: {
			background: "#1a1a1a",
			color: "#e0e0e0",
		},
	},
	mint: {
		body: {
			background: "#e8f5e9",
			color: "#1b5e20",
		},
	},
	slate: {
		body: {
			background: "#263238",
			color: "#b0bec5",
		},
	},
} as const;

export type EpubTheme = keyof typeof EPUB_THEMES;

/**
 * Generate ReactReader container styles based on the current theme.
 * This ensures the reader container background matches the EPUB content theme.
 */
function getReaderStyles(theme: EpubTheme): IReactReaderStyle {
	const themeColors = EPUB_THEMES[theme] ?? EPUB_THEMES.light;
	const isDark = theme === "dark" || theme === "slate";

	return {
		...ReactReaderStyle,
		readerArea: {
			...ReactReaderStyle.readerArea,
			backgroundColor: themeColors.body.background,
			transition: undefined,
		},
		arrow: {
			...ReactReaderStyle.arrow,
			color: isDark ? "#e0e0e0" : "#333",
		},
		arrowHover: {
			...ReactReaderStyle.arrowHover,
			color: isDark ? "#fff" : "#000",
		},
	};
}

interface EpubReaderProps {
	/** Book ID */
	bookId: string;
	/** Book title for display */
	title: string;
	/** Total pages in the book (for progress calculation) */
	totalPages: number;
	/** Starting percentage from URL parameter (0.0-1.0, overrides saved progress) */
	startPercent?: number;
	/** Callback when reader should close */
	onClose: () => void;
}

/**
 * EPUB reader component using react-reader (epub.js wrapper).
 *
 * Features:
 * - Reflowable text rendering
 * - Multiple themes (light, sepia, dark, mint, slate)
 * - Font size adjustment
 * - Progress tracking via CFI
 * - Keyboard navigation
 * - Fullscreen support
 */
export function EpubReader({
	bookId,
	title,
	totalPages,
	startPercent,
	onClose,
}: EpubReaderProps) {
	const containerRef = useRef<HTMLDivElement>(null);
	const renditionRef = useRef<Rendition | null>(null);
	const hideTimeoutRef = useRef<NodeJS.Timeout | null>(null);
	const initialLocationLoadedRef = useRef(false);
	const currentPercentageRef = useRef<number>(0);

	// CFI-based progress tracking (also syncs to backend)
	const {
		getSavedLocation,
		initialPercentage,
		isLoadingProgress,
		saveLocation,
	} = useEpubProgress({
		bookId,
		totalPages,
	});

	// Bookmarks with notes
	const {
		bookmarks,
		addBookmark,
		updateBookmark,
		removeBookmark,
		isBookmarked,
		getBookmarkByCfi,
	} = useEpubBookmarks({ bookId });

	// Use ref for saveLocation to avoid re-creating handleGetRendition
	const saveLocationRef = useRef(saveLocation);
	saveLocationRef.current = saveLocation;

	// Use ref for totalPages to access in callbacks
	const totalPagesRef = useRef(totalPages);
	totalPagesRef.current = totalPages;

	// Local state - initialize with saved CFI location from localStorage
	// Note: startPercent from URL is handled after locations are generated
	const [location, setLocation] = useState<string | number>(() => {
		// If startPercent is provided, don't load from localStorage - we'll navigate after locations are ready
		if (startPercent != null && startPercent >= 0 && startPercent <= 1) {
			return 0; // Start at 0, will navigate to startPercent after locations are generated
		}
		const saved = getSavedLocation();
		if (saved) {
			initialLocationLoadedRef.current = true;
			return saved;
		}
		return 0;
	});
	const [hasAppliedStartPercent, setHasAppliedStartPercent] = useState(false);
	const [hasAppliedApiProgress, setHasAppliedApiProgress] = useState(false);
	const [locationsReady, setLocationsReady] = useState(false);
	const [isLoading, setIsLoading] = useState(true);
	const [settingsOpened, setSettingsOpened] = useState(false);
	const [tocOpened, setTocOpened] = useState(false);
	const [bookmarksOpened, setBookmarksOpened] = useState(false);
	const [searchOpened, setSearchOpened] = useState(false);
	const [toc, setToc] = useState<NavItem[]>([]);
	const [currentHref, setCurrentHref] = useState<string | undefined>();
	const [currentCfi, setCurrentCfi] = useState<string | null>(null);
	const [currentChapterTitle, setCurrentChapterTitle] = useState<string | undefined>();

	// EPUB-specific settings from store
	const epubTheme = useReaderStore((state) => state.settings.epubTheme);
	const epubFontSize = useReaderStore((state) => state.settings.epubFontSize);

	// Use refs for initial styles to avoid re-creating handleGetRendition
	const epubThemeRef = useRef(epubTheme);
	const epubFontSizeRef = useRef(epubFontSize);
	epubThemeRef.current = epubTheme;
	epubFontSizeRef.current = epubFontSize;

	// Memoize reader styles based on theme
	const readerStyles = useMemo(() => getReaderStyles(epubTheme), [epubTheme]);

	// Reader store state
	const toolbarVisible = useReaderStore((state) => state.toolbarVisible);
	const isFullscreen = useReaderStore((state) => state.isFullscreen);
	const autoHideToolbar = useReaderStore(
		(state) => state.settings.autoHideToolbar,
	);
	const toolbarHideDelay = useReaderStore(
		(state) => state.settings.toolbarHideDelay,
	);

	// Reader store actions
	const setToolbarVisible = useReaderStore((state) => state.setToolbarVisible);
	const setFullscreen = useReaderStore((state) => state.setFullscreen);
	const toggleToolbar = useReaderStore((state) => state.toggleToolbar);

	// Generate EPUB file URL
	const epubUrl = `/api/v1/books/${bookId}/file`;

	// Track if we need to wait for startPercent navigation before showing content
	const needsStartPercentNavigation = startPercent != null && startPercent >= 0 && startPercent <= 1;
	const startPercentAppliedRef = useRef(!needsStartPercentNavigation);

	// Handle location change (CFI-based progress)
	// Note: Progress is saved in the 'relocated' event handler below,
	// where we have access to the accurate percentage value
	const handleLocationChange = useCallback((epubcfi: string) => {
		setLocation(epubcfi);
		// Don't clear loading here - let the relocated event handler do it
		// This prevents showing content before startPage navigation completes
	}, []);

	// Apply theme to rendition
	useEffect(() => {
		if (renditionRef.current?.themes) {
			const theme = EPUB_THEMES[epubTheme] ?? EPUB_THEMES.light;
			renditionRef.current.themes.override("background", theme.body.background);
			renditionRef.current.themes.override("color", theme.body.color);
		}
	}, [epubTheme]);

	// Apply font size to rendition
	useEffect(() => {
		if (renditionRef.current?.themes) {
			renditionRef.current.themes.fontSize(`${epubFontSize}%`);
		}
	}, [epubFontSize]);

	// Apply startPercent from URL (highest priority - overrides saved progress)
	useEffect(() => {
		if (
			locationsReady &&
			!hasAppliedStartPercent &&
			startPercent != null &&
			startPercent >= 0 &&
			startPercent <= 1 &&
			renditionRef.current
		) {
			// Navigate directly to percentage
			const book = renditionRef.current.book;
			if (book?.locations?.length()) {
				const cfi = book.locations.cfiFromPercentage(startPercent);
				if (cfi) {
					setLocation(cfi);
				}
			}
			setHasAppliedStartPercent(true);
			startPercentAppliedRef.current = true;
			// Clear loading now that we've navigated to the correct position
			setIsLoading(false);
		}
	}, [locationsReady, hasAppliedStartPercent, startPercent]);

	// Apply API progress for cross-device sync (only if no localStorage CFI and no startPercent)
	useEffect(() => {
		if (
			locationsReady &&
			!isLoadingProgress &&
			initialPercentage !== null &&
			!hasAppliedApiProgress &&
			!hasAppliedStartPercent &&
			!initialLocationLoadedRef.current &&
			renditionRef.current &&
			startPercent == null // Don't apply API progress if startPercent is provided
		) {
			// Navigate to percentage-based location from API
			const book = renditionRef.current.book;
			if (book?.locations?.length()) {
				const cfi = book.locations.cfiFromPercentage(initialPercentage);
				if (cfi) {
					setLocation(cfi);
				}
			}
			setHasAppliedApiProgress(true);
		}
	}, [locationsReady, isLoadingProgress, initialPercentage, hasAppliedApiProgress, hasAppliedStartPercent, startPercent]);

	// Ref for onClose to keep handleGetRendition stable
	const onCloseRef = useRef(onClose);
	onCloseRef.current = onClose;

	// Get rendition reference from ReactReader
	// This callback should be stable to prevent ReactReader from re-initializing
	const handleGetRendition = useCallback((rendition: Rendition) => {
		renditionRef.current = rendition;

		// Apply initial theme - wait for rendition to be ready
		// The themes object may not be initialized immediately
		const applyInitialStyles = () => {
			if (!rendition.themes) {
				// Themes not ready yet, try again shortly
				setTimeout(applyInitialStyles, 50);
				return;
			}
			const theme = EPUB_THEMES[epubThemeRef.current] ?? EPUB_THEMES.light;
			rendition.themes.override("background", theme.body.background);
			rendition.themes.override("color", theme.body.color);
			rendition.themes.fontSize(`${epubFontSizeRef.current}%`);
		};
		applyInitialStyles();

		// Load table of contents
		rendition.book.loaded.navigation.then((nav) => {
			setToc(nav.toc);
		});

		// Track whether locations have been generated (local variable for event handler)
		let locationsGenerated = false;

		// Generate locations for percentage calculation
		// This is required before we can get accurate percentages
		rendition.book.ready
			.then(() => {
				return rendition.book.locations.generate(1024);
			})
			.then(() => {
				locationsGenerated = true;
				setLocationsReady(true);
			});

		// Track current chapter for TOC highlighting and save progress
		rendition.on("relocated", (location: Location) => {
			setCurrentHref(location.start.href);
			// Only clear loading if we don't need to wait for startPercent navigation
			if (startPercentAppliedRef.current) {
				setIsLoading(false);
			}

			// Get percentage from book locations using the CFI
			const cfi = location.start.cfi;

			// Update current CFI for bookmark functionality
			setCurrentCfi(cfi);

			// Find chapter title for the current location
			const findChapterTitle = (
				items: NavItem[],
				href: string,
			): string | undefined => {
				for (const item of items) {
					if (item.href === href || href.startsWith(item.href.split("#")[0])) {
						return item.label;
					}
					if (item.subitems) {
						const found = findChapterTitle(item.subitems, href);
						if (found) return found;
					}
				}
				return undefined;
			};

			// Get TOC from navigation and find chapter
			rendition.book.loaded.navigation.then((nav) => {
				const chapterTitle = findChapterTitle(nav.toc, location.start.href);
				setCurrentChapterTitle(chapterTitle);
			});

			// Only save progress once locations are generated (for accurate percentage)
			if (!cfi || !locationsGenerated) {
				return;
			}

			// Calculate percentage from CFI using locations
			const calculated = rendition.book.locations.percentageFromCfi(cfi);
			const percentage = typeof calculated === "number" ? calculated : 0;

			currentPercentageRef.current = percentage;

			// Save progress - the hook handles debouncing and duplicate detection
			// Note: percentage can be 0 at the start of the book, which is valid
			saveLocationRef.current(cfi, percentage);
		});

	}, []);

	// Handle TOC navigation
	const handleTocNavigate = useCallback((href: string) => {
		renditionRef.current?.display(href);
	}, []);

	// Handle bookmark navigation
	const handleBookmarkNavigate = useCallback((cfi: string) => {
		renditionRef.current?.display(cfi);
	}, []);

	// Check if current location is bookmarked
	const isCurrentLocationBookmarked = currentCfi ? isBookmarked(currentCfi) : false;

	// Handle adding bookmark at current location
	const handleAddBookmark = useCallback(() => {
		if (!currentCfi) return;

		// Try to get a text excerpt from the current selection or visible text
		let excerpt: string | undefined;
		try {
			// Access manager through type assertion since it's not in the public type definitions
			const rendition = renditionRef.current as Rendition & {
				manager?: { getContents?: () => Array<{ window?: Window }> };
			};
			const selection = rendition?.manager?.getContents?.()?.[0]?.window?.getSelection?.();
			if (selection?.toString().trim()) {
				excerpt = selection.toString().trim().substring(0, 100);
			}
		} catch {
			// Ignore errors getting selection
		}

		addBookmark({
			cfi: currentCfi,
			percentage: currentPercentageRef.current,
			note: "",
			chapterTitle: currentChapterTitle,
			excerpt,
		});
	}, [currentCfi, currentChapterTitle, addBookmark]);

	// Handle removing bookmark at current location
	const handleRemoveCurrentBookmark = useCallback(() => {
		if (!currentCfi) return;
		const bookmark = getBookmarkByCfi(currentCfi);
		if (bookmark) {
			removeBookmark(bookmark.id);
		}
	}, [currentCfi, getBookmarkByCfi, removeBookmark]);

	// Toggle bookmark at current location (for keyboard shortcut)
	const handleToggleBookmark = useCallback(() => {
		if (isCurrentLocationBookmarked) {
			handleRemoveCurrentBookmark();
		} else {
			handleAddBookmark();
		}
	}, [isCurrentLocationBookmarked, handleAddBookmark, handleRemoveCurrentBookmark]);

	// Handle search within EPUB
	const handleSearch = useCallback(
		async (query: string): Promise<SearchResult[]> => {
			if (!renditionRef.current || !query.trim()) {
				return [];
			}

			try {
				const book = renditionRef.current.book;
				// epub.js search method exists but isn't in TypeScript types
				// Use type assertion to access it
				const bookWithSearch = book as typeof book & {
					search: (query: string) => Promise<Array<{ cfi: string; excerpt: string }>>;
				};

				// epub.js search returns an array of results with cfi and excerpt
				const results = await bookWithSearch.search(query);

				// Map results to our format and include chapter info
				return results.map((result: { cfi: string; excerpt: string }) => {
					// Try to find chapter title from the CFI
					let chapter: string | undefined;
					try {
						const section = book.spine.get(result.cfi);
						if (section) {
							const navItem = toc.find(
								(item) =>
									section.href === item.href ||
									section.href.startsWith(item.href.split("#")[0])
							);
							if (navItem) {
								chapter = navItem.label;
							}
						}
					} catch {
						// Ignore errors finding chapter
					}

					return {
						cfi: result.cfi,
						excerpt: result.excerpt,
						chapter,
					};
				});
			} catch (error) {
				console.error("EPUB search failed:", error);
				return [];
			}
		},
		[toc]
	);

	// Handle search result navigation
	const handleSearchNavigate = useCallback((cfi: string) => {
		renditionRef.current?.display(cfi);
	}, []);

	// Keyboard navigation
	// Note: Arrow key navigation is handled by ReactReader/epub.js internally via the iframe,
	// so we only handle other shortcuts here to avoid double navigation.
	useEffect(() => {
		const handleKeyDown = (event: KeyboardEvent) => {
			// Don't handle if settings modal, TOC, bookmarks, or search drawer is open, or if typing in an input
			if (
				settingsOpened ||
				tocOpened ||
				bookmarksOpened ||
				searchOpened ||
				event.target instanceof HTMLInputElement ||
				event.target instanceof HTMLTextAreaElement
			) {
				return;
			}

			switch (event.key) {
				// Arrow keys are handled by ReactReader/epub.js internally
				case "Escape":
					onClose();
					break;
				case " ":
					event.preventDefault();
					toggleToolbar();
					break;
				case "f":
					// F without Ctrl/Cmd = fullscreen toggle
					// Ctrl+F = search (handled below)
					if (!event.ctrlKey && !event.metaKey) {
						event.preventDefault();
						setFullscreen(!isFullscreen);
					} else {
						// Ctrl+F or Cmd+F = open search
						event.preventDefault();
						setSearchOpened(true);
					}
					break;
				case "F":
					if (!event.ctrlKey && !event.metaKey) {
						event.preventDefault();
						setFullscreen(!isFullscreen);
					}
					break;
				case "t":
				case "T":
					if (!event.ctrlKey && !event.metaKey) {
						event.preventDefault();
						setTocOpened((prev) => !prev);
					}
					break;
				case "b":
				case "B":
					if (!event.ctrlKey && !event.metaKey) {
						event.preventDefault();
						handleToggleBookmark();
					}
					break;
			}
		};

		window.addEventListener("keydown", handleKeyDown);
		return () => window.removeEventListener("keydown", handleKeyDown);
	}, [settingsOpened, tocOpened, bookmarksOpened, searchOpened, onClose, toggleToolbar, isFullscreen, setFullscreen, handleToggleBookmark]);

	// Fullscreen handling
	useEffect(() => {
		const handleFullscreenChange = () => {
			setFullscreen(!!document.fullscreenElement);
		};

		document.addEventListener("fullscreenchange", handleFullscreenChange);
		return () => {
			document.removeEventListener("fullscreenchange", handleFullscreenChange);
		};
	}, [setFullscreen]);

	// Enter/exit fullscreen
	useEffect(() => {
		const container = containerRef.current;
		if (!container) return;

		if (isFullscreen && !document.fullscreenElement) {
			container.requestFullscreen?.().catch(() => {
				setFullscreen(false);
			});
		} else if (!isFullscreen && document.fullscreenElement) {
			document.exitFullscreen?.();
		}
	}, [isFullscreen, setFullscreen]);

	// Auto-hide toolbar
	const resetHideTimeout = useCallback(() => {
		if (hideTimeoutRef.current) {
			clearTimeout(hideTimeoutRef.current);
		}

		if (autoHideToolbar && toolbarVisible) {
			hideTimeoutRef.current = setTimeout(() => {
				setToolbarVisible(false);
			}, toolbarHideDelay);
		}
	}, [autoHideToolbar, toolbarVisible, toolbarHideDelay, setToolbarVisible]);

	useEffect(() => {
		resetHideTimeout();
		return () => {
			if (hideTimeoutRef.current) {
				clearTimeout(hideTimeoutRef.current);
			}
		};
	}, [resetHideTimeout]);

	// Show toolbar on mouse move
	const handleMouseMove = useCallback(() => {
		if (!toolbarVisible) {
			setToolbarVisible(true);
		}
		resetHideTimeout();
	}, [toolbarVisible, setToolbarVisible, resetHideTimeout]);

	// Get background color based on theme (with fallback for hydration)
	const getBackgroundColor = () => {
		const theme = EPUB_THEMES[epubTheme] ?? EPUB_THEMES.light;
		return theme.body.background;
	};

	return (
		<Box
			ref={containerRef}
			onMouseMove={handleMouseMove}
			style={{
				width: "100vw",
				height: "100vh",
				position: "relative",
				overflow: "hidden",
				backgroundColor: getBackgroundColor(),
			}}
		>
			{/* Toolbar */}
			<ReaderToolbar
				title={title}
				visible={toolbarVisible}
				onClose={onClose}
				onOpenSettings={() => setSettingsOpened(true)}
				showPageNavigation={false}
				leftActions={
					<EpubTableOfContents
						toc={toc}
						currentHref={currentHref}
						opened={tocOpened}
						onToggle={() => setTocOpened((prev) => !prev)}
						onNavigate={handleTocNavigate}
					/>
				}
				rightActions={
					<>
						<EpubBookmarks
							bookmarks={bookmarks}
							isCurrentLocationBookmarked={isCurrentLocationBookmarked}
							opened={bookmarksOpened}
							onToggle={() => setBookmarksOpened((prev) => !prev)}
							onAddBookmark={handleAddBookmark}
							onRemoveCurrentBookmark={handleRemoveCurrentBookmark}
							onUpdateNote={(id, note) => updateBookmark(id, { note })}
							onRemoveBookmark={removeBookmark}
							onNavigate={handleBookmarkNavigate}
						/>
						<EpubSearch
							opened={searchOpened}
							onToggle={() => setSearchOpened((prev) => !prev)}
							onSearch={handleSearch}
							onNavigate={handleSearchNavigate}
						/>
					</>
				}
			/>

			{/* Loading overlay */}
			{isLoading && (
				<Center
					style={{
						position: "absolute",
						inset: 0,
						zIndex: 10,
						backgroundColor: getBackgroundColor(),
					}}
				>
					<Loader size="lg" color="gray" />
				</Center>
			)}

			{/* EPUB Reader */}
			<Box
				style={{
					height: "100%",
					paddingTop: toolbarVisible ? 56 : 0,
					transition: "padding-top 0.2s ease",
				}}
			>
				<ReactReader
					url={epubUrl}
					location={location}
					locationChanged={handleLocationChange}
					getRendition={handleGetRendition}
					readerStyles={readerStyles}
					showToc={false}
					epubInitOptions={{
						openAs: "epub",
					}}
					epubOptions={{
						allowScriptedContent: false,
					}}
				/>
			</Box>

			{/* Settings modal */}
			<EpubReaderSettings
				opened={settingsOpened}
				onClose={() => setSettingsOpened(false)}
			/>
		</Box>
	);
}
