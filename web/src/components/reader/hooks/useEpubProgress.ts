import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useCallback, useEffect, useRef } from "react";
import { readProgressApi } from "@/api/readProgress";

const STORAGE_KEY_PREFIX = "epub-cfi-";

// Threshold for considering percentage as "changed" (avoids saving tiny changes)
const PERCENTAGE_CHANGE_THRESHOLD = 0.005; // 0.5%

interface UseEpubProgressOptions {
	/** Book ID for storing progress */
	bookId: string;
	/** Total pages in the book (used as fallback page number) */
	totalPages: number;
	/** Debounce delay for saving progress (ms) */
	debounceMs?: number;
	/** Whether to enable progress tracking */
	enabled?: boolean;
}

interface UseEpubProgressReturn {
	/** Get the saved CFI location for this book (null if none saved) */
	getSavedLocation: () => string | null;
	/** Get the initial percentage from API (for cross-device sync) */
	initialPercentage: number | null;
	/** Whether API progress is still loading */
	isLoadingProgress: boolean;
	/** Save the current CFI location and percentage */
	saveLocation: (cfi: string, percentage: number) => void;
	/** Clear saved progress for this book */
	clearProgress: () => void;
}

/**
 * Hook for managing EPUB reading progress via CFI (Canonical Fragment Identifier).
 *
 * Stores CFI locations in localStorage for precise position restoration.
 * Also syncs percentage-based progress to the backend API.
 * Uses debouncing to avoid excessive writes during rapid navigation.
 *
 * CFI (Canonical Fragment Identifier) is an EPUB standard for identifying
 * locations within an EPUB document, allowing precise position restoration.
 */
export function useEpubProgress({
	bookId,
	totalPages,
	debounceMs = 1000,
	enabled = true,
}: UseEpubProgressOptions): UseEpubProgressReturn {
	const queryClient = useQueryClient();
	const debounceTimerRef = useRef<NodeJS.Timeout | null>(null);
	const lastSavedCfiRef = useRef<string | null>(null);
	const lastSavedPercentageRef = useRef<number>(0);
	const pendingCfiRef = useRef<string | null>(null);
	const pendingPercentageRef = useRef<number>(0);

	// Fetch initial progress from API for cross-device sync
	const { data: apiProgress, isLoading: isLoadingProgress } = useQuery({
		queryKey: ["readProgress", bookId],
		queryFn: () => readProgressApi.get(bookId),
		enabled: enabled && !!bookId,
		staleTime: 30000, // Cache for 30 seconds
	});

	// Get initial percentage directly from API (stored percentage for EPUBs)
	// Cast to include progress_percentage until types are regenerated
	const progressWithPercentage = apiProgress as
		| (typeof apiProgress & { progress_percentage?: number | null })
		| null
		| undefined;
	const initialPercentage = progressWithPercentage?.progress_percentage ?? null;

	// Store refs to avoid dependency issues
	const bookIdRef = useRef(bookId);
	const totalPagesRef = useRef(totalPages);

	// Keep refs up to date
	useEffect(() => {
		bookIdRef.current = bookId;
		totalPagesRef.current = totalPages;
	}, [bookId, totalPages]);

	// Initialize lastSavedPercentageRef from API progress to avoid duplicate saves
	useEffect(() => {
		if (progressWithPercentage?.progress_percentage != null) {
			lastSavedPercentageRef.current =
				progressWithPercentage.progress_percentage;
		}
	}, [progressWithPercentage]);

	const storageKey = `${STORAGE_KEY_PREFIX}${bookId}`;

	// Get saved location from localStorage
	const getSavedLocation = useCallback((): string | null => {
		if (!enabled) return null;
		try {
			return localStorage.getItem(storageKey);
		} catch {
			console.warn("Failed to read EPUB progress from localStorage");
			return null;
		}
	}, [storageKey, enabled]);

	// Save location to localStorage (internal, immediate)
	const saveToStorage = useCallback(
		(cfi: string) => {
			if (!enabled) return;
			try {
				localStorage.setItem(storageKey, cfi);
				lastSavedCfiRef.current = cfi;
			} catch {
				console.warn("Failed to save EPUB progress to localStorage");
			}
		},
		[storageKey, enabled],
	);

	// Save progress to backend API
	const saveToBackend = useCallback(
		(percentage: number) => {
			const currentBookId = bookIdRef.current;
			const currentTotalPages = totalPagesRef.current;

			// Convert percentage to page number for backwards compatibility
			// Use totalPages if available, otherwise use percentage as 0-100 scale
			const currentPage =
				currentTotalPages > 0
					? Math.max(1, Math.round(percentage * currentTotalPages))
					: Math.max(1, Math.round(percentage * 100));

			const isCompleted = percentage >= 0.98; // Consider 98%+ as completed

			readProgressApi
				.update(currentBookId, {
					currentPage,
					progressPercentage: percentage,
					completed: isCompleted,
				})
				.then(() => {
					lastSavedPercentageRef.current = percentage;
					// Invalidate related queries
					queryClient.invalidateQueries({
						queryKey: ["readProgress", currentBookId],
					});
					queryClient.invalidateQueries({ queryKey: ["book", currentBookId] });
					queryClient.invalidateQueries({ queryKey: ["books", "in-progress"] });
					queryClient.invalidateQueries({ queryKey: ["books", "recently-read"] });
				})
				.catch((error) => {
					console.error("Failed to save EPUB reading progress:", error);
				});
		},
		[queryClient],
	);

	// Debounced save - public API
	const saveLocation = useCallback(
		(cfi: string, percentage: number) => {
			if (!enabled) return;

			// Store pending values for flush on unmount
			pendingCfiRef.current = cfi;
			pendingPercentageRef.current = percentage;

			// Skip CFI save if same as last saved
			const cfiChanged = cfi !== lastSavedCfiRef.current;

			// Check if percentage changed significantly (avoids saving tiny changes)
			const percentageChanged =
				Math.abs(percentage - lastSavedPercentageRef.current) >
				PERCENTAGE_CHANGE_THRESHOLD;

			// Skip if nothing changed
			if (!cfiChanged && !percentageChanged) {
				return;
			}

			// Clear existing timer
			if (debounceTimerRef.current) {
				clearTimeout(debounceTimerRef.current);
			}

			// Set new debounced save
			// Capture values now to avoid stale closures
			const shouldSaveCfi = cfiChanged;
			const shouldSavePercentage = percentageChanged;
			const cfiToSave = cfi;
			const percentageToSave = percentage;

			debounceTimerRef.current = setTimeout(() => {
				if (shouldSaveCfi) {
					saveToStorage(cfiToSave);
				}
				if (shouldSavePercentage) {
					saveToBackend(percentageToSave);
				}
				pendingCfiRef.current = null;
				pendingPercentageRef.current = 0;
			}, debounceMs);
		},
		[enabled, debounceMs, saveToStorage, saveToBackend],
	);

	// Clear progress
	const clearProgress = useCallback(() => {
		try {
			localStorage.removeItem(storageKey);
			lastSavedCfiRef.current = null;
			pendingCfiRef.current = null;
		} catch {
			console.warn("Failed to clear EPUB progress from localStorage");
		}
	}, [storageKey]);

	// Cleanup: save pending progress on unmount
	useEffect(() => {
		// Capture current values for cleanup
		const currentStorageKey = storageKey;

		return () => {
			if (debounceTimerRef.current) {
				clearTimeout(debounceTimerRef.current);
			}
			// Flush any pending CFI to localStorage
			if (
				pendingCfiRef.current &&
				pendingCfiRef.current !== lastSavedCfiRef.current
			) {
				try {
					localStorage.setItem(currentStorageKey, pendingCfiRef.current);
				} catch {
					// Ignore errors on unmount
				}
			}
			// Flush any pending percentage to backend
			if (pendingPercentageRef.current >= 0 && pendingCfiRef.current) {
				const currentBookId = bookIdRef.current;
				const currentTotalPages = totalPagesRef.current;
				const percentage = pendingPercentageRef.current;
				const currentPage =
					currentTotalPages > 0
						? Math.max(1, Math.round(percentage * currentTotalPages))
						: Math.max(1, Math.round(percentage * 100));

				// Check if percentage changed significantly
				if (
					Math.abs(percentage - lastSavedPercentageRef.current) >
					PERCENTAGE_CHANGE_THRESHOLD
				) {
					readProgressApi
						.update(currentBookId, {
							currentPage,
							progressPercentage: percentage,
							completed: percentage >= 0.98,
						})
						.catch(() => {
							// Ignore errors on unmount
						});
				}
			}
		};
	}, [storageKey]);

	return {
		getSavedLocation,
		initialPercentage,
		isLoadingProgress,
		saveLocation,
		clearProgress,
	};
}
