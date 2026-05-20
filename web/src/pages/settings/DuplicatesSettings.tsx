import {
  ActionIcon,
  Alert,
  Anchor,
  Badge,
  Box,
  Button,
  Card,
  Group,
  Stack,
  Tabs,
  Text,
  Title,
  Tooltip,
} from "@mantine/core";
import { notifications } from "@mantine/notifications";
import {
  IconAlertCircle,
  IconBooks,
  IconCopy,
  IconLibrary,
  IconRefresh,
  IconSearch,
  IconStack2,
  IconTrash,
} from "@tabler/icons-react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useRef, useState } from "react";
import { api } from "@/api/client";
import {
  type DuplicateGroup,
  duplicatesApi,
  type SeriesDuplicateGroup,
  seriesDuplicatesApi,
} from "@/api/duplicates";
import { AppLink } from "@/components/common/AppLink";
import { CardListSkeleton } from "@/components/skeletons";
import { ResponsiveTable } from "@/components/ui";
import { useTaskProgress } from "@/hooks/useTaskProgress";
import { useShowSkeleton } from "@/lib/motion/useShowSkeleton";
import type { Book, Series } from "@/types";

// Duplicate scan task type
const DUPLICATE_SCAN_TASK_TYPE = "find_duplicates";

// Throttle duration for refresh (30 seconds)
const REFRESH_THROTTLE_MS = 30000;

// Book duplicate group card
function BookDuplicateGroupCard({
  group,
  books,
  onDelete,
  isDeleting,
}: {
  group: DuplicateGroup;
  books: Book[];
  onDelete: () => void;
  isDeleting: boolean;
}) {
  const [expanded, setExpanded] = useState(false);

  return (
    <Card withBorder padding="md">
      <Group justify="space-between" mb="md" wrap="nowrap">
        <Group gap="sm" wrap="nowrap" style={{ minWidth: 0, flex: 1 }}>
          <IconCopy size={20} style={{ flexShrink: 0 }} />
          <Box style={{ minWidth: 0, flex: 1 }}>
            <Text fw={500} truncate="end">
              {books[0]?.title ?? `${group.duplicateCount} Duplicates`}
            </Text>
            <Text size="xs" c="dimmed" style={{ fontFamily: "monospace" }}>
              {group.fileHash.slice(0, 16)}...
            </Text>
          </Box>
        </Group>
        <Group gap="xs">
          <Badge variant="light" color="orange">
            {group.duplicateCount} copies
          </Badge>
          <Button
            variant="subtle"
            size="xs"
            onClick={() => setExpanded(!expanded)}
          >
            {expanded ? "Hide" : "Show"} Details
          </Button>
          <Tooltip label="Delete duplicate group (keeps all files)">
            <ActionIcon
              variant="subtle"
              color="red"
              onClick={onDelete}
              loading={isDeleting}
              aria-label="Delete duplicate group"
            >
              <IconTrash size={16} />
            </ActionIcon>
          </Tooltip>
        </Group>
      </Group>

      {expanded && (
        <ResponsiveTable<Book>
          data={books}
          columns={[
            {
              key: "book",
              header: "Book",
              mobilePrimary: true,
              thProps: { style: { width: "20%" } },
              accessor: (book) => (
                <Anchor
                  size="sm"
                  fw={500}
                  truncate="end"
                  c="blue.4"
                  component={AppLink}
                  to={`/books/${book.id}`}
                >
                  {book.title}
                </Anchor>
              ),
            },
            {
              key: "library",
              header: "Library",
              thProps: { style: { width: "15%" } },
              accessor: (book) => (
                <Anchor
                  size="sm"
                  truncate="end"
                  c="blue.4"
                  component={AppLink}
                  to={`/libraries/${book.libraryId}`}
                >
                  {book.libraryName || "-"}
                </Anchor>
              ),
            },
            {
              key: "series",
              header: "Series",
              thProps: { style: { width: "15%" } },
              accessor: (book) =>
                book.seriesId ? (
                  <Anchor
                    size="sm"
                    truncate="end"
                    c="blue.4"
                    component={AppLink}
                    to={`/series/${book.seriesId}`}
                  >
                    {book.seriesName || "-"}
                  </Anchor>
                ) : (
                  <Text size="sm" truncate>
                    -
                  </Text>
                ),
            },
            {
              key: "path",
              header: "Path",
              thProps: { style: { width: "35%" } },
              mobileFullWidth: true,
              accessor: (book) => (
                <Tooltip label={book.filePath}>
                  <Text size="sm" truncate>
                    {book.filePath}
                  </Text>
                </Tooltip>
              ),
            },
            {
              key: "size",
              header: "Size",
              thProps: { style: { width: "15%" } },
              accessor: (book) => (
                <Text size="sm">
                  {book.fileSize
                    ? `${(book.fileSize / 1024 / 1024).toFixed(2)} MB`
                    : "-"}
                </Text>
              ),
            },
          ]}
          getRowKey={(book, index) => `${book.id}-${index}`}
          tableProps={{ layout: "fixed" }}
        />
      )}

      <Text size="xs" c="dimmed" mt="sm">
        Detected: {new Date(group.createdAt).toLocaleString()}
      </Text>
    </Card>
  );
}

// Series duplicate group card
function SeriesDuplicateGroupCard({
  group,
  seriesList,
  onDelete,
  isDeleting,
}: {
  group: SeriesDuplicateGroup;
  seriesList: Series[];
  onDelete: () => void;
  isDeleting: boolean;
}) {
  const [expanded, setExpanded] = useState(false);
  const isExternalId = group.matchType === "external_id";
  const confidence = isExternalId
    ? { color: "green", label: "High confidence" }
    : { color: "orange", label: "Possible match" };
  const primaryLabel = isExternalId ? "External ID" : "Title";

  return (
    <Card withBorder padding="md">
      <Group justify="space-between" mb="md" wrap="nowrap">
        <Group gap="sm" wrap="nowrap" style={{ minWidth: 0, flex: 1 }}>
          <IconStack2 size={20} style={{ flexShrink: 0 }} />
          <Box style={{ minWidth: 0, flex: 1 }}>
            <Text fw={500} truncate="end">
              {seriesList[0]?.title ??
                `${group.duplicateCount} Duplicate Series`}
            </Text>
            <Text size="xs" c="dimmed" style={{ fontFamily: "monospace" }}>
              {primaryLabel}: {group.matchKey}
            </Text>
          </Box>
        </Group>
        <Group gap="xs">
          <Badge variant="light" color={confidence.color}>
            {confidence.label}
          </Badge>
          <Badge variant="light" color="grape">
            {group.duplicateCount} series
          </Badge>
          <Button
            variant="subtle"
            size="xs"
            onClick={() => setExpanded(!expanded)}
          >
            {expanded ? "Hide" : "Show"} Details
          </Button>
          <Tooltip label="Delete duplicate group (keeps all series)">
            <ActionIcon
              variant="subtle"
              color="red"
              onClick={onDelete}
              loading={isDeleting}
              aria-label="Delete series duplicate group"
            >
              <IconTrash size={16} />
            </ActionIcon>
          </Tooltip>
        </Group>
      </Group>

      {expanded && (
        <ResponsiveTable<{ id: string; series: Series | null }>
          data={group.seriesIds.map((id) => ({
            id,
            series: seriesList.find((s) => s.id === id) ?? null,
          }))}
          columns={[
            {
              key: "series",
              header: "Series",
              mobilePrimary: true,
              thProps: { style: { width: "40%" } },
              accessor: ({ id, series }) =>
                series ? (
                  <Anchor
                    size="sm"
                    fw={500}
                    truncate="end"
                    c="blue.4"
                    component={AppLink}
                    to={`/series/${series.id}`}
                  >
                    {series.title}
                  </Anchor>
                ) : (
                  <Text size="sm" c="dimmed" truncate>
                    Series {id.slice(0, 8)} (unavailable)
                  </Text>
                ),
            },
            {
              key: "library",
              header: "Library",
              thProps: { style: { width: "25%" } },
              accessor: ({ series }) =>
                series ? (
                  <Anchor
                    size="sm"
                    truncate="end"
                    c="blue.4"
                    component={AppLink}
                    to={`/libraries/${series.libraryId}`}
                  >
                    {series.libraryName || "-"}
                  </Anchor>
                ) : (
                  <Text size="sm" truncate>
                    -
                  </Text>
                ),
            },
            {
              key: "books",
              header: "Books",
              thProps: { style: { width: "15%" } },
              accessor: ({ series }) => (
                <Text size="sm">{series?.bookCount ?? "-"}</Text>
              ),
            },
            {
              key: "updated",
              header: "Last Updated",
              thProps: { style: { width: "20%" } },
              accessor: ({ series }) => (
                <Text size="sm">
                  {series
                    ? new Date(series.updatedAt).toLocaleDateString()
                    : "-"}
                </Text>
              ),
            },
          ]}
          getRowKey={(row, index) => `${row.id}-${index}`}
          tableProps={{ layout: "fixed" }}
        />
      )}

      <Text size="xs" c="dimmed" mt="sm">
        Detected: {new Date(group.createdAt).toLocaleString()}
      </Text>
    </Card>
  );
}

export function DuplicatesSettings() {
  const queryClient = useQueryClient();
  const [deletingGroupId, setDeletingGroupId] = useState<string | null>(null);
  const [deletingSeriesGroupId, setDeletingSeriesGroupId] = useState<
    string | null
  >(null);
  const [bookDetailsCache, setBookDetailsCache] = useState<Map<string, Book[]>>(
    new Map(),
  );
  const [seriesDetailsCache, setSeriesDetailsCache] = useState<
    Map<string, Series[]>
  >(new Map());

  // Track completed duplicate scan tasks to trigger refresh
  const { activeTasks } = useTaskProgress();
  const lastRefreshTime = useRef<number>(0);
  const processedTaskIds = useRef<Set<string>>(new Set());

  // Fetch duplicates
  const {
    data: duplicates,
    isLoading,
    error,
    refetch: refetchDuplicates,
  } = useQuery({
    queryKey: ["duplicates"],
    queryFn: duplicatesApi.list,
  });
  const showSkeleton = useShowSkeleton(isLoading);

  // Fetch series duplicates
  const {
    data: seriesDuplicatesResponse,
    isLoading: isSeriesLoading,
    error: seriesError,
    refetch: refetchSeriesDuplicates,
  } = useQuery({
    queryKey: ["seriesDuplicates"],
    queryFn: () => seriesDuplicatesApi.list(),
  });
  const showSeriesSkeleton = useShowSkeleton(isSeriesLoading);
  const seriesDuplicates = seriesDuplicatesResponse?.duplicates ?? [];

  // Watch for duplicate scan task completions and refresh (throttled to 30s)
  useEffect(() => {
    const completedScanTasks = activeTasks.filter(
      (task) =>
        task.taskType === DUPLICATE_SCAN_TASK_TYPE &&
        task.status === "completed" &&
        !processedTaskIds.current.has(task.taskId),
    );

    if (completedScanTasks.length > 0) {
      // Mark these tasks as processed
      for (const task of completedScanTasks) {
        processedTaskIds.current.add(task.taskId);
      }

      // Throttle refresh to avoid hammering the API
      const now = Date.now();
      if (now - lastRefreshTime.current >= REFRESH_THROTTLE_MS) {
        lastRefreshTime.current = now;
        refetchDuplicates();
        refetchSeriesDuplicates();
      }
    }
  }, [activeTasks, refetchDuplicates, refetchSeriesDuplicates]);

  // Fetch book details for a group
  const fetchBooksForGroup = async (group: DuplicateGroup): Promise<Book[]> => {
    const cached = bookDetailsCache.get(group.id);
    if (cached) return cached;

    const books: Book[] = [];
    for (const bookId of group.bookIds) {
      try {
        // API returns { book: BookDto, metadata: ... }
        const response = await api.get<{ book: Book }>(`/books/${bookId}`);
        books.push(response.data.book);
      } catch (err) {
        console.error(`Failed to fetch book ${bookId}:`, err);
      }
    }

    setBookDetailsCache((prev) => new Map(prev).set(group.id, books));
    return books;
  };

  // Fetch series details for a group
  const fetchSeriesForGroup = async (
    group: SeriesDuplicateGroup,
  ): Promise<Series[]> => {
    const cached = seriesDetailsCache.get(group.id);
    if (cached) return cached;

    const seriesList: Series[] = [];
    for (const seriesId of group.seriesIds) {
      try {
        const response = await api.get<Series>(`/series/${seriesId}`);
        seriesList.push(response.data);
      } catch (err) {
        console.error(`Failed to fetch series ${seriesId}:`, err);
      }
    }

    setSeriesDetailsCache((prev) => new Map(prev).set(group.id, seriesList));
    return seriesList;
  };

  // Preload book details when duplicates change
  const { data: groupBooks } = useQuery({
    queryKey: ["duplicate-books", duplicates?.map((d) => d.id).join(",")],
    queryFn: async () => {
      if (!duplicates) return new Map<string, Book[]>();

      const results = new Map<string, Book[]>();
      for (const group of duplicates) {
        const books = await fetchBooksForGroup(group);
        results.set(group.id, books);
      }
      return results;
    },
    enabled: !!duplicates && duplicates.length > 0,
  });

  // Preload series details when series duplicates change
  const { data: groupSeries } = useQuery({
    queryKey: [
      "duplicate-series-details",
      seriesDuplicates.map((d) => d.id).join(","),
    ],
    queryFn: async () => {
      const results = new Map<string, Series[]>();
      for (const group of seriesDuplicates) {
        const list = await fetchSeriesForGroup(group);
        results.set(group.id, list);
      }
      return results;
    },
    enabled: seriesDuplicates.length > 0,
  });

  // Mutations
  const scanMutation = useMutation({
    mutationFn: duplicatesApi.scan,
    onSuccess: (result) => {
      queryClient.invalidateQueries({ queryKey: ["duplicates"] });
      queryClient.invalidateQueries({ queryKey: ["seriesDuplicates"] });
      notifications.show({
        title: "Success",
        message: result.message || "Duplicate scan started",
        color: "green",
      });
    },
    onError: () => {
      notifications.show({
        title: "Error",
        message: "Failed to start duplicate scan",
        color: "red",
      });
    },
  });

  const deleteGroupMutation = useMutation({
    mutationFn: async (groupId: string) => {
      setDeletingGroupId(groupId);
      try {
        await duplicatesApi.delete(groupId);
      } finally {
        setDeletingGroupId(null);
      }
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["duplicates"] });
      notifications.show({
        title: "Success",
        message: "Duplicate group deleted",
        color: "green",
      });
    },
    onError: () => {
      notifications.show({
        title: "Error",
        message: "Failed to delete duplicate group",
        color: "red",
      });
    },
  });

  const deleteSeriesGroupMutation = useMutation({
    mutationFn: async (groupId: string) => {
      setDeletingSeriesGroupId(groupId);
      try {
        await seriesDuplicatesApi.delete(groupId);
      } finally {
        setDeletingSeriesGroupId(null);
      }
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["seriesDuplicates"] });
      notifications.show({
        title: "Success",
        message: "Series duplicate group deleted",
        color: "green",
      });
    },
    onError: () => {
      notifications.show({
        title: "Error",
        message: "Failed to delete series duplicate group",
        color: "red",
      });
    },
  });

  const totalDuplicates =
    duplicates?.reduce((sum, group) => sum + group.duplicateCount, 0) || 0;
  const seriesStats = seriesDuplicatesResponse ?? {
    totalGroups: 0,
    totalDuplicateSeries: 0,
    externalIdGroups: 0,
    titleGroups: 0,
  };

  return (
    <Box py="xl" px="md">
      <Stack gap="xl">
        <Group justify="space-between">
          <Box>
            <Title order={1}>Duplicate Detection</Title>
            <Text c="dimmed" size="sm">
              Find and manage duplicate files and series in your library
            </Text>
          </Box>
          <Group gap="xs">
            <Button
              variant="light"
              leftSection={<IconRefresh size={16} />}
              onClick={() => {
                queryClient.invalidateQueries({ queryKey: ["duplicates"] });
                queryClient.invalidateQueries({
                  queryKey: ["seriesDuplicates"],
                });
              }}
            >
              Refresh
            </Button>
            <Button
              leftSection={<IconSearch size={16} />}
              onClick={() => scanMutation.mutate()}
              loading={scanMutation.isPending}
            >
              Scan for Duplicates
            </Button>
          </Group>
        </Group>

        <Tabs defaultValue="books">
          <Tabs.List>
            <Tabs.Tab value="books" leftSection={<IconBooks size={16} />}>
              Books
            </Tabs.Tab>
            <Tabs.Tab value="series" leftSection={<IconLibrary size={16} />}>
              Series
            </Tabs.Tab>
          </Tabs.List>

          <Tabs.Panel value="books" pt="md">
            <Stack gap="xl">
              <Card withBorder>
                <Group justify="space-around">
                  <Box style={{ textAlign: "center" }}>
                    <Text size="xl" fw={700}>
                      {duplicates?.length || 0}
                    </Text>
                    <Text size="sm" c="dimmed">
                      Duplicate Groups
                    </Text>
                  </Box>
                  <Box style={{ textAlign: "center" }}>
                    <Text size="xl" fw={700}>
                      {totalDuplicates}
                    </Text>
                    <Text size="sm" c="dimmed">
                      Total Duplicates
                    </Text>
                  </Box>
                  <Box style={{ textAlign: "center" }}>
                    <Text size="xl" fw={700}>
                      {totalDuplicates - (duplicates?.length || 0)}
                    </Text>
                    <Text size="sm" c="dimmed">
                      Redundant Copies
                    </Text>
                  </Box>
                </Group>
              </Card>

              <Alert icon={<IconAlertCircle size={16} />} color="blue">
                Duplicates are detected by comparing file hashes (SHA-256).
                Files with identical content are grouped together. Deleting a
                duplicate group only removes the tracking record - the actual
                files are not deleted.
              </Alert>

              {isLoading ? (
                showSkeleton ? (
                  <CardListSkeleton count={3} lines={4} />
                ) : null
              ) : error ? (
                <Alert icon={<IconAlertCircle size={16} />} color="red">
                  Failed to load duplicates. Please try again.
                </Alert>
              ) : duplicates && duplicates.length > 0 ? (
                <Stack gap="md">
                  {duplicates.map((group) => (
                    <BookDuplicateGroupCard
                      key={group.id}
                      group={group}
                      books={groupBooks?.get(group.id) || []}
                      onDelete={() => deleteGroupMutation.mutate(group.id)}
                      isDeleting={deletingGroupId === group.id}
                    />
                  ))}
                </Stack>
              ) : (
                <Card withBorder>
                  <Stack align="center" py="xl">
                    <IconCopy size={48} color="gray" />
                    <Text c="dimmed">No duplicate files detected.</Text>
                    <Text size="sm" c="dimmed">
                      Run a scan to check for duplicates in your library.
                    </Text>
                    <Button
                      variant="light"
                      leftSection={<IconSearch size={16} />}
                      onClick={() => scanMutation.mutate()}
                      loading={scanMutation.isPending}
                    >
                      Scan Now
                    </Button>
                  </Stack>
                </Card>
              )}
            </Stack>
          </Tabs.Panel>

          <Tabs.Panel value="series" pt="md">
            <Stack gap="xl">
              <Card withBorder>
                <Group justify="space-around">
                  <Box style={{ textAlign: "center" }}>
                    <Text size="xl" fw={700}>
                      {seriesStats.totalGroups}
                    </Text>
                    <Text size="sm" c="dimmed">
                      Duplicate Groups
                    </Text>
                  </Box>
                  <Box style={{ textAlign: "center" }}>
                    <Text size="xl" fw={700}>
                      {seriesStats.externalIdGroups}
                    </Text>
                    <Text size="sm" c="dimmed">
                      High Confidence
                    </Text>
                  </Box>
                  <Box style={{ textAlign: "center" }}>
                    <Text size="xl" fw={700}>
                      {seriesStats.titleGroups}
                    </Text>
                    <Text size="sm" c="dimmed">
                      Possible Matches
                    </Text>
                  </Box>
                  <Box style={{ textAlign: "center" }}>
                    <Text size="xl" fw={700}>
                      {seriesStats.totalDuplicateSeries}
                    </Text>
                    <Text size="sm" c="dimmed">
                      Total Series
                    </Text>
                  </Box>
                </Group>
              </Card>

              <Alert icon={<IconAlertCircle size={16} />} color="blue">
                Series duplicates are detected by two signals.{" "}
                <Text component="span" fw={600}>
                  High confidence
                </Text>{" "}
                groups share an external plugin ID (e.g. the same MangaBaka
                entry) and span libraries.{" "}
                <Text component="span" fw={600}>
                  Possible matches
                </Text>{" "}
                share a normalized title within the same library and may include
                false positives. Deleting a group only removes the tracking
                record; the underlying series are kept.
              </Alert>

              {isSeriesLoading ? (
                showSeriesSkeleton ? (
                  <CardListSkeleton count={3} lines={4} />
                ) : null
              ) : seriesError ? (
                <Alert icon={<IconAlertCircle size={16} />} color="red">
                  Failed to load series duplicates. Please try again.
                </Alert>
              ) : seriesDuplicates.length > 0 ? (
                <Stack gap="md">
                  {seriesDuplicates.map((group) => (
                    <SeriesDuplicateGroupCard
                      key={group.id}
                      group={group}
                      seriesList={groupSeries?.get(group.id) || []}
                      onDelete={() =>
                        deleteSeriesGroupMutation.mutate(group.id)
                      }
                      isDeleting={deletingSeriesGroupId === group.id}
                    />
                  ))}
                </Stack>
              ) : (
                <Card withBorder>
                  <Stack align="center" py="xl">
                    <IconStack2 size={48} color="gray" />
                    <Text c="dimmed">No duplicate series detected.</Text>
                    <Text size="sm" c="dimmed">
                      Run a scan to check for series sharing external IDs or
                      titles.
                    </Text>
                    <Button
                      variant="light"
                      leftSection={<IconSearch size={16} />}
                      onClick={() => scanMutation.mutate()}
                      loading={scanMutation.isPending}
                    >
                      Scan Now
                    </Button>
                  </Stack>
                </Card>
              )}
            </Stack>
          </Tabs.Panel>
        </Tabs>
      </Stack>
    </Box>
  );
}
