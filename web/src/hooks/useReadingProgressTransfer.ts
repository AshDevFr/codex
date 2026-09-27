import { notifications } from "@mantine/notifications";
import { useMutation } from "@tanstack/react-query";
import {
  type ImportReadingProgressRequest,
  readingProgressTransferApi,
} from "@/api/readingProgressTransfer";

type ApiErrorLike = Error & {
  response?: { data?: { message?: string; error?: string } };
};

function errorMessage(error: ApiErrorLike, fallback: string): string {
  return (
    error.response?.data?.message ||
    error.response?.data?.error ||
    error.message ||
    fallback
  );
}

/**
 * Export the current user's reading progress and trigger a browser download.
 * The document is fetched as JSON (not a blob endpoint), so the download is
 * built client-side from the response body.
 */
export function useExportReadingProgress() {
  return useMutation({
    mutationFn: async (includeSessions: boolean) => {
      const exported =
        await readingProgressTransferApi.exportProgress(includeSessions);

      const timestamp = new Date().toISOString().slice(0, 10);
      const filename = `codex-reading-progress-${timestamp}.json`;
      const blob = new Blob([JSON.stringify(exported, null, 2)], {
        type: "application/json",
      });
      const url = URL.createObjectURL(blob);
      const anchor = document.createElement("a");
      anchor.href = url;
      anchor.download = filename;
      document.body.appendChild(anchor);
      anchor.click();
      document.body.removeChild(anchor);
      URL.revokeObjectURL(url);

      return exported;
    },
    onError: (error: ApiErrorLike) => {
      notifications.show({
        title: "Export failed",
        message: errorMessage(error, "Could not export reading progress."),
        color: "red",
      });
    },
  });
}

/**
 * Run an import (dry run or real, per `request.dryRun`). Errors are surfaced
 * to the caller rather than shown as a notification here, because the
 * calling page renders a full report either way.
 */
export function useImportReadingProgress() {
  return useMutation({
    mutationFn: (request: ImportReadingProgressRequest) =>
      readingProgressTransferApi.importProgress(request),
  });
}
