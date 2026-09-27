import type { components } from "@/types/api.generated";
import { api } from "./client";

// Re-export generated types for convenience. The document, request, and
// report shapes intentionally use snake_case (see the backend's DTO module
// doc comment): this is a portable file format, not an ordinary camelCase
// API response, so the field names here are passed straight through.
export type ReadingProgressExportDocument =
  components["schemas"]["ReadingProgressExportDocument"];
export type ImportReadingProgressRequest =
  components["schemas"]["ImportReadingProgressRequest"];
export type ImportReadingProgressResponse =
  components["schemas"]["ImportReadingProgressResponse"];
export type ImportSeriesReport = components["schemas"]["ImportSeriesReport"];
export type ImportBookReport = components["schemas"]["ImportBookReport"];
export type ImportSummary = components["schemas"]["ImportSummary"];
export type HashMode = components["schemas"]["HashMode"];
export type ConflictPolicy = components["schemas"]["ConflictPolicy"];
export type SeriesDisposition = components["schemas"]["SeriesDisposition"];
export type BookDisposition = components["schemas"]["BookDisposition"];

const IMPORT_TIMEOUT_MS = 10 * 60_000;

export const readingProgressTransferApi = {
  /** Export the current user's reading progress as a downloadable document. */
  exportProgress: async (
    includeSessions = true,
  ): Promise<ReadingProgressExportDocument> => {
    const response = await api.get<ReadingProgressExportDocument>(
      "/reading-progress/export",
      { params: { include_sessions: includeSessions } },
    );
    return response.data;
  },

  /**
   * Import reading progress from a previously exported document.
   * Pass `dryRun: true` on the request to preview without writing.
   */
  importProgress: async (
    request: ImportReadingProgressRequest,
  ): Promise<ImportReadingProgressResponse> => {
    const response = await api.post<ImportReadingProgressResponse>(
      "/reading-progress/import",
      request,
      // A large history applies one series at a time; the client-wide 30 s
      // timeout would abandon it part-way and lose the report of what landed.
      { timeout: IMPORT_TIMEOUT_MS },
    );
    return response.data;
  },
};
