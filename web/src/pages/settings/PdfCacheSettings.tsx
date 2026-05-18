import {
  Alert,
  Badge,
  Box,
  Button,
  Card,
  Group,
  Modal,
  ScrollArea,
  SimpleGrid,
  Skeleton,
  Stack,
  Table,
  Text,
  Title,
  Tooltip,
} from "@mantine/core";
import { notifications } from "@mantine/notifications";
import {
  IconAlertCircle,
  IconBook,
  IconClock,
  IconDatabaseCog,
  IconFile,
  IconFolder,
  IconRefresh,
  IconStack2,
  IconTarget,
  IconTrash,
} from "@tabler/icons-react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useRef, useState } from "react";
import type {
  PdfCacheCleanupResultDto,
  PdfCacheStatsDto,
  PdfHandleCacheClearResultDto,
} from "@/api/pdfCache";
import { pdfCacheApi } from "@/api/pdfCache";
import { useTaskProgress } from "@/hooks/useTaskProgress";
import { useShowSkeleton } from "@/lib/motion/useShowSkeleton";

// Cleanup task types that should trigger a stats refresh
const PDF_CACHE_TASK_TYPES = ["cleanup_pdf_cache"];

// Throttle duration for stats refresh (30 seconds)
const REFRESH_THROTTLE_MS = 30000;

// Stat card component
function StatCard({
  title,
  value,
  subtitle,
  color,
  icon,
}: {
  title: string;
  value: string | number;
  subtitle?: string;
  color: string;
  icon: React.ReactNode;
}) {
  return (
    <Card withBorder padding="md">
      <Group justify="space-between">
        <div>
          <Text size="xs" c="dimmed" tt="uppercase" fw={700}>
            {title}
          </Text>
          <Text size="xl" fw={700}>
            {typeof value === "number" ? value.toLocaleString() : value}
          </Text>
          {subtitle && (
            <Text size="xs" c="dimmed">
              {subtitle}
            </Text>
          )}
        </div>
        <Box c={color}>{icon}</Box>
      </Group>
    </Card>
  );
}

function formatIdleSeconds(seconds: number): string {
  if (seconds < 60) return `${seconds}s`;
  if (seconds < 3600) return `${Math.round(seconds / 60)}m`;
  return `${(seconds / 3600).toFixed(1)}h`;
}

export function PdfCacheSettings() {
  const queryClient = useQueryClient();
  const [clearPagesModalOpened, setClearPagesModalOpened] = useState(false);
  const [cleanupModalOpened, setCleanupModalOpened] = useState(false);
  const [clearHandlesModalOpened, setClearHandlesModalOpened] = useState(false);

  // Track completed cleanup tasks to trigger refresh
  const { activeTasks } = useTaskProgress();
  const lastRefreshTime = useRef<number>(0);
  const processedTaskIds = useRef<Set<string>>(new Set());

  // Fetch combined cache stats
  const {
    data: stats,
    isLoading: statsLoading,
    refetch: refetchStats,
  } = useQuery<PdfCacheStatsDto>({
    queryKey: ["pdf-cache-stats"],
    queryFn: () => pdfCacheApi.getStats(),
  });
  const showSkeleton = useShowSkeleton(statsLoading);

  const pageStats = stats?.pages;
  const handleStats = stats?.handles;

  // Watch for cleanup task completions and refresh stats (throttled to 30s)
  useEffect(() => {
    const completedCleanupTasks = activeTasks.filter(
      (task) =>
        PDF_CACHE_TASK_TYPES.includes(task.taskType) &&
        task.status === "completed" &&
        !processedTaskIds.current.has(task.taskId),
    );

    if (completedCleanupTasks.length > 0) {
      for (const task of completedCleanupTasks) {
        processedTaskIds.current.add(task.taskId);
      }

      const now = Date.now();
      if (now - lastRefreshTime.current >= REFRESH_THROTTLE_MS) {
        lastRefreshTime.current = now;
        refetchStats();
      }
    }
  }, [activeTasks, refetchStats]);

  // Page cache: trigger async cleanup (background task)
  const triggerCleanupMutation = useMutation({
    mutationFn: pdfCacheApi.triggerCleanup,
    onSuccess: (data) => {
      setCleanupModalOpened(false);
      notifications.show({
        title: "Cleanup Task Queued",
        message: `Task ${data.taskId.slice(0, 8)}... has been queued. Cleaning pages older than ${data.maxAgeDays} days.`,
        color: "blue",
      });
      queryClient.invalidateQueries({ queryKey: ["tasks"] });
      queryClient.invalidateQueries({ queryKey: ["task-stats"] });
    },
    onError: () => {
      notifications.show({
        title: "Error",
        message: "Failed to queue cleanup task",
        color: "red",
      });
    },
  });

  // Page cache: clear immediately (sync)
  const clearPageCacheMutation = useMutation<PdfCacheCleanupResultDto>({
    mutationFn: pdfCacheApi.clearPageCache,
    onSuccess: (data) => {
      setClearPagesModalOpened(false);
      queryClient.invalidateQueries({ queryKey: ["pdf-cache-stats"] });

      if (data.filesDeleted > 0) {
        notifications.show({
          title: "Page Cache Cleared",
          message: `Deleted ${data.filesDeleted.toLocaleString()} cached pages, freed ${data.bytesReclaimedHuman}`,
          color: "green",
        });
      } else {
        notifications.show({
          title: "Page Cache Cleared",
          message: "Page cache was already empty",
          color: "blue",
        });
      }
    },
    onError: () => {
      notifications.show({
        title: "Error",
        message: "Failed to clear page cache",
        color: "red",
      });
    },
  });

  // Handle cache: clear immediately
  const clearHandleCacheMutation = useMutation<PdfHandleCacheClearResultDto>({
    mutationFn: pdfCacheApi.clearHandleCache,
    onSuccess: (data) => {
      setClearHandlesModalOpened(false);
      queryClient.invalidateQueries({ queryKey: ["pdf-cache-stats"] });

      if (data.handlesClosed > 0) {
        notifications.show({
          title: "Handles Closed",
          message: `Closed ${data.handlesClosed.toLocaleString()} open PDF document handle${data.handlesClosed === 1 ? "" : "s"}`,
          color: "green",
        });
      } else {
        notifications.show({
          title: "Nothing to close",
          message: "No open document handles were in the cache",
          color: "blue",
        });
      }
    },
    onError: () => {
      notifications.show({
        title: "Error",
        message: "Failed to clear handle cache",
        color: "red",
      });
    },
  });

  // Handle cache: evict a single book
  const evictBookHandleMutation = useMutation<
    PdfHandleCacheClearResultDto,
    Error,
    string
  >({
    mutationFn: (bookId: string) => pdfCacheApi.evictBookHandle(bookId),
    onSuccess: (data, bookId) => {
      queryClient.invalidateQueries({ queryKey: ["pdf-cache-stats"] });
      notifications.show({
        title: data.handlesClosed > 0 ? "Handle Closed" : "No handle to close",
        message:
          data.handlesClosed > 0
            ? `Closed PDF document handle for ${bookId.slice(0, 8)}...`
            : "That book had no cached PDFium handle",
        color: data.handlesClosed > 0 ? "green" : "blue",
      });
    },
    onError: () => {
      notifications.show({
        title: "Error",
        message: "Failed to evict book handle",
        color: "red",
      });
    },
  });

  const hasCachedFiles = (pageStats?.totalFiles || 0) > 0;
  const hasCachedHandles = (handleStats?.currentSize || 0) > 0;
  const pageCacheEnabled = pageStats?.cacheEnabled ?? false;
  const handleCacheEnabled = handleStats?.enabled ?? false;

  if (statsLoading) {
    return showSkeleton ? (
      <Box py="xl" px="md">
        <Stack gap="md">
          <Skeleton height={28} width="40%" radius="sm" />
          <SimpleGrid cols={{ base: 1, sm: 3 }} spacing="md">
            <Skeleton height={96} radius="md" />
            <Skeleton height={96} radius="md" />
            <Skeleton height={96} radius="md" />
          </SimpleGrid>
        </Stack>
      </Box>
    ) : null;
  }

  return (
    <Box py="xl" px="md">
      <Stack gap="xl">
        <Group justify="space-between">
          <div>
            <Title order={1}>PDF Cache</Title>
            <Text c="dimmed" size="sm">
              Manage rendered-page and open-document caches
            </Text>
          </div>
          <Group gap="xs">
            <Button
              variant="light"
              leftSection={<IconRefresh size={16} />}
              onClick={() => refetchStats()}
            >
              Refresh
            </Button>
          </Group>
        </Group>

        {/* Rendered Pages Section */}
        <Stack gap="md">
          <Group justify="space-between" align="flex-end">
            <div>
              <Title order={2}>Rendered Pages (on disk)</Title>
              <Text c="dimmed" size="sm">
                JPEGs of PDF pages rendered for streaming-mode readers
              </Text>
            </div>
            <Group gap="xs">
              {hasCachedFiles && (
                <>
                  <Button
                    variant="light"
                    color="blue"
                    leftSection={<IconTrash size={16} />}
                    onClick={() => setCleanupModalOpened(true)}
                  >
                    Cleanup Old
                  </Button>
                  <Button
                    variant="filled"
                    color="orange"
                    leftSection={<IconTrash size={16} />}
                    onClick={() => setClearPagesModalOpened(true)}
                  >
                    Clear All
                  </Button>
                </>
              )}
            </Group>
          </Group>

          <Alert
            icon={<IconAlertCircle size={16} />}
            color="blue"
            title="About the Page Cache"
          >
            When using streaming mode for PDF files, pages are rendered on the
            server and cached to improve subsequent load times. The cache is
            automatically cleaned up based on the configured max age (default 30
            days). You can manually trigger cleanup or clear the entire cache
            here.
          </Alert>

          {!pageCacheEnabled && (
            <Alert icon={<IconAlertCircle size={16} />} color="yellow">
              PDF page caching is currently disabled. Enable it in your server
              configuration to improve PDF streaming performance.
            </Alert>
          )}

          <SimpleGrid cols={{ base: 1, sm: 2, md: 4 }} spacing="md">
            <StatCard
              title="Cached Pages"
              value={pageStats?.totalFiles || 0}
              subtitle="Total rendered page images"
              color={hasCachedFiles ? "blue" : "gray"}
              icon={<IconFile size={32} />}
            />
            <StatCard
              title="Cache Size"
              value={pageStats?.totalSizeHuman || "0 B"}
              subtitle={`${(pageStats?.totalSizeBytes || 0).toLocaleString()} bytes`}
              color={hasCachedFiles ? "blue" : "gray"}
              icon={<IconFolder size={32} />}
            />
            <StatCard
              title="Books Cached"
              value={pageStats?.bookCount || 0}
              subtitle="Unique books with cached pages"
              color={hasCachedFiles ? "blue" : "gray"}
              icon={<IconBook size={32} />}
            />
            <StatCard
              title="Oldest Page"
              value={
                pageStats?.oldestFileAgeDays !== undefined &&
                pageStats?.oldestFileAgeDays !== null
                  ? `${pageStats.oldestFileAgeDays} days`
                  : "N/A"
              }
              subtitle="Age of oldest cached page"
              color={
                pageStats?.oldestFileAgeDays && pageStats.oldestFileAgeDays > 30
                  ? "orange"
                  : "gray"
              }
              icon={<IconClock size={32} />}
            />
          </SimpleGrid>

          <Card withBorder>
            <Stack gap="md">
              <Group justify="space-between">
                <Title order={4}>Status</Title>
                {pageCacheEnabled ? (
                  hasCachedFiles ? (
                    <Badge color="blue" size="lg">
                      {(pageStats?.totalFiles ?? 0).toLocaleString()} pages
                      cached
                    </Badge>
                  ) : (
                    <Badge color="gray" size="lg">
                      Cache empty
                    </Badge>
                  )
                ) : (
                  <Badge color="yellow" size="lg">
                    Cache disabled
                  </Badge>
                )}
              </Group>
              <Group gap="xs">
                <Text size="sm" c="dimmed">
                  Cache directory:
                </Text>
                <Text size="sm" ff="monospace">
                  {pageStats?.cacheDir || "N/A"}
                </Text>
              </Group>
              {hasCachedFiles ? (
                <Text c="dimmed">
                  The cache contains{" "}
                  {(pageStats?.totalFiles ?? 0).toLocaleString()} rendered pages
                  from {(pageStats?.bookCount ?? 0).toLocaleString()} books,
                  using {pageStats?.totalSizeHuman} of disk space.
                  {pageStats?.oldestFileAgeDays !== undefined &&
                    pageStats?.oldestFileAgeDays !== null && (
                      <>
                        {" "}
                        The oldest cached page is {pageStats.oldestFileAgeDays}{" "}
                        days old.
                      </>
                    )}
                </Text>
              ) : (
                <Text c="dimmed">
                  No pages are currently cached. Pages will be cached as PDFs
                  are viewed in streaming mode.
                </Text>
              )}
            </Stack>
          </Card>
        </Stack>

        {/* Open Documents Section */}
        <Stack gap="md">
          <Group justify="space-between" align="flex-end">
            <div>
              <Title order={2}>Open Documents (in memory)</Title>
              <Text c="dimmed" size="sm">
                PDFium handles cached so the same book is opened at most once
              </Text>
            </div>
            <Group gap="xs">
              {hasCachedHandles && (
                <Button
                  variant="filled"
                  color="orange"
                  leftSection={<IconTrash size={16} />}
                  onClick={() => setClearHandlesModalOpened(true)}
                >
                  Close All
                </Button>
              )}
            </Group>
          </Group>

          <Alert
            icon={<IconAlertCircle size={16} />}
            color="blue"
            title="About Open Documents"
          >
            Codex keeps a bounded LRU of already-opened PDFium documents so that
            subsequent page renders for the same book don&apos;t pay the
            per-request open cost. Handles are automatically evicted when their
            idle TTL expires or when a book file changes.
          </Alert>

          {!handleCacheEnabled && (
            <Alert icon={<IconAlertCircle size={16} />} color="yellow">
              Handle caching is currently disabled. Enable it in your server
              configuration to speed up PDF page renders.
            </Alert>
          )}

          <SimpleGrid cols={{ base: 1, sm: 2, md: 4 }} spacing="md">
            <StatCard
              title="Open Handles"
              value={handleStats?.currentSize || 0}
              subtitle={`Capacity: ${(handleStats?.capacity ?? 0).toLocaleString()}`}
              color={hasCachedHandles ? "blue" : "gray"}
              icon={<IconStack2 size={32} />}
            />
            <StatCard
              title="Cache Hits"
              value={handleStats?.hits || 0}
              subtitle="Renders that reused an open handle"
              color="green"
              icon={<IconTarget size={32} />}
            />
            <StatCard
              title="Opens"
              value={handleStats?.opens || 0}
              subtitle="PDFium opens since startup"
              color={hasCachedHandles ? "blue" : "gray"}
              icon={<IconDatabaseCog size={32} />}
            />
            <StatCard
              title="Evictions"
              value={
                (handleStats?.evictions || 0) +
                (handleStats?.idleEvictions || 0)
              }
              subtitle={`Idle: ${(handleStats?.idleEvictions ?? 0).toLocaleString()} / Capacity: ${(handleStats?.evictions ?? 0).toLocaleString()}`}
              color="gray"
              icon={<IconClock size={32} />}
            />
          </SimpleGrid>

          <Card withBorder>
            <Stack gap="md">
              <Group justify="space-between">
                <Title order={4}>Resident Documents</Title>
                {handleCacheEnabled ? (
                  hasCachedHandles ? (
                    <Badge color="blue" size="lg">
                      {(handleStats?.currentSize ?? 0).toLocaleString()} open
                    </Badge>
                  ) : (
                    <Badge color="gray" size="lg">
                      No open handles
                    </Badge>
                  )
                ) : (
                  <Badge color="yellow" size="lg">
                    Disabled
                  </Badge>
                )}
              </Group>
              {hasCachedHandles ? (
                <ScrollArea.Autosize mah={360}>
                  <Table striped highlightOnHover withTableBorder>
                    <Table.Thead>
                      <Table.Tr>
                        <Table.Th>Book ID</Table.Th>
                        <Table.Th>File</Table.Th>
                        <Table.Th>Renders</Table.Th>
                        <Table.Th>Idle</Table.Th>
                        <Table.Th>Age</Table.Th>
                        <Table.Th />
                      </Table.Tr>
                    </Table.Thead>
                    <Table.Tbody>
                      {(handleStats?.entries ?? []).map((entry) => (
                        <Table.Tr key={entry.bookId}>
                          <Table.Td>
                            <Text ff="monospace" size="xs">
                              {entry.bookId.slice(0, 8)}…
                            </Text>
                          </Table.Td>
                          <Table.Td>
                            <Tooltip label={entry.filePath} withinPortal>
                              <Text size="xs" ff="monospace" truncate maw={320}>
                                {entry.filePath}
                              </Text>
                            </Tooltip>
                          </Table.Td>
                          <Table.Td>
                            {entry.renderCount.toLocaleString()}
                          </Table.Td>
                          <Table.Td>
                            {formatIdleSeconds(entry.idleSeconds)}
                          </Table.Td>
                          <Table.Td>
                            {formatIdleSeconds(entry.ageSeconds)}
                          </Table.Td>
                          <Table.Td>
                            <Button
                              variant="subtle"
                              size="compact-xs"
                              color="orange"
                              leftSection={<IconTrash size={12} />}
                              loading={
                                evictBookHandleMutation.isPending &&
                                evictBookHandleMutation.variables ===
                                  entry.bookId
                              }
                              onClick={() =>
                                evictBookHandleMutation.mutate(entry.bookId)
                              }
                            >
                              Close
                            </Button>
                          </Table.Td>
                        </Table.Tr>
                      ))}
                    </Table.Tbody>
                  </Table>
                </ScrollArea.Autosize>
              ) : (
                <Text c="dimmed">
                  No documents are currently open in memory. Handles are
                  populated as users read PDFs in streaming mode.
                </Text>
              )}
            </Stack>
          </Card>
        </Stack>

        {/* Cleanup Modal */}
        <Modal
          opened={cleanupModalOpened}
          onClose={() => setCleanupModalOpened(false)}
          title="Cleanup Old Cache Entries"
          centered
        >
          <Stack gap="md">
            <Text>
              This will queue a background task to remove cached pages older
              than the configured max age (default 30 days). Recently accessed
              pages will be preserved.
            </Text>
            <Text size="sm" c="dimmed">
              Current cache:{" "}
              {(pageStats?.totalFiles ?? 0).toLocaleString() || 0} pages (
              {pageStats?.totalSizeHuman || "0 B"})
            </Text>
            <Group justify="flex-end">
              <Button
                variant="subtle"
                onClick={() => setCleanupModalOpened(false)}
              >
                Cancel
              </Button>
              <Button
                color="blue"
                loading={triggerCleanupMutation.isPending}
                onClick={() => triggerCleanupMutation.mutate()}
              >
                Queue Cleanup
              </Button>
            </Group>
          </Stack>
        </Modal>

        {/* Clear Pages Modal */}
        <Modal
          opened={clearPagesModalOpened}
          onClose={() => setClearPagesModalOpened(false)}
          title="Clear Entire Page Cache"
          centered
        >
          <Stack gap="md">
            <Alert icon={<IconAlertCircle size={16} />} color="orange">
              This will immediately delete all cached PDF pages on disk. PDFs
              will need to be re-rendered when next viewed in streaming mode.
            </Alert>
            <Text>
              Are you sure you want to clear the entire PDF page cache?
            </Text>
            <Text size="sm" c="dimmed">
              This will delete{" "}
              {(pageStats?.totalFiles ?? 0).toLocaleString() || 0} cached pages
              and free {pageStats?.totalSizeHuman || "0 B"} of disk space.
            </Text>
            <Group justify="flex-end">
              <Button
                variant="subtle"
                onClick={() => setClearPagesModalOpened(false)}
              >
                Cancel
              </Button>
              <Button
                color="orange"
                loading={clearPageCacheMutation.isPending}
                onClick={() => clearPageCacheMutation.mutate()}
              >
                Clear Page Cache
              </Button>
            </Group>
          </Stack>
        </Modal>

        {/* Clear Handles Modal */}
        <Modal
          opened={clearHandlesModalOpened}
          onClose={() => setClearHandlesModalOpened(false)}
          title="Close All Open Documents"
          centered
        >
          <Stack gap="md">
            <Alert icon={<IconAlertCircle size={16} />} color="orange">
              This will close all in-memory PDFium document handles. The next
              page request for each book will re-open the underlying PDF, which
              is slower for the first page after.
            </Alert>
            <Text size="sm" c="dimmed">
              Currently open:{" "}
              {(handleStats?.currentSize ?? 0).toLocaleString() || 0} document
              {handleStats?.currentSize === 1 ? "" : "s"}.
            </Text>
            <Group justify="flex-end">
              <Button
                variant="subtle"
                onClick={() => setClearHandlesModalOpened(false)}
              >
                Cancel
              </Button>
              <Button
                color="orange"
                loading={clearHandleCacheMutation.isPending}
                onClick={() => clearHandleCacheMutation.mutate()}
              >
                Close All Handles
              </Button>
            </Group>
          </Stack>
        </Modal>
      </Stack>
    </Box>
  );
}
