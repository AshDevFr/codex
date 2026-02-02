import { useCallback, useEffect, useState } from "react";
import { type PdfMode, useReaderStore } from "@/store/readerStore";

/** Settings that can be stored per-book */
export interface PerBookSettings {
  /** PDF rendering mode preference */
  pdfMode?: PdfMode;
}

/** LocalStorage key prefix for per-book settings */
const STORAGE_KEY_PREFIX = "codex-book-settings-";

/**
 * Hook for managing per-book reader settings.
 *
 * Stores and retrieves settings specific to individual books,
 * with fallback to global settings when no per-book preference exists.
 *
 * @param bookId - The book ID to store/retrieve settings for
 * @param format - The book format (only applies PDF settings to PDFs)
 */
export function usePerBookSettings(bookId: string, format: string) {
  const [isLoaded, setIsLoaded] = useState(false);
  const [hasPerBookPdfMode, setHasPerBookPdfMode] = useState(false);

  // Global settings from store
  const globalPdfMode = useReaderStore((state) => state.settings.pdfMode);
  const setPdfMode = useReaderStore((state) => state.setPdfMode);

  // Storage key for this book
  const storageKey = `${STORAGE_KEY_PREFIX}${bookId}`;

  // Load per-book settings on mount
  useEffect(() => {
    if (!bookId) return;

    try {
      const stored = localStorage.getItem(storageKey);
      if (stored) {
        const settings: PerBookSettings = JSON.parse(stored);
        if (settings.pdfMode && format.toLowerCase() === "pdf") {
          setPdfMode(settings.pdfMode);
          setHasPerBookPdfMode(true);
        }
      }
    } catch (error) {
      console.warn("Failed to load per-book settings:", error);
    }
    setIsLoaded(true);
  }, [bookId, format, storageKey, setPdfMode]);

  // Save per-book PDF mode preference
  const savePerBookPdfMode = useCallback(
    (mode: PdfMode) => {
      try {
        const stored = localStorage.getItem(storageKey);
        const settings: PerBookSettings = stored ? JSON.parse(stored) : {};
        settings.pdfMode = mode;
        localStorage.setItem(storageKey, JSON.stringify(settings));
        setHasPerBookPdfMode(true);
        setPdfMode(mode);
      } catch (error) {
        console.warn("Failed to save per-book settings:", error);
      }
    },
    [storageKey, setPdfMode],
  );

  // Clear per-book PDF mode preference (revert to global)
  const clearPerBookPdfMode = useCallback(() => {
    try {
      const stored = localStorage.getItem(storageKey);
      if (stored) {
        const settings: PerBookSettings = JSON.parse(stored);
        delete settings.pdfMode;
        if (Object.keys(settings).length === 0) {
          localStorage.removeItem(storageKey);
        } else {
          localStorage.setItem(storageKey, JSON.stringify(settings));
        }
      }
      setHasPerBookPdfMode(false);
    } catch (error) {
      console.warn("Failed to clear per-book settings:", error);
    }
  }, [storageKey]);

  return {
    /** Whether per-book settings have been loaded */
    isLoaded,
    /** Whether this book has a per-book PDF mode preference */
    hasPerBookPdfMode,
    /** Current effective PDF mode (per-book or global) */
    effectivePdfMode: globalPdfMode,
    /** Save a per-book PDF mode preference */
    savePerBookPdfMode,
    /** Clear per-book PDF mode preference (use global) */
    clearPerBookPdfMode,
  };
}
