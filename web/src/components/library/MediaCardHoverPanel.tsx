import { Stack, Text } from "@mantine/core";
import {
  formatChapterCount,
  formatLocalSeriesCounts,
} from "@/components/series/seriesCounts";
import type { Book, Series } from "@/types";

type MediaCardHoverPanelProps =
  | { type: "series"; title: string; data: Series }
  | { type: "book"; title: string; data: Book };

/** Treats empty strings and the `-` placeholder as "no value". */
function presentable(value: string | null | undefined): string | null {
  if (!value) return null;
  const trimmed = value.trim();
  return trimmed === "" || trimmed === "-" ? null : trimmed;
}

/**
 * Dropdown body for a media card's hover card. Purely presentational: it reads
 * only fields already present on the list DTO, so hovering never triggers a
 * network request.
 */
export function MediaCardHoverPanel(props: MediaCardHoverPanelProps) {
  return (
    <Stack gap={6}>
      <Text fw={700} size="sm" lh={1.3}>
        {props.title}
      </Text>
      {props.type === "series"
        ? renderSeries(props.data)
        : renderBook(props.data)}
    </Stack>
  );
}

function renderSeries(series: Series) {
  const counts = formatLocalSeriesCounts({
    bookCount: series.bookCount,
    localMaxVolume: series.localMaxVolume,
    localMaxChapter: series.localMaxChapter,
  });
  const summary = presentable(series.summary);

  return (
    <>
      {counts && (
        <Text size="xs" c="dimmed">
          {counts}
        </Text>
      )}
      {summary && (
        <Text size="sm" c="dimmed" lineClamp={6}>
          {summary}
        </Text>
      )}
    </>
  );
}

function renderBook(book: Book) {
  const seriesName = presentable(book.seriesName);
  const summary = presentable(book.summary);

  const meta: string[] = [];
  if (typeof book.volume === "number") meta.push(`Vol ${book.volume}`);
  if (typeof book.chapter === "number") {
    meta.push(`Ch ${formatChapterCount(book.chapter)}`);
  }
  if (typeof book.pageCount === "number") {
    meta.push(`${book.pageCount} pages`);
  }
  if (book.fileFormat) meta.push(book.fileFormat.toUpperCase());

  return (
    <>
      {seriesName && (
        <Text size="xs" c="dimmed">
          {seriesName}
        </Text>
      )}
      {meta.length > 0 && (
        <Text size="xs" c="dimmed">
          {meta.join(" · ")}
        </Text>
      )}
      {summary && (
        <Text size="sm" c="dimmed" lineClamp={6}>
          {summary}
        </Text>
      )}
    </>
  );
}
