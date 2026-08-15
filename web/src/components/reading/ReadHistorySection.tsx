import {
  ActionIcon,
  Alert,
  Button,
  Collapse,
  Group,
  List,
  Modal,
  Stack,
  Text,
  Tooltip,
} from "@mantine/core";
import {
  IconChevronDown,
  IconChevronRight,
  IconHistory,
  IconTrash,
  IconX,
} from "@tabler/icons-react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { type ReadHistoryResponse, readProgressApi } from "@/api/readProgress";

/** What the section is describing, which changes the wording throughout. */
export type ReadHistoryScope = "book" | "series";

export interface ReadHistorySectionProps {
  scope: ReadHistoryScope;
  /** Book id or series id, depending on `scope`. */
  id: string;
}

/** Query key for one entity's history, exported so callers can invalidate it. */
export function readHistoryQueryKey(scope: ReadHistoryScope, id: string) {
  return ["read-history", scope, id] as const;
}

/**
 * Render a completion date in the viewer's locale. The API returns UTC.
 */
function formatDate(iso: string): string {
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return iso;
  return date.toLocaleDateString(undefined, {
    year: "numeric",
    month: "short",
    day: "numeric",
  });
}

/** "Completed 2 times, last on 5 Mar 2025" */
function summary(
  history: ReadHistoryResponse,
  scope: ReadHistoryScope,
): string {
  const noun = scope === "book" ? "book" : "series";
  const times = history.readCount === 1 ? "once" : `${history.readCount} times`;
  const last = history.lastCompletedAt
    ? `, last on ${formatDate(history.lastCompletedAt)}`
    : "";
  return `You've finished this ${noun} ${times}${last}`;
}

/**
 * Completion history for a book or series, with a reset control.
 *
 * Renders nothing until the history is loaded and non-empty: a reader who has
 * never finished the thing has no history to reason about, and an empty section
 * would just be noise on every unread book.
 */
export function ReadHistorySection({ scope, id }: ReadHistorySectionProps) {
  const queryClient = useQueryClient();
  const [expanded, setExpanded] = useState(false);
  const [confirmOpen, setConfirmOpen] = useState(false);

  const { data: history } = useQuery({
    queryKey: readHistoryQueryKey(scope, id),
    queryFn: () =>
      scope === "book"
        ? readProgressApi.getBookHistory(id)
        : readProgressApi.getSeriesHistory(id),
    enabled: Boolean(id),
  });

  /**
   * Remove one entry.
   *
   * Only offered where an entry is a real row, which is the book scope. A
   * series entry is an aggregate over several books, so there is nothing
   * single to delete and the API sends no id for it.
   */
  const deleteEntryMutation = useMutation({
    mutationFn: (completionId: string) =>
      readProgressApi.deleteBookHistoryEntry(id, completionId),
    onSuccess: () => {
      queryClient.invalidateQueries({
        queryKey: readHistoryQueryKey(scope, id),
      });
      queryClient.invalidateQueries({ queryKey: ["book", id] });
      // A series' count is recomputed from its books, so any series view of
      // this book is stale too.
      queryClient.invalidateQueries({ queryKey: ["read-history", "series"] });
      queryClient.invalidateQueries({ queryKey: ["series"] });
    },
  });

  const clearMutation = useMutation({
    mutationFn: () =>
      scope === "book"
        ? readProgressApi.clearBookHistory(id)
        : readProgressApi.clearSeriesHistory(id),
    onSuccess: () => {
      queryClient.invalidateQueries({
        queryKey: readHistoryQueryKey(scope, id),
      });
      // The detail DTOs carry readCount, so their queries are stale too.
      queryClient.invalidateQueries({
        queryKey: [scope === "book" ? "book" : "series", id],
      });
      setConfirmOpen(false);
      setExpanded(false);
    },
  });

  // Nothing finished yet: no section at all.
  if (!history || history.readCount === 0) {
    return null;
  }

  const entryNoun = scope === "book" ? "read-through" : "full read-through";

  return (
    <>
      <Stack gap={6}>
        <Group gap="xs" wrap="nowrap" justify="space-between">
          <Group gap={6} wrap="nowrap" style={{ minWidth: 0 }}>
            <ActionIcon
              variant="subtle"
              size="sm"
              onClick={() => setExpanded((value) => !value)}
              aria-label={expanded ? "Hide read history" : "Show read history"}
            >
              {expanded ? (
                <IconChevronDown size={14} />
              ) : (
                <IconChevronRight size={14} />
              )}
            </ActionIcon>
            <IconHistory size={14} />
            <Text size="sm">{summary(history, scope)}</Text>
          </Group>
          <Tooltip label="Clear read history">
            <ActionIcon
              variant="subtle"
              color="red"
              size="sm"
              onClick={() => setConfirmOpen(true)}
              aria-label="Clear read history"
            >
              <IconTrash size={14} />
            </ActionIcon>
          </Tooltip>
        </Group>

        <Collapse in={expanded}>
          <List size="sm" spacing={2} pl={30} withPadding>
            {history.entries.map((entry, index) => (
              <List.Item
                key={
                  entry.id ?? `${entry.completedAt}-${entry.startedAt}-${index}`
                }
              >
                <Group gap={6} wrap="nowrap">
                  <Text size="sm">
                    {formatDate(entry.completedAt)}
                    <Text span size="xs" c="dimmed">
                      {" "}
                      (started {formatDate(entry.startedAt)})
                    </Text>
                  </Text>
                  {entry.id && (
                    <Tooltip label="Remove this read-through">
                      <ActionIcon
                        variant="subtle"
                        color="red"
                        size="xs"
                        aria-label={`Remove the read-through finished on ${formatDate(entry.completedAt)}`}
                        loading={
                          deleteEntryMutation.isPending &&
                          deleteEntryMutation.variables === entry.id
                        }
                        onClick={() =>
                          deleteEntryMutation.mutate(entry.id as string)
                        }
                      >
                        <IconX size={12} />
                      </ActionIcon>
                    </Tooltip>
                  )}
                </Group>
              </List.Item>
            ))}
          </List>
          {history.entries.length === 0 && (
            <Text size="xs" c="dimmed" pl={30}>
              No dated entries to show.
            </Text>
          )}
        </Collapse>
      </Stack>

      <Modal
        opened={confirmOpen}
        onClose={() => setConfirmOpen(false)}
        title="Clear read history"
        centered
      >
        <Stack gap="md">
          <Text size="sm">
            Forget that you have finished this {scope}{" "}
            {history.readCount === 1 ? "once" : `${history.readCount} times`}?
            The {entryNoun} dates will be deleted.
          </Text>
          <Alert variant="light" color="blue">
            Your current reading progress is not affected. This only clears the
            record of past completions.
          </Alert>
          <Group justify="flex-end">
            <Button variant="subtle" onClick={() => setConfirmOpen(false)}>
              Cancel
            </Button>
            <Button
              color="red"
              loading={clearMutation.isPending}
              onClick={() => clearMutation.mutate()}
            >
              Clear history
            </Button>
          </Group>
        </Stack>
      </Modal>
    </>
  );
}
