import {
  ActionIcon,
  Box,
  Button,
  Drawer,
  Group,
  ScrollArea,
  Stack,
  Text,
  Textarea,
  Tooltip,
} from "@mantine/core";
import {
  IconBookmark,
  IconBookmarkFilled,
  IconEdit,
  IconTrash,
} from "@tabler/icons-react";
import { useState } from "react";

import type { EpubBookmark } from "./hooks/useEpubBookmarks";

interface EpubBookmarksProps {
  /** All bookmarks for this book */
  bookmarks: EpubBookmark[];
  /** Whether current location is bookmarked */
  isCurrentLocationBookmarked: boolean;
  /** Whether the drawer is open */
  opened: boolean;
  /** Callback to open/close the drawer */
  onToggle: () => void;
  /** Callback to add bookmark at current location */
  onAddBookmark: () => void;
  /** Callback to remove bookmark at current location */
  onRemoveCurrentBookmark: () => void;
  /** Callback to update a bookmark's note */
  onUpdateNote: (id: string, note: string) => void;
  /** Callback to remove a bookmark by id */
  onRemoveBookmark: (id: string) => void;
  /** Callback when a bookmark is clicked to navigate */
  onNavigate: (cfi: string) => void;
}

/**
 * Format a timestamp as a relative date string
 */
function formatDate(timestamp: number): string {
  const date = new Date(timestamp);
  const now = new Date();
  const diff = now.getTime() - date.getTime();

  // Less than a minute
  if (diff < 60000) {
    return "Just now";
  }
  // Less than an hour
  if (diff < 3600000) {
    const minutes = Math.floor(diff / 60000);
    return `${minutes} minute${minutes !== 1 ? "s" : ""} ago`;
  }
  // Less than a day
  if (diff < 86400000) {
    const hours = Math.floor(diff / 3600000);
    return `${hours} hour${hours !== 1 ? "s" : ""} ago`;
  }
  // Less than a week
  if (diff < 604800000) {
    const days = Math.floor(diff / 86400000);
    return `${days} day${days !== 1 ? "s" : ""} ago`;
  }
  // Otherwise, show date
  return date.toLocaleDateString();
}

interface BookmarkItemProps {
  bookmark: EpubBookmark;
  onNavigate: (cfi: string) => void;
  onUpdateNote: (id: string, note: string) => void;
  onRemove: (id: string) => void;
  onCloseDrawer: () => void;
}

function BookmarkItem({
  bookmark,
  onNavigate,
  onUpdateNote,
  onRemove,
  onCloseDrawer,
}: BookmarkItemProps) {
  const [isEditing, setIsEditing] = useState(false);
  const [noteText, setNoteText] = useState(bookmark.note);

  const handleSaveNote = () => {
    onUpdateNote(bookmark.id, noteText);
    setIsEditing(false);
  };

  const handleCancelEdit = () => {
    setNoteText(bookmark.note);
    setIsEditing(false);
  };

  const handleNavigate = () => {
    onNavigate(bookmark.cfi);
    onCloseDrawer();
  };

  return (
    <Box
      p="sm"
      style={{
        borderRadius: "var(--mantine-radius-sm)",
        border: "1px solid var(--mantine-color-dark-4)",
      }}
    >
      <Group justify="space-between" mb={bookmark.note || isEditing ? "xs" : 0}>
        <Box style={{ flex: 1, cursor: "pointer" }} onClick={handleNavigate}>
          {bookmark.chapterTitle && (
            <Text size="sm" fw={500} lineClamp={1}>
              {bookmark.chapterTitle}
            </Text>
          )}
          <Text size="xs" c="dimmed">
            {Math.round(bookmark.percentage * 100)}% •{" "}
            {formatDate(bookmark.createdAt)}
          </Text>
          {bookmark.excerpt && (
            <Text size="xs" c="dimmed" fs="italic" lineClamp={2} mt={4}>
              "{bookmark.excerpt}"
            </Text>
          )}
        </Box>
        <Group gap="xs">
          <Tooltip label="Edit note">
            <ActionIcon
              variant="subtle"
              color="gray"
              size="sm"
              onClick={() => setIsEditing(true)}
              aria-label="Edit note"
            >
              <IconEdit size={16} />
            </ActionIcon>
          </Tooltip>
          <Tooltip label="Remove bookmark">
            <ActionIcon
              variant="subtle"
              color="red"
              size="sm"
              onClick={() => onRemove(bookmark.id)}
              aria-label="Remove bookmark"
            >
              <IconTrash size={16} />
            </ActionIcon>
          </Tooltip>
        </Group>
      </Group>

      {isEditing ? (
        <Stack gap="xs">
          <Textarea
            value={noteText}
            onChange={(e) => setNoteText(e.target.value)}
            placeholder="Add a note..."
            autosize
            minRows={2}
            maxRows={4}
            autoFocus
          />
          <Group gap="xs" justify="flex-end">
            <Button size="xs" variant="subtle" onClick={handleCancelEdit}>
              Cancel
            </Button>
            <Button size="xs" onClick={handleSaveNote}>
              Save
            </Button>
          </Group>
        </Stack>
      ) : bookmark.note ? (
        <Text size="sm" mt="xs">
          {bookmark.note}
        </Text>
      ) : null}
    </Box>
  );
}

/**
 * Bookmarks drawer for EPUB reader.
 *
 * Displays a list of bookmarks with notes, sorted by position in the book.
 * Allows adding, editing, and removing bookmarks.
 */
export function EpubBookmarks({
  bookmarks,
  isCurrentLocationBookmarked,
  opened,
  onToggle,
  onAddBookmark,
  onRemoveCurrentBookmark,
  onUpdateNote,
  onRemoveBookmark,
  onNavigate,
}: EpubBookmarksProps) {
  // Sort bookmarks by percentage (position in book)
  const sortedBookmarks = [...bookmarks].sort(
    (a, b) => a.percentage - b.percentage,
  );

  const handleBookmarkToggle = () => {
    if (isCurrentLocationBookmarked) {
      onRemoveCurrentBookmark();
    } else {
      onAddBookmark();
    }
  };

  return (
    <>
      {/* Toggle button */}
      <Tooltip
        label={isCurrentLocationBookmarked ? "Remove bookmark" : "Add bookmark"}
        position="bottom"
      >
        <ActionIcon
          variant="subtle"
          color={isCurrentLocationBookmarked ? "yellow" : "gray"}
          size="lg"
          onClick={handleBookmarkToggle}
          aria-label={
            isCurrentLocationBookmarked ? "Remove bookmark" : "Add bookmark"
          }
        >
          {isCurrentLocationBookmarked ? (
            <IconBookmarkFilled size={20} />
          ) : (
            <IconBookmark size={20} />
          )}
        </ActionIcon>
      </Tooltip>

      {/* List button - only show if there are bookmarks */}
      {bookmarks.length > 0 && (
        <Tooltip label="View bookmarks" position="bottom">
          <ActionIcon
            variant="subtle"
            color="gray"
            size="lg"
            onClick={onToggle}
            aria-label="View bookmarks"
          >
            <Text size="xs" fw={600}>
              {bookmarks.length}
            </Text>
          </ActionIcon>
        </Tooltip>
      )}

      {/* Bookmarks Drawer */}
      <Drawer
        opened={opened}
        onClose={onToggle}
        title={`Bookmarks (${bookmarks.length})`}
        position="right"
        size="sm"
        styles={{
          body: {
            padding: 0,
          },
        }}
      >
        <ScrollArea h="calc(100vh - 60px)" px="md" pb="md">
          {sortedBookmarks.length === 0 ? (
            <Text c="dimmed" size="sm" ta="center" py="xl">
              No bookmarks yet. Click the bookmark icon to add one.
            </Text>
          ) : (
            <Stack gap="sm">
              {sortedBookmarks.map((bookmark) => (
                <BookmarkItem
                  key={bookmark.id}
                  bookmark={bookmark}
                  onNavigate={onNavigate}
                  onUpdateNote={onUpdateNote}
                  onRemove={onRemoveBookmark}
                  onCloseDrawer={onToggle}
                />
              ))}
            </Stack>
          )}
        </ScrollArea>
      </Drawer>
    </>
  );
}
