import { ActionIcon, Box, Center, Group, Loader, Tooltip } from "@mantine/core";
import { IconPlayerSkipBack, IconPlayerSkipForward } from "@tabler/icons-react";
import type { Location, NavItem, Rendition } from "epubjs";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  type IReactReaderStyle,
  ReactReader,
  ReactReaderStyle,
} from "react-reader";

import { booksApi } from "@/api/books";
import { useReaderStore } from "@/store/readerStore";

import { BoundaryNotification } from "./BoundaryNotification";
import { EpubBookmarks } from "./EpubBookmarks";
import { EpubReaderSettings } from "./EpubReaderSettings";
import { EpubSearch, type SearchResult } from "./EpubSearch";
import { EpubTableOfContents } from "./EpubTableOfContents";
import { useAdjacentBooks } from "./hooks/useAdjacentBooks";
import { useBoundaryNotification } from "./hooks/useBoundaryNotification";
import { useEpubBookmarks } from "./hooks/useEpubBookmarks";
import { useEpubProgress } from "./hooks/useEpubProgress";
import { useSeriesNavigation } from "./hooks/useSeriesNavigation";
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
  // New themes
  night: {
    body: {
      background: "#000000", // True black for OLED screens
      color: "#cccccc",
    },
  },
  paper: {
    body: {
      background: "#f5f2e8", // Warm off-white, easier on eyes
      color: "#3d3d3d",
    },
  },
  ocean: {
    body: {
      background: "#1a2634", // Deep blue-gray for evening reading
      color: "#a8c7d9",
    },
  },
  forest: {
    body: {
      background: "#1e2e1e", // Dark forest green
      color: "#a8c9a8",
    },
  },
  rose: {
    body: {
      background: "#f9f0f0", // Soft pink/rose tint
      color: "#4a3535",
    },
  },
} as const;

export type EpubTheme = keyof typeof EPUB_THEMES;

// Font family CSS values mapping
const EPUB_FONT_FAMILIES = {
  default: "inherit",
  serif: "Georgia, 'Times New Roman', serif",
  "sans-serif": "'Helvetica Neue', Arial, sans-serif",
  monospace: "'Courier New', Consolas, monospace",
  dyslexic: "OpenDyslexic, 'Comic Sans MS', sans-serif",
} as const;

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
  /** Series ID (for series navigation) */
  seriesId: string | null;
  /** Book title for display */
  title: string;
  /** Total pages in the book (for progress calculation) */
  totalPages: number;
  /** Incognito mode - when true, progress tracking is disabled */
  incognito?: boolean;
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
  seriesId,
  title,
  totalPages,
  incognito,
  onClose,
}: EpubReaderProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const renditionRef = useRef<Rendition | null>(null);
  const hideTimeoutRef = useRef<NodeJS.Timeout | null>(null);
  const initialLocationLoadedRef = useRef(false);
  const currentPercentageRef = useRef<number>(0);

  // CFI-based progress tracking (also syncs to backend, disabled in incognito mode)
  const {
    getSavedLocation,
    getLocalTimestamp,
    initialPercentage,
    initialCfi,
    initialHref,
    initialProgression,
    apiTimestamp,
    isLoadingProgress,
    saveLocation,
  } = useEpubProgress({
    bookId,
    totalPages,
    enabled: !incognito,
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

  // Series navigation - fetch adjacent books and handle boundary navigation
  useAdjacentBooks({ bookId, enabled: seriesId !== null });

  // Boundary notification with auto-hide and state reset
  const {
    message: boundaryMessage,
    onBoundaryChange,
    clearNotification,
  } = useBoundaryNotification();

  // Series navigation with boundary handling
  const {
    canGoPrevBook,
    canGoNextBook,
    goToPrevBook,
    goToNextBook,
    boundaryState,
    isSeriesEnd,
    isSeriesStart,
  } = useSeriesNavigation({
    onBoundaryChange,
    clearNotification,
    onBeforeNavigateToNext: incognito
      ? undefined
      : () => {
          booksApi.markAsRead(bookId);
        },
  });

  // Use ref for saveLocation to avoid re-creating handleGetRendition
  const saveLocationRef = useRef(saveLocation);
  saveLocationRef.current = saveLocation;

  // Use ref for totalPages to access in callbacks
  const totalPagesRef = useRef(totalPages);
  totalPagesRef.current = totalPages;

  // Local state - initialize with saved CFI location from localStorage
  // Note: This provides instant restore, but the cross-device sync effect
  // below may override it if the API has newer progress.
  const [location, setLocation] = useState<string | number>(() => {
    const saved = getSavedLocation();
    if (saved) {
      initialLocationLoadedRef.current = true;
      return saved;
    }
    return 0;
  });
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
  const [currentChapterTitle, setCurrentChapterTitle] = useState<
    string | undefined
  >();

  // EPUB-specific settings from store
  const epubTheme = useReaderStore((state) => state.settings.epubTheme);
  const epubFontSize = useReaderStore((state) => state.settings.epubFontSize);
  const epubFontFamily = useReaderStore(
    (state) => state.settings.epubFontFamily,
  );
  const epubLineHeight = useReaderStore(
    (state) => state.settings.epubLineHeight,
  );
  const epubMargin = useReaderStore((state) => state.settings.epubMargin);
  const epubSpread = useReaderStore((state) => state.settings.epubSpread);

  // Use refs for initial styles to avoid re-creating handleGetRendition
  const epubThemeRef = useRef(epubTheme);
  const epubFontSizeRef = useRef(epubFontSize);
  const epubFontFamilyRef = useRef(epubFontFamily);
  const epubLineHeightRef = useRef(epubLineHeight);
  const epubMarginRef = useRef(epubMargin);
  epubThemeRef.current = epubTheme;
  epubFontSizeRef.current = epubFontSize;
  epubFontFamilyRef.current = epubFontFamily;
  epubLineHeightRef.current = epubLineHeight;
  epubMarginRef.current = epubMargin;

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

  // Apply font family to rendition
  useEffect(() => {
    if (renditionRef.current?.themes) {
      const fontFamily =
        EPUB_FONT_FAMILIES[epubFontFamily] ?? EPUB_FONT_FAMILIES.default;
      renditionRef.current.themes.override("font-family", fontFamily);
    }
  }, [epubFontFamily]);

  // Apply line height to rendition
  useEffect(() => {
    if (renditionRef.current?.themes) {
      renditionRef.current.themes.override("line-height", `${epubLineHeight}%`);
    }
  }, [epubLineHeight]);

  // Apply margin to rendition (via padding on body)
  useEffect(() => {
    if (renditionRef.current?.themes) {
      renditionRef.current.themes.override("padding", `0 ${epubMargin}%`);
    }
  }, [epubMargin]);

  // Apply spread mode to rendition
  useEffect(() => {
    if (renditionRef.current) {
      // epub.js spread() accepts "none" (single), "always" (double), or "auto" (responsive)
      // For "always", set minSpreadWidth to 0 so it never collapses to single page
      const minWidth = epubSpread === "always" ? 0 : 800;
      renditionRef.current.spread(epubSpread, minWidth);
    }
  }, [epubSpread]);

  // Helper: check if API progress is newer than localStorage
  const isApiNewer = useCallback(() => {
    if (!initialLocationLoadedRef.current) return true; // No local data, always apply
    if (!apiTimestamp) return false;
    const localTs = getLocalTimestamp();
    if (!localTs) return true; // No local timestamp, prefer API
    return new Date(apiTimestamp).getTime() > new Date(localTs).getTime();
  }, [apiTimestamp, getLocalTimestamp]);

  // Apply CFI-based API progress immediately (no need to wait for locations generation).
  // This handles cross-device sync when the R2Progression was saved by another Codex web
  // instance (which includes a precise CFI).
  useEffect(() => {
    if (
      !isLoadingProgress &&
      initialCfi !== null &&
      !hasAppliedApiProgress &&
      renditionRef.current &&
      isApiNewer()
    ) {
      setLocation(initialCfi);
      setHasAppliedApiProgress(true);
    }
  }, [isLoadingProgress, initialCfi, hasAppliedApiProgress, isApiNewer]);

  // Whether we need cross-app sync (Komic/Readium): no CFI, but has href, and API is newer.
  // When true, we show a loading spinner until locations are ready for precise positioning.
  const needsCrossAppSync = useMemo(() => {
    if (isLoadingProgress) return false;
    return initialCfi === null && initialHref !== null && isApiNewer();
  }, [isLoadingProgress, initialCfi, initialHref, isApiNewer]);

  // Ref so the relocated callback can check if cross-app sync is pending
  const pendingCrossAppSyncRef = useRef(false);
  useEffect(() => {
    pendingCrossAppSyncRef.current =
      needsCrossAppSync && !hasAppliedApiProgress;
  }, [needsCrossAppSync, hasAppliedApiProgress]);

  // Cross-app sync: navigate precisely using href + within-resource progression.
  // Waits for locations to be generated so we can position accurately within the chapter.
  // The loading spinner stays visible until this completes.
  useEffect(() => {
    if (
      locationsReady &&
      needsCrossAppSync &&
      !hasAppliedApiProgress &&
      renditionRef.current
    ) {
      const book = renditionRef.current.book;
      if (book?.locations?.length()) {
        const spine = book.spine as {
          items?: Array<{ href: string; cfiBase: string }>;
        };
        const spineItem = spine.items?.find(
          (item) =>
            item.href === initialHref ||
            item.href.endsWith(initialHref!) ||
            initialHref!.endsWith(item.href),
        );

        const locations = book.locations;
        const total = locations.length();

        if (
          spineItem &&
          initialProgression !== null &&
          initialProgression > 0
        ) {
          // Interpolate within the section's book-level percentage range
          // eslint-disable-next-line @typescript-eslint/no-explicit-any
          const allLocs: string[] = (locations as any)._locations ?? [];

          let firstIdx = -1;
          let lastIdx = -1;
          for (let i = 0; i < allLocs.length; i++) {
            if (allLocs[i].includes(spineItem.cfiBase)) {
              if (firstIdx === -1) firstIdx = i;
              lastIdx = i;
            }
          }

          if (firstIdx >= 0 && total > 0) {
            const sectionStart = firstIdx / total;
            const sectionEnd = (lastIdx + 1) / total;
            const targetPct =
              sectionStart + initialProgression * (sectionEnd - sectionStart);
            const cfi = locations.cfiFromPercentage(
              Math.min(targetPct, 0.9999),
            );
            if (cfi) {
              setLocation(cfi);
            }
          } else if (spineItem) {
            renditionRef.current?.display(spineItem.href);
          }
        } else if (
          initialPercentage !== null &&
          initialPercentage > 0 &&
          total > 0
        ) {
          // No within-resource progression, but we have totalProgression.
          // Use it directly to position via book-level percentage.
          const cfi = locations.cfiFromPercentage(
            Math.min(initialPercentage, 0.9999),
          );
          if (cfi) {
            setLocation(cfi);
          } else if (spineItem) {
            renditionRef.current?.display(spineItem.href);
          }
        } else if (spineItem) {
          // No progression data at all, navigate to start of chapter
          renditionRef.current?.display(spineItem.href);
        }
      }
      setHasAppliedApiProgress(true);
      // Clear the loading spinner now that we've navigated to the right spot
      setIsLoading(false);
    }
  }, [
    locationsReady,
    needsCrossAppSync,
    initialHref,
    initialProgression,
    initialPercentage,
    hasAppliedApiProgress,
  ]);

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
      // Apply font family
      const fontFamily =
        EPUB_FONT_FAMILIES[epubFontFamilyRef.current] ??
        EPUB_FONT_FAMILIES.default;
      rendition.themes.override("font-family", fontFamily);
      // Apply line height
      rendition.themes.override("line-height", `${epubLineHeightRef.current}%`);
      // Apply margin (via padding)
      rendition.themes.override("padding", `0 ${epubMarginRef.current}%`);
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
      // Keep spinner visible while waiting for cross-app position sync
      if (!pendingCrossAppSyncRef.current) {
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
      // Resolve href to full EPUB-internal path (e.g., "OEBPS/chapter1.xhtml")
      // epub.js returns href relative to the OPF directory, but Readium-based
      // apps (like Komic) expect the full path within the EPUB archive.
      const bookDir =
        (rendition.book.path as { directory?: string })?.directory ?? "";
      const stripped = bookDir === "/" ? "" : bookDir;
      const normalizedDir = stripped.startsWith("/")
        ? stripped.slice(1)
        : stripped;
      const fullHref = normalizedDir
        ? `${normalizedDir}${location.start.href}`
        : location.start.href;
      saveLocationRef.current(cfi, percentage, fullHref);
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
  const isCurrentLocationBookmarked = currentCfi
    ? isBookmarked(currentCfi)
    : false;

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
      const selection = rendition?.manager
        ?.getContents?.()?.[0]
        ?.window?.getSelection?.();
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
  }, [
    isCurrentLocationBookmarked,
    handleAddBookmark,
    handleRemoveCurrentBookmark,
  ]);

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
          search: (
            query: string,
          ) => Promise<Array<{ cfi: string; excerpt: string }>>;
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
                  section.href.startsWith(item.href.split("#")[0]),
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
    [toc],
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
  }, [
    settingsOpened,
    tocOpened,
    bookmarksOpened,
    searchOpened,
    onClose,
    toggleToolbar,
    isFullscreen,
    setFullscreen,
    handleToggleBookmark,
  ]);

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
          <Group gap="xs">
            {/* Previous book in series */}
            <Tooltip label="Previous book in series" disabled={!canGoPrevBook}>
              <ActionIcon
                variant="subtle"
                color="gray"
                size="lg"
                onClick={goToPrevBook}
                disabled={!canGoPrevBook}
                aria-label="Previous book"
              >
                <IconPlayerSkipBack size={20} />
              </ActionIcon>
            </Tooltip>

            {/* Next book in series */}
            <Tooltip label="Next book in series" disabled={!canGoNextBook}>
              <ActionIcon
                variant="subtle"
                color="gray"
                size="lg"
                onClick={goToNextBook}
                disabled={!canGoNextBook}
                aria-label="Next book"
              >
                <IconPlayerSkipForward size={20} />
              </ActionIcon>
            </Tooltip>

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
          </Group>
        }
      />

      {/* Boundary notification for series navigation */}
      <BoundaryNotification
        visible={boundaryState !== "none" && boundaryMessage !== null}
        message={boundaryMessage}
        type={boundaryState}
        isSeriesEnd={isSeriesEnd || isSeriesStart}
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
            spread: epubSpread,
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
