import { notifications } from "@mantine/notifications";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  type CreateReadListRequest,
  type ReadList,
  type ReadListBookSort,
  readListsApi,
  type UpdateReadListRequest,
} from "@/api/readlists";
import type { Book } from "@/types";

const READLISTS_KEY = "readlists";

export function useReadLists() {
  return useQuery<ReadList[]>({
    queryKey: [READLISTS_KEY],
    queryFn: readListsApi.list,
  });
}

export function useReadList(id: string | undefined) {
  return useQuery<ReadList>({
    queryKey: [READLISTS_KEY, id],
    queryFn: () => readListsApi.get(id ?? ""),
    enabled: Boolean(id),
  });
}

export function useReadListBooks(
  id: string | undefined,
  sort?: ReadListBookSort,
) {
  return useQuery<Book[]>({
    // Sort lives in slot 4 so the [key, id, "books"] prefix used by the
    // add/remove/reorder invalidations still matches every sort variant.
    queryKey: [READLISTS_KEY, id, "books", sort ?? "default"],
    queryFn: () => readListsApi.getBooks(id ?? "", sort),
    enabled: Boolean(id),
  });
}

export function useReadListsForBook(bookId: string | undefined) {
  return useQuery<ReadList[]>({
    queryKey: ["books", bookId, "readlists"],
    queryFn: () => readListsApi.forBook(bookId ?? ""),
    enabled: Boolean(bookId),
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

export function useCreateReadList() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (body: CreateReadListRequest) => readListsApi.create(body),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: [READLISTS_KEY] });
    },
    onError: notifyError("Failed to create read list"),
  });
}

export function useUpdateReadList(id: string) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (body: UpdateReadListRequest) => readListsApi.update(id, body),
    onSuccess: (data) => {
      queryClient.setQueryData([READLISTS_KEY, id], data);
      queryClient.invalidateQueries({ queryKey: [READLISTS_KEY] });
    },
    onError: notifyError("Failed to update read list"),
  });
}

export function useDeleteReadList() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => readListsApi.delete(id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: [READLISTS_KEY] });
    },
    onError: notifyError("Failed to delete read list"),
  });
}

export function useAddBooksToReadList() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({
      readListId,
      bookIds,
    }: {
      readListId: string;
      bookIds: string[];
    }) => readListsApi.addBooks(readListId, bookIds),
    onSuccess: (_data, { readListId }) => {
      queryClient.invalidateQueries({ queryKey: [READLISTS_KEY, readListId] });
      queryClient.invalidateQueries({ queryKey: [READLISTS_KEY] });
      queryClient.invalidateQueries({ queryKey: ["books"] });
    },
    onError: notifyError("Failed to add book to read list"),
  });
}

export function useRemoveBookFromReadList(readListId: string) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (bookId: string) => readListsApi.removeBook(readListId, bookId),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: [READLISTS_KEY, readListId] });
      queryClient.invalidateQueries({ queryKey: [READLISTS_KEY] });
    },
    onError: notifyError("Failed to remove book from read list"),
  });
}

/**
 * Remove a book from a read list identified at call time (for menus acting
 * across many read lists).
 */
export function useRemoveBookFromReadLists() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({
      readListId,
      bookId,
    }: {
      readListId: string;
      bookId: string;
    }) => readListsApi.removeBook(readListId, bookId),
    onSuccess: (_data, { readListId, bookId }) => {
      queryClient.invalidateQueries({ queryKey: [READLISTS_KEY, readListId] });
      queryClient.invalidateQueries({ queryKey: [READLISTS_KEY] });
      queryClient.invalidateQueries({
        queryKey: ["books", bookId, "readlists"],
      });
    },
    onError: notifyError("Failed to remove book from read list"),
  });
}

export function useReorderReadList(readListId: string) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (bookIds: string[]) =>
      readListsApi.reorder(readListId, bookIds),
    onSuccess: () => {
      queryClient.invalidateQueries({
        queryKey: [READLISTS_KEY, readListId, "books"],
      });
    },
    onError: notifyError("Failed to reorder read list"),
  });
}
