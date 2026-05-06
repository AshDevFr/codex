import {
  ActionIcon,
  Anchor,
  Badge,
  Checkbox,
  Group,
  Stack,
  Table,
  Text,
  Tooltip,
} from "@mantine/core";
import {
  IconCheck,
  IconExternalLink,
  IconTrash,
  IconX,
} from "@tabler/icons-react";
import { format } from "date-fns";
import { Link } from "react-router-dom";
import type { ReleaseLedgerEntry, ReleaseSource } from "@/api/releases";
import { MediaUrlIcon } from "./MediaUrlIcon";

const STATE_BADGE: Record<string, { color: string; label: string }> = {
  announced: { color: "blue", label: "New" },
  marked_acquired: { color: "green", label: "Acquired" },
  dismissed: { color: "gray", label: "Dismissed" },
  hidden: { color: "gray", label: "Hidden" },
};

interface ReleasesTableProps {
  entries: ReleaseLedgerEntry[];
  sourceById: Map<string, ReleaseSource>;
  selected: Set<string>;
  onToggleOne: (id: string) => void;
  onToggleAll: () => void;
  onDismiss: (id: string) => void;
  onMarkAcquired: (id: string) => void;
  onDelete: (id: string) => void;
  /** When true, render a Series column linking to the series detail page.
   *  Off when the table is already scoped to a single series. */
  showSeriesColumn?: boolean;
  /** Disable per-row action buttons while a mutation is in flight. */
  isDismissPending?: boolean;
  isMarkAcquiredPending?: boolean;
  isDeletePending?: boolean;
  /** Visual density. The page-level inbox uses "sm"; the embedded panel
   *  uses "xs" so it doesn't dominate the surrounding card. */
  verticalSpacing?: "xs" | "sm";
}

function formatChapterVolume(entry: ReleaseLedgerEntry): string {
  const hasChapter = entry.chapter !== null && entry.chapter !== undefined;
  const hasVolume = entry.volume !== null && entry.volume !== undefined;
  if (!hasChapter && !hasVolume) return "—";
  const chapter = hasChapter ? `Ch ${entry.chapter}` : "";
  const volume = hasVolume
    ? hasChapter
      ? ` · Vol ${entry.volume}`
      : `Vol ${entry.volume}`
    : "";
  return `${chapter}${volume}`;
}

export function ReleasesTable({
  entries,
  sourceById,
  selected,
  onToggleOne,
  onToggleAll,
  onDismiss,
  onMarkAcquired,
  onDelete,
  showSeriesColumn = false,
  isDismissPending = false,
  isMarkAcquiredPending = false,
  isDeletePending = false,
  verticalSpacing = "sm",
}: ReleasesTableProps) {
  const allSelected =
    entries.length > 0 && entries.every((e) => selected.has(e.id));
  const someSelected = entries.some((e) => selected.has(e.id)) && !allSelected;

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
              <Table.Td>
                <Checkbox
                  aria-label={`Select release ${entry.id}`}
                  checked={isSelected}
                  onChange={() => onToggleOne(entry.id)}
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
                  {entry.state === "announced" && (
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
                    </>
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
