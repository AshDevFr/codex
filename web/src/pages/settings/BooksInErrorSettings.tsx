import {
  Accordion,
  ActionIcon,
  Alert,
  Badge,
  Box,
  Button,
  Card,
  Group,
  Image,
  Loader,
  SimpleGrid,
  Skeleton,
  Stack,
  Text,
  Title,
  Tooltip,
} from "@mantine/core";
import { notifications } from "@mantine/notifications";
import {
  IconAlertCircle,
  IconAlertTriangle,
  IconCopy,
  IconDatabase,
  IconDownload,
  IconFileAlert,
  IconFileBroken,
  IconFileUnknown,
  IconPdf,
  IconPhoto,
  IconRefresh,
} from "@tabler/icons-react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useCallback, useEffect, useRef, useState } from "react";
import { Link } from "react-router-dom";
import type {
  BookErrorTypeDto,
  BooksWithErrorsResponse,
  BookWithErrorsDto,
  ErrorGroupDto,
} from "@/api/books";
import { booksApi } from "@/api/books";
import { useTaskProgress } from "@/hooks/useTaskProgress";
import {
  ERROR_TYPES_ORDER,
  getErrorTypeColor,
  getErrorTypeDescription,
  getErrorTypeLabel,
} from "@/utils/bookErrors";

/**
 * Copy text to clipboard and show a notification
 */
function copyToClipboard(text: string, label: string) {
  navigator.clipboard.writeText(text).then(
    () => {
      notifications.show({
        title: "Copied",
        message: `${label} copied to clipboard`,
        color: "green",
      });
    },
    () => {
      notifications.show({
        title: "Error",
        message: "Failed to copy to clipboard",
        color: "red",
      });
    },
  );
}

/**
 * Format a date string for CSV export
 */
function formatDateForCsv(dateStr: string): string {
  try {
    return new Date(dateStr).toISOString();
  } catch {
    return dateStr;
  }
}

/**
 * Escape a value for CSV (handles quotes and commas)
 */
function escapeCsvValue(value: string): string {
  if (value.includes('"') || value.includes(",") || value.includes("\n")) {
    return `"${value.replace(/"/g, '""')}"`;
  }
  return value;
}

/**
 * Export errors to CSV format
 */
function exportErrorsToCsv(groups: ErrorGroupDto[]) {
  const headers = [
    "Book Title",
    "Series Name",
    "File Path",
    "File Format",
    "Page Count",
    "Error Type",
    "Error Message",
    "Error Occurred At",
    "Book Created At",
  ];

  const rows: string[][] = [];

  // Sort groups by predefined order
  const sortedGroups = [...groups].sort((a, b) => {
    const aIndex = ERROR_TYPES_ORDER.indexOf(a.errorType);
    const bIndex = ERROR_TYPES_ORDER.indexOf(b.errorType);
    return aIndex - bIndex;
  });

  for (const group of sortedGroups) {
    // Sort books within group by title, then by file path
    const sortedBooks = [...group.books].sort((a, b) => {
      const titleCompare = a.book.title.localeCompare(b.book.title);
      if (titleCompare !== 0) return titleCompare;
      return a.book.filePath.localeCompare(b.book.filePath);
    });

    for (const bookWithErrors of sortedBooks) {
      const { book, errors } = bookWithErrors;

      // Sort errors by type order, then by occurred date
      const sortedErrors = [...errors].sort((a, b) => {
        const aIndex = ERROR_TYPES_ORDER.indexOf(a.errorType);
        const bIndex = ERROR_TYPES_ORDER.indexOf(b.errorType);
        if (aIndex !== bIndex) return aIndex - bIndex;
        return (
          new Date(a.occurredAt).getTime() - new Date(b.occurredAt).getTime()
        );
      });

      for (const error of sortedErrors) {
        rows.push([
          escapeCsvValue(book.title),
          escapeCsvValue(book.seriesName || ""),
          escapeCsvValue(book.filePath),
          book.fileFormat.toUpperCase(),
          book.pageCount?.toString() || "",
          getErrorTypeLabel(error.errorType),
          escapeCsvValue(error.message),
          formatDateForCsv(error.occurredAt),
          formatDateForCsv(book.createdAt),
        ]);
      }
    }
  }

  const csvContent = [
    headers.join(","),
    ...rows.map((row) => row.join(",")),
  ].join("\n");

  // Create and download the file
  const blob = new Blob([csvContent], { type: "text/csv;charset=utf-8;" });
  const link = document.createElement("a");
  const url = URL.createObjectURL(blob);
  link.setAttribute("href", url);
  link.setAttribute(
    "download",
    `book-errors-${new Date().toISOString().split("T")[0]}.csv`,
  );
  link.style.visibility = "hidden";
  document.body.appendChild(link);
  link.click();
  document.body.removeChild(link);
  URL.revokeObjectURL(url);

  notifications.show({
    title: "Export Complete",
    message: `Exported ${rows.length} error(s) to CSV`,
    color: "green",
  });
}

// Task types that indicate book error changes
const BOOK_ERROR_TASK_TYPES = ["analyze_book", "generate_thumbnail"];

// Throttle duration for refresh (10 seconds)
const REFRESH_THROTTLE_MS = 10000;

// Icon component for each error type
function ErrorTypeIcon({
  errorType,
  size = 24,
}: {
  errorType: BookErrorTypeDto;
  size?: number;
}) {
  const iconProps = { size };
  switch (errorType) {
    case "format_detection":
      return <IconFileUnknown {...iconProps} />;
    case "parser":
      return <IconFileAlert {...iconProps} />;
    case "metadata":
      return <IconDatabase {...iconProps} />;
    case "thumbnail":
      return <IconPhoto {...iconProps} />;
    case "page_extraction":
      return <IconFileBroken {...iconProps} />;
    case "pdf_rendering":
      return <IconPdf {...iconProps} />;
    default:
      return <IconAlertCircle {...iconProps} />;
  }
}

// Stat card component
function StatCard({
  title,
  value,
  color,
  icon,
  onClick,
}: {
  title: string;
  value: number;
  color: string;
  icon: React.ReactNode;
  onClick?: () => void;
}) {
  return (
    <Card
      withBorder
      padding="md"
      onClick={onClick}
      style={{ cursor: onClick ? "pointer" : "default" }}
    >
      <Group justify="space-between">
        <div>
          <Text size="xs" c="dimmed" tt="uppercase" fw={700}>
            {title}
          </Text>
          <Text size="xl" fw={700}>
            {value.toLocaleString()}
          </Text>
        </div>
        <Box c={color}>{icon}</Box>
      </Group>
    </Card>
  );
}

// Book error card component
function BookErrorCard({
  bookWithErrors,
  onRetry,
  isRetrying,
}: {
  bookWithErrors: BookWithErrorsDto;
  onRetry: (bookId: string, errorTypes?: BookErrorTypeDto[]) => void;
  isRetrying: boolean;
}) {
  const { book, errors } = bookWithErrors;
  const [imageLoaded, setImageLoaded] = useState(false);

  const handleImageLoad = useCallback(() => {
    setImageLoaded(true);
  }, []);

  const handleImageError = useCallback(() => {
    setImageLoaded(true); // Stop showing skeleton on error
  }, []);

  // biome-ignore lint/correctness/useExhaustiveDependencies: intentionally reset on ID change
  useEffect(() => {
    setImageLoaded(false);
  }, [book.id]);

  const thumbnailUrl = `/api/v1/books/${book.id}/thumbnail`;

  return (
    <Card withBorder padding="sm">
      <Group align="flex-start" gap="md" wrap="nowrap">
        {/* Book thumbnail */}
        <Box
          style={{
            width: 60,
            height: 85,
            flexShrink: 0,
            position: "relative",
            overflow: "hidden",
            borderRadius: 4,
          }}
        >
          {!imageLoaded && (
            <Skeleton
              style={{
                position: "absolute",
                top: 0,
                left: 0,
                width: "100%",
                height: "100%",
              }}
              animate
            />
          )}
          <Image
            src={thumbnailUrl}
            alt={book.title}
            fit="cover"
            style={{
              width: "100%",
              height: "100%",
              objectFit: "cover",
              opacity: imageLoaded ? 1 : 0,
              transition: "opacity 0.2s ease-in-out",
            }}
            onLoad={handleImageLoad}
            onError={handleImageError}
            fallbackSrc="data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='60' height='85'%3E%3Crect fill='%23ddd' width='60' height='85'/%3E%3Ctext fill='%23999' font-family='sans-serif' font-size='8' x='50%25' y='50%25' text-anchor='middle' dy='.3em'%3ENo Cover%3C/text%3E%3C/svg%3E"
          />
        </Box>

        {/* Book info and errors */}
        <Stack gap="xs" style={{ flex: 1, minWidth: 0 }}>
          <Group justify="space-between" wrap="nowrap">
            <div style={{ minWidth: 0, flex: 1 }}>
              <Text
                component={Link}
                to={`/books/${book.id}`}
                fw={600}
                size="sm"
                lineClamp={1}
                style={{
                  textDecoration: "none",
                  color: "inherit",
                }}
                className="hover-underline"
              >
                {book.title}
              </Text>
              {book.seriesName && (
                <Text size="xs" c="dimmed" lineClamp={1}>
                  {book.seriesName}
                </Text>
              )}
            </div>
            <Button
              size="xs"
              variant="light"
              leftSection={<IconRefresh size={14} />}
              onClick={() => onRetry(book.id)}
              loading={isRetrying}
            >
              Retry
            </Button>
          </Group>

          {/* Error messages */}
          <Stack gap={4}>
            {[...errors]
              .sort((a, b) => {
                // Sort by error type order first, then by occurred date
                const aIndex = ERROR_TYPES_ORDER.indexOf(a.errorType);
                const bIndex = ERROR_TYPES_ORDER.indexOf(b.errorType);
                if (aIndex !== bIndex) return aIndex - bIndex;
                return (
                  new Date(a.occurredAt).getTime() -
                  new Date(b.occurredAt).getTime()
                );
              })
              .map((error) => (
                <Group key={error.errorType} gap="xs" wrap="nowrap">
                  <Badge
                    size="xs"
                    color={getErrorTypeColor(error.errorType)}
                    variant="light"
                    leftSection={
                      <ErrorTypeIcon errorType={error.errorType} size={12} />
                    }
                  >
                    {getErrorTypeLabel(error.errorType)}
                  </Badge>
                  <Tooltip
                    label={error.message}
                    multiline
                    maw={400}
                    openDelay={300}
                  >
                    <Text size="xs" c="red" lineClamp={1} style={{ flex: 1 }}>
                      {error.message}
                    </Text>
                  </Tooltip>
                  <Tooltip label="Copy error to clipboard">
                    <ActionIcon
                      size="xs"
                      variant="subtle"
                      color="gray"
                      onClick={() => copyToClipboard(error.message, "Error")}
                    >
                      <IconCopy size={14} />
                    </ActionIcon>
                  </Tooltip>
                </Group>
              ))}
          </Stack>

          <Text size="xs" c="dimmed">
            {book.fileFormat.toUpperCase()}
            {book.pageCount && ` - ${book.pageCount} pages`}
          </Text>
        </Stack>
      </Group>
    </Card>
  );
}

// Error group accordion item
function ErrorGroupAccordion({
  group,
  onRetry,
  onRetryAll,
  retryingBookIds,
  isRetryingAll,
}: {
  group: ErrorGroupDto;
  onRetry: (bookId: string, errorTypes?: BookErrorTypeDto[]) => void;
  onRetryAll: (errorType: BookErrorTypeDto) => void;
  retryingBookIds: Set<string>;
  isRetryingAll: boolean;
}) {
  return (
    <Accordion.Item value={group.errorType}>
      <Accordion.Control>
        <Group gap="sm">
          <ErrorTypeIcon errorType={group.errorType} size={20} />
          <Text fw={500}>{group.label}</Text>
          <Badge color={getErrorTypeColor(group.errorType)} variant="filled">
            {group.count}
          </Badge>
        </Group>
      </Accordion.Control>
      <Accordion.Panel>
        <Stack gap="md">
          <Group justify="space-between">
            <Tooltip
              label={getErrorTypeDescription(group.errorType)}
              multiline
              maw={400}
            >
              <Text size="sm" c="dimmed">
                {getErrorTypeDescription(group.errorType)}
              </Text>
            </Tooltip>
            <Button
              size="xs"
              variant="light"
              color={getErrorTypeColor(group.errorType)}
              leftSection={<IconRefresh size={14} />}
              onClick={() => onRetryAll(group.errorType)}
              loading={isRetryingAll}
            >
              Retry All ({group.count})
            </Button>
          </Group>

          <Stack gap="sm">
            {[...group.books]
              .sort((a, b) => {
                // Sort by title first, then by file path for consistency
                const titleCompare = a.book.title.localeCompare(b.book.title);
                if (titleCompare !== 0) return titleCompare;
                return a.book.filePath.localeCompare(b.book.filePath);
              })
              .map((bookWithErrors) => (
                <BookErrorCard
                  key={bookWithErrors.book.id}
                  bookWithErrors={bookWithErrors}
                  onRetry={onRetry}
                  isRetrying={retryingBookIds.has(bookWithErrors.book.id)}
                />
              ))}
          </Stack>
        </Stack>
      </Accordion.Panel>
    </Accordion.Item>
  );
}

export function BooksInErrorSettings() {
  const queryClient = useQueryClient();
  const [retryingBookIds, setRetryingBookIds] = useState<Set<string>>(
    new Set(),
  );
  const [retryingErrorTypes, setRetryingErrorTypes] = useState<Set<string>>(
    new Set(),
  );

  // Track completed tasks to trigger refresh
  const { activeTasks } = useTaskProgress();
  const lastRefreshTime = useRef<number>(0);
  const processedTaskIds = useRef<Set<string>>(new Set());

  // Fetch books with errors
  const {
    data: errorsData,
    isLoading,
    refetch,
  } = useQuery<BooksWithErrorsResponse>({
    queryKey: ["books-with-errors"],
    queryFn: () => booksApi.getBooksWithErrors({ pageSize: 100 }),
  });

  // Watch for completed analysis/thumbnail tasks and refresh
  useEffect(() => {
    const completedTasks = activeTasks.filter(
      (task) =>
        BOOK_ERROR_TASK_TYPES.includes(task.taskType) &&
        task.status === "completed" &&
        !processedTaskIds.current.has(task.taskId),
    );

    if (completedTasks.length > 0) {
      // Mark these tasks as processed
      for (const task of completedTasks) {
        processedTaskIds.current.add(task.taskId);
      }

      // Throttle refresh
      const now = Date.now();
      if (now - lastRefreshTime.current >= REFRESH_THROTTLE_MS) {
        lastRefreshTime.current = now;
        refetch();
      }
    }
  }, [activeTasks, refetch]);

  // Retry single book mutation
  const retryBookMutation = useMutation({
    mutationFn: ({
      bookId,
      errorTypes,
    }: {
      bookId: string;
      errorTypes?: BookErrorTypeDto[];
    }) => booksApi.retryBookErrors(bookId, errorTypes),
    onMutate: ({ bookId }) => {
      setRetryingBookIds((prev) => new Set(prev).add(bookId));
    },
    onSuccess: (data) => {
      notifications.show({
        title: "Retry Queued",
        message: data.message,
        color: "blue",
      });
      queryClient.invalidateQueries({ queryKey: ["tasks"] });
      queryClient.invalidateQueries({ queryKey: ["task-stats"] });
    },
    onError: (error: Error) => {
      notifications.show({
        title: "Error",
        message: error.message || "Failed to queue retry",
        color: "red",
      });
    },
    onSettled: (_, __, { bookId }) => {
      setRetryingBookIds((prev) => {
        const next = new Set(prev);
        next.delete(bookId);
        return next;
      });
    },
  });

  // Retry all errors mutation
  const retryAllMutation = useMutation({
    mutationFn: ({ errorType }: { errorType?: BookErrorTypeDto }) =>
      booksApi.retryAllErrors({ errorType }),
    onMutate: ({ errorType }) => {
      if (errorType) {
        setRetryingErrorTypes((prev) => new Set(prev).add(errorType));
      }
    },
    onSuccess: (data) => {
      notifications.show({
        title: "Retry Queued",
        message: data.message,
        color: "blue",
      });
      queryClient.invalidateQueries({ queryKey: ["tasks"] });
      queryClient.invalidateQueries({ queryKey: ["task-stats"] });
    },
    onError: (error: Error) => {
      notifications.show({
        title: "Error",
        message: error.message || "Failed to queue retries",
        color: "red",
      });
    },
    onSettled: (_, __, { errorType }) => {
      if (errorType) {
        setRetryingErrorTypes((prev) => {
          const next = new Set(prev);
          next.delete(errorType);
          return next;
        });
      }
    },
  });

  const handleRetryBook = useCallback(
    (bookId: string, errorTypes?: BookErrorTypeDto[]) => {
      retryBookMutation.mutate({ bookId, errorTypes });
    },
    [retryBookMutation],
  );

  const handleRetryAllByType = useCallback(
    (errorType: BookErrorTypeDto) => {
      retryAllMutation.mutate({ errorType });
    },
    [retryAllMutation],
  );

  const handleRetryAllErrors = useCallback(() => {
    retryAllMutation.mutate({});
  }, [retryAllMutation]);

  // Sort groups by the predefined order
  const sortedGroups = errorsData?.groups
    ? [...errorsData.groups].sort((a, b) => {
        const aIndex = ERROR_TYPES_ORDER.indexOf(a.errorType);
        const bIndex = ERROR_TYPES_ORDER.indexOf(b.errorType);
        return aIndex - bIndex;
      })
    : [];

  const totalBooks = errorsData?.totalBooksWithErrors || 0;
  const hasErrors = totalBooks > 0;

  if (isLoading) {
    return (
      <Box py="xl" px="md">
        <Stack gap="xl" align="center">
          <Loader size="lg" />
          <Text c="dimmed">Loading books with errors...</Text>
        </Stack>
      </Box>
    );
  }

  return (
    <Box py="xl" px="md">
      <Stack gap="xl">
        <Group justify="space-between">
          <div>
            <Title order={1}>Books in Error</Title>
            <Text c="dimmed" size="sm">
              View and retry books that failed analysis or thumbnail generation
            </Text>
          </div>
          <Group gap="xs">
            {hasErrors && (
              <Button
                variant="light"
                leftSection={<IconDownload size={16} />}
                onClick={() => exportErrorsToCsv(errorsData?.groups || [])}
              >
                Export CSV
              </Button>
            )}
            <Button
              variant="light"
              leftSection={<IconRefresh size={16} />}
              onClick={() => refetch()}
            >
              Refresh
            </Button>
            {hasErrors && (
              <Button
                variant="filled"
                color="blue"
                leftSection={<IconRefresh size={16} />}
                onClick={handleRetryAllErrors}
                loading={
                  retryAllMutation.isPending &&
                  !retryAllMutation.variables?.errorType
                }
              >
                Retry All ({totalBooks})
              </Button>
            )}
          </Group>
        </Group>

        {/* Info Alert */}
        <Alert
          icon={<IconAlertTriangle size={16} />}
          color="yellow"
          title="About Book Errors"
        >
          Books may fail to process due to corrupted files, unsupported formats,
          or missing dependencies. You can retry individual books or all books
          of a specific error type. Successfully processed books will be removed
          from this list automatically.
        </Alert>

        {/* Stats Overview */}
        <SimpleGrid cols={{ base: 2, sm: 3, md: 4 }} spacing="md">
          <StatCard
            title="Total Errors"
            value={totalBooks}
            color={hasErrors ? "red" : "green"}
            icon={<IconAlertCircle size={32} />}
          />
          {sortedGroups.slice(0, 3).map((group) => (
            <StatCard
              key={group.errorType}
              title={group.label}
              value={group.count}
              color={getErrorTypeColor(group.errorType)}
              icon={<ErrorTypeIcon errorType={group.errorType} size={32} />}
            />
          ))}
        </SimpleGrid>

        {/* Error Groups */}
        {hasErrors ? (
          <Accordion
            variant="separated"
            defaultValue={sortedGroups[0]?.errorType}
          >
            {sortedGroups.map((group) => (
              <ErrorGroupAccordion
                key={group.errorType}
                group={group}
                onRetry={handleRetryBook}
                onRetryAll={handleRetryAllByType}
                retryingBookIds={retryingBookIds}
                isRetryingAll={retryingErrorTypes.has(group.errorType)}
              />
            ))}
          </Accordion>
        ) : (
          <Card withBorder>
            <Stack align="center" py="xl">
              <IconAlertCircle size={48} color="var(--mantine-color-green-6)" />
              <Title order={3}>No Books in Error</Title>
              <Text c="dimmed" ta="center">
                All books have been processed successfully. If you add new books
                and they fail to process, they will appear here.
              </Text>
            </Stack>
          </Card>
        )}
      </Stack>
    </Box>
  );
}
