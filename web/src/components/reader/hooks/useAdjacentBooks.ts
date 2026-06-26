import { useQuery } from "@tanstack/react-query";
import { useEffect } from "react";
import { booksApi } from "@/api/books";
import type { AdjacentBook } from "@/store/readerStore";
import { useReaderStore } from "@/store/readerStore";
import type { Book } from "@/types";

/** Map an API book payload to the reader's AdjacentBook shape. */
function toAdjacentBook(book: Book | null | undefined): AdjacentBook | null {
  if (!book) return null;
  return {
    id: book.id,
    title: book.title,
    pageCount: book.pageCount,
    seriesName: book.seriesName,
    number: book.number ?? null,
    volume: book.volume ?? null,
    chapter: book.chapter ?? null,
  };
}

interface UseAdjacentBooksOptions {
  /** Book ID to fetch adjacent books for */
  bookId: string;
  /** Whether to enable the query */
  enabled?: boolean;
}

interface UseAdjacentBooksResult {
  /** Previous book in the series (if any) */
  prevBook: AdjacentBook | null;
  /** Next book in the series (if any) */
  nextBook: AdjacentBook | null;
  /** Whether the query is loading */
  isLoading: boolean;
  /** Whether there was an error */
  isError: boolean;
}

/**
 * Hook to fetch adjacent books (prev/next) in the same series.
 * Automatically syncs results to the reader store for use in navigation.
 */
export function useAdjacentBooks({
  bookId,
  enabled = true,
}: UseAdjacentBooksOptions): UseAdjacentBooksResult {
  const setAdjacentBooks = useReaderStore((state) => state.setAdjacentBooks);

  const { data, isLoading, isError } = useQuery({
    queryKey: ["adjacentBooks", bookId],
    queryFn: () => booksApi.getAdjacent(bookId),
    enabled: enabled && !!bookId,
    staleTime: 5 * 60 * 1000, // 5 minutes
  });

  // Transform API response to AdjacentBook format and sync to store
  useEffect(() => {
    if (data) {
      setAdjacentBooks({
        prev: toAdjacentBook(data.prev),
        next: toAdjacentBook(data.next),
      });
    }
  }, [data, setAdjacentBooks]);

  // Derive return values from data
  const prevBook = toAdjacentBook(data?.prev);
  const nextBook = toAdjacentBook(data?.next);

  return {
    prevBook,
    nextBook,
    isLoading,
    isError,
  };
}
