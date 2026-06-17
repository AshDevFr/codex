import { notifications } from "@mantine/notifications";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  type WantToReadEntry,
  type WantToReadSort,
  wantToReadApi,
} from "@/api/wantToRead";
import {
  BOOKS_LIST_SECTIONS,
  SERIES_LIST_SECTIONS,
} from "@/hooks/listSections";

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
 * Invalidate everything that renders the changed item's `wantToRead` flag:
 *
 *  - the queue (`["want-to-read"]`);
 *  - the entity's detail query — prefix-based, so `["series", id]` matches
 *    `["series", id, "full"]`, which drives the detail-page toggle;
 *  - the LIST/grid/home sections. Cards source `wantToRead` from list queries
 *    keyed `["series"/"books", <section>, ...]`, whose slot 2 is a section
 *    string, not an id — so the detail-id prefix above never reaches them.
 *    Without this, toggling from a card never flips the button until reload.
 */
function useInvalidateAfterChange() {
  const queryClient = useQueryClient();
  return ({ itemType, id }: WantToReadTarget) => {
    queryClient.invalidateQueries({ queryKey: [QUEUE_KEY] });
    const root = itemType === "series" ? "series" : "books";
    queryClient.invalidateQueries({ queryKey: [root, id] });
    const sections =
      itemType === "series" ? SERIES_LIST_SECTIONS : BOOKS_LIST_SECTIONS;
    for (const section of sections) {
      queryClient.invalidateQueries({ queryKey: [root, section] });
    }
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
