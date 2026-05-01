import { notifications } from "@mantine/notifications";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  type CreateSeriesAliasRequest,
  type SeriesAlias,
  type SeriesTracking,
  trackingApi,
  type UpdateSeriesTrackingRequest,
} from "@/api/tracking";

const trackingKey = (seriesId: string) =>
  ["series", seriesId, "tracking"] as const;
const aliasesKey = (seriesId: string) =>
  ["series", seriesId, "aliases"] as const;

export function useSeriesTracking(seriesId: string, enabled = true) {
  return useQuery<SeriesTracking>({
    queryKey: trackingKey(seriesId),
    queryFn: () => trackingApi.getTracking(seriesId),
    enabled: enabled && Boolean(seriesId),
  });
}

export function useUpdateSeriesTracking(seriesId: string) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (update: UpdateSeriesTrackingRequest) =>
      trackingApi.updateTracking(seriesId, update),
    onSuccess: (data) => {
      queryClient.setQueryData(trackingKey(seriesId), data);
    },
    onError: (error: Error & { response?: { data?: { error?: string } } }) => {
      notifications.show({
        title: "Failed to update tracking",
        message:
          error.response?.data?.error || error.message || "Unknown error",
        color: "red",
      });
    },
  });
}

export function useSeriesAliases(seriesId: string, enabled = true) {
  return useQuery<SeriesAlias[]>({
    queryKey: aliasesKey(seriesId),
    queryFn: () => trackingApi.listAliases(seriesId),
    enabled: enabled && Boolean(seriesId),
  });
}

export function useCreateSeriesAlias(seriesId: string) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (request: CreateSeriesAliasRequest) =>
      trackingApi.createAlias(seriesId, request),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: aliasesKey(seriesId) });
    },
    onError: (error: Error & { response?: { data?: { error?: string } } }) => {
      notifications.show({
        title: "Failed to add alias",
        message:
          error.response?.data?.error || error.message || "Unknown error",
        color: "red",
      });
    },
  });
}

export function useDeleteSeriesAlias(seriesId: string) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (aliasId: string) => trackingApi.deleteAlias(seriesId, aliasId),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: aliasesKey(seriesId) });
    },
    onError: (error: Error & { response?: { data?: { error?: string } } }) => {
      notifications.show({
        title: "Failed to remove alias",
        message:
          error.response?.data?.error || error.message || "Unknown error",
        color: "red",
      });
    },
  });
}
