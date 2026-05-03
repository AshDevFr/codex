import { notifications } from "@mantine/notifications";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  type DryRunRequest,
  type DryRunResponse,
  type FieldGroup,
  type MetadataRefreshConfig,
  type MetadataRefreshConfigPatchInput,
  metadataRefreshApi,
  type RunNowResponse,
} from "@/api/metadataRefresh";

const configKey = (libraryId: string) =>
  ["library", libraryId, "metadata-refresh"] as const;
const fieldGroupsKey = ["metadata-refresh", "field-groups"] as const;

/** Read the saved per-library scheduled refresh config. */
export function useMetadataRefreshConfig(libraryId: string | undefined | null) {
  return useQuery<MetadataRefreshConfig>({
    queryKey: configKey(libraryId ?? ""),
    queryFn: () => metadataRefreshApi.get(libraryId as string),
    enabled: !!libraryId,
    staleTime: 30_000,
  });
}

/** Static-ish field-group catalog from the server (cached aggressively). */
export function useFieldGroups() {
  return useQuery<FieldGroup[]>({
    queryKey: [...fieldGroupsKey],
    queryFn: () => metadataRefreshApi.listFieldGroups(),
    staleTime: 60 * 60 * 1000,
    gcTime: 24 * 60 * 60 * 1000,
  });
}

/** Save a partial config update. */
export function useUpdateMetadataRefreshConfig(libraryId: string) {
  const queryClient = useQueryClient();

  return useMutation<
    MetadataRefreshConfig,
    Error & { response?: { data?: { error?: string } } },
    MetadataRefreshConfigPatchInput
  >({
    mutationFn: (patch) => metadataRefreshApi.update(libraryId, patch),
    onSuccess: (data) => {
      queryClient.setQueryData(configKey(libraryId), data);
      notifications.show({
        title: "Schedule saved",
        message: data.enabled
          ? "Scheduled metadata refresh is enabled."
          : "Scheduled metadata refresh is disabled.",
        color: "green",
      });
    },
    onError: (error) => {
      notifications.show({
        title: "Could not save schedule",
        message:
          error.response?.data?.error || error.message || "Unknown error",
        color: "red",
      });
    },
  });
}

/**
 * Trigger an immediate refresh task. The returned response carries the task
 * ID; callers can pipe that into `useTaskProgress().getTask(id)` to render
 * progress.
 */
export function useRunMetadataRefreshNow(libraryId: string) {
  return useMutation<
    RunNowResponse,
    Error & { response?: { data?: { error?: string } } }
  >({
    mutationFn: () => metadataRefreshApi.runNow(libraryId),
    onSuccess: () => {
      notifications.show({
        title: "Metadata refresh started",
        message: "Tracking progress in the background.",
        color: "blue",
      });
    },
    onError: (error) => {
      notifications.show({
        title: "Could not start refresh",
        message:
          error.response?.data?.error || error.message || "Unknown error",
        color: "red",
      });
    },
  });
}

/** Synchronous dry-run preview. */
export function useDryRunMetadataRefresh(libraryId: string) {
  return useMutation<
    DryRunResponse,
    Error & { response?: { data?: { error?: string } } },
    DryRunRequest | undefined
  >({
    mutationFn: (request) => metadataRefreshApi.dryRun(libraryId, request),
    onError: (error) => {
      notifications.show({
        title: "Preview failed",
        message:
          error.response?.data?.error || error.message || "Unknown error",
        color: "red",
      });
    },
  });
}
