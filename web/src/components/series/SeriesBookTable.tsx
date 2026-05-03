import {
  ActionIcon,
  Badge,
  Checkbox,
  Group,
  Progress,
  Stack,
  Table,
  Text,
  Tooltip,
} from "@mantine/core";
import { IconBook, IconTrash } from "@tabler/icons-react";
import { memo } from "react";
import { useNavigate } from "react-router-dom";
import { BookKindBadge } from "@/components/book/BookKindBadge";
import type { Book } from "@/types";

interface SeriesBookTableProps {
  books: Book[];
  /** Receives id, shiftKey, and the row index in the current page slice. */
  onSelect: (id: string, shiftKey: boolean, index?: number) => void;
  selectedIds: Set<string>;
  isSelectionMode: boolean;
  canBeSelected: boolean;
}

const dateFormatter = new Intl.DateTimeFormat(undefined, {
  year: "numeric",
  month: "short",
  day: "numeric",
});

function formatAdded(value: string): string {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return "—";
  return dateFormatter.format(date);
}

function ProgressCell({ book }: { book: Book }) {
  const progress = book.readProgress;

  if (progress?.completed) {
    return (
      <Badge size="xs" color="green" variant="light">
        Read
      </Badge>
    );
  }

  if (progress) {
    const ratio =
      progress.progressPercentage != null
        ? progress.progressPercentage
        : book.pageCount
          ? progress.currentPage / book.pageCount
          : 0;
    const percent = Math.max(0, Math.min(100, Math.round(ratio * 100)));
    const tooltip =
      book.pageCount > 0
        ? `Page ${progress.currentPage} of ${book.pageCount} (${percent}%)`
        : `${percent}%`;
    return (
      <Tooltip label={tooltip} openDelay={300}>
        <Stack gap={2} miw={80}>
          <Progress value={percent} size="sm" color="red" radius="xl" />
          <Text size="xs" c="dimmed">
            {percent}%
          </Text>
        </Stack>
      </Tooltip>
    );
  }

  return (
    <Badge size="xs" color="orange" variant="light">
      Unread
    </Badge>
  );
}

export const SeriesBookTable = memo(function SeriesBookTable({
  books,
  onSelect,
  selectedIds,
  isSelectionMode,
  canBeSelected,
}: SeriesBookTableProps) {
  const navigate = useNavigate();

  const handleRowClick = (book: Book, index: number, shiftKey: boolean) => {
    if (isSelectionMode) {
      if (canBeSelected) {
        onSelect(book.id, shiftKey, index);
      }
      return;
    }
    navigate(`/books/${book.id}`);
  };

  const handleReadClick = (book: Book) => {
    if (book.deleted) return;
    const page = book.readProgress?.currentPage || 1;
    navigate(`/reader/${book.id}?page=${page}`);
  };

  return (
    <Table.ScrollContainer minWidth={600}>
      <Table highlightOnHover striped withTableBorder>
        <Table.Thead>
          <Table.Tr>
            <Table.Th style={{ width: 40 }} aria-label="Select" />
            <Table.Th style={{ width: 64 }}>#</Table.Th>
            <Table.Th style={{ width: 130 }}>Kind</Table.Th>
            <Table.Th>Title</Table.Th>
            <Table.Th style={{ width: 120 }}>Status</Table.Th>
            <Table.Th style={{ width: 80 }}>Pages</Table.Th>
            <Table.Th style={{ width: 90 }}>Format</Table.Th>
            <Table.Th style={{ width: 130 }}>Added</Table.Th>
            <Table.Th style={{ width: 60 }} aria-label="Actions" />
          </Table.Tr>
        </Table.Thead>
        <Table.Tbody>
          {books.map((book, index) => {
            const isSelected = selectedIds.has(book.id);
            const cursor =
              isSelectionMode && !canBeSelected ? "not-allowed" : "pointer";
            return (
              <Table.Tr
                key={book.id}
                data-testid={`series-book-row-${book.id}`}
                onClick={(e) => handleRowClick(book, index, e.shiftKey)}
                bg={
                  isSelected ? "var(--mantine-color-orange-light)" : undefined
                }
                style={{ cursor, opacity: book.deleted ? 0.6 : 1 }}
              >
                <Table.Td onClick={(e) => e.stopPropagation()}>
                  <Checkbox
                    checked={isSelected}
                    disabled={!canBeSelected}
                    color="orange"
                    aria-label={`Select ${book.title}`}
                    onChange={(e) => {
                      if (!canBeSelected) return;
                      const native = e.nativeEvent as unknown as
                        | MouseEvent
                        | undefined;
                      onSelect(book.id, native?.shiftKey ?? false, index);
                    }}
                  />
                </Table.Td>
                <Table.Td>
                  <Text size="sm" c="dimmed">
                    {book.number ?? "—"}
                  </Text>
                </Table.Td>
                <Table.Td>
                  {book.volume != null || book.chapter != null ? (
                    <BookKindBadge
                      volume={book.volume}
                      chapter={book.chapter}
                      size="xs"
                      variant="light"
                    />
                  ) : (
                    <Text size="sm" c="dimmed">
                      —
                    </Text>
                  )}
                </Table.Td>
                <Table.Td style={{ minWidth: 0 }}>
                  <Group
                    gap="xs"
                    wrap="nowrap"
                    style={{ minWidth: 0, maxWidth: "100%" }}
                  >
                    {book.deleted && (
                      <Badge
                        size="xs"
                        color="red"
                        variant="light"
                        leftSection={<IconTrash size={10} />}
                        style={{ flexShrink: 0 }}
                      >
                        Deleted
                      </Badge>
                    )}
                    <Tooltip label={book.title} openDelay={500} withArrow>
                      <Text
                        size="sm"
                        fw={500}
                        style={{
                          overflow: "hidden",
                          textOverflow: "ellipsis",
                          whiteSpace: "nowrap",
                          minWidth: 0,
                          flex: 1,
                        }}
                      >
                        {book.title}
                      </Text>
                    </Tooltip>
                  </Group>
                </Table.Td>
                <Table.Td>
                  <ProgressCell book={book} />
                </Table.Td>
                <Table.Td>
                  <Text size="sm" c="dimmed">
                    {book.pageCount ?? "—"}
                  </Text>
                </Table.Td>
                <Table.Td>
                  <Text size="sm" c="dimmed">
                    {book.fileFormat?.toUpperCase() ?? "—"}
                  </Text>
                </Table.Td>
                <Table.Td>
                  <Text size="sm" c="dimmed">
                    {formatAdded(book.createdAt)}
                  </Text>
                </Table.Td>
                <Table.Td onClick={(e) => e.stopPropagation()}>
                  {!book.deleted && (
                    <Tooltip
                      label={book.readProgress ? "Continue reading" : "Read"}
                      openDelay={300}
                    >
                      <ActionIcon
                        variant="subtle"
                        color="red"
                        size="md"
                        onClick={() => handleReadClick(book)}
                        aria-label={
                          book.readProgress
                            ? `Continue reading ${book.title}`
                            : `Read ${book.title}`
                        }
                      >
                        <IconBook size={18} />
                      </ActionIcon>
                    </Tooltip>
                  )}
                </Table.Td>
              </Table.Tr>
            );
          })}
        </Table.Tbody>
      </Table>
    </Table.ScrollContainer>
  );
});
