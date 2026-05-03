import {
  ActionIcon,
  Badge,
  Box,
  Breadcrumbs,
  Button,
  Center,
  Grid,
  Group,
  Image,
  Loader,
  Menu,
  Modal,
  Stack,
  Text,
  Title,
  Tooltip,
} from "@mantine/core";
import { useDisclosure } from "@mantine/hooks";
import { notifications } from "@mantine/notifications";
import {
  IconAnalyze,
  IconBook,
  IconBookOff,
  IconCheck,
  IconChevronDown,
  IconChevronRight,
  IconChevronUp,
  IconDotsVertical,
  IconDownload,
  IconEdit,
  IconInfoCircle,
  IconListNumbers,
  IconPhoto,
  IconRefresh,
  IconRestore,
  IconSearch,
  IconWand,
} from "@tabler/icons-react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useMemo, useState } from "react";
import { Link, useNavigate, useParams } from "react-router-dom";
import {
  type PluginActionDto,
  pluginActionsApi,
  pluginsApi,
} from "@/api/plugins";
import { seriesApi } from "@/api/series";
import { seriesMetadataApi } from "@/api/seriesMetadata";
import { settingsApi } from "@/api/settings";
import { sharingTagsApi } from "@/api/sharingTags";
import { AuthorsList } from "@/components/book/AuthorsList";
import { ExternalIdEditModal } from "@/components/common";
import { BulkSelectionToolbar } from "@/components/library/BulkSelectionToolbar";
import { MetadataApplyFlow } from "@/components/metadata";
import {
  AlternateTitles,
  CommunityRating,
  CustomMetadataDisplay,
  ExternalIds,
  ExternalLinks,
  ExternalRatings,
  GenreTagChips,
  SeriesBookList,
  SeriesInfoModal,
  SeriesMetadataEditModal,
  SeriesRating,
} from "@/components/series";
import { formatSeriesCounts } from "@/components/series/seriesCounts";
import { useDynamicDocumentTitle } from "@/hooks/useDocumentTitle";
import { usePermissions } from "@/hooks/usePermissions";
import { useCoverUpdatesStore } from "@/store/coverUpdatesStore";
import { PERMISSIONS } from "@/types/permissions";
import { transformFullSeriesToSeriesContext } from "@/utils/templateUtils";

// Helper to format reading direction
function formatReadingDirection(dir?: string | null): string | null {
  if (!dir) return null;
  const map: Record<string, string> = {
    ltr: "Left to Right",
    rtl: "Right to Left",
    ttb: "Vertical",
    webtoon: "Webtoon",
  };
  return map[dir] || dir;
}

// Helper to format status
function formatStatus(status?: string | null): string | null {
  if (!status) return null;
  return status.charAt(0).toUpperCase() + status.slice(1);
}

export function SeriesDetail() {
  const { seriesId } = useParams<{ seriesId: string }>();
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const { isAdmin, hasPermission } = usePermissions();
  const canEditSeries = hasPermission(PERMISSIONS.SERIES_WRITE);
  const [summaryOpened, { toggle: toggleSummary }] = useDisclosure(false);
  const [editModalOpened, { open: openEditModal, close: closeEditModal }] =
    useDisclosure(false);
  const [infoModalOpened, { open: openInfoModal, close: closeInfoModal }] =
    useDisclosure(false);

  // Get cover update timestamp for cache-busting (forces image reload when cover is regenerated via SSE)
  const coverTimestamp = useCoverUpdatesStore((state) =>
    seriesId ? state.updates[seriesId] : undefined,
  );

  // Plugin metadata flow state
  const [selectedPlugin, setSelectedPlugin] = useState<PluginActionDto | null>(
    null,
  );
  const [preprocessedSearchTitle, setPreprocessedSearchTitle] = useState<
    string | null
  >(null);
  const [
    metadataFlowOpened,
    { open: openMetadataFlow, close: closeMetadataFlow },
  ] = useDisclosure(false);
  const [
    externalIdModalOpened,
    { open: openExternalIdModal, close: closeExternalIdModal },
  ] = useDisclosure(false);
  const [resetConfirmOpened, setResetConfirmOpened] = useState(false);

  // Fetch full series data (includes metadata, genres, tags, etc.)
  const {
    data: series,
    isLoading: seriesLoading,
    error: seriesError,
  } = useQuery({
    queryKey: ["series", seriesId, "full"],
    queryFn: () => seriesApi.getById(seriesId!, { full: true }),
    enabled: !!seriesId,
  });

  // Fetch public settings (for custom metadata template)
  const { data: publicSettings } = useQuery({
    queryKey: ["public-settings"],
    queryFn: () => settingsApi.getPublicSettings(),
    staleTime: 5 * 60 * 1000, // Cache for 5 minutes
  });

  // Fetch sharing tags for this series (admin only)
  const { data: seriesSharingTags } = useQuery({
    queryKey: ["series-sharing-tags", seriesId],
    queryFn: () => sharingTagsApi.getForSeries(seriesId!),
    enabled: !!seriesId && isAdmin,
  });

  // Fetch available plugin actions for series:detail scope, filtered by library
  const { data: pluginActions } = useQuery({
    queryKey: ["plugin-actions", "series:detail", series?.libraryId],
    queryFn: () => pluginsApi.getActions("series:detail", series?.libraryId),
    staleTime: 5 * 60 * 1000, // Cache for 5 minutes
    enabled: canEditSeries && !!series, // Only fetch if user can edit series and series is loaded
  });

  // Fetch books for this series to determine the next book to continue reading
  const { data: seriesBooks } = useQuery({
    queryKey: ["series-books", seriesId, false],
    queryFn: () => seriesApi.getBooks(seriesId!),
    enabled: !!seriesId,
  });

  // Find the next book to read: first in-progress book (by number), or first unread book
  const nextBook = useMemo(() => {
    if (!seriesBooks?.length) return null;
    const sorted = [...seriesBooks].sort(
      (a, b) => (a.number ?? 0) - (b.number ?? 0),
    );
    // Prefer the first book that is in-progress (has progress but not completed)
    const inProgress = sorted.find(
      (b) => b.readProgress && !b.readProgress.completed,
    );
    if (inProgress) return inProgress;
    // Otherwise, the first book with no progress at all
    const unread = sorted.find((b) => !b.readProgress);
    return unread ?? null;
  }, [seriesBooks]);

  // Mutation to fetch preprocessed search title before opening modal
  const searchTitleMutation = useMutation({
    mutationFn: (pluginId: string) => {
      if (!seriesId) throw new Error("Series ID is required");
      return pluginActionsApi.getSearchTitle(seriesId, pluginId);
    },
    onSuccess: (data) => {
      setPreprocessedSearchTitle(data.searchTitle);
      openMetadataFlow();
    },
    onError: (error: Error) => {
      notifications.show({
        title: "Failed to get search title",
        message: error.message,
        color: "red",
      });
    },
  });

  // Handler for plugin action click (fetches preprocessed title, then opens search modal)
  const handlePluginAction = (plugin: PluginActionDto) => {
    setSelectedPlugin(plugin);
    searchTitleMutation.mutate(plugin.pluginId);
  };

  // Auto-match mutation - uses task queue for proper preprocessing rule support
  const autoMatchMutation = useMutation({
    mutationFn: (pluginId: string) => {
      if (!seriesId) throw new Error("Series ID is required");
      return pluginActionsApi.enqueueAutoMatchTask(seriesId, pluginId);
    },
    onSuccess: (data) => {
      const taskId = data.taskIds[0];
      notifications.show({
        title: "Auto-match Started",
        message: taskId
          ? `Task queued (ID: ${taskId.slice(0, 8)}...)`
          : data.message,
        color: "blue",
      });
      // Note: Series will be refreshed via SSE when the task completes
    },
    onError: (error: Error) => {
      notifications.show({
        title: "Auto-match Failed",
        message: error.message,
        color: "red",
      });
    },
  });

  // Handler for auto-match action
  const handleAutoMatch = (plugin: PluginActionDto) => {
    autoMatchMutation.mutate(plugin.pluginId);
  };

  // Handler for metadata apply success
  const handleMetadataApplySuccess = () => {
    // Refetch series data to show updated metadata
    queryClient.invalidateQueries({ queryKey: ["series", seriesId] });
  };

  // Mark as read mutation
  const markAsReadMutation = useMutation({
    mutationFn: () => seriesApi.markAsRead(seriesId!),
    onSuccess: (data) => {
      notifications.show({
        title: "Marked as read",
        message: data.message,
        color: "green",
      });
      // Refetch all book and series related queries to update UI
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
    },
    onError: (error: Error) => {
      notifications.show({
        title: "Failed",
        message: error.message,
        color: "red",
      });
    },
  });

  // Mark as unread mutation
  const markAsUnreadMutation = useMutation({
    mutationFn: () => seriesApi.markAsUnread(seriesId!),
    onSuccess: (data) => {
      notifications.show({
        title: "Marked as unread",
        message: data.message,
        color: "blue",
      });
      // Refetch all book and series related queries to update UI
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
    },
    onError: (error: Error) => {
      notifications.show({
        title: "Failed",
        message: error.message,
        color: "red",
      });
    },
  });

  // Analyze mutation
  const analyzeMutation = useMutation({
    mutationFn: () => seriesApi.analyze(seriesId!),
    onSuccess: () => {
      notifications.show({
        title: "Analysis started",
        message: "All books in series queued for analysis",
        color: "blue",
      });
    },
    onError: (error: Error) => {
      notifications.show({
        title: "Failed",
        message: error.message,
        color: "red",
      });
    },
  });

  // Analyze unanalyzed mutation
  const analyzeUnanalyzedMutation = useMutation({
    mutationFn: () => seriesApi.analyzeUnanalyzed(seriesId!),
    onSuccess: () => {
      notifications.show({
        title: "Analysis started",
        message: "Unanalyzed books queued for analysis",
        color: "blue",
      });
    },
    onError: (error: Error) => {
      notifications.show({
        title: "Failed",
        message: error.message,
        color: "red",
      });
    },
  });

  // Renumber books mutation
  const renumberMutation = useMutation({
    mutationFn: () => seriesApi.renumber(seriesId!),
    onSuccess: () => {
      notifications.show({
        title: "Renumber started",
        message: "Books will be renumbered using library rules",
        color: "blue",
      });
    },
    onError: (error: Error) => {
      notifications.show({
        title: "Failed to renumber",
        message: error.message,
        color: "red",
      });
    },
  });

  // Generate missing book thumbnails mutation
  const generateMissingBookThumbnailsMutation = useMutation({
    mutationFn: () => seriesApi.generateMissingBookThumbnails(seriesId!),
    onSuccess: () => {
      notifications.show({
        title: "Thumbnail generation started",
        message: "Missing book thumbnails queued for generation",
        color: "blue",
      });
    },
    onError: (error: Error) => {
      notifications.show({
        title: "Failed",
        message: error.message,
        color: "red",
      });
    },
  });

  // Regenerate all book thumbnails mutation
  const regenerateBookThumbnailsMutation = useMutation({
    mutationFn: () => seriesApi.regenerateBookThumbnails(seriesId!),
    onSuccess: () => {
      notifications.show({
        title: "Thumbnail regeneration started",
        message: "All books queued for thumbnail regeneration",
        color: "blue",
      });
    },
    onError: (error: Error) => {
      notifications.show({
        title: "Failed",
        message: error.message,
        color: "red",
      });
    },
  });

  // Generate series cover thumbnail if missing mutation
  const generateSeriesThumbnailIfMissingMutation = useMutation({
    mutationFn: () => seriesApi.generateSeriesThumbnailIfMissing(seriesId!),
    onSuccess: () => {
      notifications.show({
        title: "Series cover generation started",
        message: "Series cover thumbnail will be generated if missing",
        color: "blue",
      });
      queryClient.invalidateQueries({ queryKey: ["series", seriesId] });
    },
    onError: (error: Error) => {
      notifications.show({
        title: "Failed",
        message: error.message,
        color: "red",
      });
    },
  });

  // Regenerate series cover thumbnail mutation
  const regenerateSeriesThumbnailMutation = useMutation({
    mutationFn: () => seriesApi.regenerateSeriesThumbnail(seriesId!),
    onSuccess: () => {
      notifications.show({
        title: "Series cover regeneration started",
        message: "Series cover thumbnail will be regenerated",
        color: "blue",
      });
      queryClient.invalidateQueries({ queryKey: ["series", seriesId] });
    },
    onError: (error: Error) => {
      notifications.show({
        title: "Failed",
        message: error.message,
        color: "red",
      });
    },
  });

  // Reprocess series title mutation (applies library preprocessing rules)
  const reprocessTitleMutation = useMutation({
    mutationFn: () => seriesApi.reprocessTitle(seriesId!),
    onSuccess: () => {
      notifications.show({
        title: "Reprocessing title",
        message: "Series title will be reprocessed using library rules",
        color: "blue",
      });
      queryClient.invalidateQueries({ queryKey: ["series", seriesId] });
    },
    onError: (error: Error) => {
      notifications.show({
        title: "Failed",
        message: error.message,
        color: "red",
      });
    },
  });

  // Reset metadata mutation
  const resetMetadataMutation = useMutation({
    mutationFn: () => seriesMetadataApi.resetMetadata(seriesId!),
    onSuccess: () => {
      notifications.show({
        title: "Metadata reset",
        message: "All metadata has been reset to filesystem defaults",
        color: "green",
      });
      setResetConfirmOpened(false);
      queryClient.invalidateQueries({ queryKey: ["series", seriesId] });
    },
    onError: (error: Error) => {
      notifications.show({
        title: "Failed to reset metadata",
        message: error.message,
        color: "red",
      });
    },
  });

  // Set document title to series name for browser history
  useDynamicDocumentTitle(series?.metadata?.title, "Series");

  const isLoading = seriesLoading;
  const error = seriesError;

  if (isLoading) {
    return (
      <Center h={400}>
        <Loader size="lg" />
      </Center>
    );
  }

  if (error || !series) {
    return (
      <Center h={400}>
        <Stack align="center" gap="md">
          <Text size="xl" fw={600}>
            Series Not Found
          </Text>
          <Text c="dimmed">The requested series could not be found.</Text>
          <Button onClick={() => navigate(-1)}>Go Back</Button>
        </Stack>
      </Center>
    );
  }

  // Use coverTimestamp from SSE events for cache-busting, fall back to series.updatedAt
  const coverCacheBuster = coverTimestamp ?? series.updatedAt;
  const coverUrl = `/api/v1/series/${series.id}/thumbnail?v=${encodeURIComponent(String(coverCacheBuster))}`;
  const hasUnread = (series.unreadCount ?? 0) > 0;
  const hasRead = (series.bookCount ?? 0) > (series.unreadCount ?? 0);
  // Access metadata fields from the nested metadata object
  const metadata = series.metadata;
  const readingDirection = formatReadingDirection(metadata?.readingDirection);
  const status = formatStatus(metadata?.status);
  // Access genres, tags, etc. from top-level of FullSeriesResponse
  const seriesTitle = metadata?.title ?? "Unknown Series";

  // Build breadcrumbs
  const breadcrumbItems: { title: string; href: string }[] = [
    { title: "Home", href: "/" },
  ];

  if (series.libraryId) {
    breadcrumbItems.push({
      title: series.libraryName,
      href: `/libraries/${series.libraryId}/series`,
    });
  }

  breadcrumbItems.push({
    title: seriesTitle,
    href: `/series/${series.id}`,
  });

  return (
    <Box py="md" px="md">
      <Stack gap="md">
        {/* Breadcrumbs */}
        <Breadcrumbs separator={<IconChevronRight size={14} />}>
          {breadcrumbItems.map((item, index) =>
            index === breadcrumbItems.length - 1 ? (
              <Text key={item.href} size="sm">
                {item.title}
              </Text>
            ) : (
              <Text
                key={item.href}
                component={Link}
                to={item.href}
                size="sm"
                c="dimmed"
                style={{ textDecoration: "none" }}
              >
                {item.title}
              </Text>
            ),
          )}
        </Breadcrumbs>

        {/* Header: Cover + Info side by side */}
        <Grid gutter="md">
          {/* Cover - smaller */}
          <Grid.Col span={{ base: 4, xs: 3, sm: 2 }}>
            <Image
              src={coverUrl}
              alt={seriesTitle}
              radius="sm"
              fallbackSrc="data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='150' height='212'%3E%3Crect fill='%23333' width='150' height='212'/%3E%3Ctext fill='%23666' font-family='sans-serif' font-size='12' x='50%25' y='50%25' text-anchor='middle' dy='.3em'%3ENo Cover%3C/text%3E%3C/svg%3E"
              style={{ aspectRatio: "150/212.125" }}
            />
          </Grid.Col>

          {/* Info */}
          <Grid.Col span={{ base: 8, xs: 9, sm: 10 }}>
            <Stack gap="xs">
              {/* Title row with badges and menu */}
              <Group justify="space-between" align="flex-start" wrap="nowrap">
                <Box style={{ flex: 1, minWidth: 0 }}>
                  <Group gap="sm" align="center" wrap="wrap">
                    <Title order={2} style={{ wordBreak: "break-word" }}>
                      {seriesTitle}
                    </Title>
                    {metadata?.publisher && (
                      <Text size="sm" c="dimmed">
                        in {series.libraryName}
                      </Text>
                    )}
                  </Group>
                  <Group gap="xs" mt={4}>
                    {status && (
                      <Badge
                        size="sm"
                        variant="filled"
                        color={status === "Ended" ? "green" : "blue"}
                      >
                        {status}
                      </Badge>
                    )}
                    {readingDirection && (
                      <Badge size="sm" variant="outline">
                        {readingDirection}
                      </Badge>
                    )}
                    {metadata?.ageRating != null && metadata.ageRating > 0 && (
                      <Badge size="sm" variant="outline" color="orange">
                        {metadata.ageRating}+
                      </Badge>
                    )}
                  </Group>
                </Box>

                <Menu shadow="md" width={240} position="bottom-end">
                  <Menu.Target>
                    <ActionIcon variant="subtle" size="lg">
                      <IconDotsVertical size={20} />
                    </ActionIcon>
                  </Menu.Target>
                  <Menu.Dropdown>
                    {hasUnread && (
                      <Menu.Item
                        leftSection={<IconCheck size={14} />}
                        onClick={() => markAsReadMutation.mutate()}
                        disabled={markAsReadMutation.isPending}
                      >
                        Mark as Read
                      </Menu.Item>
                    )}
                    {hasRead && (
                      <Menu.Item
                        leftSection={<IconBookOff size={14} />}
                        onClick={() => markAsUnreadMutation.mutate()}
                        disabled={markAsUnreadMutation.isPending}
                      >
                        Mark as Unread
                      </Menu.Item>
                    )}
                    {canEditSeries && (
                      <>
                        <Menu.Divider />
                        <Menu.Item
                          leftSection={<IconAnalyze size={14} />}
                          onClick={() => analyzeMutation.mutate()}
                          disabled={analyzeMutation.isPending}
                        >
                          Analyze All Books
                        </Menu.Item>
                        <Menu.Item
                          leftSection={<IconAnalyze size={14} />}
                          onClick={() => analyzeUnanalyzedMutation.mutate()}
                          disabled={analyzeUnanalyzedMutation.isPending}
                        >
                          Analyze Unanalyzed Books
                        </Menu.Item>
                        <Menu.Item
                          leftSection={<IconListNumbers size={14} />}
                          onClick={() => renumberMutation.mutate()}
                          disabled={renumberMutation.isPending}
                        >
                          Renumber Books
                        </Menu.Item>
                        <Menu.Divider />
                        <Menu.Label>Book Thumbnails</Menu.Label>
                        <Menu.Item
                          leftSection={<IconPhoto size={14} />}
                          onClick={() =>
                            generateMissingBookThumbnailsMutation.mutate()
                          }
                          disabled={
                            generateMissingBookThumbnailsMutation.isPending
                          }
                        >
                          Generate Missing
                        </Menu.Item>
                        <Menu.Item
                          leftSection={<IconPhoto size={14} />}
                          onClick={() =>
                            regenerateBookThumbnailsMutation.mutate()
                          }
                          disabled={regenerateBookThumbnailsMutation.isPending}
                        >
                          Regenerate All
                        </Menu.Item>
                        <Menu.Divider />
                        <Menu.Label>Series Thumbnail</Menu.Label>
                        <Menu.Item
                          leftSection={<IconPhoto size={14} />}
                          onClick={() =>
                            generateSeriesThumbnailIfMissingMutation.mutate()
                          }
                          disabled={
                            generateSeriesThumbnailIfMissingMutation.isPending
                          }
                        >
                          Generate If Missing
                        </Menu.Item>
                        <Menu.Item
                          leftSection={<IconPhoto size={14} />}
                          onClick={() =>
                            regenerateSeriesThumbnailMutation.mutate()
                          }
                          disabled={regenerateSeriesThumbnailMutation.isPending}
                        >
                          Regenerate
                        </Menu.Item>
                        <Menu.Divider />
                        <Menu.Item
                          leftSection={<IconEdit size={14} />}
                          onClick={openEditModal}
                        >
                          Edit Metadata
                        </Menu.Item>
                        <Menu.Item
                          leftSection={<IconRefresh size={14} />}
                          onClick={() => reprocessTitleMutation.mutate()}
                          disabled={reprocessTitleMutation.isPending}
                        >
                          Reprocess Title
                        </Menu.Item>
                        <Menu.Item
                          leftSection={<IconRestore size={14} />}
                          color="red"
                          onClick={() => setResetConfirmOpened(true)}
                          disabled={resetMetadataMutation.isPending}
                        >
                          Reset Metadata
                        </Menu.Item>
                        {/* Plugin actions for metadata fetching */}
                        {pluginActions && pluginActions.actions.length > 0 && (
                          <>
                            <Menu.Divider />
                            <Menu.Label>Fetch Metadata</Menu.Label>
                            {pluginActions.actions.map((action) => (
                              <Menu.Item
                                key={`search-${action.pluginId}`}
                                leftSection={<IconSearch size={14} />}
                                onClick={() => handlePluginAction(action)}
                              >
                                {action.label}
                              </Menu.Item>
                            ))}
                            <Menu.Divider />
                            <Menu.Label>Auto-Apply Metadata</Menu.Label>
                            {pluginActions.actions.map((action) => (
                              <Menu.Item
                                key={`auto-${action.pluginId}`}
                                leftSection={<IconWand size={14} />}
                                onClick={() => handleAutoMatch(action)}
                                disabled={autoMatchMutation.isPending}
                              >
                                {action.pluginDisplayName}
                              </Menu.Item>
                            ))}
                          </>
                        )}
                      </>
                    )}
                  </Menu.Dropdown>
                </Menu>
              </Group>

              {/* Book count */}
              {(() => {
                const counts = formatSeriesCounts({
                  localCount: series.bookCount ?? null,
                  totalVolumeCount: metadata?.totalVolumeCount ?? null,
                  totalChapterCount: metadata?.totalChapterCount ?? null,
                  localMaxVolume: series.localMaxVolume ?? null,
                  localMaxChapter: series.localMaxChapter ?? null,
                });
                return counts ? (
                  <Text size="sm" c="dimmed">
                    {counts}
                  </Text>
                ) : null;
              })()}

              {/* Alternate titles inline */}
              {series.alternateTitles && series.alternateTitles.length > 0 && (
                <AlternateTitles titles={series.alternateTitles} compact />
              )}

              {/* Action buttons */}
              <Group gap="sm" mt="xs">
                {nextBook && (
                  <Button
                    size="xs"
                    variant="filled"
                    leftSection={<IconBook size={14} />}
                    onClick={() => {
                      if (nextBook.fileFormat === "epub") {
                        navigate(`/reader/${nextBook.id}`);
                      } else {
                        const page = nextBook.readProgress?.currentPage ?? 1;
                        navigate(`/reader/${nextBook.id}?page=${page}`);
                      }
                    }}
                  >
                    {nextBook.readProgress ? "Continue" : "Read"}
                  </Button>
                )}
                <Button
                  size="xs"
                  variant={nextBook ? "light" : "filled"}
                  component="a"
                  href={`/api/v1/series/${series.id}/download`}
                  leftSection={<IconDownload size={14} />}
                >
                  Download
                </Button>
                <Tooltip label="Series Info">
                  <ActionIcon
                    variant="subtle"
                    size="md"
                    onClick={openInfoModal}
                  >
                    <IconInfoCircle size={18} />
                  </ActionIcon>
                </Tooltip>
              </Group>

              {/* Summary - show preview with expand if long */}
              {metadata?.summary && (
                <Box mt="xs">
                  <Text
                    size="sm"
                    style={{ whiteSpace: "pre-wrap" }}
                    lineClamp={summaryOpened ? undefined : 2}
                  >
                    {metadata.summary}
                  </Text>
                  {/* Only show READ MORE if summary is long enough (roughly > 150 chars or has newlines) */}
                  {(metadata.summary.length > 150 ||
                    metadata.summary.includes("\n")) && (
                    <Text
                      size="sm"
                      c="dimmed"
                      style={{
                        cursor: "pointer",
                        display: "inline-flex",
                        alignItems: "center",
                        gap: 4,
                      }}
                      onClick={toggleSummary}
                      mt={4}
                    >
                      {summaryOpened ? "READ LESS" : "READ MORE"}
                      {summaryOpened ? (
                        <IconChevronUp size={14} />
                      ) : (
                        <IconChevronDown size={14} />
                      )}
                    </Text>
                  )}
                </Box>
              )}
            </Stack>
          </Grid.Col>
        </Grid>

        {/* Metadata rows - Komga style */}
        <Stack gap="xs">
          {/* Publisher */}
          {metadata?.publisher && (
            <Group gap="md">
              <Text size="sm" c="dimmed" w={100}>
                PUBLISHER
              </Text>
              <Badge variant="outline" size="sm">
                {metadata.publisher}
              </Badge>
            </Group>
          )}

          {/* Authors */}
          {metadata?.authors && metadata.authors.length > 0 && (
            <Group gap="md" align="flex-start">
              <Text size="sm" c="dimmed" w={100}>
                {metadata.authors.length > 1 ? "AUTHORS" : "AUTHOR"}
              </Text>
              <AuthorsList authors={metadata.authors} showRoles />
            </Group>
          )}

          {/* Genres */}
          {series.genres && series.genres.length > 0 && (
            <Group gap="md" align="flex-start">
              <Text size="sm" c="dimmed" w={100}>
                GENRE
              </Text>
              <GenreTagChips
                genres={series.genres}
                libraryId={series.libraryId}
                maxDisplay={8}
              />
            </Group>
          )}

          {/* Tags */}
          {series.tags && series.tags.length > 0 && (
            <Group gap="md" align="flex-start">
              <Text size="sm" c="dimmed" w={100}>
                TAGS
              </Text>
              <GenreTagChips
                tags={series.tags}
                libraryId={series.libraryId}
                maxDisplay={8}
              />
            </Group>
          )}

          {/* Sharing Tags (admin only) */}
          {isAdmin && seriesSharingTags && seriesSharingTags.length > 0 && (
            <Group gap="md" align="flex-start">
              <Text size="sm" c="dimmed" w={100}>
                SHARING
              </Text>
              <Group gap="xs">
                {seriesSharingTags.map((tag) => (
                  <Tooltip
                    key={tag.id}
                    label={tag.description || "Sharing tag"}
                  >
                    <Badge variant="light" color="violet" size="sm">
                      {tag.name}
                    </Badge>
                  </Tooltip>
                ))}
              </Group>
            </Group>
          )}

          {/* External IDs */}
          {(series.externalIds?.length > 0 || canEditSeries) && (
            <Group gap="md" align="flex-start">
              <Text size="sm" c="dimmed" w={100}>
                EXTERNAL IDS
              </Text>
              <ExternalIds
                externalIds={series.externalIds ?? []}
                onEdit={canEditSeries ? openExternalIdModal : undefined}
              />
            </Group>
          )}

          {/* External Links */}
          {series.externalLinks && series.externalLinks.length > 0 && (
            <Group gap="md" align="flex-start">
              <Text size="sm" c="dimmed" w={100}>
                LINKS
              </Text>
              <ExternalLinks links={series.externalLinks} />
            </Group>
          )}

          {/* Ratings Section - Your rating, Community average, External ratings */}
          <Group gap="md" align="flex-start">
            <Text size="sm" c="dimmed" w={100}>
              RATINGS
            </Text>
            <Group gap="lg" wrap="wrap">
              {/* Your rating */}
              <SeriesRating seriesId={series.id} />
              {/* Community average */}
              <CommunityRating seriesId={series.id} />
              {/* External ratings (MAL, AniList, etc.) */}
              {series.externalRatings && series.externalRatings.length > 0 && (
                <ExternalRatings ratings={series.externalRatings} />
              )}
            </Group>
          </Group>

          {/* Custom Metadata */}
          {series && (
            <CustomMetadataDisplay
              context={transformFullSeriesToSeriesContext(series)}
              template={
                publicSettings?.["display.custom_metadata_template"]?.value
              }
            />
          )}
        </Stack>

        {/* Bulk Selection Toolbar - shows when items are selected */}
        <BulkSelectionToolbar />

        {/* Books list */}
        <SeriesBookList
          seriesId={series.id}
          seriesName={seriesTitle}
          bookCount={series.bookCount ?? 0}
          libraryId={series.libraryId}
        />
      </Stack>

      {/* Edit Metadata Modal */}
      <SeriesMetadataEditModal
        opened={editModalOpened}
        onClose={closeEditModal}
        seriesId={series.id}
        seriesTitle={seriesTitle}
      />

      {/* Plugin Metadata Apply Flow */}
      {selectedPlugin && (
        <MetadataApplyFlow
          opened={metadataFlowOpened}
          onClose={() => {
            closeMetadataFlow();
            setPreprocessedSearchTitle(null);
          }}
          plugin={selectedPlugin}
          entityId={series.id}
          entityTitle={preprocessedSearchTitle ?? seriesTitle}
          contentType="series"
          onApplySuccess={handleMetadataApplySuccess}
        />
      )}

      {/* Series Info Modal */}
      <SeriesInfoModal
        opened={infoModalOpened}
        onClose={closeInfoModal}
        series={series}
      />

      {/* External ID Edit Modal */}
      <ExternalIdEditModal
        entityType="series"
        entityId={series.id}
        opened={externalIdModalOpened}
        onClose={closeExternalIdModal}
      />

      {/* Reset Metadata Confirmation Modal */}
      <Modal
        opened={resetConfirmOpened}
        onClose={() => setResetConfirmOpened(false)}
        title="Reset Metadata"
        centered
      >
        <Stack gap="md">
          <Text>
            Are you sure you want to reset all metadata for this series? This
            will clear all genres, tags, alternate titles, external links,
            ratings, covers, and lock states. The title will revert to the
            directory name.
          </Text>
          <Text size="sm" c="dimmed">
            User ratings, read progress, and book data will be preserved.
          </Text>
          <Group justify="flex-end" gap="sm">
            <Button
              variant="subtle"
              onClick={() => setResetConfirmOpened(false)}
            >
              Cancel
            </Button>
            <Button
              color="red"
              onClick={() => resetMetadataMutation.mutate()}
              loading={resetMetadataMutation.isPending}
            >
              Reset
            </Button>
          </Group>
        </Stack>
      </Modal>
    </Box>
  );
}
