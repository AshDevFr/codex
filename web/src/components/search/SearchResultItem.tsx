import { Group, Image, Stack, Text } from "@mantine/core";
import type { Book, Series } from "@/types";

const FALLBACK_THUMB =
  "data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='40' height='56'%3E%3Crect fill='%23333' width='40' height='56'/%3E%3C/svg%3E";

export function SeriesResultContent({ series }: { series: Series }) {
  return (
    <Group gap="sm" wrap="nowrap">
      <Image
        src={`/api/v1/series/${series.id}/thumbnail`}
        alt={series.title}
        w={40}
        h={56}
        fit="cover"
        radius="sm"
        fallbackSrc={FALLBACK_THUMB}
      />
      <Stack gap={2} style={{ flex: 1, minWidth: 0 }}>
        <Text size="sm" fw={500} lineClamp={1}>
          {series.title}
        </Text>
        <Text size="xs" c="dimmed">
          {series.bookCount} book{series.bookCount !== 1 ? "s" : ""}
        </Text>
      </Stack>
    </Group>
  );
}

export function BookResultContent({ book }: { book: Book }) {
  return (
    <Group gap="sm" wrap="nowrap">
      <Image
        src={`/api/v1/books/${book.id}/thumbnail`}
        alt={book.title}
        w={40}
        h={56}
        fit="cover"
        radius="sm"
        fallbackSrc={FALLBACK_THUMB}
      />
      <Stack gap={2} style={{ flex: 1, minWidth: 0 }}>
        <Text size="sm" fw={500} lineClamp={1}>
          {book.number !== undefined && book.number !== null
            ? `${book.number} - ${book.title}`
            : book.title}
        </Text>
        {book.seriesName && (
          <Text size="xs" c="dimmed" lineClamp={1}>
            {book.seriesName}
          </Text>
        )}
      </Stack>
    </Group>
  );
}
