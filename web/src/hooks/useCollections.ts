import { notifications } from "@mantine/notifications";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  type Collection,
  type CollectionSeriesSort,
  type CreateCollectionRequest,
  collectionsApi,
  type UpdateCollectionRequest,
} from "@/api/collections";
import type { Series } from "@/types";

const COLLECTIONS_KEY = "collections";

export function useCollections() {
  return useQuery<Collection[]>({
    queryKey: [COLLECTIONS_KEY],
    queryFn: collectionsApi.list,
  });
}

export function useCollection(id: string | undefined) {
  return useQuery<Collection>({
    queryKey: [COLLECTIONS_KEY, id],
    queryFn: () => collectionsApi.get(id ?? ""),
    enabled: Boolean(id),
  });
}

export function useCollectionSeries(
  id: string | undefined,
  sort?: CollectionSeriesSort,
) {
  return useQuery<Series[]>({
    // Sort lives in slot 4 so the [key, id, "series"] prefix used by the
    // add/remove/reorder invalidations still matches every sort variant.
    queryKey: [COLLECTIONS_KEY, id, "series", sort ?? "default"],
    queryFn: () => collectionsApi.getSeries(id ?? "", sort),
    enabled: Boolean(id),
  });
}

export function useCollectionsForSeries(seriesId: string | undefined) {
  return useQuery<Collection[]>({
    queryKey: ["series", seriesId, "collections"],
    queryFn: () => collectionsApi.forSeries(seriesId ?? ""),
    enabled: Boolean(seriesId),
  });
}

function notifyError(title: string) {
  return (error: Error) =>
    notifications.show({
      title,
      message: error.message || "Unknown error",
      color: "red",
    });
}

export function useCreateCollection() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (body: CreateCollectionRequest) => collectionsApi.create(body),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: [COLLECTIONS_KEY] });
    },
    onError: notifyError("Failed to create collection"),
  });
}

export function useUpdateCollection(id: string) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (body: UpdateCollectionRequest) =>
      collectionsApi.update(id, body),
    onSuccess: (data) => {
      queryClient.setQueryData([COLLECTIONS_KEY, id], data);
      queryClient.invalidateQueries({ queryKey: [COLLECTIONS_KEY] });
    },
    onError: notifyError("Failed to update collection"),
  });
}

export function useDeleteCollection() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => collectionsApi.delete(id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: [COLLECTIONS_KEY] });
    },
    onError: notifyError("Failed to delete collection"),
  });
}

export function useAddSeriesToCollection() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({
      collectionId,
      seriesIds,
    }: {
      collectionId: string;
      seriesIds: string[];
    }) => collectionsApi.addSeries(collectionId, seriesIds),
    onSuccess: (_data, { collectionId }) => {
      queryClient.invalidateQueries({
        queryKey: [COLLECTIONS_KEY, collectionId],
      });
      queryClient.invalidateQueries({ queryKey: [COLLECTIONS_KEY] });
      queryClient.invalidateQueries({ queryKey: ["series"] });
    },
    onError: notifyError("Failed to add series to collection"),
  });
}

export function useRemoveSeriesFromCollection(collectionId: string) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (seriesId: string) =>
      collectionsApi.removeSeries(collectionId, seriesId),
    onSuccess: () => {
      queryClient.invalidateQueries({
        queryKey: [COLLECTIONS_KEY, collectionId],
      });
      queryClient.invalidateQueries({ queryKey: [COLLECTIONS_KEY] });
    },
    onError: notifyError("Failed to remove series from collection"),
  });
}

/**
 * Remove a series from a collection identified at call time (for menus that act
 * across many collections, where the id isn't known when the hook is created).
 */
export function useRemoveSeriesFromCollections() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({
      collectionId,
      seriesId,
    }: {
      collectionId: string;
      seriesId: string;
    }) => collectionsApi.removeSeries(collectionId, seriesId),
    onSuccess: (_data, { collectionId, seriesId }) => {
      queryClient.invalidateQueries({
        queryKey: [COLLECTIONS_KEY, collectionId],
      });
      queryClient.invalidateQueries({ queryKey: [COLLECTIONS_KEY] });
      queryClient.invalidateQueries({
        queryKey: ["series", seriesId, "collections"],
      });
    },
    onError: notifyError("Failed to remove series from collection"),
  });
}

export function useReorderCollection(collectionId: string) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (seriesIds: string[]) =>
      collectionsApi.reorder(collectionId, seriesIds),
    onSuccess: () => {
      queryClient.invalidateQueries({
        queryKey: [COLLECTIONS_KEY, collectionId, "series"],
      });
    },
    onError: notifyError("Failed to reorder collection"),
  });
}
