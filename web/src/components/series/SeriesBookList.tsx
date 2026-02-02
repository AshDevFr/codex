import {
  Box,
  Center,
  Group,
  Loader,
  Menu,
  Pagination,
  Select,
  Stack,
  Text,
  Title,
} from "@mantine/core";
import { IconSortAscending } from "@tabler/icons-react";
import { useQuery } from "@tanstack/react-query";
import { useCallback, useMemo, useRef, useState } from "react";
import { seriesApi } from "@/api/series";
import { MediaCard } from "@/components/library/MediaCard";
import {
  selectCanSelectType,
  selectIsSelectionMode,
  useBulkSelectionStore,
} from "@/store/bulkSelectionStore";
import { useUserPreferencesStore } from "@/store/userPreferencesStore";
import type { Book } from "@/types";

interface SeriesBookListProps {
  seriesId: string;
  seriesName: string;
  bookCount: number;
}

type SortOption = {
  value: string;
  label: string;
};

const SORT_OPTIONS: SortOption[] = [
  { value: "number,asc", label: "Number (Ascending)" },
  { value: "number,desc", label: "Number (Descending)" },
  { value: "title,asc", label: "Title (A-Z)" },
  { value: "title,desc", label: "Title (Z-A)" },
  { value: "created_at,desc", label: "Recently Added" },
  { value: "release_date,desc", label: "Release Date (Newest)" },
  { value: "release_date,asc", label: "Release Date (Oldest)" },
];

const PAGE_SIZES = [20, 50, 100];

export function SeriesBookList({
  seriesId,
  seriesName: _seriesName,
  bookCount,
}: SeriesBookListProps) {
  const [page, setPage] = useState(1);
  const [pageSize, setPageSize] = useState(20);
  const [sort, setSort] = useState("number,asc");

  // Bulk selection state - use stable selectors to minimize re-renders
  const isSelectionMode = useBulkSelectionStore(selectIsSelectionMode);
  const canSelectBooks = useBulkSelectionStore(selectCanSelectType("book"));
  const toggleSelection = useBulkSelectionStore(
    (state) => state.toggleSelection,
  );
  const selectRange = useBulkSelectionStore((state) => state.selectRange);
  const getLastSelectedIndex = useBulkSelectionStore(
    (state) => state.getLastSelectedIndex,
  );
  // Get the Set directly for O(1) lookups - only re-renders when the Set changes
  const selectedIds = useBulkSelectionStore((state) => state.selectedIds);

  // Grid ID for range selection tracking
  const gridId = `series-books-${seriesId}`;

  // Ref for storing books data for range selection (updated after query)
  const booksDataRef = useRef<Book[]>([]);

  // Get show deleted preference from user preferences store
  const showDeletedBooks = useUserPreferencesStore((state) =>
    state.getPreference("library.show_deleted_books"),
  );

  // Fetch books for this series
  const {
    data: allBooks,
    isLoading,
    error,
  } = useQuery({
    queryKey: ["series-books", seriesId, showDeletedBooks],
    queryFn: () =>
      seriesApi.getBooks(seriesId, { includeDeleted: showDeletedBooks }),
  });

  // Sort and paginate client-side since the API returns all books
  const sortedBooks = useMemo(() => {
    if (!allBooks) return [];
    const books = [...allBooks];
    const [field, direction] = sort.split(",");
    books.sort((a, b) => {
      let comparison = 0;
      switch (field) {
        case "number":
          comparison = (a.number ?? 0) - (b.number ?? 0);
          break;
        case "title":
          comparison = a.title.localeCompare(b.title);
          break;
        case "created_at":
          comparison =
            new Date(a.createdAt).getTime() - new Date(b.createdAt).getTime();
          break;
        case "release_date":
          // Fall back to createdAt since releaseDate is not in BookDto
          comparison =
            new Date(a.createdAt).getTime() - new Date(b.createdAt).getTime();
          break;
        default:
          comparison = 0;
      }
      return direction === "desc" ? -comparison : comparison;
    });
    return books;
  }, [allBooks, sort]);

  const paginatedBooks = useMemo(() => {
    const start = (page - 1) * pageSize;
    return sortedBooks.slice(start, start + pageSize);
  }, [sortedBooks, page, pageSize]);

  const data = useMemo(
    () => ({
      data: paginatedBooks,
      total: sortedBooks.length,
    }),
    [paginatedBooks, sortedBooks.length],
  );

  // Update booksDataRef when data changes (for range selection)
  if (paginatedBooks) {
    booksDataRef.current = paginatedBooks;
  }

  const totalPages = data ? Math.ceil(data.total / pageSize) : 1;

  // Handle selection with shift+click range support
  const handleSelect = useCallback(
    (id: string, shiftKey: boolean, index?: number) => {
      if (shiftKey && isSelectionMode && index !== undefined) {
        // Shift+click: select range from last selected to current
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
      // Normal click: toggle selection
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

  const currentSortLabel =
    SORT_OPTIONS.find((opt) => opt.value === sort)?.label || "Sort";

  if (error) {
    return (
      <Stack gap="md">
        <Title order={4}>Books ({bookCount})</Title>
        <Text c="red">Failed to load books</Text>
      </Stack>
    );
  }

  return (
    <Stack gap="md">
      <Group justify="space-between" align="center">
        <Title order={4}>Books ({bookCount})</Title>

        <Group gap="sm">
          <Menu shadow="md" width={200}>
            <Menu.Target>
              <Box
                style={{ cursor: "pointer" }}
                component="button"
                bg="transparent"
                bd="none"
              >
                <Group gap="xs">
                  <IconSortAscending size={16} />
                  <Text size="sm">{currentSortLabel}</Text>
                </Group>
              </Box>
            </Menu.Target>
            <Menu.Dropdown>
              {SORT_OPTIONS.map((option) => (
                <Menu.Item
                  key={option.value}
                  onClick={() => {
                    setSort(option.value);
                    setPage(1);
                  }}
                  style={{
                    fontWeight: sort === option.value ? 600 : 400,
                  }}
                >
                  {option.label}
                </Menu.Item>
              ))}
            </Menu.Dropdown>
          </Menu>

          <Select
            size="xs"
            w={80}
            value={pageSize.toString()}
            onChange={(value) => {
              if (value) {
                setPageSize(parseInt(value, 10));
                setPage(1);
              }
            }}
            data={PAGE_SIZES.map((size) => ({
              value: size.toString(),
              label: size.toString(),
            }))}
          />
        </Group>
      </Group>

      {isLoading ? (
        <Center py="xl">
          <Loader size="lg" />
        </Center>
      ) : data?.data.length === 0 ? (
        <Text c="dimmed">No books in this series</Text>
      ) : (
        <>
          <div
            style={{
              display: "grid",
              gridTemplateColumns: "repeat(auto-fill, minmax(150px, 1fr))",
              gap: "var(--mantine-spacing-md)",
              width: "100%",
            }}
          >
            {data?.data.map((book, index) => (
              <MediaCard
                key={book.id}
                type="book"
                data={book}
                hideSeriesName
                index={index}
                onSelect={handleSelect}
                isSelected={selectedIds.has(book.id)}
                isSelectionMode={isSelectionMode}
                canBeSelected={canSelectBooks}
              />
            ))}
          </div>

          {totalPages > 1 && (
            <Center mt="md">
              <Pagination
                value={page}
                onChange={setPage}
                total={totalPages}
                siblings={1}
                boundaries={1}
              />
            </Center>
          )}
        </>
      )}
    </Stack>
  );
}
