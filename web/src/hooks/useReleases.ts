import { notifications } from "@mantine/notifications";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  type PaginatedReleases,
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

export const releasesKeys = {
  inbox: (params: ReleaseInboxParams) => ["releases", "inbox", params] as const,
  series: (seriesId: string, params: SeriesReleaseListParams) =>
    ["series", seriesId, "releases", params] as const,
  inboxRoot: ["releases", "inbox"] as const,
  sourcesRoot: ["release-sources"] as const,
};

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
  return useQuery<ReleaseSource[]>({
    queryKey: releasesKeys.sourcesRoot,
    queryFn: () => releaseSourcesApi.list(),
  });
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
