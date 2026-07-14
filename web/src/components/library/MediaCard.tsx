import {
  ActionIcon,
  Card,
  Checkbox,
  Group,
  HoverCard,
  Image,
  Menu,
  Progress,
  Skeleton,
  Stack,
  Text,
  Tooltip,
} from "@mantine/core";
import { notifications } from "@mantine/notifications";
import {
  IconAnalyze,
  IconBell,
  IconBellOff,
  IconBellRinging,
  IconBook,
  IconBookmark,
  IconBookmarkFilled,
  IconBookOff,
  IconCheck,
  IconDotsVertical,
  IconTrash,
} from "@tabler/icons-react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { memo, useCallback, useEffect, useState } from "react";
import { useNavigate } from "react-router-dom";
import { booksApi } from "@/api/books";
import { seriesApi } from "@/api/series";
import { trackingApi } from "@/api/tracking";
import { CollectionFormModal } from "@/components/collections/CollectionFormModal";
import { CollectionMembershipSub } from "@/components/collections/CollectionMembershipSub";
import { AppLink } from "@/components/common";
import { MediaCardHoverPanel } from "@/components/library/MediaCardHoverPanel";
import { ReadListFormModal } from "@/components/readlists/ReadListFormModal";
import { ReadListMembershipSub } from "@/components/readlists/ReadListMembershipSub";
import { useAddSeriesToCollection } from "@/hooks/useCollections";
import { usePermissions } from "@/hooks/usePermissions";
import { useAddBooksToReadList } from "@/hooks/useReadLists";
import {
  useAddToWantToRead,
  useRemoveFromWantToRead,
} from "@/hooks/useWantToRead";
import { useCoverUpdatesStore } from "@/store/coverUpdatesStore";
import type { Book, Series } from "@/types";
import { PERMISSIONS } from "@/types/permissions";

interface MediaCardProps {
  type: "book" | "series";
  data: Book | Series;
  hideSeriesName?: boolean;
  /** Callback when item is selected/deselected. Receives id, shiftKey, and optional index. */
  onSelect?: (id: string, shiftKey: boolean, index?: number) => void;
  /** Whether this item is currently selected */
  isSelected?: boolean;
  /** Whether bulk selection mode is active (at least one item selected) */
  isSelectionMode?: boolean;
  /** Whether this item can be selected (type matches current selection) */
  canBeSelected?: boolean;
  /** Index of this item in the grid (for range selection) */
  index?: number;
}

export const MediaCard = memo(function MediaCard({
  type,
  data,
  hideSeriesName = false,
  onSelect,
  isSelected = false,
  isSelectionMode = false,
  canBeSelected = true,
  index,
}: MediaCardProps) {
  const queryClient = useQueryClient();
  const navigate = useNavigate();
  const { hasPermission } = usePermissions();
  const canWriteBooks = hasPermission(PERMISSIONS.BOOKS_WRITE);
  const canWriteSeries = hasPermission(PERMISSIONS.SERIES_WRITE);
  const canManageCollections = hasPermission(PERMISSIONS.COLLECTIONS_WRITE);
  const canManageReadLists = hasPermission(PERMISSIONS.READLISTS_WRITE);

  // Inline-create modals for the membership submenus. These must be rendered
  // outside the card's Menu: clicking the "New…" item closes the menu, which
  // would unmount a modal living inside the dropdown before it could open.
  const [createCollectionOpen, setCreateCollectionOpen] = useState(false);
  const [createReadListOpen, setCreateReadListOpen] = useState(false);
  // Separate mutation instances drive "create then auto-add" from the modals;
  // the submenus own their own add/remove for toggling existing memberships.
  const addSeriesToCollection = useAddSeriesToCollection();
  const addBooksToReadList = useAddBooksToReadList();
  // Want-to-read queue is per-user (no permission gate) and is a single boolean
  // on the DTO, so the menu shows a plain add/remove toggle rather than a submenu.
  const addToWantToRead = useAddToWantToRead();
  const removeFromWantToRead = useRemoveFromWantToRead();
  const wantToReadPending =
    addToWantToRead.isPending || removeFromWantToRead.isPending;
  const toggleWantToRead = useCallback(
    (itemType: "book" | "series", id: string, active: boolean) => {
      if (active) {
        removeFromWantToRead.mutate({ itemType, id });
      } else {
        addToWantToRead.mutate({ itemType, id });
      }
    },
    [addToWantToRead, removeFromWantToRead],
  );

  // Get cover update timestamp for cache-busting (forces image reload when cover is regenerated)
  const coverTimestamp = useCoverUpdatesStore(
    (state) => state.updates[data.id],
  );

  // Handle card click navigation or selection
  const handleCardClick = (e: React.MouseEvent) => {
    // Don't navigate if clicking the menu button, dropdown, or checkbox
    if ((e.target as HTMLElement).closest("[data-menu]")) return;
    if ((e.target as HTMLElement).closest("[data-selection-checkbox]")) return;

    // In selection mode, clicking the card toggles selection (if allowed)
    // or does nothing (if type mismatch)
    if (isSelectionMode && onSelect) {
      if (canBeSelected) {
        onSelect(data.id, e.shiftKey, index);
      }
      // In selection mode, don't navigate regardless of canBeSelected
      return;
    }

    // Normal navigation (only when not in selection mode)
    if (type === "series") {
      navigate(`/series/${(data as Series).id}`);
    } else {
      navigate(`/books/${(data as Book).id}`);
    }
  };

  // Use API endpoint directly - browser will send auth cookie automatically
  // Add cache-busting parameter to force the browser to reload the image when
  // the cover changes. We use SSE event timestamp if available (real-time update),
  // otherwise fall back to updatedAt from the entity data.
  const baseCoverUrl =
    type === "book"
      ? `/api/v1/books/${(data as Book).id}/thumbnail`
      : `/api/v1/series/${(data as Series).id}/thumbnail`;
  const coverCacheBuster = coverTimestamp ?? data.updatedAt;
  const coverUrl = `${baseCoverUrl}?v=${encodeURIComponent(String(coverCacheBuster))}`;

  const book = type === "book" ? (data as Book) : null;
  const series = type === "series" ? (data as Series) : null;

  // Whether the series dropdown's reading group renders any item. Used to
  // decide if the following groups need a leading divider (an empty series
  // shows neither "Mark as Read" nor "Mark as Unread").
  const seriesHasReadingActions =
    !!series &&
    ((series.unreadCount ?? 0) > 0 ||
      (series.bookCount ?? 0) > (series.unreadCount ?? 0));

  // Track image loading state for skeleton placeholder
  const [imageLoaded, setImageLoaded] = useState(false);
  const [_imageError, setImageError] = useState(false);

  // Reset loading state when the item ID or cover cache buster changes
  // (e.g., different book/series, or cover was regenerated via SSE event or data refetch)
  // biome-ignore lint/correctness/useExhaustiveDependencies: intentionally reset on ID/cache buster change
  useEffect(() => {
    setImageLoaded(false);
    setImageError(false);
  }, [data.id, coverCacheBuster]);

  const handleImageLoad = useCallback(() => {
    setImageLoaded(true);
  }, []);

  const handleImageError = useCallback(() => {
    setImageError(true);
    setImageLoaded(true); // Stop showing skeleton on error
  }, []);

  // Track if item is newly created (for animation)
  const [isNew, setIsNew] = useState(false);

  useEffect(() => {
    // Check if item was created recently (within last 5 seconds)
    const createdAt = new Date(data.createdAt);
    const now = new Date();
    const diffMs = now.getTime() - createdAt.getTime();

    if (diffMs < 5000) {
      setIsNew(true);
      // Remove animation after 3 seconds
      const timer = setTimeout(() => setIsNew(false), 3000);
      return () => clearTimeout(timer);
    }
  }, [data.createdAt]);

  // Handle read button click - navigate directly to reader
  const handleReadClick = (e: React.MouseEvent) => {
    e.stopPropagation();
    if (type === "book" && book) {
      // Start from current page if there's progress, otherwise page 1
      const page = book.readProgress?.currentPage || 1;
      navigate(`/reader/${book.id}?page=${page}`);
    }
  };

  // Calculate progress percentage for books
  // Prefer progressPercentage (from R2Progression) for EPUBs where page_count
  // is spine items, not actual pages.
  const progressPercentage =
    book?.readProgress?.progressPercentage != null
      ? book.readProgress.progressPercentage * 100
      : book?.readProgress && book.pageCount
        ? (book.readProgress.currentPage / book.pageCount) * 100
        : 0;

  // Book analysis mutation
  const bookAnalyzeMutation = useMutation({
    mutationFn: () => {
      if (!book) throw new Error("Book not available");
      return booksApi.analyze(book.id);
    },
    onSuccess: () => {
      notifications.show({
        title: "Analysis started",
        message: "Book analysis has been queued",
        color: "blue",
      });
      // Analysis is async: only a task is queued here, nothing has changed yet.
      // The real refresh arrives via the book_updated SSE event on completion,
      // so nudge just this book's detail (cheap) rather than the whole
      // ["books"] namespace, which would refetch every open list/detail tab.
      if (book) {
        queryClient.invalidateQueries({
          queryKey: ["books", book.id, "detail"],
        });
      }
    },
    onError: (error: Error) => {
      notifications.show({
        title: "Analysis failed",
        message: error.message || "Failed to start book analysis",
        color: "red",
      });
    },
  });

  // Series analysis mutations
  const seriesAnalyzeMutation = useMutation({
    mutationFn: () => {
      if (!series) throw new Error("Series not available");
      return seriesApi.analyze(series.id);
    },
    onSuccess: () => {
      notifications.show({
        title: "Analysis started",
        message: "All books in series queued for analysis",
        color: "blue",
      });
      // Async enqueue — refresh just this series' detail; the series_metadata
      // /book_updated SSE events refresh the rest on completion (was ["series"],
      // which refetched every open detail tab + list for nothing).
      if (series) {
        queryClient.invalidateQueries({
          queryKey: ["series", series.id, "full"],
        });
      }
    },
    onError: (error: Error) => {
      notifications.show({
        title: "Analysis failed",
        message: error.message || "Failed to start series analysis",
        color: "red",
      });
    },
  });

  const seriesAnalyzeUnanalyzedMutation = useMutation({
    mutationFn: () => {
      if (!series) throw new Error("Series not available");
      return seriesApi.analyzeUnanalyzed(series.id);
    },
    onSuccess: () => {
      notifications.show({
        title: "Analysis started",
        message: "Unanalyzed books queued for analysis",
        color: "blue",
      });
      // Async enqueue — refresh just this series' detail; SSE refreshes the
      // rest on completion (was the whole ["series"] namespace).
      if (series) {
        queryClient.invalidateQueries({
          queryKey: ["series", series.id, "full"],
        });
      }
    },
    onError: (error: Error) => {
      notifications.show({
        title: "Analysis failed",
        message: error.message || "Failed to start analysis",
        color: "red",
      });
    },
  });

  // Book mark as read/unread mutations
  const bookMarkAsReadMutation = useMutation({
    mutationFn: () => {
      if (!book) throw new Error("Book not available");
      return booksApi.markAsRead(book.id);
    },
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
          return key === "books" || key === "series" || key === "series-books";
        },
      });
    },
    onError: (error: Error) => {
      notifications.show({
        title: "Failed to mark as read",
        message: error.message || "Failed to mark book as read",
        color: "red",
      });
    },
  });

  const bookMarkAsUnreadMutation = useMutation({
    mutationFn: () => {
      if (!book) throw new Error("Book not available");
      return booksApi.markAsUnread(book.id);
    },
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
          return key === "books" || key === "series" || key === "series-books";
        },
      });
    },
    onError: (error: Error) => {
      notifications.show({
        title: "Failed to mark as unread",
        message: error.message || "Failed to mark book as unread",
        color: "red",
      });
    },
  });

  // Series mark as read/unread mutations
  const seriesMarkAsReadMutation = useMutation({
    mutationFn: () => {
      if (!series) throw new Error("Series not available");
      return seriesApi.markAsRead(series.id);
    },
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
          return key === "books" || key === "series" || key === "series-books";
        },
      });
    },
    onError: (error: Error) => {
      notifications.show({
        title: "Failed to mark as read",
        message: error.message || "Failed to mark series as read",
        color: "red",
      });
    },
  });

  const seriesMarkAsUnreadMutation = useMutation({
    mutationFn: () => {
      if (!series) throw new Error("Series not available");
      return seriesApi.markAsUnread(series.id);
    },
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
          return key === "books" || key === "series" || key === "series-books";
        },
      });
    },
    onError: (error: Error) => {
      notifications.show({
        title: "Failed to mark as unread",
        message: error.message || "Failed to mark series as unread",
        color: "red",
      });
    },
  });

  // Tracking toggle for the series card. The SeriesDto carries `tracked`, so
  // the card knows its own state and can render the indicator + dropdown
  // label without a per-card fetch. On success we refresh series queries so
  // the tracked indicator updates immediately, and prime the per-series
  // tracking query cache so the detail page reads consistent state.
  const seriesTrackToggleMutation = useMutation({
    mutationFn: (next: boolean) => {
      if (!series) throw new Error("Series not available");
      return trackingApi.updateTracking(series.id, { tracked: next });
    },
    onSuccess: (data) => {
      if (series) {
        queryClient.setQueryData(["series", series.id, "tracking"], data);
      }
      queryClient.refetchQueries({
        predicate: (query) => {
          const key = query.queryKey[0] as string;
          return key === "series" || key === "series-detail";
        },
      });
      notifications.show({
        title: data.tracked ? "Tracking enabled" : "Tracking disabled",
        message: data.tracked
          ? "This series will now be tracked for releases."
          : "Release tracking has been turned off.",
        color: data.tracked ? "green" : "gray",
      });
    },
    onError: (error: Error) => {
      notifications.show({
        title: "Failed to update tracking",
        message: error.message || "Could not toggle release tracking",
        color: "red",
      });
    },
  });

  const title = book
    ? `${book.number !== undefined && book.number !== null ? `${book.number} - ` : ""}${book.title}`
    : series?.title || "";
  const altText = book ? book.title : series?.title || "";

  // Build class names for selection state
  const cardClassNames = [
    isSelectionMode && "media-card--selection-mode",
    isSelected && "media-card--selected",
    isSelectionMode && !canBeSelected && "media-card--disabled",
  ]
    .filter(Boolean)
    .join(" ");

  return (
    <>
      <Card
        shadow="sm"
        padding={0}
        radius="md"
        withBorder
        onClick={handleCardClick}
        className={cardClassNames || undefined}
        // Opts into the Phase 3 press/hover affordance (press scale 0.98 +
        // hover lift to --shadow-md). Disabled when the card is in selection
        // mode but can't be selected, so the press doesn't suggest action.
        data-pressable={isSelectionMode && !canBeSelected ? undefined : "true"}
        style={{
          height: "100%",
          display: "flex",
          flexDirection: "column",
          minHeight: 0,
          width: "100%", // Ensure full width of grid cell
          boxSizing: "border-box", // Include border in width calculation
          animation: isNew ? "fadeIn 0.5s ease-in" : undefined,
          border: isNew
            ? "1px solid var(--mantine-color-blue-6)"
            : isSelected
              ? "1px solid var(--mantine-color-orange-6)"
              : undefined,
          cursor: isSelectionMode && !canBeSelected ? "not-allowed" : "pointer",
        }}
      >
        {/* HoverCard surfaces the title, description and volume/chapter counts
            on hover so users don't have to open the detail page. The target is
            the whole card body (cover + footer) so hovering anywhere reveals
            it; the card stays clickable and the dropdown renders lazily. */}
        <HoverCard
          openDelay={400}
          closeDelay={100}
          position="right-start"
          withinPortal
          width={300}
          shadow="md"
        >
          <HoverCard.Target>
            <Stack gap={0} style={{ height: "100%", minHeight: 0 }}>
              {/* Cover Image - Fixed height section (Komga ratio: 150px width, 212.125px height = 1.414) */}
              <div
                className="media-card-cover"
                style={{
                  position: "relative",
                  width: "100%",
                  aspectRatio: "150/212.125",
                  flexShrink: 0,
                  overflow: "hidden",
                }}
              >
                {book?.deleted ? (
                  <div
                    style={{
                      width: "100%",
                      height: "100%",
                      backgroundColor: "var(--mantine-color-dark-6)",
                      display: "flex",
                      flexDirection: "column",
                      alignItems: "center",
                      justifyContent: "center",
                      gap: "8px",
                    }}
                  >
                    <IconTrash
                      size={48}
                      style={{
                        color: "var(--mantine-color-red-6)",
                        opacity: 0.7,
                      }}
                    />
                    <Text size="sm" fw={500} c="dimmed">
                      Deleted
                    </Text>
                  </div>
                ) : (
                  <>
                    {/* Skeleton placeholder shown while image is loading */}
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
                      src={coverUrl}
                      alt={altText}
                      fit="cover"
                      // The opacity → 1 toggle drives the fade-in once onLoad
                      // fires; the matching `transition` lives in index.css
                      // (.media-card-cover .mantine-Image-root) so the mobile
                      // 150ms / desktop 200ms split via @media can win without
                      // fighting an inline style.
                      style={{
                        width: "100%",
                        height: "100%",
                        objectFit: "cover",
                        opacity: imageLoaded ? 1 : 0,
                      }}
                      onLoad={handleImageLoad}
                      onError={handleImageError}
                      fallbackSrc="data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='200' height='300'%3E%3Crect fill='%23ddd' width='200' height='300'/%3E%3Ctext fill='%23999' font-family='sans-serif' font-size='14' x='50%25' y='50%25' text-anchor='middle' dy='.3em'%3ENo Cover%3C/text%3E%3C/svg%3E"
                    />
                  </>
                )}
                {/* Selection checkbox - top left */}
                {onSelect && (
                  <div
                    data-selection-checkbox
                    className={`media-card-checkbox ${isSelected ? "media-card-checkbox--selected" : ""} ${isSelectionMode ? "media-card-checkbox--visible" : ""}`}
                  >
                    <Checkbox
                      checked={isSelected}
                      onChange={(e) => {
                        // Prevent event from bubbling to card click handler
                        e.stopPropagation();
                        if (canBeSelected && onSelect) {
                          // Get the native event to check for shift key
                          const nativeEvent =
                            e.nativeEvent as unknown as MouseEvent;
                          onSelect(
                            data.id,
                            nativeEvent?.shiftKey ?? false,
                            index,
                          );
                        }
                      }}
                      disabled={!canBeSelected}
                      color="orange"
                      size="md"
                      aria-label={`Select ${type === "book" ? book?.title : series?.title}`}
                      styles={{
                        input: {
                          cursor: canBeSelected ? "pointer" : "not-allowed",
                        },
                      }}
                    />
                  </div>
                )}
                {/* Tracking indicator - bell glyph top-left, centered on the same
              slot the selection checkbox occupies so the corner stays stable
              when toggling in/out of selection mode. Hidden in selection
              mode so the checkbox takes over. Drop shadow keeps it legible
              on light covers. */}
                {type === "series" && series?.tracked && !isSelectionMode && (
                  <Tooltip
                    label="Release tracking enabled"
                    openDelay={300}
                    withinPortal
                  >
                    <div
                      role="img"
                      aria-label="Release tracking enabled"
                      style={{
                        position: "absolute",
                        top: 12,
                        left: 12,
                        width: 20,
                        height: 20,
                        color: "#ff6b35",
                        display: "flex",
                        alignItems: "center",
                        justifyContent: "center",
                        filter: "drop-shadow(0 1px 2px rgba(0, 0, 0, 0.6))",
                        zIndex: 2,
                        pointerEvents: "auto",
                      }}
                    >
                      <IconBellRinging size={20} stroke={2.25} />
                    </div>
                  </Tooltip>
                )}
                {/* Unread indicator - Triangle for books, Square for series */}
                {type === "book" && book && !book.readProgress && (
                  <div
                    style={{
                      position: "absolute",
                      top: 0,
                      right: 0,
                      width: 0,
                      height: 0,
                      borderTop: "24px solid #ff6b35",
                      borderLeft: "24px solid transparent",
                      zIndex: 2,
                    }}
                  />
                )}
                {type === "series" &&
                  series &&
                  (series.unreadCount ?? 0) > 0 && (
                    <div
                      style={{
                        position: "absolute",
                        top: 0,
                        right: 0,
                        width: "28px",
                        height: "28px",
                        backgroundColor: "#ff6b35",
                        display: "flex",
                        alignItems: "center",
                        justifyContent: "center",
                        zIndex: 2,
                        borderBottomLeftRadius: "4px",
                        // Match the cover's Phase 5 top-right 10px curve so the
                        // badge follows the rounded corner instead of being
                        // clipped flat by overflow:hidden on the cover container.
                        borderTopRightRadius: "10px",
                      }}
                    >
                      <Text
                        size="xs"
                        fw={700}
                        c="white"
                        style={{
                          fontSize: "12px",
                          lineHeight: 1,
                        }}
                      >
                        {(series.unreadCount ?? 0) > 99
                          ? "99+"
                          : series.unreadCount}
                      </Text>
                    </div>
                  )}
                {/* Menu overlay */}
                <div
                  data-menu
                  style={{
                    position: "absolute",
                    bottom: 8,
                    right: 8,
                    zIndex: 3,
                  }}
                >
                  <Menu position="top-end" shadow="md" withinPortal>
                    <Menu.Target>
                      <ActionIcon
                        variant="filled"
                        color="dark"
                        size="sm"
                        aria-label="Card actions"
                        style={{ opacity: 0.8 }}
                        onClick={(e: React.MouseEvent) => e.stopPropagation()}
                      >
                        <IconDotsVertical size={16} />
                      </ActionIcon>
                    </Menu.Target>
                    <Menu.Dropdown>
                      {type === "book" ? (
                        <>
                          {/* Show Mark as Read if book is unread (no progress or not completed) */}
                          {(!book?.readProgress ||
                            !book.readProgress.completed) && (
                            <Menu.Item
                              leftSection={<IconCheck size={14} />}
                              onClick={(e: React.MouseEvent) => {
                                e.stopPropagation();
                                bookMarkAsReadMutation.mutate();
                              }}
                              disabled={bookMarkAsReadMutation.isPending}
                            >
                              {bookMarkAsReadMutation.isPending
                                ? "Marking..."
                                : "Mark as Read"}
                            </Menu.Item>
                          )}
                          {/* Show Mark as Unread if book has progress */}
                          {book?.readProgress && (
                            <Menu.Item
                              leftSection={<IconBookOff size={14} />}
                              onClick={(e: React.MouseEvent) => {
                                e.stopPropagation();
                                bookMarkAsUnreadMutation.mutate();
                              }}
                              disabled={bookMarkAsUnreadMutation.isPending}
                            >
                              {bookMarkAsUnreadMutation.isPending
                                ? "Marking..."
                                : "Mark as Unread"}
                            </Menu.Item>
                          )}
                          {canWriteBooks && (
                            <>
                              <Menu.Divider />
                              <Menu.Item
                                leftSection={<IconAnalyze size={14} />}
                                onClick={(e: React.MouseEvent) => {
                                  e.stopPropagation();
                                  bookAnalyzeMutation.mutate();
                                }}
                                disabled={bookAnalyzeMutation.isPending}
                              >
                                {bookAnalyzeMutation.isPending
                                  ? "Analyzing..."
                                  : "Force Analyze"}
                              </Menu.Item>
                            </>
                          )}
                          {book && (
                            <>
                              <Menu.Divider />
                              <Menu.Item
                                leftSection={
                                  book.wantToRead ? (
                                    <IconBookmarkFilled size={14} />
                                  ) : (
                                    <IconBookmark size={14} />
                                  )
                                }
                                onClick={(e: React.MouseEvent) => {
                                  e.stopPropagation();
                                  toggleWantToRead(
                                    "book",
                                    book.id,
                                    Boolean(book.wantToRead),
                                  );
                                }}
                                disabled={wantToReadPending}
                              >
                                {book.wantToRead
                                  ? "Remove from Want to Read"
                                  : "Add to Want to Read"}
                              </Menu.Item>
                            </>
                          )}
                          {canManageReadLists && book && (
                            <>
                              <Menu.Divider />
                              <ReadListMembershipSub
                                bookId={book.id}
                                onRequestCreate={() =>
                                  setCreateReadListOpen(true)
                                }
                              />
                            </>
                          )}
                        </>
                      ) : (
                        <>
                          {/* Show Mark as Read if series has any unread books */}
                          {series && (series.unreadCount ?? 0) > 0 && (
                            <Menu.Item
                              leftSection={<IconCheck size={14} />}
                              onClick={(e: React.MouseEvent) => {
                                e.stopPropagation();
                                seriesMarkAsReadMutation.mutate();
                              }}
                              disabled={seriesMarkAsReadMutation.isPending}
                            >
                              {seriesMarkAsReadMutation.isPending
                                ? "Marking..."
                                : "Mark as Read"}
                            </Menu.Item>
                          )}
                          {/* Show Mark as Unread if series has any read books */}
                          {series &&
                            (series.bookCount ?? 0) >
                              (series.unreadCount ?? 0) && (
                              <Menu.Item
                                leftSection={<IconBookOff size={14} />}
                                onClick={(e: React.MouseEvent) => {
                                  e.stopPropagation();
                                  seriesMarkAsUnreadMutation.mutate();
                                }}
                                disabled={seriesMarkAsUnreadMutation.isPending}
                              >
                                {seriesMarkAsUnreadMutation.isPending
                                  ? "Marking..."
                                  : "Mark as Unread"}
                              </Menu.Item>
                            )}
                          {canWriteSeries && series && (
                            <>
                              {seriesHasReadingActions && <Menu.Divider />}
                              <Menu.Item
                                leftSection={
                                  series.tracked ? (
                                    <IconBellOff size={14} />
                                  ) : (
                                    <IconBell size={14} />
                                  )
                                }
                                onClick={(e: React.MouseEvent) => {
                                  e.stopPropagation();
                                  seriesTrackToggleMutation.mutate(
                                    !series.tracked,
                                  );
                                }}
                                disabled={seriesTrackToggleMutation.isPending}
                              >
                                {seriesTrackToggleMutation.isPending
                                  ? "Updating..."
                                  : series.tracked
                                    ? "Stop Tracking"
                                    : "Start Tracking"}
                              </Menu.Item>
                              <Menu.Item
                                leftSection={<IconAnalyze size={14} />}
                                onClick={(e: React.MouseEvent) => {
                                  e.stopPropagation();
                                  seriesAnalyzeMutation.mutate();
                                }}
                                disabled={seriesAnalyzeMutation.isPending}
                              >
                                {seriesAnalyzeMutation.isPending
                                  ? "Analyzing..."
                                  : "Force Analyze All"}
                              </Menu.Item>
                              <Menu.Item
                                leftSection={<IconAnalyze size={14} />}
                                onClick={(e: React.MouseEvent) => {
                                  e.stopPropagation();
                                  seriesAnalyzeUnanalyzedMutation.mutate();
                                }}
                                disabled={
                                  seriesAnalyzeUnanalyzedMutation.isPending
                                }
                              >
                                {seriesAnalyzeUnanalyzedMutation.isPending
                                  ? "Analyzing..."
                                  : "Analyze Unanalyzed"}
                              </Menu.Item>
                            </>
                          )}
                          {series && (
                            <>
                              {(seriesHasReadingActions || canWriteSeries) && (
                                <Menu.Divider />
                              )}
                              <Menu.Item
                                leftSection={
                                  series.wantToRead ? (
                                    <IconBookmarkFilled size={14} />
                                  ) : (
                                    <IconBookmark size={14} />
                                  )
                                }
                                onClick={(e: React.MouseEvent) => {
                                  e.stopPropagation();
                                  toggleWantToRead(
                                    "series",
                                    series.id,
                                    Boolean(series.wantToRead),
                                  );
                                }}
                                disabled={wantToReadPending}
                              >
                                {series.wantToRead
                                  ? "Remove from Want to Read"
                                  : "Add to Want to Read"}
                              </Menu.Item>
                            </>
                          )}
                          {canManageCollections && series && (
                            <>
                              <Menu.Divider />
                              <CollectionMembershipSub
                                seriesId={series.id}
                                onRequestCreate={() =>
                                  setCreateCollectionOpen(true)
                                }
                              />
                            </>
                          )}
                        </>
                      )}
                    </Menu.Dropdown>
                  </Menu>
                </div>
                {/* Read button overlay - shows on hover for books only */}
                {type === "book" && !book?.deleted && (
                  <div
                    className="media-card-read-overlay"
                    style={{
                      position: "absolute",
                      top: 0,
                      left: 0,
                      right: 0,
                      bottom: 0,
                      display: "flex",
                      alignItems: "center",
                      justifyContent: "center",
                      backgroundColor: "rgba(0, 0, 0, 0.5)",
                      transition: "opacity 0.2s ease",
                      zIndex: 2,
                    }}
                  >
                    <ActionIcon
                      variant="filled"
                      color="red"
                      size={56}
                      radius="xl"
                      onClick={handleReadClick}
                      aria-label="Read book"
                    >
                      <IconBook size={28} />
                    </ActionIcon>
                  </div>
                )}
                {/* Progress bar - shows at bottom of cover for books with progress */}
                {type === "book" &&
                  book?.readProgress &&
                  !book.readProgress.completed &&
                  progressPercentage > 0 && (
                    <Progress
                      value={progressPercentage}
                      size="sm"
                      color="red"
                      style={{
                        position: "absolute",
                        bottom: 0,
                        left: 0,
                        right: 0,
                        zIndex: 4,
                        borderRadius: 0,
                      }}
                    />
                  )}
              </div>
              {/* Card Content - Fixed height section (Komga: 94px = 5.875rem at 16px base) */}
              <Stack
                gap={4}
                p="sm"
                style={{
                  flexShrink: 0,
                  height: "5.875rem",
                  minHeight: "5.875rem",
                  overflow: "visible",
                }}
              >
                {!hideSeriesName &&
                  type === "book" &&
                  book?.seriesName &&
                  book.seriesName.trim() !== "" &&
                  book.seriesName.trim() !== "-" && (
                    // No tooltip here: the card HoverCard already surfaces the
                    // full series name, so a second hover label is redundant.
                    <AppLink
                      to={`/series/${book.seriesId}`}
                      stopPropagation
                      style={{
                        overflow: "hidden",
                        textOverflow: "ellipsis",
                        whiteSpace: "nowrap",
                        display: "block",
                      }}
                      className="hover-underline"
                    >
                      <Text fw={500} lineClamp={1} c="dimmed" size="xs">
                        {book.seriesName}
                      </Text>
                    </AppLink>
                  )}
                {/* Title tooltip dropped in favour of the cover HoverCard, which
                already surfaces the full title (plus description and counts). */}
                <div style={{ minWidth: 0, width: "100%" }}>
                  <AppLink
                    to={
                      type === "series"
                        ? `/series/${(data as Series).id}`
                        : `/books/${(data as Book).id}`
                    }
                    stopPropagation
                    className="hover-underline"
                  >
                    <Text
                      fw={600}
                      size="sm"
                      style={{
                        display: "-webkit-box",
                        WebkitLineClamp: hideSeriesName ? 2 : 1,
                        WebkitBoxOrient: "vertical",
                        overflow: "hidden",
                        wordBreak: "break-all",
                      }}
                    >
                      {title}
                    </Text>
                  </AppLink>
                </div>
                <Group gap="xs" mt="auto" style={{ flexShrink: 0 }}>
                  {book && (
                    <>
                      {book.pageCount && (
                        <Text size="xs" c="dimmed">
                          {book.pageCount} pages
                        </Text>
                      )}
                      <Text size="xs" c="dimmed">
                        {book.fileFormat.toUpperCase()}
                      </Text>
                    </>
                  )}
                  {series && (
                    <>
                      {series.bookCount !== undefined && (
                        <Text size="xs" c="dimmed">
                          {series.bookCount} book
                          {series.bookCount !== 1 ? "s" : ""}
                        </Text>
                      )}
                      {series.year && (
                        <Text size="xs" c="dimmed">
                          {series.year}
                        </Text>
                      )}
                    </>
                  )}
                </Group>
              </Stack>
            </Stack>
          </HoverCard.Target>
          <HoverCard.Dropdown>
            {type === "series" && series ? (
              <MediaCardHoverPanel type="series" title={title} data={series} />
            ) : book ? (
              <MediaCardHoverPanel type="book" title={title} data={book} />
            ) : null}
          </HoverCard.Dropdown>
        </HoverCard>
      </Card>
      {/* Inline-create modals for the membership submenus. Rendered as siblings
          of the Card (not children) so their portaled clicks don't bubble
          through React's tree to the card's navigation handler, and so they
          stay mounted when the dropdown closes on item click. */}
      {type === "series" && series && (
        <CollectionFormModal
          opened={createCollectionOpen}
          onClose={() => setCreateCollectionOpen(false)}
          onCreated={(c) =>
            addSeriesToCollection.mutate({
              collectionId: c.id,
              seriesIds: [series.id],
            })
          }
        />
      )}
      {type === "book" && book && (
        <ReadListFormModal
          opened={createReadListOpen}
          onClose={() => setCreateReadListOpen(false)}
          onCreated={(r) =>
            addBooksToReadList.mutate({
              readListId: r.id,
              bookIds: [book.id],
            })
          }
        />
      )}
    </>
  );
});
