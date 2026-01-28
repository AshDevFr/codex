import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useCallback, useEffect, useRef } from "react";
import { readProgressApi } from "@/api/readProgress";
import { useReaderStore } from "@/store/readerStore";

interface UseReadProgressOptions {
	/** Book ID to track progress for */
	bookId: string;
	/** Total pages in the book */
	totalPages: number;
	/** Debounce delay for progress updates (ms) */
	debounceMs?: number;
	/** Whether to enable progress tracking */
	enabled?: boolean;
}

interface UseReadProgressReturn {
	/** Initial page to start reading from (based on saved progress) */
	initialPage: number;
	/** Whether the progress is loading */
	isLoading: boolean;
	/** Whether the book is completed */
	isCompleted: boolean;
	/** Save progress immediately */
	saveProgress: (page: number) => void;
}

/**
 * Hook for managing reading progress with backend sync.
 *
 * Features:
 * - Fetches initial progress when mounting
 * - Debounced updates to reduce API calls
 * - Auto-complete detection when reaching last page
 * - Invalidates book queries on progress update
 */
export function useReadProgress({
	bookId,
	totalPages,
	debounceMs = 1000,
	enabled = true,
}: UseReadProgressOptions): UseReadProgressReturn {
	const queryClient = useQueryClient();
	const currentPage = useReaderStore((state) => state.currentPage);
	const debounceTimerRef = useRef<NodeJS.Timeout | null>(null);
	const lastSavedPageRef = useRef<number>(0);
	// Store refs to avoid dependency issues with callbacks
	const bookIdRef = useRef(bookId);
	const totalPagesRef = useRef(totalPages);

	// Keep refs up to date
	useEffect(() => {
		bookIdRef.current = bookId;
		totalPagesRef.current = totalPages;
	}, [bookId, totalPages]);

	// Fetch existing progress
	const { data: progress, isLoading } = useQuery({
		queryKey: ["readProgress", bookId],
		queryFn: () => readProgressApi.get(bookId),
		enabled: enabled && !!bookId,
		staleTime: 30000, // Consider fresh for 30s
	});

	// Stable save function that uses refs
	const saveToBackend = useCallback(
		(page: number) => {
			const currentBookId = bookIdRef.current;
			const currentTotalPages = totalPagesRef.current;

			readProgressApi
				.update(currentBookId, {
					current_page: page,
					completed: page >= currentTotalPages,
				})
				.then((updatedProgress) => {
					// Update cache directly instead of refetching
					queryClient.setQueryData(
						["readProgress", currentBookId],
						updatedProgress,
					);
					// Invalidate book detail to update progress display
					queryClient.invalidateQueries({ queryKey: ["book", currentBookId] });
					// Also invalidate in-progress and recently-read lists
					queryClient.invalidateQueries({ queryKey: ["books", "in-progress"] });
					queryClient.invalidateQueries({
						queryKey: ["books", "recently-read"],
					});
				})
				.catch((error) => {
					console.error("Failed to save reading progress:", error);
				});
		},
		[queryClient],
	);

	// Debounced progress save - stable callback
	const debouncedSave = useCallback(
		(page: number) => {
			// Clear any existing timer
			if (debounceTimerRef.current) {
				clearTimeout(debounceTimerRef.current);
			}

			// Only save if page has actually changed from last saved
			if (page === lastSavedPageRef.current) {
				return;
			}

			debounceTimerRef.current = setTimeout(() => {
				saveToBackend(page);
				lastSavedPageRef.current = page;
			}, debounceMs);
		},
		[debounceMs, saveToBackend],
	);

	// Immediate save (bypasses debounce)
	const saveProgress = useCallback(
		(page: number) => {
			// Clear any pending debounced save
			if (debounceTimerRef.current) {
				clearTimeout(debounceTimerRef.current);
				debounceTimerRef.current = null;
			}

			if (page !== lastSavedPageRef.current) {
				saveToBackend(page);
				lastSavedPageRef.current = page;
			}
		},
		[saveToBackend],
	);

	// Watch for page changes and trigger debounced save
	useEffect(() => {
		if (!enabled || currentPage === 0) return;

		debouncedSave(currentPage);
	}, [currentPage, enabled, debouncedSave]);

	// Store enabled ref for cleanup
	const enabledRef = useRef(enabled);
	useEffect(() => {
		enabledRef.current = enabled;
	}, [enabled]);

	// Cleanup debounce timer and save on unmount
	useEffect(() => {
		return () => {
			if (debounceTimerRef.current) {
				clearTimeout(debounceTimerRef.current);
				// Save final progress before unmount (only if tracking is enabled)
				if (enabledRef.current) {
					const finalPage = useReaderStore.getState().currentPage;
					if (finalPage !== lastSavedPageRef.current && finalPage > 0) {
						saveToBackend(finalPage);
					}
				}
			}
		};
	}, [saveToBackend]);

	// Calculate initial page from saved progress (1-indexed)
	const initialPage = progress?.current_page
		? Math.min(progress.current_page, totalPages)
		: 1;

	return {
		initialPage,
		isLoading,
		isCompleted: progress?.completed ?? false,
		saveProgress,
	};
}
