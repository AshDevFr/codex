import { useCallback, useEffect, useState } from "react";

const STORAGE_KEY_PREFIX = "epub-bookmarks-";

export interface EpubBookmark {
	/** Unique identifier for the bookmark */
	id: string;
	/** CFI location in the EPUB */
	cfi: string;
	/** Percentage through the book (0.0-1.0) */
	percentage: number;
	/** Optional note/annotation */
	note: string;
	/** Chapter/section name if available */
	chapterTitle?: string;
	/** Text excerpt from the bookmarked location */
	excerpt?: string;
	/** Timestamp when bookmark was created */
	createdAt: number;
}

interface UseEpubBookmarksOptions {
	/** Book ID for storing bookmarks */
	bookId: string;
	/** Whether to enable bookmark functionality */
	enabled?: boolean;
}

interface UseEpubBookmarksReturn {
	/** All bookmarks for this book */
	bookmarks: EpubBookmark[];
	/** Add a new bookmark */
	addBookmark: (
		bookmark: Omit<EpubBookmark, "id" | "createdAt">,
	) => EpubBookmark;
	/** Update an existing bookmark */
	updateBookmark: (
		id: string,
		updates: Partial<Pick<EpubBookmark, "note">>,
	) => void;
	/** Remove a bookmark */
	removeBookmark: (id: string) => void;
	/** Check if current CFI location is bookmarked */
	isBookmarked: (cfi: string) => boolean;
	/** Get bookmark at a specific CFI location */
	getBookmarkByCfi: (cfi: string) => EpubBookmark | undefined;
	/** Clear all bookmarks for this book */
	clearBookmarks: () => void;
}

/**
 * Generate a unique ID for a bookmark
 */
function generateId(): string {
	return `${Date.now()}-${Math.random().toString(36).substring(2, 9)}`;
}

/**
 * Hook for managing EPUB bookmarks with notes.
 *
 * Stores bookmarks in localStorage with CFI locations for precise position restoration.
 * Each bookmark can have an optional note/annotation.
 */
export function useEpubBookmarks({
	bookId,
	enabled = true,
}: UseEpubBookmarksOptions): UseEpubBookmarksReturn {
	const storageKey = `${STORAGE_KEY_PREFIX}${bookId}`;

	// Initialize bookmarks from localStorage
	const [bookmarks, setBookmarks] = useState<EpubBookmark[]>(() => {
		if (!enabled) return [];
		try {
			const stored = localStorage.getItem(storageKey);
			if (stored) {
				const parsed = JSON.parse(stored);
				// Validate and return bookmarks array
				if (Array.isArray(parsed)) {
					return parsed;
				}
			}
		} catch {
			console.warn("Failed to read EPUB bookmarks from localStorage");
		}
		return [];
	});

	// Persist bookmarks to localStorage whenever they change
	useEffect(() => {
		if (!enabled) return;
		try {
			localStorage.setItem(storageKey, JSON.stringify(bookmarks));
		} catch {
			console.warn("Failed to save EPUB bookmarks to localStorage");
		}
	}, [bookmarks, storageKey, enabled]);

	// Add a new bookmark
	const addBookmark = useCallback(
		(bookmark: Omit<EpubBookmark, "id" | "createdAt">): EpubBookmark => {
			const newBookmark: EpubBookmark = {
				...bookmark,
				id: generateId(),
				createdAt: Date.now(),
			};
			setBookmarks((prev) => [...prev, newBookmark]);
			return newBookmark;
		},
		[],
	);

	// Update an existing bookmark
	const updateBookmark = useCallback(
		(id: string, updates: Partial<Pick<EpubBookmark, "note">>) => {
			setBookmarks((prev) =>
				prev.map((bookmark) =>
					bookmark.id === id ? { ...bookmark, ...updates } : bookmark,
				),
			);
		},
		[],
	);

	// Remove a bookmark
	const removeBookmark = useCallback((id: string) => {
		setBookmarks((prev) => prev.filter((bookmark) => bookmark.id !== id));
	}, []);

	// Check if a CFI location is bookmarked
	const isBookmarked = useCallback(
		(cfi: string): boolean => {
			return bookmarks.some((bookmark) => bookmark.cfi === cfi);
		},
		[bookmarks],
	);

	// Get bookmark at a specific CFI location
	const getBookmarkByCfi = useCallback(
		(cfi: string): EpubBookmark | undefined => {
			return bookmarks.find((bookmark) => bookmark.cfi === cfi);
		},
		[bookmarks],
	);

	// Clear all bookmarks
	const clearBookmarks = useCallback(() => {
		setBookmarks([]);
		try {
			localStorage.removeItem(storageKey);
		} catch {
			console.warn("Failed to clear EPUB bookmarks from localStorage");
		}
	}, [storageKey]);

	return {
		bookmarks,
		addBookmark,
		updateBookmark,
		removeBookmark,
		isBookmarked,
		getBookmarkByCfi,
		clearBookmarks,
	};
}
