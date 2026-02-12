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
  Progress,
  Stack,
  Text,
  Title,
  Tooltip,
} from "@mantine/core";
import { useDisclosure, useMediaQuery } from "@mantine/hooks";
import { notifications } from "@mantine/notifications";
import {
  IconAnalyze,
  IconBarcode,
  IconBook,
  IconBookOff,
  IconCheck,
  IconChevronDown,
  IconChevronLeft,
  IconChevronRight,
  IconChevronUp,
  IconDotsVertical,
  IconDownload,
  IconEdit,
  IconEyeOff,
  IconInfoCircle,
  IconPhoto,
  IconSearch,
  IconTrash,
  IconWand,
} from "@tabler/icons-react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { Link, useNavigate, useParams } from "react-router-dom";
import { booksApi } from "@/api/books";
import {
  type PluginActionDto,
  pluginActionsApi,
  pluginsApi,
} from "@/api/plugins";
import {
  BookExternalIds,
  BookInfoModal,
  BookTypeBadge,
} from "@/components/book";
import { BookMetadataEditModal } from "@/components/books/BookMetadataEditModal";
import { ExternalIdEditModal } from "@/components/common";
import { MetadataApplyFlow } from "@/components/metadata";
import { ExternalLinks } from "@/components/series";
import { useDynamicDocumentTitle } from "@/hooks/useDocumentTitle";
import { usePermissions } from "@/hooks/usePermissions";
import type { ExtendedBookMetadata } from "@/types/book-metadata";
import { PERMISSIONS } from "@/types/permissions";

// Language code mapping
const LANGUAGE_DISPLAY: Record<string, string> = {
  en: "English",
  ja: "Japanese",
  ko: "Korean",
  zh: "Chinese",
  fr: "French",
  de: "German",
  es: "Spanish",
  it: "Italian",
  pt: "Portuguese",
  ru: "Russian",
};

function formatFileSize(bytes: number): string {
  if (bytes >= 1073741824) {
    return `${(bytes / 1073741824).toFixed(2)} GB`;
  }
  if (bytes >= 1048576) {
    return `${(bytes / 1048576).toFixed(2)} MB`;
  }
  if (bytes >= 1024) {
    return `${(bytes / 1024).toFixed(2)} KB`;
  }
  return `${bytes} B`;
}

export function BookDetail() {
  const { bookId } = useParams<{ bookId: string }>();
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const [summaryOpened, { toggle: toggleSummary }] = useDisclosure(false);
  const [editModalOpened, { open: openEditModal, close: closeEditModal }] =
    useDisclosure(false);
  const [infoModalOpened, { open: openInfoModal, close: closeInfoModal }] =
    useDisclosure(false);
  const isWideScreen = useMediaQuery("(min-width: 768px)");

  // Permission checks
  const { hasPermission } = usePermissions();
  const canEditBook = hasPermission(PERMISSIONS.BOOKS_WRITE);

  // Plugin metadata flow state
  const [selectedPlugin, setSelectedPlugin] = useState<PluginActionDto | null>(
    null,
  );
  const [
    metadataFlowOpened,
    { open: openMetadataFlow, close: closeMetadataFlow },
  ] = useDisclosure(false);
  const [
    externalIdModalOpened,
    { open: openExternalIdModal, close: closeExternalIdModal },
  ] = useDisclosure(false);

  // Fetch book details
  const {
    data: bookDetail,
    isLoading,
    error,
  } = useQuery({
    queryKey: ["book-detail", bookId],
    queryFn: () => booksApi.getDetail(bookId!),
    enabled: !!bookId,
  });

  // Fetch adjacent books for series navigation
  const { data: adjacentBooks } = useQuery({
    queryKey: ["adjacent-books", bookId],
    queryFn: () => booksApi.getAdjacent(bookId!),
    enabled: !!bookId,
  });

  // Fetch external IDs for this book
  const { data: externalIds } = useQuery({
    queryKey: ["books", bookId, "external-ids"],
    queryFn: () => booksApi.listExternalIds(bookId!),
    enabled: !!bookId,
  });

  // Fetch external links for this book
  const { data: externalLinks } = useQuery({
    queryKey: ["books", bookId, "external-links"],
    queryFn: () => booksApi.listExternalLinks(bookId!),
    enabled: !!bookId,
  });

  // Set document title to book name for browser history
  useDynamicDocumentTitle(bookDetail?.book?.title, "Book");

  const book = bookDetail?.book;
  const metadata = bookDetail?.metadata;
  // Extended metadata fields (will be populated when Phase 6 API updates are complete)
  // For now, cast metadata to access potential future fields
  const extendedMetadata = metadata as
    | (typeof metadata & ExtendedBookMetadata)
    | undefined;
  const prevBook = adjacentBooks?.prev;
  const nextBook = adjacentBooks?.next;

  // Fetch available plugin actions for book:detail scope, filtered by library
  const { data: pluginActions } = useQuery({
    queryKey: ["plugin-actions", "book:detail", book?.libraryId],
    queryFn: () => pluginsApi.getActions("book:detail", book?.libraryId),
    staleTime: 5 * 60 * 1000,
    enabled: canEditBook && !!book,
  });

  // Handler for plugin action click
  const handlePluginAction = (plugin: PluginActionDto) => {
    setSelectedPlugin(plugin);
    openMetadataFlow();
  };

  // Handler for metadata apply success
  const handleMetadataApplySuccess = () => {
    queryClient.invalidateQueries({ queryKey: ["book-detail", bookId] });
    queryClient.invalidateQueries({
      queryKey: ["books", bookId, "external-ids"],
    });
    queryClient.invalidateQueries({
      queryKey: ["books", bookId, "external-links"],
    });
  };

  // Auto-match mutation - enqueues a task for the book's parent series
  const autoMatchMutation = useMutation({
    mutationFn: (pluginId: string) => {
      if (!book?.seriesId) throw new Error("Series ID is required");
      return pluginActionsApi.enqueueAutoMatchTask(book.seriesId, pluginId);
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

  // Mark as read mutation
  const markAsReadMutation = useMutation({
    mutationFn: () => booksApi.markAsRead(bookId!),
    onSuccess: () => {
      notifications.show({
        title: "Marked as read",
        message: "Book marked as read",
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
    mutationFn: () => booksApi.markAsUnread(bookId!),
    onSuccess: () => {
      notifications.show({
        title: "Marked as unread",
        message: "Book marked as unread",
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
    mutationFn: () => booksApi.analyze(bookId!),
    onSuccess: () => {
      notifications.show({
        title: "Analysis started",
        message: "Book queued for analysis",
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

  // Generate thumbnail mutation
  const generateThumbnailMutation = useMutation({
    mutationFn: () => booksApi.generateThumbnail(bookId ?? ""),
    onSuccess: () => {
      notifications.show({
        title: "Thumbnail generation started",
        message: "Book queued for thumbnail generation",
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

  if (isLoading) {
    return (
      <Center h={400}>
        <Loader size="lg" />
      </Center>
    );
  }

  if (error || !book) {
    return (
      <Center h={400}>
        <Stack align="center" gap="md">
          <Text size="xl" fw={600}>
            Book Not Found
          </Text>
          <Text c="dimmed">The requested book could not be found.</Text>
          <Button onClick={() => navigate(-1)}>Go Back</Button>
        </Stack>
      </Center>
    );
  }

  const coverUrl = `/api/v1/books/${book.id}/thumbnail`;
  const downloadUrl = `/api/v1/books/${book.id}/file`;
  const hasProgress = !!book.readProgress;
  const isCompleted = book.readProgress?.completed ?? false;

  // Build display title
  const baseTitle =
    book.number !== undefined && book.number !== null
      ? `${book.number} - ${book.title}`
      : book.title;
  const displayTitle = book.deleted ? `(Deleted) ${baseTitle}` : baseTitle;

  // Build breadcrumbs
  const breadcrumbItems = [
    { title: "Home", href: "/" },
    { title: book.libraryName, href: `/libraries/${book.libraryId}/series` },
    { title: book.seriesName, href: `/series/${book.seriesId}` },
    { title: displayTitle, href: `/books/${book.id}` },
  ];

  // Calculate reading progress (current_page is 1-indexed)
  const currentPage = book.readProgress ? book.readProgress.currentPage : 0;
  const percentage =
    book.pageCount > 0 ? (currentPage / book.pageCount) * 100 : 0;

  // Extract metadata values
  const languageDisplay = metadata?.languageIso
    ? LANGUAGE_DISPLAY[metadata.languageIso] || metadata.languageIso
    : null;
  const releaseYear = metadata?.releaseDate
    ? new Date(metadata.releaseDate).getFullYear()
    : null;

  // Collect all creators
  const creators: { role: string; names: string[] }[] = [
    { role: "WRITERS", names: metadata?.writers || [] },
    { role: "PENCILLERS", names: metadata?.pencillers || [] },
    { role: "INKERS", names: metadata?.inkers || [] },
    { role: "COLORISTS", names: metadata?.colorists || [] },
    { role: "LETTERERS", names: metadata?.letterers || [] },
    { role: "COVER ARTISTS", names: metadata?.coverArtists || [] },
    { role: "EDITORS", names: metadata?.editors || [] },
  ].filter((c) => c.names.length > 0);

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
            <Box pos="relative">
              {book.deleted ? (
                <Box
                  style={{
                    aspectRatio: "150/212.125",
                    display: "flex",
                    flexDirection: "column",
                    alignItems: "center",
                    justifyContent: "center",
                    backgroundColor: "var(--mantine-color-dark-6)",
                    borderRadius: "var(--mantine-radius-sm)",
                    border: "2px dashed var(--mantine-color-red-6)",
                  }}
                >
                  <IconTrash
                    size={48}
                    style={{
                      color: "var(--mantine-color-red-6)",
                      opacity: 0.7,
                    }}
                  />
                  <Text size="sm" fw={500} c="red" mt="xs">
                    Deleted
                  </Text>
                </Box>
              ) : (
                <Image
                  src={coverUrl}
                  alt={book.title}
                  radius="sm"
                  fallbackSrc="data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='150' height='212'%3E%3Crect fill='%23333' width='150' height='212'/%3E%3Ctext fill='%23666' font-family='sans-serif' font-size='12' x='50%25' y='50%25' text-anchor='middle' dy='.3em'%3ENo Cover%3C/text%3E%3C/svg%3E"
                  style={{ aspectRatio: "150/212.125" }}
                />
              )}
            </Box>
          </Grid.Col>

          {/* Info */}
          <Grid.Col span={{ base: 8, xs: 9, sm: 10 }}>
            <Stack gap="xs">
              {/* Title row with badges and menu */}
              <Group justify="space-between" align="flex-start" wrap="nowrap">
                <Box style={{ flex: 1, minWidth: 0 }}>
                  <Group gap="sm" align="center" wrap="wrap">
                    <Title order={2} style={{ wordBreak: "break-word" }}>
                      {displayTitle}
                    </Title>
                  </Group>
                  <Group gap="xs" mt={4}>
                    {book.deleted && (
                      <Badge
                        size="sm"
                        variant="filled"
                        color="red"
                        leftSection={<IconTrash size={12} />}
                      >
                        Deleted
                      </Badge>
                    )}
                    <Badge size="sm" variant="filled">
                      {book.fileFormat.toUpperCase()}
                    </Badge>
                    {/* Book type badge - will show when API provides bookType */}
                    <BookTypeBadge
                      bookType={
                        (extendedMetadata as ExtendedBookMetadata | undefined)
                          ?.bookType
                      }
                      size="sm"
                      variant="light"
                    />
                    {isCompleted && (
                      <Badge size="sm" variant="filled" color="green">
                        Completed
                      </Badge>
                    )}
                    {hasProgress && !isCompleted && (
                      <Badge size="sm" variant="outline" color="blue">
                        In Progress
                      </Badge>
                    )}
                  </Group>
                </Box>

                <Menu shadow="md" width={200} position="bottom-end">
                  <Menu.Target>
                    <ActionIcon variant="subtle" size="lg">
                      <IconDotsVertical size={20} />
                    </ActionIcon>
                  </Menu.Target>
                  <Menu.Dropdown>
                    {!isCompleted && (
                      <Menu.Item
                        leftSection={<IconCheck size={14} />}
                        onClick={() => markAsReadMutation.mutate()}
                        disabled={markAsReadMutation.isPending}
                      >
                        Mark as Read
                      </Menu.Item>
                    )}
                    {hasProgress && (
                      <Menu.Item
                        leftSection={<IconBookOff size={14} />}
                        onClick={() => markAsUnreadMutation.mutate()}
                        disabled={markAsUnreadMutation.isPending}
                      >
                        Mark as Unread
                      </Menu.Item>
                    )}
                    {canEditBook && (
                      <>
                        <Menu.Divider />
                        <Menu.Item
                          leftSection={<IconAnalyze size={14} />}
                          onClick={() => analyzeMutation.mutate()}
                          disabled={analyzeMutation.isPending}
                        >
                          Analyze Book
                        </Menu.Item>
                        <Menu.Item
                          leftSection={<IconPhoto size={14} />}
                          onClick={() => generateThumbnailMutation.mutate()}
                          disabled={generateThumbnailMutation.isPending}
                        >
                          Regenerate Thumbnail
                        </Menu.Item>
                        <Menu.Divider />
                        <Menu.Item
                          leftSection={<IconEdit size={14} />}
                          onClick={openEditModal}
                        >
                          Edit Metadata
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

              {/* Subtitle (if available from extended metadata) */}
              {extendedMetadata?.subtitle && (
                <Text size="md" c="dimmed" fs="italic">
                  {extendedMetadata.subtitle}
                </Text>
              )}

              {/* Series link */}
              <Text
                component={Link}
                to={`/series/${book.seriesId}`}
                size="sm"
                c="dimmed"
                className="hover-underline"
                style={{ textDecoration: "none", width: "fit-content" }}
              >
                in {book.seriesName}
              </Text>

              {/* Reading progress */}
              {hasProgress && !isCompleted && (
                <Group gap="sm" align="center">
                  {book.fileFormat !== "epub" && (
                    <Text size="sm">
                      Page {currentPage} of {book.pageCount}
                    </Text>
                  )}
                  <Progress
                    value={percentage}
                    size="sm"
                    style={{ flex: 1, maxWidth: 200 }}
                  />
                  <Text size="sm" c="dimmed">
                    {Math.round(percentage)}%
                  </Text>
                </Group>
              )}

              {/* Action buttons */}
              <Group gap="sm" mt="xs">
                <Button
                  size="xs"
                  variant="filled"
                  leftSection={<IconBook size={14} />}
                  onClick={() => {
                    const page = book.readProgress?.currentPage ?? 1;
                    navigate(`/reader/${book.id}?page=${page}`);
                  }}
                >
                  {hasProgress && !isCompleted ? "Continue" : "Read"}
                </Button>
                <Tooltip label="Read without tracking progress">
                  <Button
                    size="xs"
                    variant="outline"
                    leftSection={<IconEyeOff size={14} />}
                    onClick={() =>
                      navigate(`/reader/${book.id}?page=1&incognito=true`)
                    }
                  >
                    Incognito
                  </Button>
                </Tooltip>
                <Button
                  size="xs"
                  variant="outline"
                  component="a"
                  href={downloadUrl}
                  leftSection={<IconDownload size={14} />}
                >
                  Download
                </Button>
                <Tooltip label="Book Info">
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

              {/* Analysis error */}
              {book.analysisError && (
                <Box
                  p="xs"
                  style={{
                    backgroundColor: "var(--mantine-color-red-light)",
                    borderRadius: "var(--mantine-radius-sm)",
                  }}
                >
                  <Text size="sm" c="red">
                    Analysis Error: {book.analysisError}
                  </Text>
                </Box>
              )}
            </Stack>
          </Grid.Col>
        </Grid>

        {/* Metadata rows - Komga style */}
        <Stack gap="xs">
          {/* File Info */}
          <Group gap="md" align="center">
            <Text size="sm" c="dimmed" w={100}>
              SIZE
            </Text>
            <Text size="sm">{formatFileSize(book.fileSize)}</Text>
          </Group>

          <Group gap="md" align="center">
            <Text size="sm" c="dimmed" w={100}>
              PAGES
            </Text>
            <Text size="sm">{book.pageCount}</Text>
          </Group>

          {/* Publisher */}
          {metadata?.publisher && (
            <Group gap="md" align="center">
              <Text size="sm" c="dimmed" w={100}>
                PUBLISHER
              </Text>
              <Badge variant="outline" size="sm">
                {metadata.publisher}
              </Badge>
            </Group>
          )}

          {/* Imprint */}
          {metadata?.imprint && (
            <Group gap="md" align="center">
              <Text size="sm" c="dimmed" w={100}>
                IMPRINT
              </Text>
              <Badge variant="outline" size="sm">
                {metadata.imprint}
              </Badge>
            </Group>
          )}

          {/* Release Year */}
          {releaseYear && (
            <Group gap="md" align="center">
              <Text size="sm" c="dimmed" w={100}>
                YEAR
              </Text>
              <Text size="sm">{releaseYear}</Text>
            </Group>
          )}

          {/* Language */}
          {languageDisplay && (
            <Group gap="md" align="center">
              <Text size="sm" c="dimmed" w={100}>
                LANGUAGE
              </Text>
              <Text size="sm">{languageDisplay}</Text>
            </Group>
          )}

          {/* Genre */}
          {metadata?.genre && (
            <Group gap="md" align="center">
              <Text size="sm" c="dimmed" w={100}>
                GENRE
              </Text>
              <Badge variant="light" size="sm">
                {metadata.genre}
              </Badge>
            </Group>
          )}

          {/* ISBN(s) */}
          {extendedMetadata?.isbns && (
            <Group gap="md" align="center">
              <Text size="sm" c="dimmed" w={100}>
                ISBN
              </Text>
              <Group gap="xs">
                {extendedMetadata.isbns.split(",").map((isbn: string) => (
                  <Badge
                    key={isbn.trim()}
                    variant="outline"
                    size="sm"
                    leftSection={<IconBarcode size={10} />}
                  >
                    {isbn.trim()}
                  </Badge>
                ))}
              </Group>
            </Group>
          )}

          {/* Edition (from extended metadata) */}
          {extendedMetadata?.edition && (
            <Group gap="md" align="center">
              <Text size="sm" c="dimmed" w={100}>
                EDITION
              </Text>
              <Text size="sm">{extendedMetadata.edition}</Text>
            </Group>
          )}

          {/* Original Title (from extended metadata) */}
          {extendedMetadata?.originalTitle && (
            <Group gap="md" align="center">
              <Text size="sm" c="dimmed" w={100}>
                ORIGINAL
              </Text>
              <Group gap="xs">
                <Text size="sm">{extendedMetadata.originalTitle}</Text>
                {extendedMetadata.originalYear && (
                  <Text size="sm" c="dimmed">
                    ({extendedMetadata.originalYear})
                  </Text>
                )}
              </Group>
            </Group>
          )}

          {/* Translator (from extended metadata) */}
          {extendedMetadata?.translator && (
            <Group gap="md" align="center">
              <Text size="sm" c="dimmed" w={100}>
                TRANSLATOR
              </Text>
              <Badge variant="light" size="sm" color="orange">
                {extendedMetadata.translator}
              </Badge>
            </Group>
          )}

          {/* Creators */}
          {creators.map(({ role, names }) => (
            <Group key={role} gap="md" align="flex-start">
              <Text size="sm" c="dimmed" w={100}>
                {role}
              </Text>
              <Group gap="xs">
                {names.map((name) => (
                  <Badge key={`${role}-${name}`} variant="light" size="sm">
                    {name}
                  </Badge>
                ))}
              </Group>
            </Group>
          ))}

          {/* External Links */}
          {externalLinks && externalLinks.length > 0 && (
            <Group gap="md" align="flex-start">
              <Text size="sm" c="dimmed" w={100}>
                LINKS
              </Text>
              <ExternalLinks links={externalLinks} />
            </Group>
          )}

          {/* External IDs */}
          {((externalIds && externalIds.length > 0) || canEditBook) && (
            <Group gap="md" align="flex-start">
              <Text size="sm" c="dimmed" w={100}>
                EXTERNAL IDS
              </Text>
              <BookExternalIds
                externalIds={externalIds ?? []}
                onEdit={canEditBook ? openExternalIdModal : undefined}
              />
            </Group>
          )}

          {/* File Path */}
          <Group gap="md" align="center">
            <Text size="sm" c="dimmed" w={100}>
              FILE
            </Text>
            <Tooltip label={book.filePath} position="top" multiline maw={400}>
              <Text size="sm" style={{ cursor: "help" }}>
                {book.filePath.split("/").pop() || book.filePath}
              </Text>
            </Tooltip>
          </Group>

          {/* Hash */}
          <Group gap="md" align="center">
            <Text size="sm" c="dimmed" w={100}>
              HASH
            </Text>
            {isWideScreen ? (
              <Text size="sm">{book.fileHash}</Text>
            ) : (
              <Tooltip label={book.fileHash} position="top">
                <Text size="sm" style={{ cursor: "help" }}>
                  {book.fileHash.substring(0, 16)}...
                </Text>
              </Tooltip>
            )}
          </Group>
        </Stack>

        {/* Series navigation */}
        <Group justify="space-between" mt="md">
          {prevBook ? (
            <Tooltip label={prevBook.title} position="top">
              <Button
                component={Link}
                to={`/books/${prevBook.id}`}
                variant="subtle"
                size="xs"
                leftSection={<IconChevronLeft size={14} />}
              >
                Previous
              </Button>
            </Tooltip>
          ) : (
            <Button
              component={Link}
              to={`/series/${book.seriesId}`}
              variant="subtle"
              size="xs"
              leftSection={<IconChevronLeft size={14} />}
            >
              Back to Series
            </Button>
          )}

          {nextBook && (
            <Tooltip label={nextBook.title} position="top">
              <Button
                component={Link}
                to={`/books/${nextBook.id}`}
                variant="subtle"
                size="xs"
                rightSection={<IconChevronRight size={14} />}
              >
                Next
              </Button>
            </Tooltip>
          )}
        </Group>
      </Stack>

      {/* Edit Metadata Modal */}
      <BookMetadataEditModal
        opened={editModalOpened}
        onClose={closeEditModal}
        bookId={book.id}
        bookTitle={book.title}
      />

      {/* Book Info Modal */}
      <BookInfoModal
        opened={infoModalOpened}
        onClose={closeInfoModal}
        book={book}
      />

      {/* Plugin Metadata Apply Flow */}
      {selectedPlugin && (
        <MetadataApplyFlow
          opened={metadataFlowOpened}
          onClose={closeMetadataFlow}
          plugin={selectedPlugin}
          entityId={book.id}
          entityTitle={book.title}
          entityAuthor={metadata?.authors?.[0]?.name ?? metadata?.writers?.[0]}
          contentType="book"
          onApplySuccess={handleMetadataApplySuccess}
        />
      )}

      {/* External ID Edit Modal */}
      <ExternalIdEditModal
        entityType="book"
        entityId={book.id}
        opened={externalIdModalOpened}
        onClose={closeExternalIdModal}
      />
    </Box>
  );
}
