import { notifications } from "@mantine/notifications";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  type WantToReadEntry,
  type WantToReadSort,
  wantToReadApi,
} from "@/api/wantToRead";

const QUEUE_KEY = "want-to-read";

/** A target for adding/removing: a series or a book by id. */
export interface WantToReadTarget {
  itemType: "series" | "book";
  id: string;
}

export function useWantToReadQueue(sort: WantToReadSort = "newest") {
  return useQuery<WantToReadEntry[]>({
    queryKey: [QUEUE_KEY, sort],
    queryFn: () => wantToReadApi.list(sort),
  });
}

/**
 * Invalidate the queue plus the changed series/book detail query so its
 * `wantToRead` flag refreshes. Query-key invalidation is prefix-based, so
 * `["series", id]` matches `["series", id, "full"]` etc.
 */
function useInvalidateAfterChange() {
  const queryClient = useQueryClient();
  return ({ itemType, id }: WantToReadTarget) => {
    queryClient.invalidateQueries({ queryKey: [QUEUE_KEY] });
    queryClient.invalidateQueries({
      queryKey: [itemType === "series" ? "series" : "books", id],
    });
  };
}

export function useAddToWantToRead() {
  const invalidate = useInvalidateAfterChange();
  return useMutation({
    mutationFn: (target: WantToReadTarget) =>
      target.itemType === "series"
        ? wantToReadApi.addSeries(target.id)
        : wantToReadApi.addBook(target.id),
    onSuccess: (_data, target) => invalidate(target),
    onError: (error: Error) =>
      notifications.show({
        title: "Failed to add to Want to Read",
        message: error.message || "Unknown error",
        color: "red",
      }),
  });
}

/**
 * Add many series or books to the queue in a single request. Used by the bulk
 * selection toolbar. Invalidates the queue and the relevant list queries so the
 * grid's `wantToRead` flags refresh.
 */
export function useBulkAddToWantToRead() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({
      itemType,
      ids,
    }: {
      itemType: "series" | "book";
      ids: string[];
    }) =>
      itemType === "series"
        ? wantToReadApi.bulkAddSeries(ids)
        : wantToReadApi.bulkAddBooks(ids),
    onSuccess: (_data, { itemType }) => {
      queryClient.invalidateQueries({ queryKey: [QUEUE_KEY] });
      queryClient.invalidateQueries({
        queryKey: [itemType === "series" ? "series" : "books"],
      });
    },
    onError: (error: Error) =>
      notifications.show({
        title: "Failed to add to Want to Read",
        message: error.message || "Unknown error",
        color: "red",
      }),
  });
}

export function useRemoveFromWantToRead() {
  const invalidate = useInvalidateAfterChange();
  return useMutation({
    mutationFn: (target: WantToReadTarget) =>
      target.itemType === "series"
        ? wantToReadApi.removeSeries(target.id)
        : wantToReadApi.removeBook(target.id),
    onSuccess: (_data, target) => invalidate(target),
    onError: (error: Error) =>
      notifications.show({
        title: "Failed to remove from Want to Read",
        message: error.message || "Unknown error",
        color: "red",
      }),
  });
}
