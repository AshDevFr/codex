import { Card, Group, Pagination, Stack, Text } from "@mantine/core";
import { useQuery } from "@tanstack/react-query";
import { useCallback, useEffect, useRef } from "react";
import { useNavigate } from "react-router-dom";
import { booksApi } from "@/api/books";
import { MediaCard } from "@/components/library/MediaCard";
import { CoverGridSkeleton } from "@/components/skeletons";
import { useShowSkeleton } from "@/lib/motion/useShowSkeleton";
import {
  selectCanSelectType,
  selectIsSelectionMode,
  useBulkSelectionStore,
} from "@/store/bulkSelectionStore";
import type { Book, PaginatedResponse } from "@/types";

/** Reading feeds backed by paginated, sort-less book collection endpoints. */
export type ReadingFeed = "in-progress" | "on-deck";

interface FeedConfig {
  /** Fetches one page of the feed for the given library ("all" = global). */
  fetchPage: (
    libraryId: string,
    params: { page: number; pageSize: number },
  ) => Promise<PaginatedResponse<Book>>;
  emptyTitle: string;
  emptyText: string;
}

const FEED_CONFIG: Record<ReadingFeed, FeedConfig> = {
  "in-progress": {
    fetchPage: (libraryId, params) => booksApi.getInProgress(libraryId, params),
    emptyTitle: "Nothing in progress",
    emptyText: "Books you start reading will show up here",
  },
  "on-deck": {
    fetchPage: (libraryId, params) => booksApi.getOnDeck(libraryId, params),
    emptyTitle: "Nothing on deck",
    emptyText: "The next book in series you've been reading will show up here",
  },
};

interface ReadingFeedSectionProps {
  libraryId: string;
  feed: ReadingFeed;
  searchParams: URLSearchParams;
  onTotalChange?: (total: number) => void;
}

/**
 * Full, paginated view of a reading feed (Keep Reading / On Deck).
 *
 * Mirrors {@link BooksSection}'s cover-grid + numbered-pagination layout, but
 * the underlying endpoints expose no filters or sort, so this component only
 * tracks `page`/`pageSize` from the URL. The natural order returned by the
 * endpoint (recent activity / series order) is preserved.
 */
export function ReadingFeedSection({
  libraryId,
  feed,
  searchParams,
  onTotalChange,
}: ReadingFeedSectionProps) {
  const navigate = useNavigate();
  const config = FEED_CONFIG[feed];

  // Bulk selection state - stable selectors to minimize re-renders
  const isSelectionMode = useBulkSelectionStore(selectIsSelectionMode);
  const canSelectBooks = useBulkSelectionStore(selectCanSelectType("book"));
  const toggleSelection = useBulkSelectionStore(
    (state) => state.toggleSelection,
  );
  const selectRange = useBulkSelectionStore((state) => state.selectRange);
  const getLastSelectedIndex = useBulkSelectionStore(
    (state) => state.getLastSelectedIndex,
  );
  const selectedIds = useBulkSelectionStore((state) => state.selectedIds);
  const setPageItems = useBulkSelectionStore((state) => state.setPageItems);

  const gridId = `reading-${feed}-${libraryId}`;
  const booksDataRef = useRef<Book[]>([]);

  // Read pagination from the URL (1-indexed pages for user-friendly URLs)
  const page = parseInt(searchParams.get("page") || "1", 10);
  const pageSize = parseInt(searchParams.get("pageSize") || "50", 10);

  const { data: booksData, isLoading: queryLoading } = useQuery({
    queryKey: ["books", feed, libraryId, page, pageSize],
    queryFn: () => config.fetchPage(libraryId, { page, pageSize }),
    staleTime: 30000,
    refetchOnMount: true,
  });

  // Gate the skeleton on a 150ms delay so fast loads stay flash-free.
  const showSkeleton = useShowSkeleton(queryLoading);

  const handlePageChange = (newPage: number) => {
    const params = new URLSearchParams(searchParams);
    params.set("page", newPage.toString());
    navigate({ search: params.toString() }, { replace: true });
  };

  const totalPages = booksData
    ? Math.ceil(booksData.total / booksData.pageSize)
    : 1;
  const showPagination = booksData ? booksData.total > pageSize : false;

  // Notify parent of total count change
  useEffect(() => {
    if (booksData && onTotalChange) {
      onTotalChange(booksData.total);
    }
  }, [booksData, onTotalChange]);

  if (booksData?.data) {
    booksDataRef.current = booksData.data;
  }

  // Register visible page items for Select All functionality
  useEffect(() => {
    if (booksData?.data && booksData.data.length > 0) {
      setPageItems({
        ids: booksData.data.map((b) => b.id),
        type: "book",
      });
    } else {
      setPageItems(null);
    }
    return () => setPageItems(null);
  }, [booksData?.data, setPageItems]);

  // Handle selection with shift+click range support
  const handleSelect = useCallback(
    (id: string, shiftKey: boolean, index?: number) => {
      if (shiftKey && isSelectionMode && index !== undefined) {
        const lastIndex = getLastSelectedIndex(gridId);
        if (lastIndex !== undefined && lastIndex !== index) {
          const start = Math.min(lastIndex, index);
          const end = Math.max(lastIndex, index);
          const rangeIds = booksDataRef.current
            .slice(start, end + 1)
            .map((item) => item.id);
          selectRange(rangeIds, "book");
          return;
        }
      }
      toggleSelection(id, "book", gridId, index);
    },
    [
      toggleSelection,
      selectRange,
      getLastSelectedIndex,
      gridId,
      isSelectionMode,
    ],
  );

  return (
    <Stack gap="md">
      {queryLoading ? (
        showSkeleton ? (
          <CoverGridSkeleton count={pageSize > 12 ? 12 : pageSize} exactCount />
        ) : null
      ) : booksData?.data && booksData.data.length > 0 ? (
        <>
          {/* Top Pagination */}
          {showPagination && (
            <Group justify="center">
              <Pagination
                value={page}
                onChange={handlePageChange}
                total={totalPages}
              />
            </Group>
          )}

          <div
            data-stagger-grid="true"
            style={{
              display: "grid",
              gridTemplateColumns: "repeat(auto-fill, minmax(150px, 1fr))",
              gap: "var(--mantine-spacing-md)",
              width: "100%",
            }}
          >
            {booksData.data.map((book, index) => (
              <MediaCard
                key={book.id}
                type="book"
                data={book}
                index={index}
                onSelect={handleSelect}
                isSelected={selectedIds.has(book.id)}
                isSelectionMode={isSelectionMode}
                canBeSelected={canSelectBooks}
              />
            ))}
          </div>

          {/* Bottom Pagination */}
          {showPagination && (
            <Group justify="center" mt="xl">
              <Pagination
                value={page}
                onChange={handlePageChange}
                total={totalPages}
              />
            </Group>
          )}

          {/* Results info */}
          <Text size="sm" c="dimmed" ta="center">
            Showing {(page - 1) * pageSize + 1} to{" "}
            {Math.min(page * pageSize, booksData.total)} of {booksData.total}{" "}
            books
          </Text>
        </>
      ) : (
        <Card p="xl" withBorder>
          <Stack align="center" gap="sm">
            <Text size="lg" fw={600}>
              {config.emptyTitle}
            </Text>
            <Text size="sm" c="dimmed">
              {config.emptyText}
            </Text>
          </Stack>
        </Card>
      )}
    </Stack>
  );
}
