import {
  ActionIcon,
  Button,
  Group,
  Loader,
  Menu,
  Text,
  Tooltip,
} from "@mantine/core";
import { notifications } from "@mantine/notifications";
import {
  IconAnalyze,
  IconBook,
  IconBookOff,
  IconChevronDown,
  IconPhotoPlus,
  IconRefresh,
  IconWand,
  IconX,
} from "@tabler/icons-react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useMemo } from "react";
import { booksApi } from "@/api/books";
import { pluginActionsApi, pluginsApi } from "@/api/plugins";
import { seriesApi } from "@/api/series";
import { usePermissions } from "@/hooks/usePermissions";
import {
  selectSelectionCount,
  selectSelectionType,
  useBulkSelectionStore,
} from "@/store/bulkSelectionStore";
import { PERMISSIONS } from "@/types/permissions";

/**
 * BulkSelectionToolbar - Fixed header toolbar that appears when items are selected
 *
 * Shows:
 * - X button to clear selection
 * - Count of selected items
 * - Action buttons: Mark Read, Mark Unread
 * - More menu: Analyze, Thumbnails (generate missing / regenerate all), Reprocess Titles
 * - Plugin actions dropdown for series:bulk scope
 *
 * Uses bulk API endpoints for efficient batch operations.
 */
export function BulkSelectionToolbar() {
  const queryClient = useQueryClient();
  const { hasPermission } = usePermissions();
  const canWriteBooks = hasPermission(PERMISSIONS.BOOKS_WRITE);
  const canWriteSeries = hasPermission(PERMISSIONS.SERIES_WRITE);
  const canWriteTasks = hasPermission(PERMISSIONS.TASKS_WRITE);

  // Selection state
  const count = useBulkSelectionStore(selectSelectionCount);
  const selectionType = useBulkSelectionStore(selectSelectionType);
  // Get the Set directly and convert to array with useMemo for stable reference
  const selectedIdsSet = useBulkSelectionStore((state) => state.selectedIds);
  const selectedIds = useMemo(
    () => Array.from(selectedIdsSet),
    [selectedIdsSet],
  );
  const clearSelection = useBulkSelectionStore((state) => state.clearSelection);

  // Fetch plugin actions for series:bulk scope (only when series are selected)
  const { data: seriesPluginActions } = useQuery({
    queryKey: ["plugin-actions", "series:bulk"],
    queryFn: () => pluginsApi.getActions("series:bulk"),
    staleTime: 5 * 60 * 1000, // Cache for 5 minutes
    enabled: selectionType === "series" && count > 0,
  });

  // Fetch plugin actions for book:bulk scope (only when books are selected)
  const { data: bookPluginActions } = useQuery({
    queryKey: ["plugin-actions", "book:bulk"],
    queryFn: () => pluginsApi.getActions("book:bulk"),
    staleTime: 5 * 60 * 1000,
    enabled: selectionType === "book" && count > 0,
  });

  // Helper to refetch all related queries
  const refetchAll = () => {
    queryClient.refetchQueries({
      predicate: (query) => {
        const key = query.queryKey[0] as string;
        return (
          key === "books" ||
          key === "series" ||
          key === "series-books" ||
          key === "book-detail"
        );
      },
    });
  };

  // Bulk mark books as read
  const bulkMarkBooksReadMutation = useMutation({
    mutationFn: (bookIds: string[]) => booksApi.bulkMarkAsRead(bookIds),
    onSuccess: (data) => {
      notifications.show({
        title: "Marked as read",
        message: data.message,
        color: "green",
      });
      refetchAll();
      clearSelection();
    },
    onError: (error: Error) => {
      notifications.show({
        title: "Failed to mark as read",
        message: error.message || "Failed to mark books as read",
        color: "red",
      });
    },
  });

  // Bulk mark books as unread
  const bulkMarkBooksUnreadMutation = useMutation({
    mutationFn: (bookIds: string[]) => booksApi.bulkMarkAsUnread(bookIds),
    onSuccess: (data) => {
      notifications.show({
        title: "Marked as unread",
        message: data.message,
        color: "blue",
      });
      refetchAll();
      clearSelection();
    },
    onError: (error: Error) => {
      notifications.show({
        title: "Failed to mark as unread",
        message: error.message || "Failed to mark books as unread",
        color: "red",
      });
    },
  });

  // Bulk analyze books
  const bulkAnalyzeBooksMutation = useMutation({
    mutationFn: (bookIds: string[]) => booksApi.bulkAnalyze(bookIds),
    onSuccess: (data) => {
      notifications.show({
        title: "Analysis started",
        message: data.message,
        color: "blue",
      });
      refetchAll();
      clearSelection();
    },
    onError: (error: Error) => {
      notifications.show({
        title: "Failed to start analysis",
        message: error.message || "Failed to queue book analysis",
        color: "red",
      });
    },
  });

  // Bulk mark series as read
  const bulkMarkSeriesReadMutation = useMutation({
    mutationFn: (seriesIds: string[]) => seriesApi.bulkMarkAsRead(seriesIds),
    onSuccess: (data) => {
      notifications.show({
        title: "Marked as read",
        message: data.message,
        color: "green",
      });
      refetchAll();
      clearSelection();
    },
    onError: (error: Error) => {
      notifications.show({
        title: "Failed to mark as read",
        message: error.message || "Failed to mark series as read",
        color: "red",
      });
    },
  });

  // Bulk mark series as unread
  const bulkMarkSeriesUnreadMutation = useMutation({
    mutationFn: (seriesIds: string[]) => seriesApi.bulkMarkAsUnread(seriesIds),
    onSuccess: (data) => {
      notifications.show({
        title: "Marked as unread",
        message: data.message,
        color: "blue",
      });
      refetchAll();
      clearSelection();
    },
    onError: (error: Error) => {
      notifications.show({
        title: "Failed to mark as unread",
        message: error.message || "Failed to mark series as unread",
        color: "red",
      });
    },
  });

  // Bulk analyze series
  const bulkAnalyzeSeriesMutation = useMutation({
    mutationFn: (seriesIds: string[]) => seriesApi.bulkAnalyze(seriesIds),
    onSuccess: (data) => {
      notifications.show({
        title: "Analysis started",
        message: data.message,
        color: "blue",
      });
      refetchAll();
      clearSelection();
    },
    onError: (error: Error) => {
      notifications.show({
        title: "Failed to start analysis",
        message: error.message || "Failed to queue series analysis",
        color: "red",
      });
    },
  });

  // Bulk auto-match series metadata using a plugin
  const bulkAutoMatchMutation = useMutation({
    mutationFn: ({
      pluginId,
      seriesIds,
    }: {
      pluginId: string;
      seriesIds: string[];
    }) => pluginActionsApi.enqueueBulkAutoMatchTasks(pluginId, seriesIds),
    onSuccess: (data) => {
      if (data.success) {
        notifications.show({
          title: "Auto-match started",
          message: data.message,
          color: "blue",
        });
      } else {
        notifications.show({
          title: "Auto-match",
          message: data.message,
          color: "yellow",
        });
      }
      refetchAll();
      clearSelection();
    },
    onError: (error: Error) => {
      notifications.show({
        title: "Auto-match failed",
        message: error.message || "Failed to start auto-match",
        color: "red",
      });
    },
  });

  // Bulk generate book thumbnails (by book IDs)
  const bulkGenerateBookThumbnailsMutation = useMutation({
    mutationFn: ({ bookIds, force }: { bookIds: string[]; force: boolean }) =>
      booksApi.bulkGenerateThumbnails(bookIds, force),
    onSuccess: (data) => {
      notifications.show({
        title: "Thumbnail generation started",
        message: data.message,
        color: "blue",
      });
      clearSelection();
    },
    onError: (error: Error) => {
      notifications.show({
        title: "Failed to start thumbnail generation",
        message: error.message || "Failed to queue thumbnail generation",
        color: "red",
      });
    },
  });

  // Bulk generate series thumbnails
  const bulkGenerateSeriesThumbnailsMutation = useMutation({
    mutationFn: ({
      seriesIds,
      force,
    }: {
      seriesIds: string[];
      force: boolean;
    }) => seriesApi.bulkGenerateSeriesThumbnails(seriesIds, force),
    onSuccess: (data) => {
      notifications.show({
        title: "Series thumbnail generation started",
        message: data.message,
        color: "blue",
      });
      clearSelection();
    },
    onError: (error: Error) => {
      notifications.show({
        title: "Failed to start thumbnail generation",
        message: error.message || "Failed to queue thumbnail generation",
        color: "red",
      });
    },
  });

  // Bulk generate book thumbnails for series
  const bulkGenerateSeriesBookThumbnailsMutation = useMutation({
    mutationFn: ({
      seriesIds,
      force,
    }: {
      seriesIds: string[];
      force: boolean;
    }) => seriesApi.bulkGenerateBookThumbnails(seriesIds, force),
    onSuccess: (data) => {
      notifications.show({
        title: "Book thumbnail generation started",
        message: data.message,
        color: "blue",
      });
      clearSelection();
    },
    onError: (error: Error) => {
      notifications.show({
        title: "Failed to start book thumbnail generation",
        message: error.message || "Failed to queue book thumbnail generation",
        color: "red",
      });
    },
  });

  // Bulk reprocess series titles
  const bulkReprocessTitlesMutation = useMutation({
    mutationFn: (seriesIds: string[]) =>
      seriesApi.bulkReprocessTitles(seriesIds),
    onSuccess: (data) => {
      notifications.show({
        title: "Title reprocessing started",
        message: data.message,
        color: "blue",
      });
      refetchAll();
      clearSelection();
    },
    onError: (error: Error) => {
      notifications.show({
        title: "Failed to start title reprocessing",
        message: error.message || "Failed to queue title reprocessing",
        color: "red",
      });
    },
  });

  // Keyboard shortcut: Escape to clear selection
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape" && count > 0) {
        clearSelection();
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [count, clearSelection]);

  // Don't render if nothing is selected
  if (count === 0) {
    return null;
  }

  // Determine which mutations to use based on selection type
  const isBooks = selectionType === "book";
  const markReadMutation = isBooks
    ? bulkMarkBooksReadMutation
    : bulkMarkSeriesReadMutation;
  const markUnreadMutation = isBooks
    ? bulkMarkBooksUnreadMutation
    : bulkMarkSeriesUnreadMutation;

  const isAnyPending =
    markReadMutation.isPending ||
    markUnreadMutation.isPending ||
    bulkAnalyzeBooksMutation.isPending ||
    bulkAnalyzeSeriesMutation.isPending ||
    bulkAutoMatchMutation.isPending ||
    bulkGenerateBookThumbnailsMutation.isPending ||
    bulkGenerateSeriesThumbnailsMutation.isPending ||
    bulkGenerateSeriesBookThumbnailsMutation.isPending ||
    bulkReprocessTitlesMutation.isPending;

  // Determine if the "More" menu should be shown based on permissions
  const showBooksMoreMenu = isBooks && (canWriteBooks || canWriteTasks);
  const showSeriesMoreMenu = !isBooks && (canWriteSeries || canWriteTasks);

  // Get available plugin actions based on selection type
  const pluginActions = isBooks
    ? (bookPluginActions?.actions ?? [])
    : (seriesPluginActions?.actions ?? []);
  const hasPluginActions = pluginActions.length > 0 && canWriteSeries;

  // Handle plugin auto-match action
  // Note: Currently only series bulk auto-match is supported
  // Book bulk actions will need a different API when plugins support it
  const handlePluginAutoMatch = (pluginId: string) => {
    if (!isBooks) {
      bulkAutoMatchMutation.mutate({ pluginId, seriesIds: selectedIds });
    }
    // Future: Add book bulk plugin action support here
  };

  const handleMarkRead = () => {
    if (isBooks) {
      bulkMarkBooksReadMutation.mutate(selectedIds);
    } else {
      bulkMarkSeriesReadMutation.mutate(selectedIds);
    }
  };

  const handleMarkUnread = () => {
    if (isBooks) {
      bulkMarkBooksUnreadMutation.mutate(selectedIds);
    } else {
      bulkMarkSeriesUnreadMutation.mutate(selectedIds);
    }
  };

  const handleAnalyze = () => {
    if (isBooks) {
      bulkAnalyzeBooksMutation.mutate(selectedIds);
    } else {
      bulkAnalyzeSeriesMutation.mutate(selectedIds);
    }
  };

  // Book thumbnail handlers (for books selection)
  const handleGenerateMissingBookThumbnails = () => {
    bulkGenerateBookThumbnailsMutation.mutate({
      bookIds: selectedIds,
      force: false,
    });
  };

  const handleRegenerateAllBookThumbnails = () => {
    bulkGenerateBookThumbnailsMutation.mutate({
      bookIds: selectedIds,
      force: true,
    });
  };

  // Series thumbnail handlers (for series selection)
  const handleGenerateMissingSeriesThumbnails = () => {
    bulkGenerateSeriesThumbnailsMutation.mutate({
      seriesIds: selectedIds,
      force: false,
    });
  };

  const handleRegenerateAllSeriesThumbnails = () => {
    bulkGenerateSeriesThumbnailsMutation.mutate({
      seriesIds: selectedIds,
      force: true,
    });
  };

  // Books in series thumbnail handlers (for series selection)
  const handleGenerateMissingBooksInSeriesThumbnails = () => {
    bulkGenerateSeriesBookThumbnailsMutation.mutate({
      seriesIds: selectedIds,
      force: false,
    });
  };

  const handleRegenerateAllBooksInSeriesThumbnails = () => {
    bulkGenerateSeriesBookThumbnailsMutation.mutate({
      seriesIds: selectedIds,
      force: true,
    });
  };

  const handleReprocessTitles = () => {
    bulkReprocessTitlesMutation.mutate(selectedIds);
  };

  const itemLabel = isBooks
    ? count === 1
      ? "book"
      : "books"
    : count === 1
      ? "series"
      : "series";

  return (
    <Group
      className="bulk-selection-toolbar"
      gap="sm"
      px="md"
      py="xs"
      role="toolbar"
      aria-label={`Bulk actions for ${count} selected ${itemLabel}`}
      style={{
        backgroundColor: "var(--mantine-color-orange-filled)",
        borderRadius: "var(--mantine-radius-md)",
      }}
    >
      {/* Close button */}
      <Tooltip label="Clear selection (Esc)">
        <ActionIcon
          variant="transparent"
          color="white"
          size="md"
          onClick={clearSelection}
          aria-label="Clear selection"
        >
          <IconX size={18} />
        </ActionIcon>
      </Tooltip>

      {/* Selection count - announced to screen readers */}
      <Text size="sm" fw={600} c="white" aria-live="polite">
        {count} {itemLabel} selected
      </Text>

      {/* Action buttons */}
      <Group gap="xs" ml="auto">
        {isAnyPending && <Loader size="xs" color="white" />}

        <Tooltip label={`Mark ${count} ${itemLabel} as read`}>
          <Button
            variant="white"
            size="xs"
            leftSection={<IconBook size={16} />}
            onClick={handleMarkRead}
            disabled={isAnyPending}
            loading={markReadMutation.isPending}
          >
            Mark Read
          </Button>
        </Tooltip>

        <Tooltip label={`Mark ${count} ${itemLabel} as unread`}>
          <Button
            variant="white"
            size="xs"
            leftSection={<IconBookOff size={16} />}
            onClick={handleMarkUnread}
            disabled={isAnyPending}
            loading={markUnreadMutation.isPending}
          >
            Mark Unread
          </Button>
        </Tooltip>

        {/* More actions menu - for books (requires write permissions) */}
        {showBooksMoreMenu && (
          <Menu shadow="md" width={220} position="bottom-end">
            <Menu.Target>
              <Tooltip label="More actions">
                <Button
                  variant="white"
                  size="xs"
                  rightSection={<IconChevronDown size={14} />}
                  disabled={isAnyPending}
                  aria-label="More actions"
                >
                  More
                </Button>
              </Tooltip>
            </Menu.Target>

            <Menu.Dropdown>
              {canWriteBooks && (
                <>
                  <Menu.Label>Analysis</Menu.Label>
                  <Menu.Item
                    leftSection={<IconAnalyze size={16} />}
                    onClick={handleAnalyze}
                    disabled={isAnyPending}
                  >
                    Analyze
                  </Menu.Item>
                </>
              )}

              {canWriteTasks && (
                <>
                  {canWriteBooks && <Menu.Divider />}
                  <Menu.Label>Book Thumbnails</Menu.Label>
                  <Menu.Item
                    leftSection={<IconPhotoPlus size={16} />}
                    onClick={handleGenerateMissingBookThumbnails}
                    disabled={isAnyPending}
                  >
                    Generate Missing
                  </Menu.Item>
                  <Menu.Item
                    leftSection={<IconRefresh size={16} />}
                    onClick={handleRegenerateAllBookThumbnails}
                    disabled={isAnyPending}
                  >
                    Regenerate All
                  </Menu.Item>
                </>
              )}
            </Menu.Dropdown>
          </Menu>
        )}

        {/* More actions menu - for series (requires write permissions) */}
        {showSeriesMoreMenu && (
          <Menu shadow="md" width={220} position="bottom-end">
            <Menu.Target>
              <Tooltip label="More actions">
                <Button
                  variant="white"
                  size="xs"
                  rightSection={<IconChevronDown size={14} />}
                  disabled={isAnyPending}
                  aria-label="More actions"
                >
                  More
                </Button>
              </Tooltip>
            </Menu.Target>

            <Menu.Dropdown>
              {canWriteSeries && (
                <>
                  <Menu.Label>Analysis</Menu.Label>
                  <Menu.Item
                    leftSection={<IconAnalyze size={16} />}
                    onClick={handleAnalyze}
                    disabled={isAnyPending}
                  >
                    Analyze
                  </Menu.Item>
                </>
              )}

              {canWriteTasks && (
                <>
                  {canWriteSeries && <Menu.Divider />}
                  <Menu.Label>Series Thumbnails</Menu.Label>
                  <Menu.Item
                    leftSection={<IconPhotoPlus size={16} />}
                    onClick={handleGenerateMissingSeriesThumbnails}
                    disabled={isAnyPending}
                  >
                    Generate Missing
                  </Menu.Item>
                  <Menu.Item
                    leftSection={<IconRefresh size={16} />}
                    onClick={handleRegenerateAllSeriesThumbnails}
                    disabled={isAnyPending}
                  >
                    Regenerate All
                  </Menu.Item>

                  <Menu.Divider />

                  <Menu.Label>Books in Series Thumbnails</Menu.Label>
                  <Menu.Item
                    leftSection={<IconPhotoPlus size={16} />}
                    onClick={handleGenerateMissingBooksInSeriesThumbnails}
                    disabled={isAnyPending}
                  >
                    Generate Missing
                  </Menu.Item>
                  <Menu.Item
                    leftSection={<IconRefresh size={16} />}
                    onClick={handleRegenerateAllBooksInSeriesThumbnails}
                    disabled={isAnyPending}
                  >
                    Regenerate All
                  </Menu.Item>
                </>
              )}

              {canWriteSeries && (
                <>
                  {canWriteTasks && <Menu.Divider />}
                  <Menu.Label>Title Management</Menu.Label>
                  <Menu.Item
                    leftSection={<IconRefresh size={16} />}
                    onClick={handleReprocessTitles}
                    disabled={isAnyPending}
                  >
                    Reprocess Titles
                  </Menu.Item>
                </>
              )}
            </Menu.Dropdown>
          </Menu>
        )}

        {/* Plugin actions menu */}
        {hasPluginActions && (
          <Menu shadow="md" width={200} position="bottom-end">
            <Menu.Target>
              <Tooltip label="Apply metadata from plugins">
                <Button
                  variant="white"
                  size="xs"
                  leftSection={<IconWand size={16} />}
                  rightSection={<IconChevronDown size={14} />}
                  disabled={isAnyPending}
                  loading={bulkAutoMatchMutation.isPending}
                  aria-label="Plugin actions"
                >
                  Plugins
                </Button>
              </Tooltip>
            </Menu.Target>

            <Menu.Dropdown>
              <Menu.Label>Auto-Apply Metadata</Menu.Label>
              {pluginActions.map((action) => (
                <Menu.Item
                  key={action.pluginId}
                  leftSection={<IconWand size={16} />}
                  onClick={() => handlePluginAutoMatch(action.pluginId)}
                  disabled={isAnyPending}
                >
                  {action.pluginDisplayName}
                </Menu.Item>
              ))}
            </Menu.Dropdown>
          </Menu>
        )}
      </Group>
    </Group>
  );
}
