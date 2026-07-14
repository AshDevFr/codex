import {
  Center,
  Container,
  Group,
  SegmentedControl,
  Stack,
  Text,
  Title,
} from "@mantine/core";
import { IconBookmark } from "@tabler/icons-react";
import { useQueries } from "@tanstack/react-query";
import { useState } from "react";
import { booksApi } from "@/api/books";
import { seriesApi } from "@/api/series";
import type { WantToReadSort } from "@/api/wantToRead";
import { MediaGrid, type MediaGridItem } from "@/components/library/MediaGrid";
import {
  useRemoveFromWantToRead,
  useWantToReadQueue,
} from "@/hooks/useWantToRead";
import type { Book, Series } from "@/types";

export function WantToRead() {
  const [sort, setSort] = useState<WantToReadSort>("newest");
  const { data: entries, isLoading } = useWantToReadQueue(sort);
  const removeMutation = useRemoveFromWantToRead();

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
  (entries ?? []).forEach((entry, i) => {
    const isSeries = entry.itemType === "series";
    const id = (isSeries ? entry.seriesId : entry.bookId) ?? "";
    if (!id) return;
    const query = entryQueries[i];
    if (query?.isLoading) {
      items.push({ id, type: isSeries ? "series" : "book" });
    } else if (query?.data) {
      items.push({ id, type: isSeries ? "series" : "book", data: query.data });
    }
  });

  return (
    <Container size="xl" py="md">
      <Group justify="space-between" align="center" mb="lg">
        <Group gap="xs">
          <IconBookmark size={28} />
          <Title order={2}>Want to Read</Title>
        </Group>
        <SegmentedControl
          value={sort}
          onChange={(value) => setSort(value as WantToReadSort)}
          data={[
            { label: "Newest", value: "newest" },
            { label: "Oldest", value: "oldest" },
          ]}
          aria-label="Sort order"
        />
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
        />
      )}
    </Container>
  );
}
