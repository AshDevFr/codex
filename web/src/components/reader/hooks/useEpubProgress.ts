import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useCallback, useEffect, useRef } from "react";
import { type R2Progression, readProgressApi } from "@/api/readProgress";

const STORAGE_KEY_PREFIX = "epub-cfi-";

// Threshold for considering percentage as "changed" (avoids saving tiny changes)
const PERCENTAGE_CHANGE_THRESHOLD = 0.005; // 0.5%

const CODEX_DEVICE_ID = "codex-web";
const CODEX_DEVICE_NAME = "Codex Web Reader";

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
  /** Get the initial CFI from R2Progression (for cross-device sync with Codex web) */
  initialCfi: string | null;
  /** Whether API progress is still loading */
  isLoadingProgress: boolean;
  /** Save the current CFI location, percentage, and chapter href */
  saveLocation: (cfi: string, percentage: number, href: string) => void;
  /** Clear saved progress for this book */
  clearProgress: () => void;
}

/**
 * Hook for managing EPUB reading progress via CFI (Canonical Fragment Identifier).
 *
 * Stores CFI locations in localStorage for precise position restoration.
 * Syncs R2Progression (Readium standard) to the backend API for cross-device
 * and cross-app sync (e.g., between Codex web reader and Komic).
 * Also syncs percentage-based progress to the backend API for backwards compat.
 * Uses debouncing to avoid excessive writes during rapid navigation.
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
  const pendingHrefRef = useRef<string>("");

  // Fetch initial progress from API for cross-device sync
  const { data: apiProgress, isLoading: isLoadingProgress } = useQuery({
    queryKey: ["readProgress", bookId],
    queryFn: () => readProgressApi.get(bookId),
    enabled: enabled && !!bookId,
    staleTime: 30000,
  });

  // Fetch R2Progression for cross-device/cross-app sync
  const { data: r2Progression, isLoading: isLoadingProgression } = useQuery({
    queryKey: ["progression", bookId],
    queryFn: () => readProgressApi.getProgression(bookId),
    enabled: enabled && !!bookId,
    staleTime: 30000,
  });

  // Get initial percentage from API or R2Progression
  const progressWithPercentage = apiProgress as
    | (typeof apiProgress & { progress_percentage?: number | null })
    | null
    | undefined;

  // Prefer R2Progression totalProgression, fall back to legacy progress_percentage
  const initialPercentage =
    r2Progression?.locator?.locations?.totalProgression ??
    progressWithPercentage?.progress_percentage ??
    null;

  // Get CFI from R2Progression if it was saved by Codex web (has cfi extension)
  const initialCfi = r2Progression?.locator?.locations?.cfi ?? null;

  // Store refs to avoid dependency issues
  const bookIdRef = useRef(bookId);
  const totalPagesRef = useRef(totalPages);

  useEffect(() => {
    bookIdRef.current = bookId;
    totalPagesRef.current = totalPages;
  }, [bookId, totalPages]);

  // Initialize lastSavedPercentageRef from API progress to avoid duplicate saves
  useEffect(() => {
    if (r2Progression?.locator?.locations?.totalProgression != null) {
      lastSavedPercentageRef.current =
        r2Progression.locator.locations.totalProgression;
    } else if (progressWithPercentage?.progress_percentage != null) {
      lastSavedPercentageRef.current =
        progressWithPercentage.progress_percentage;
    }
  }, [r2Progression, progressWithPercentage]);

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

  // Save progress to backend API (both legacy progress and R2Progression)
  const saveToBackend = useCallback(
    (percentage: number, cfi: string, href: string) => {
      const currentBookId = bookIdRef.current;
      const currentTotalPages = totalPagesRef.current;

      const currentPage =
        currentTotalPages > 0
          ? Math.max(1, Math.round(percentage * currentTotalPages))
          : Math.max(1, Math.round(percentage * 100));

      const isCompleted = percentage >= 0.98;

      // Build R2Progression
      const progression: R2Progression = {
        device: { id: CODEX_DEVICE_ID, name: CODEX_DEVICE_NAME },
        locator: {
          href,
          locations: {
            position: currentPage,
            totalProgression: percentage,
            cfi,
          },
          type: "application/xhtml+xml",
        },
        modified: new Date().toISOString(),
      };

      // Save both in parallel
      Promise.all([
        readProgressApi.update(currentBookId, {
          currentPage,
          progressPercentage: percentage,
          completed: isCompleted,
        }),
        readProgressApi.updateProgression(currentBookId, progression),
      ])
        .then(() => {
          lastSavedPercentageRef.current = percentage;
          queryClient.invalidateQueries({
            queryKey: ["readProgress", currentBookId],
          });
          queryClient.invalidateQueries({
            queryKey: ["progression", currentBookId],
          });
          queryClient.invalidateQueries({
            queryKey: ["book", currentBookId],
          });
          queryClient.invalidateQueries({
            queryKey: ["books", "in-progress"],
          });
          queryClient.invalidateQueries({
            queryKey: ["books", "recently-read"],
          });
        })
        .catch((error) => {
          console.error("Failed to save EPUB reading progress:", error);
        });
    },
    [queryClient],
  );

  // Debounced save - public API
  const saveLocation = useCallback(
    (cfi: string, percentage: number, href: string) => {
      if (!enabled) return;

      // Store pending values for flush on unmount
      pendingCfiRef.current = cfi;
      pendingPercentageRef.current = percentage;
      pendingHrefRef.current = href;

      // Skip CFI save if same as last saved
      const cfiChanged = cfi !== lastSavedCfiRef.current;

      // Check if percentage changed significantly
      const percentageChanged =
        Math.abs(percentage - lastSavedPercentageRef.current) >
        PERCENTAGE_CHANGE_THRESHOLD;

      if (!cfiChanged && !percentageChanged) {
        return;
      }

      // Clear existing timer
      if (debounceTimerRef.current) {
        clearTimeout(debounceTimerRef.current);
      }

      const shouldSaveCfi = cfiChanged;
      const shouldSavePercentage = percentageChanged;
      const cfiToSave = cfi;
      const percentageToSave = percentage;
      const hrefToSave = href;

      debounceTimerRef.current = setTimeout(() => {
        if (shouldSaveCfi) {
          saveToStorage(cfiToSave);
        }
        if (shouldSavePercentage) {
          saveToBackend(percentageToSave, cfiToSave, hrefToSave);
        }
        pendingCfiRef.current = null;
        pendingPercentageRef.current = 0;
        pendingHrefRef.current = "";
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

  // Store enabled ref for cleanup
  const enabledRef = useRef(enabled);
  useEffect(() => {
    enabledRef.current = enabled;
  }, [enabled]);

  // Cleanup: save pending progress on unmount
  useEffect(() => {
    const currentStorageKey = storageKey;

    return () => {
      if (debounceTimerRef.current) {
        clearTimeout(debounceTimerRef.current);
      }

      if (!enabledRef.current) {
        return;
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
        const cfi = pendingCfiRef.current;
        const href = pendingHrefRef.current;
        const currentPage =
          currentTotalPages > 0
            ? Math.max(1, Math.round(percentage * currentTotalPages))
            : Math.max(1, Math.round(percentage * 100));

        if (
          Math.abs(percentage - lastSavedPercentageRef.current) >
          PERCENTAGE_CHANGE_THRESHOLD
        ) {
          const isCompleted = percentage >= 0.98;

          // Save both legacy progress and R2Progression on unmount
          readProgressApi
            .update(currentBookId, {
              currentPage,
              progressPercentage: percentage,
              completed: isCompleted,
            })
            .catch(() => {});

          readProgressApi
            .updateProgression(currentBookId, {
              device: { id: CODEX_DEVICE_ID, name: CODEX_DEVICE_NAME },
              locator: {
                href,
                locations: {
                  position: currentPage,
                  totalProgression: percentage,
                  cfi,
                },
                type: "application/xhtml+xml",
              },
              modified: new Date().toISOString(),
            })
            .catch(() => {});
        }
      }
    };
  }, [storageKey]);

  return {
    getSavedLocation,
    initialPercentage,
    initialCfi,
    isLoadingProgress: isLoadingProgress || isLoadingProgression,
    saveLocation,
    clearProgress,
  };
}
