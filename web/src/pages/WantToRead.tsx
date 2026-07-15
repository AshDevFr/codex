import {
  ActionIcon,
  Center,
  Container,
  Group,
  SegmentedControl,
  Stack,
  Text,
  Title,
  Tooltip,
} from "@mantine/core";
import { IconBookmark, IconLock, IconLockOpen } from "@tabler/icons-react";
import { useQueries } from "@tanstack/react-query";
import { useState } from "react";
import { booksApi } from "@/api/books";
import { seriesApi } from "@/api/series";
import type { WantToReadSort } from "@/api/wantToRead";
import { MediaGrid, type MediaGridItem } from "@/components/library/MediaGrid";
import {
  useRemoveFromWantToRead,
  useReorderWantToRead,
  useWantToReadQueue,
} from "@/hooks/useWantToRead";
import { useUserPreferencesStore } from "@/store/userPreferencesStore";
import type { Book, Series } from "@/types";

export function WantToRead() {
  // The chosen sort persists as a per-user preference, so the page reopens in
  // the same order (e.g. a manually curated queue stays on Manual).
  const sort = useUserPreferencesStore((state) =>
    state.getPreference("want_to_read.sort"),
  );
  const setPreference = useUserPreferencesStore((state) => state.setPreference);
  const setSort = (value: WantToReadSort) =>
    setPreference("want_to_read.sort", value);
  // Reordering rewrites the manual queue order with no undo, so it stays
  // locked until explicitly enabled to keep stray drags harmless.
  const [reorderUnlocked, setReorderUnlocked] = useState(false);
  const { data: entries, isLoading } = useWantToReadQueue(sort);
  const removeMutation = useRemoveFromWantToRead();
  const reorderMutation = useReorderWantToRead();

  // The queue only carries IDs, so each entry's series/book is fetched on
  // demand (React Query caches/dedupes across the app). Order follows the
  // queue; entries whose target is gone (deleted / inaccessible) render
  // nothing.
  const entryQueries = useQueries({
    queries: (entries ?? []).map((entry) => {
      const isSeries = entry.itemType === "series";
      const id = (isSeries ? entry.seriesId : entry.bookId) ?? "";
      return {
        queryKey: isSeries ? ["series", id] : ["books", id],
        queryFn: (): Promise<Series | Book> =>
          isSeries ? seriesApi.getById(id) : booksApi.getById(id),
        enabled: Boolean(id),
      };
    }),
  });

  const items: MediaGridItem[] = [];
  // Grid items are keyed by target (series/book) id; map back to the queue
  // entry ids that the remove/reorder endpoints expect.
  const entryIdByTarget = new Map<string, string>();
  (entries ?? []).forEach((entry, i) => {
    const isSeries = entry.itemType === "series";
    const id = (isSeries ? entry.seriesId : entry.bookId) ?? "";
    if (!id) return;
    entryIdByTarget.set(id, entry.id);
    const query = entryQueries[i];
    if (query?.isLoading) {
      items.push({ id, type: isSeries ? "series" : "book" });
    } else if (query?.data) {
      items.push({ id, type: isSeries ? "series" : "book", data: query.data });
    }
  });

  const manualView = sort === "custom";
  const handleReorder = (targetIds: string[]) => {
    const entryIds = targetIds
      .map((id) => entryIdByTarget.get(id))
      .filter((id): id is string => Boolean(id));
    reorderMutation.mutate(entryIds);
  };

  return (
    <Container fluid py="md">
      <Group justify="space-between" align="center" mb="lg">
        <Group gap="xs">
          <IconBookmark size={28} />
          <Title order={2}>Want to Read</Title>
        </Group>
        <Group gap="xs">
          <SegmentedControl
            value={sort}
            onChange={(value) => setSort(value as WantToReadSort)}
            data={[
              { label: "Newest", value: "newest" },
              { label: "Oldest", value: "oldest" },
              { label: "Manual", value: "custom" },
            ]}
            aria-label="Sort order"
          />
          {manualView && items.length > 1 && (
            <Tooltip
              label={
                reorderUnlocked
                  ? "Lock reordering"
                  : "Unlock reordering (drag & drop)"
              }
            >
              <ActionIcon
                variant={reorderUnlocked ? "filled" : "default"}
                size="lg"
                onClick={() => setReorderUnlocked((v) => !v)}
                aria-label={
                  reorderUnlocked ? "Lock reordering" : "Unlock reordering"
                }
              >
                {reorderUnlocked ? (
                  <IconLockOpen size={16} />
                ) : (
                  <IconLock size={16} />
                )}
              </ActionIcon>
            </Tooltip>
          )}
        </Group>
      </Group>

      {isLoading ? (
        <MediaGrid items={[]} loading />
      ) : !entries || entries.length === 0 ? (
        <Center mih={240}>
          <Stack align="center" gap="xs">
            <IconBookmark size={48} opacity={0.4} />
            <Text c="dimmed">Your Want to Read queue is empty.</Text>
            <Text c="dimmed" size="sm">
              Add a series or book from its detail page to read it later.
            </Text>
          </Stack>
        </Center>
      ) : (
        <MediaGrid
          items={items}
          onRemove={(item) =>
            removeMutation.mutate({ itemType: item.type, id: item.id })
          }
          removeLabel="Remove from Want to Read"
          removingId={
            removeMutation.isPending ? removeMutation.variables?.id : undefined
          }
          reorderable={manualView && reorderUnlocked}
          onReorder={handleReorder}
          reorderPending={reorderMutation.isPending}
        />
      )}
    </Container>
  );
}
