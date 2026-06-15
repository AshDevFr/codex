import {
  Button,
  Center,
  Container,
  Group,
  SegmentedControl,
  SimpleGrid,
  Skeleton,
  Stack,
  Text,
  Title,
} from "@mantine/core";
import { IconBookmark, IconX } from "@tabler/icons-react";
import { useQuery } from "@tanstack/react-query";
import { useState } from "react";
import { booksApi } from "@/api/books";
import { seriesApi } from "@/api/series";
import type { WantToReadEntry, WantToReadSort } from "@/api/wantToRead";
import { MediaCard } from "@/components/library/MediaCard";
import {
  useRemoveFromWantToRead,
  useWantToReadQueue,
  type WantToReadTarget,
} from "@/hooks/useWantToRead";

interface QueueItemProps {
  entry: WantToReadEntry;
  onRemove: (target: WantToReadTarget) => void;
  removing: boolean;
}

/**
 * Renders one queue entry. The queue only carries IDs, so we fetch the series
 * or book on demand (React Query caches/dedupes) and reuse the standard
 * MediaCard. Entries whose target is gone (deleted / inaccessible) render
 * nothing.
 */
function QueueItem({ entry, onRemove, removing }: QueueItemProps) {
  const isSeries = entry.itemType === "series";
  const id = (isSeries ? entry.seriesId : entry.bookId) ?? "";

  const seriesQuery = useQuery({
    queryKey: ["series", id],
    queryFn: () => seriesApi.getById(id),
    enabled: isSeries && Boolean(id),
  });
  const bookQuery = useQuery({
    queryKey: ["books", id],
    queryFn: () => booksApi.getById(id),
    enabled: !isSeries && Boolean(id),
  });

  const isLoading = isSeries ? seriesQuery.isLoading : bookQuery.isLoading;
  const data = isSeries ? seriesQuery.data : bookQuery.data;

  if (isLoading) {
    return <Skeleton height={300} radius="md" />;
  }
  if (!data) {
    return null;
  }

  return (
    <Stack gap={4}>
      <MediaCard type={isSeries ? "series" : "book"} data={data} />
      <Button
        variant="subtle"
        color="gray"
        size="xs"
        leftSection={<IconX size={14} />}
        loading={removing}
        onClick={() => onRemove({ itemType: entry.itemType, id })}
      >
        Remove
      </Button>
    </Stack>
  );
}

export function WantToRead() {
  const [sort, setSort] = useState<WantToReadSort>("newest");
  const { data: entries, isLoading } = useWantToReadQueue(sort);
  const removeMutation = useRemoveFromWantToRead();
  const removingId = removeMutation.isPending
    ? removeMutation.variables?.id
    : undefined;

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
        <SimpleGrid cols={{ base: 2, sm: 3, md: 4, lg: 6 }} spacing="md">
          {Array.from({ length: 6 }).map((_, i) => (
            // biome-ignore lint/suspicious/noArrayIndexKey: static skeleton placeholders
            <Skeleton key={i} height={340} radius="md" />
          ))}
        </SimpleGrid>
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
        <SimpleGrid cols={{ base: 2, sm: 3, md: 4, lg: 6 }} spacing="md">
          {entries.map((entry) => (
            <QueueItem
              key={entry.id}
              entry={entry}
              onRemove={(target) => removeMutation.mutate(target)}
              removing={
                removingId ===
                (entry.itemType === "series" ? entry.seriesId : entry.bookId)
              }
            />
          ))}
        </SimpleGrid>
      )}
    </Container>
  );
}
