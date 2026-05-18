import type { components } from "@/types/api.generated";
import { api } from "./client";

// Re-export generated types for convenience
export type PdfCacheStatsDto = components["schemas"]["PdfCacheStatsDto"];
export type PdfPageCacheStatsDto =
  components["schemas"]["PdfPageCacheStatsDto"];
export type PdfHandleCacheStatsDto =
  components["schemas"]["PdfHandleCacheStatsDto"];
export type PdfHandleCacheEntryDto =
  components["schemas"]["PdfHandleCacheEntryDto"];
export type PdfHandleCacheClearResultDto =
  components["schemas"]["PdfHandleCacheClearResultDto"];
export type PdfCacheCleanupResultDto =
  components["schemas"]["PdfCacheCleanupResultDto"];
export type TriggerPdfCacheCleanupResponse =
  components["schemas"]["TriggerPdfCacheCleanupResponse"];

export const pdfCacheApi = {
  /**
   * Get combined statistics for both the disk page cache and the
   * in-memory PDFium handle cache (admin only).
   */
  getStats: async (): Promise<PdfCacheStatsDto> => {
    const response = await api.get<PdfCacheStatsDto>("/admin/pdf-cache");
    return response.data;
  },

  /**
   * Get statistics about the in-memory PDFium handle cache (admin only),
   * including the list of currently-resident open documents.
   */
  getHandleStats: async (): Promise<PdfHandleCacheStatsDto> => {
    const response = await api.get<PdfHandleCacheStatsDto>(
      "/admin/pdf-cache/handles",
    );
    return response.data;
  },

  /**
   * Trigger an asynchronous cleanup of the on-disk rendered-page cache
   * (admin only). Enqueues a background task that removes cached pages
   * older than the configured max age.
   */
  triggerCleanup: async (): Promise<TriggerPdfCacheCleanupResponse> => {
    const response = await api.post<TriggerPdfCacheCleanupResponse>(
      "/admin/pdf-cache/pages/cleanup",
    );
    return response.data;
  },

  /**
   * Clear the on-disk rendered-page cache immediately (admin only).
   */
  clearPageCache: async (): Promise<PdfCacheCleanupResultDto> => {
    const response = await api.delete<PdfCacheCleanupResultDto>(
      "/admin/pdf-cache/pages",
    );
    return response.data;
  },

  /**
   * Close every PDFium handle currently held in memory (admin only).
   */
  clearHandleCache: async (): Promise<PdfHandleCacheClearResultDto> => {
    const response = await api.delete<PdfHandleCacheClearResultDto>(
      "/admin/pdf-cache/handles",
    );
    return response.data;
  },

  /**
   * Evict a single book's PDFium handle (admin only). Returns the count
   * of handles closed (0 if the book was not in the cache, 1 otherwise).
   */
  evictBookHandle: async (
    bookId: string,
  ): Promise<PdfHandleCacheClearResultDto> => {
    const response = await api.delete<PdfHandleCacheClearResultDto>(
      `/admin/pdf-cache/handles/${bookId}`,
    );
    return response.data;
  },
};
