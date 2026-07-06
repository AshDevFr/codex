import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useCallback, useEffect, useRef } from "react";
import { readProgressApi } from "@/api/readProgress";
import { isOfflineQueuedError } from "@/lib/offline/outbox";
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
  /** Cancel any pending debounced save and suppress the unmount save */
  cancelPendingSave: () => void;
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

  // Completion latches for the reading session. Once a book is marked
  // complete, a later position save must not flip it back to in-progress.
  // This matters for double-page mode, where the final spread leaves
  // currentPage one short of the last page: the end-of-book overlay saves
  // completion via saveProgress(totalPages), but the debounced/unmount saves
  // report the raw spread page (e.g. 173 of 174) and would otherwise clobber
  // it back to `completed: false`. Reset when switching books.
  const completedRef = useRef(false);

  // Keep refs up to date, and reset the completion latch when the book
  // changes (series navigation) so book B never inherits book A's completion.
  useEffect(() => {
    if (bookIdRef.current !== bookId) {
      completedRef.current = false;
    }
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

      // Completion is sticky for the session: once complete, a later position
      // save cannot downgrade it. A completed book is pinned to its last page
      // so progress displays read 100%, not 99% (double-page final spread).
      const completed = completedRef.current || page >= currentTotalPages;
      if (completed) {
        completedRef.current = true;
      }
      const savedPage = completed ? Math.max(page, currentTotalPages) : page;

      readProgressApi
        .update(currentBookId, {
          currentPage: savedPage,
          completed,
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
          // Queued for offline delivery is success-equivalent for our
          // purposes: the outbox will replay the write when the network
          // returns. Don't surface as an error.
          if (isOfflineQueuedError(error)) return;
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

  // Cancel any pending debounced save and suppress the unmount save.
  // Used before markAsRead to prevent stale progress from overwriting it.
  const cancelledRef = useRef(false);
  const cancelPendingSave = useCallback(() => {
    if (debounceTimerRef.current) {
      clearTimeout(debounceTimerRef.current);
      debounceTimerRef.current = null;
    }
    cancelledRef.current = true;
  }, []);

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
      }
      // Save final progress before unmount, unless cancelled (e.g. markAsRead
      // was called and we don't want to overwrite it with stale page data)
      if (!cancelledRef.current && enabledRef.current) {
        const finalPage = useReaderStore.getState().currentPage;
        if (finalPage !== lastSavedPageRef.current && finalPage > 0) {
          saveToBackend(finalPage);
        }
      }
    };
  }, [saveToBackend]);

  // Calculate initial page from saved progress (1-indexed)
  const initialPage = progress?.currentPage
    ? Math.min(progress.currentPage, totalPages)
    : 1;

  return {
    initialPage,
    isLoading,
    isCompleted: progress?.completed ?? false,
    saveProgress,
    cancelPendingSave,
  };
}
