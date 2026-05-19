import {
  ActionIcon,
  Anchor,
  Badge,
  Card,
  Checkbox,
  Group,
  Stack,
  Table,
  Text,
  Tooltip,
} from "@mantine/core";
import { useMediaQuery } from "@mantine/hooks";
import {
  IconCheck,
  IconExternalLink,
  IconEyeOff,
  IconRefresh,
  IconTrash,
  IconX,
} from "@tabler/icons-react";
import { format } from "date-fns";
import { Link } from "react-router-dom";
import type { ReleaseLedgerEntry, ReleaseSource } from "@/api/releases";
import { MOBILE_MEDIA_QUERY } from "@/components/ui";
import { MediaUrlIcon } from "./MediaUrlIcon";

const STATE_BADGE: Record<string, { color: string; label: string }> = {
  announced: { color: "blue", label: "New" },
  marked_acquired: { color: "green", label: "Acquired" },
  dismissed: { color: "gray", label: "Dismissed" },
  ignored: { color: "gray", label: "Ignored" },
  hidden: { color: "gray", label: "Hidden" },
};

interface ReleasesTableProps {
  entries: ReleaseLedgerEntry[];
  sourceById: Map<string, ReleaseSource>;
  selected: Set<string>;
  /** `shiftKey` is true when the user shift-clicks for range selection. */
  onToggleOne: (id: string, shiftKey: boolean) => void;
  onToggleAll: () => void;
  onDismiss: (id: string) => void;
  onMarkAcquired: (id: string) => void;
  onIgnore: (id: string) => void;
  onReset: (id: string) => void;
  onDelete: (id: string) => void;
  /** When true, render a Series column linking to the series detail page.
   *  Off when the table is already scoped to a single series. */
  showSeriesColumn?: boolean;
  /** Disable per-row action buttons while a mutation is in flight. */
  isDismissPending?: boolean;
  isMarkAcquiredPending?: boolean;
  isIgnorePending?: boolean;
  isResetPending?: boolean;
  isDeletePending?: boolean;
  /** Visual density. The page-level inbox uses "sm"; the embedded panel
   *  uses "xs" so it doesn't dominate the surrounding card. */
  verticalSpacing?: "xs" | "sm";
}

interface NumericSpan {
  start: number;
  end: number;
}

/**
 * Render one normalized span list (e.g. `[{1,4},{6,9}]`) as a compact
 * human-readable string ("1-4, 6-9"). Single-point spans collapse to the
 * value; range spans render as `start-end`. Disjoint compilations (the
 * `v01-04 + v06-09` case) keep their gap so the user sees the truth.
 */
function formatSpans(spans: NumericSpan[] | null | undefined): string | null {
  if (!spans || spans.length === 0) return null;
  return spans
    .map((s) => (s.start === s.end ? `${s.start}` : `${s.start}-${s.end}`))
    .join(", ");
}

function formatChapterVolume(entry: ReleaseLedgerEntry): string {
  const chapterStr = formatSpans(entry.chapters);
  const volumeStr = formatSpans(entry.volumes);
  if (chapterStr === null && volumeStr === null) return "—";
  const parts: string[] = [];
  if (chapterStr !== null) parts.push(`Ch ${chapterStr}`);
  if (volumeStr !== null) parts.push(`Vol ${volumeStr}`);
  return parts.join(" · ");
}

export function ReleasesTable({
  entries,
  sourceById,
  selected,
  onToggleOne,
  onToggleAll,
  onDismiss,
  onMarkAcquired,
  onIgnore,
  onReset,
  onDelete,
  showSeriesColumn = false,
  isDismissPending = false,
  isMarkAcquiredPending = false,
  isIgnorePending = false,
  isResetPending = false,
  isDeletePending = false,
  verticalSpacing = "sm",
}: ReleasesTableProps) {
  const allSelected =
    entries.length > 0 && entries.every((e) => selected.has(e.id));
  const someSelected = entries.some((e) => selected.has(e.id)) && !allSelected;

  // Below xs the wide release table clips off the side. Render a stack of
  // cards instead — each card carries the same controls and shows series /
  // chapter / source on its own line. useMediaQuery (rather than CSS-only
  // `visibleFrom`) keeps only one DOM tree mounted so tests that query the
  // row checkboxes / actions don't see duplicate matches.
  const isMobile = useMediaQuery(MOBILE_MEDIA_QUERY) ?? false;

  if (isMobile) {
    return (
      <Stack gap="sm" p="sm">
        <Group gap="xs" px="xs">
          <Checkbox
            aria-label="Select all releases"
            checked={allSelected}
            indeterminate={someSelected}
            onChange={onToggleAll}
          />
          <Text size="xs" c="dimmed">
            {selected.size > 0 ? `${selected.size} selected` : "Select all"}
          </Text>
        </Group>
        {entries.map((entry) => {
          const stateInfo = STATE_BADGE[entry.state] ?? {
            color: "gray",
            label: entry.state,
          };
          const isSelected = selected.has(entry.id);
          const source = sourceById.get(entry.sourceId);
          const sourceLabel =
            source?.displayName ?? `${entry.sourceId.slice(0, 8)}…`;
          return (
            <Card
              key={entry.id}
              withBorder
              padding="md"
              bg={isSelected ? "var(--mantine-color-blue-light)" : undefined}
            >
              <Group justify="space-between" gap="xs" wrap="nowrap">
                <Group gap="xs" wrap="nowrap" style={{ minWidth: 0, flex: 1 }}>
                  <Checkbox
                    aria-label={`Select release ${entry.id}`}
                    checked={isSelected}
                    onChange={(event) => {
                      const shiftKey =
                        event.nativeEvent instanceof MouseEvent &&
                        event.nativeEvent.shiftKey;
                      onToggleOne(entry.id, shiftKey);
                    }}
                  />
                  {showSeriesColumn ? (
                    <Anchor
                      component={Link}
                      to={`/series/${entry.seriesId}#releases`}
                      size="sm"
                      fw={500}
                      style={{ minWidth: 0, wordBreak: "break-word" }}
                    >
                      {entry.seriesTitle.length > 0
                        ? entry.seriesTitle
                        : `${entry.seriesId.slice(0, 8)}…`}
                    </Anchor>
                  ) : (
                    <Text
                      size="sm"
                      fw={500}
                      style={{ minWidth: 0, wordBreak: "break-word" }}
                    >
                      {formatChapterVolume(entry)}
                    </Text>
                  )}
                </Group>
                <Badge color={stateInfo.color} variant="light" size="sm">
                  {stateInfo.label}
                </Badge>
              </Group>
              <Stack gap={2} mt="xs">
                {showSeriesColumn && (
                  <Text size="sm" fw={500}>
                    {formatChapterVolume(entry)}
                  </Text>
                )}
                <Text size="xs" c="dimmed">
                  {sourceLabel}
                  {entry.groupOrUploader &&
                  entry.groupOrUploader !== sourceLabel
                    ? ` · ${entry.groupOrUploader}`
                    : ""}
                  {entry.language ? ` · ${entry.language}` : ""}
                </Text>
                <Text size="xs" c="dimmed">
                  Observed {format(new Date(entry.observedAt), "yyyy-MM-dd")}
                </Text>
              </Stack>
              <Group gap={4} justify="flex-end" wrap="nowrap" mt="sm">
                <Tooltip label="Open payload URL">
                  <ActionIcon
                    component="a"
                    href={entry.payloadUrl}
                    target="_blank"
                    rel="noopener noreferrer"
                    variant="subtle"
                    aria-label="Open payload URL"
                  >
                    <IconExternalLink size={16} />
                  </ActionIcon>
                </Tooltip>
                {entry.mediaUrl && (
                  <MediaUrlIcon
                    url={entry.mediaUrl}
                    kind={entry.mediaUrlKind}
                  />
                )}
                {entry.state === "announced" ? (
                  <>
                    <Tooltip label="Mark acquired">
                      <ActionIcon
                        variant="subtle"
                        color="green"
                        loading={isMarkAcquiredPending}
                        onClick={() => onMarkAcquired(entry.id)}
                        aria-label="Mark acquired"
                      >
                        <IconCheck size={16} />
                      </ActionIcon>
                    </Tooltip>
                    <Tooltip label="Dismiss">
                      <ActionIcon
                        variant="subtle"
                        color="gray"
                        loading={isDismissPending}
                        onClick={() => onDismiss(entry.id)}
                        aria-label="Dismiss"
                      >
                        <IconX size={16} />
                      </ActionIcon>
                    </Tooltip>
                    <Tooltip label="Ignore">
                      <ActionIcon
                        variant="subtle"
                        color="gray"
                        loading={isIgnorePending}
                        onClick={() => onIgnore(entry.id)}
                        aria-label="Ignore"
                      >
                        <IconEyeOff size={16} />
                      </ActionIcon>
                    </Tooltip>
                  </>
                ) : (
                  <Tooltip label="Reset to new">
                    <ActionIcon
                      variant="subtle"
                      color="blue"
                      loading={isResetPending}
                      onClick={() => onReset(entry.id)}
                      aria-label="Reset"
                    >
                      <IconRefresh size={16} />
                    </ActionIcon>
                  </Tooltip>
                )}
                <Tooltip label="Delete (will reappear on next poll)">
                  <ActionIcon
                    variant="subtle"
                    color="red"
                    loading={isDeletePending}
                    onClick={() => onDelete(entry.id)}
                    aria-label="Delete"
                  >
                    <IconTrash size={16} />
                  </ActionIcon>
                </Tooltip>
              </Group>
            </Card>
          );
        })}
      </Stack>
    );
  }

  return (
    <Table verticalSpacing={verticalSpacing} highlightOnHover>
      <Table.Thead>
        <Table.Tr>
          <Table.Th w={36}>
            <Checkbox
              aria-label="Select all releases"
              checked={allSelected}
              indeterminate={someSelected}
              onChange={onToggleAll}
            />
          </Table.Th>
          {showSeriesColumn && <Table.Th>Series</Table.Th>}
          <Table.Th>Ch / Vol</Table.Th>
          <Table.Th>Source / Group</Table.Th>
          <Table.Th>Lang</Table.Th>
          <Table.Th>State</Table.Th>
          <Table.Th>Observed</Table.Th>
          <Table.Th aria-label="Actions" />
        </Table.Tr>
      </Table.Thead>
      <Table.Tbody>
        {entries.map((entry) => {
          const stateInfo = STATE_BADGE[entry.state] ?? {
            color: "gray",
            label: entry.state,
          };
          const isSelected = selected.has(entry.id);
          const source = sourceById.get(entry.sourceId);
          const sourceLabel =
            source?.displayName ?? `${entry.sourceId.slice(0, 8)}…`;
          return (
            <Table.Tr
              key={entry.id}
              bg={isSelected ? "var(--mantine-color-blue-light)" : undefined}
            >
              <Table.Td
                // Suppress the browser's shift-click text selection so the
                // checkbox range gesture doesn't leave a ghost highlight
                // across rows.
                onMouseDown={(e) => {
                  if (e.shiftKey) e.preventDefault();
                }}
              >
                <Checkbox
                  aria-label={`Select release ${entry.id}`}
                  checked={isSelected}
                  onChange={(event) => {
                    // `nativeEvent` is typed as `Event`; the click path
                    // delivers a MouseEvent whose `shiftKey` is the gesture
                    // we want. Keyboard toggling won't have it set.
                    const shiftKey =
                      event.nativeEvent instanceof MouseEvent &&
                      event.nativeEvent.shiftKey;
                    onToggleOne(entry.id, shiftKey);
                  }}
                />
              </Table.Td>
              {showSeriesColumn && (
                <Table.Td>
                  <Anchor
                    component={Link}
                    to={`/series/${entry.seriesId}#releases`}
                    size="sm"
                    lineClamp={1}
                  >
                    {entry.seriesTitle.length > 0
                      ? entry.seriesTitle
                      : `${entry.seriesId.slice(0, 8)}…`}
                  </Anchor>
                </Table.Td>
              )}
              <Table.Td>
                <Text size="sm" fw={500}>
                  {formatChapterVolume(entry)}
                </Text>
              </Table.Td>
              <Table.Td>
                <Stack gap={2}>
                  {entry.groupOrUploader &&
                    entry.groupOrUploader !== sourceLabel && (
                      <Text size="sm">{entry.groupOrUploader}</Text>
                    )}
                  <Text size="sm" fw={500}>
                    {sourceLabel}
                  </Text>
                </Stack>
              </Table.Td>
              <Table.Td>
                <Text size="sm">{entry.language ?? "—"}</Text>
              </Table.Td>
              <Table.Td>
                <Badge color={stateInfo.color} variant="light" size="sm">
                  {stateInfo.label}
                </Badge>
              </Table.Td>
              <Table.Td>
                <Text size="xs" c="dimmed">
                  {format(new Date(entry.observedAt), "yyyy-MM-dd")}
                </Text>
              </Table.Td>
              <Table.Td>
                <Group gap={4} justify="flex-end" wrap="nowrap">
                  <Tooltip label="Open payload URL">
                    <ActionIcon
                      component="a"
                      href={entry.payloadUrl}
                      target="_blank"
                      rel="noopener noreferrer"
                      variant="subtle"
                      size="sm"
                      aria-label="Open payload URL"
                    >
                      <IconExternalLink size={16} />
                    </ActionIcon>
                  </Tooltip>
                  {entry.mediaUrl && (
                    <MediaUrlIcon
                      url={entry.mediaUrl}
                      kind={entry.mediaUrlKind}
                    />
                  )}
                  {entry.state === "announced" ? (
                    <>
                      <Tooltip label="Mark acquired">
                        <ActionIcon
                          variant="subtle"
                          size="sm"
                          color="green"
                          loading={isMarkAcquiredPending}
                          onClick={() => onMarkAcquired(entry.id)}
                          aria-label="Mark acquired"
                        >
                          <IconCheck size={16} />
                        </ActionIcon>
                      </Tooltip>
                      <Tooltip label="Dismiss">
                        <ActionIcon
                          variant="subtle"
                          size="sm"
                          color="gray"
                          loading={isDismissPending}
                          onClick={() => onDismiss(entry.id)}
                          aria-label="Dismiss"
                        >
                          <IconX size={16} />
                        </ActionIcon>
                      </Tooltip>
                      <Tooltip label="Ignore">
                        <ActionIcon
                          variant="subtle"
                          size="sm"
                          color="gray"
                          loading={isIgnorePending}
                          onClick={() => onIgnore(entry.id)}
                          aria-label="Ignore"
                        >
                          <IconEyeOff size={16} />
                        </ActionIcon>
                      </Tooltip>
                    </>
                  ) : (
                    <Tooltip label="Reset to new">
                      <ActionIcon
                        variant="subtle"
                        size="sm"
                        color="blue"
                        loading={isResetPending}
                        onClick={() => onReset(entry.id)}
                        aria-label="Reset"
                      >
                        <IconRefresh size={16} />
                      </ActionIcon>
                    </Tooltip>
                  )}
                  <Tooltip label="Delete (will reappear on next poll)">
                    <ActionIcon
                      variant="subtle"
                      size="sm"
                      color="red"
                      loading={isDeletePending}
                      onClick={() => onDelete(entry.id)}
                      aria-label="Delete"
                    >
                      <IconTrash size={16} />
                    </ActionIcon>
                  </Tooltip>
                </Group>
              </Table.Td>
            </Table.Tr>
          );
        })}
      </Table.Tbody>
    </Table>
  );
}
