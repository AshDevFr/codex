import { notifications } from "@mantine/notifications";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useRef } from "react";
import {
  type CreateSeriesExportRequest,
  type ExportFieldCatalogResponse,
  type SeriesExportDto,
  seriesExportsApi,
} from "@/api/seriesExports";
import { useTaskProgress } from "@/hooks/useTaskProgress";

const QUERY_KEY = ["seriesExports"] as const;
const FIELDS_QUERY_KEY = ["seriesExports", "fields"] as const;

/** Task types that should trigger a list refresh */
const EXPORT_TASK_TYPES = ["export_series"];

/**
 * Hook for listing the current user's series exports.
 * Auto-refreshes when export_series tasks complete.
 */
export function useSeriesExportsList() {
  const queryClient = useQueryClient();
  const { activeTasks } = useTaskProgress();

  const query = useQuery<SeriesExportDto[]>({
    queryKey: [...QUERY_KEY],
    queryFn: () => seriesExportsApi.list(),
    // Poll every 5s while any export is pending/running, stop once all are terminal
    refetchInterval: (query) => {
      const exports = query.state.data;
      if (!exports) return false;
      const hasActive = exports.some(
        (e) => e.status === "pending" || e.status === "running",
      );
      return hasActive ? 5000 : false;
    },
  });

  // Also refresh immediately when SSE reports an export task completing
  const prevTasksRef = useRef<Map<string, string>>(new Map());
  useEffect(() => {
    const prevStatuses = prevTasksRef.current;
    const currentStatuses = new Map<string, string>();

    for (const task of activeTasks) {
      if (EXPORT_TASK_TYPES.includes(task.taskType)) {
        currentStatuses.set(task.taskId, task.status);

        const prevStatus = prevStatuses.get(task.taskId);
        if (
          prevStatus &&
          prevStatus !== task.status &&
          (task.status === "completed" || task.status === "failed")
        ) {
          queryClient.invalidateQueries({ queryKey: [...QUERY_KEY] });
        }
      }
    }

    prevTasksRef.current = currentStatuses;
  }, [activeTasks, queryClient]);

  return query;
}

/**
 * Hook for the export field catalog (rarely changes, cached aggressively).
 */
export function useExportFieldCatalog() {
  return useQuery<ExportFieldCatalogResponse>({
    queryKey: [...FIELDS_QUERY_KEY],
    queryFn: () => seriesExportsApi.getFieldCatalog(),
    staleTime: Number.POSITIVE_INFINITY,
  });
}

/**
 * Hook for creating a new series export.
 */
export function useCreateSeriesExport() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (request: CreateSeriesExportRequest) =>
      seriesExportsApi.create(request),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: [...QUERY_KEY] });
      notifications.show({
        title: "Export started",
        message: "Your export is being generated in the background.",
        color: "blue",
      });
    },
    onError: (error: Error & { response?: { data?: { error?: string } } }) => {
      notifications.show({
        title: "Export failed",
        message:
          error.response?.data?.error || error.message || "Unknown error",
        color: "red",
      });
    },
  });
}

/**
 * Hook for deleting a series export.
 */
export function useDeleteSeriesExport() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (id: string) => seriesExportsApi.delete(id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: [...QUERY_KEY] });
      notifications.show({
        title: "Export deleted",
        message: "The export has been removed.",
        color: "green",
      });
    },
    onError: (error: Error & { response?: { data?: { error?: string } } }) => {
      notifications.show({
        title: "Delete failed",
        message:
          error.response?.data?.error || error.message || "Unknown error",
        color: "red",
      });
    },
  });
}

/**
 * Download an export file and trigger a browser download.
 */
export function useDownloadSeriesExport() {
  return useMutation({
    mutationFn: async ({
      id,
      format,
      createdAt,
    }: {
      id: string;
      format: string;
      createdAt: string;
    }) => {
      const blob = await seriesExportsApi.download(id);
      const ext = format === "csv" ? "csv" : format === "md" ? "md" : "json";
      const timestamp = new Date(createdAt)
        .toISOString()
        .replace(/[:.]/g, "-")
        .slice(0, 19);
      const filename = `codex-export-${timestamp}.${ext}`;

      // Trigger browser download
      const url = URL.createObjectURL(blob);
      const a = document.createElement("a");
      a.href = url;
      a.download = filename;
      document.body.appendChild(a);
      a.click();
      document.body.removeChild(a);
      URL.revokeObjectURL(url);
    },
    onError: (error: Error & { response?: { data?: { error?: string } } }) => {
      notifications.show({
        title: "Download failed",
        message:
          error.response?.data?.error || error.message || "Unknown error",
        color: "red",
      });
    },
  });
}
