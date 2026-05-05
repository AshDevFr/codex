import { notifications } from "@mantine/notifications";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useRef } from "react";
import {
  type BulkReleaseActionRequest,
  type BulkReleaseActionResponse,
  type DeleteReleaseResponse,
  type PaginatedReleases,
  type ReleaseFacets,
  type ReleaseFacetsParams,
  type ReleaseInboxParams,
  type ReleaseLedgerEntry,
  type ReleaseSource,
  type ResetReleaseSourceResponse,
  releaseSourcesApi,
  releasesApi,
  type SeriesReleaseListParams,
  type UpdateReleaseLedgerEntryRequest,
  type UpdateReleaseSourceRequest,
} from "@/api/releases";
import { useTaskProgress } from "@/hooks/useTaskProgress";

const RELEASE_POLL_TASK_TYPE = "poll_release_source";

export const releasesKeys = {
  inbox: (params: ReleaseInboxParams) => ["releases", "inbox", params] as const,
  facets: (params: ReleaseFacetsParams) =>
    ["releases", "facets", params] as const,
  series: (seriesId: string, params: SeriesReleaseListParams) =>
    ["series", seriesId, "releases", params] as const,
  inboxRoot: ["releases", "inbox"] as const,
  sourcesRoot: ["release-sources"] as const,
};

export function useReleaseFacets(params: ReleaseFacetsParams = {}) {
  return useQuery<ReleaseFacets>({
    queryKey: releasesKeys.facets(params),
    queryFn: () => releasesApi.facets(params),
  });
}

export function useDeleteRelease() {
  const queryClient = useQueryClient();
  return useMutation<DeleteReleaseResponse, Error, string>({
    mutationFn: (releaseId) => releasesApi.delete(releaseId),
    onSuccess: () => {
      // Delete touches the ledger and (server-side) the source's etag.
      // Invalidate both so the table and the source-admin row refresh.
      queryClient.invalidateQueries({ queryKey: ["releases"] });
      queryClient.invalidateQueries({ queryKey: ["series"] });
      queryClient.invalidateQueries({ queryKey: releasesKeys.sourcesRoot });
    },
    onError: notifyError("Failed to delete release"),
  });
}

export function useBulkReleaseAction() {
  const queryClient = useQueryClient();
  return useMutation<
    BulkReleaseActionResponse,
    Error,
    BulkReleaseActionRequest
  >({
    mutationFn: (request) => releasesApi.bulk(request),
    onSuccess: (data) => {
      const { affected, action } = data;
      const verb =
        action === "dismiss"
          ? "Dismissed"
          : action === "mark-acquired"
            ? "Marked acquired"
            : "Deleted";
      const noun = affected === 1 ? "release" : "releases";
      notifications.show({
        title: `${verb} ${affected} ${noun}`,
        // Surface the etag-clear side effect for delete so the user knows
        // the row will come back on the next poll.
        message:
          action === "delete"
            ? "Affected sources will re-fetch on the next poll."
            : undefined,
        color: action === "delete" ? "orange" : "blue",
      });
      queryClient.invalidateQueries({ queryKey: ["releases"] });
      queryClient.invalidateQueries({ queryKey: ["series"] });
      if (action === "delete") {
        queryClient.invalidateQueries({ queryKey: releasesKeys.sourcesRoot });
      }
    },
    onError: notifyError("Bulk action failed"),
  });
}

export function useReleaseInbox(params: ReleaseInboxParams = {}) {
  return useQuery<PaginatedReleases>({
    queryKey: releasesKeys.inbox(params),
    queryFn: () => releasesApi.listInbox(params),
  });
}

export function useSeriesReleases(
  seriesId: string,
  params: SeriesReleaseListParams = {},
  enabled = true,
) {
  return useQuery<PaginatedReleases>({
    queryKey: releasesKeys.series(seriesId, params),
    queryFn: () => releasesApi.listForSeries(seriesId, params),
    enabled: enabled && Boolean(seriesId),
  });
}

function notifyError(title: string) {
  return (error: Error & { response?: { data?: { error?: string } } }) => {
    notifications.show({
      title,
      message: error.response?.data?.error || error.message || "Unknown error",
      color: "red",
    });
  };
}

export function useDismissRelease() {
  const queryClient = useQueryClient();
  return useMutation<ReleaseLedgerEntry, Error, string>({
    mutationFn: (releaseId) => releasesApi.dismiss(releaseId),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["releases"] });
      queryClient.invalidateQueries({ queryKey: ["series"] });
    },
    onError: notifyError("Failed to dismiss release"),
  });
}

export function useMarkReleaseAcquired() {
  const queryClient = useQueryClient();
  return useMutation<ReleaseLedgerEntry, Error, string>({
    mutationFn: (releaseId) => releasesApi.markAcquired(releaseId),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["releases"] });
      queryClient.invalidateQueries({ queryKey: ["series"] });
    },
    onError: notifyError("Failed to mark release acquired"),
  });
}

export function usePatchRelease() {
  const queryClient = useQueryClient();
  return useMutation<
    ReleaseLedgerEntry,
    Error,
    { releaseId: string; update: UpdateReleaseLedgerEntryRequest }
  >({
    mutationFn: ({ releaseId, update }) =>
      releasesApi.patchEntry(releaseId, update),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["releases"] });
      queryClient.invalidateQueries({ queryKey: ["series"] });
    },
    onError: notifyError("Failed to update release"),
  });
}

export function useReleaseSources() {
  const queryClient = useQueryClient();
  const { activeTasks } = useTaskProgress();

  const query = useQuery<ReleaseSource[]>({
    queryKey: releasesKeys.sourcesRoot,
    queryFn: () => releaseSourcesApi.list(),
    // Belt-and-braces: SSE `release_source_polled` invalidates this query, but
    // very fast polls can race the event pipeline. While any release-poll task
    // is in flight, refetch every 5s so `lastPolledAt` / `lastSummary` /
    // `lastError` catch up even if the event is missed. Stops once no polls
    // are active.
    refetchInterval: () => {
      const hasActivePoll = Array.from(activeTasks.values()).some(
        (task) =>
          task.taskType === RELEASE_POLL_TASK_TYPE &&
          (task.status === "pending" || task.status === "running"),
      );
      return hasActivePoll ? 5000 : false;
    },
  });

  // Refresh immediately when a release-poll task transitions to a terminal
  // state. `useTaskProgress` keeps completed/failed entries around briefly,
  // so we watch for the status flip rather than disappearance.
  const prevStatusesRef = useRef<Map<string, string>>(new Map());
  useEffect(() => {
    const prev = prevStatusesRef.current;
    const next = new Map<string, string>();

    for (const task of activeTasks.values()) {
      if (task.taskType !== RELEASE_POLL_TASK_TYPE) continue;
      next.set(task.taskId, task.status);

      const prevStatus = prev.get(task.taskId);
      if (
        prevStatus &&
        prevStatus !== task.status &&
        (task.status === "completed" || task.status === "failed")
      ) {
        queryClient.invalidateQueries({ queryKey: releasesKeys.sourcesRoot });
      }
    }

    prevStatusesRef.current = next;
  }, [activeTasks, queryClient]);

  return query;
}

export function useUpdateReleaseSource() {
  const queryClient = useQueryClient();
  return useMutation<
    ReleaseSource,
    Error,
    { sourceId: string; update: UpdateReleaseSourceRequest }
  >({
    mutationFn: ({ sourceId, update }) =>
      releaseSourcesApi.update(sourceId, update),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: releasesKeys.sourcesRoot });
    },
    onError: notifyError("Failed to update source"),
  });
}

export function usePollReleaseSourceNow() {
  const queryClient = useQueryClient();
  return useMutation<{ status: string; message: string }, Error, string>({
    mutationFn: (sourceId) => releaseSourcesApi.pollNow(sourceId),
    onSuccess: () => {
      notifications.show({
        title: "Poll enqueued",
        message: "The release source will be polled shortly.",
        color: "blue",
      });
      queryClient.invalidateQueries({ queryKey: releasesKeys.sourcesRoot });
    },
    onError: notifyError("Failed to enqueue poll"),
  });
}

export function useResetReleaseSource() {
  const queryClient = useQueryClient();
  return useMutation<ResetReleaseSourceResponse, Error, string>({
    mutationFn: (sourceId) => releaseSourcesApi.reset(sourceId),
    onSuccess: (data) => {
      notifications.show({
        title: "Source reset",
        message: `Cleared ${data.deletedLedgerEntries} ledger ${
          data.deletedLedgerEntries === 1 ? "entry" : "entries"
        }. Click "Poll now" to re-fetch.`,
        color: "blue",
      });
      // Reset wipes ledger rows, so invalidate everything that reads them.
      queryClient.invalidateQueries({ queryKey: releasesKeys.sourcesRoot });
      queryClient.invalidateQueries({ queryKey: releasesKeys.inboxRoot });
      queryClient.invalidateQueries({ queryKey: ["series"] });
    },
    onError: notifyError("Failed to reset source"),
  });
}
