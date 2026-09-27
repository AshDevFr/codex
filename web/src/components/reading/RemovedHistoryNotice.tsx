/**
 * Reading of books since deleted from the server, and the way to discard it.
 *
 * That reading keeps counting towards every statistic. This notice says so and
 * lets the reader decide it should not, stating exactly what that costs first.
 *
 * It is driven by its own unwindowed request rather than by the "Removed from
 * library" row in the series panel. That row only exists when the reading falls
 * inside the dates on screen, ranks in the top few, and is non-zero for the
 * chosen metric, so a reader could hold removed history with no row, and no
 * control, anywhere on the page. A purge is not windowed either, so the row's
 * figures would understate what is lost.
 */

import {
  Alert,
  Button,
  Group,
  Loader,
  Modal,
  Stack,
  Text,
} from "@mantine/core";
import { notifications } from "@mantine/notifications";
import { IconAlertTriangle, IconInfoCircle } from "@tabler/icons-react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { type OrphanedHistory, readingStatsApi } from "@/api/readingStats";
import { formatDuration } from "./readingStatsFormat";

const ORPHANED_KEY = ["readingStats", "orphaned"] as const;

function plural(count: number, one: string, many: string): string {
  return `${count} ${count === 1 ? one : many}`;
}

function isEmpty(totals: OrphanedHistory): boolean {
  return totals.sessions === 0 && totals.completions === 0;
}

/** What the totals amount to, in one clause. */
function describe(totals: OrphanedHistory): string {
  const parts = [];
  if (totals.sessions > 0) {
    parts.push(
      `${plural(totals.sessions, "sitting", "sittings")} (${formatDuration(totals.duration.totalMs)} of reading)`,
    );
  }
  if (totals.completions > 0) {
    parts.push(plural(totals.completions, "finished read", "finished reads"));
  }
  return parts.join(" and ");
}

export function RemovedHistoryNotice() {
  const [opened, setOpened] = useState(false);
  const queryClient = useQueryClient();

  const totals = useQuery({
    queryKey: ORPHANED_KEY,
    queryFn: () => readingStatsApi.orphaned(),
    staleTime: 60_000,
  });

  const purge = useMutation({
    mutationFn: () => readingStatsApi.purgeOrphaned(),
    onSuccess: () => {
      setOpened(false);
      // Every total on the page included this history, and coverage may have
      // moved too, so everything under the prefix is stale.
      queryClient.invalidateQueries({ queryKey: ["readingStats"] });
      notifications.show({
        title: "Removed history deleted",
        message: "Reading of books no longer on the server no longer counts.",
        color: "green",
      });
    },
    onError: () => {
      notifications.show({
        title: "Could not delete removed history",
        message: "Nothing was deleted. Try again in a moment.",
        color: "red",
      });
    },
  });

  const data = totals.data;
  // Stays mounted while the dialog is open, so a refetch that finds nothing
  // left can say so instead of the dialog vanishing under the reader.
  if (!data || (isEmpty(data) && !opened)) return null;

  const open = () => {
    setOpened(true);
    // The dialog's whole job is to be accurate, so it never confirms against
    // figures fetched before it opened.
    totals.refetch();
  };

  // Settled means the figures on screen are the ones just fetched.
  const settled = !totals.isFetching && !totals.isError;
  const nothingLeft = settled && isEmpty(data);

  return (
    <>
      {!isEmpty(data) && (
        <Alert
          variant="light"
          color="gray"
          icon={<IconInfoCircle size={16} />}
          title="Reading of removed books"
        >
          <Group justify="space-between" wrap="wrap" gap="sm">
            <Text size="sm">
              {describe(data)} from books since deleted from the server still
              count in your statistics, shown as "Removed from library".
            </Text>
            <Button
              variant="subtle"
              color="red"
              size="compact-sm"
              aria-label="Delete reading of removed books"
              onClick={open}
            >
              Delete
            </Button>
          </Group>
        </Alert>
      )}
      <Modal
        opened={opened}
        onClose={() => setOpened(false)}
        title="Delete reading of removed books?"
        centered
      >
        <Stack gap="md">
          <Text size="sm">
            These books have been deleted from the server. Their reading still
            counts in your statistics, but which book or series it came from is
            no longer known.
          </Text>
          {totals.isFetching && (
            <Group justify="center">
              <Loader size="sm" />
            </Group>
          )}
          {totals.isError && !totals.isFetching && (
            <Alert color="red" icon={<IconAlertTriangle size={16} />}>
              Could not total the removed history, so nothing can be deleted
              right now.
            </Alert>
          )}
          {settled && !nothingLeft && (
            <Alert color="red" icon={<IconAlertTriangle size={16} />}>
              This permanently deletes {describe(data)}, across all dates, not
              only the ones shown. It cannot be undone.
            </Alert>
          )}
          {settled && !nothingLeft && (
            <Text size="sm" c="dimmed">
              If you have a reading progress export taken before these books
              were removed, importing it puts this reading back on its books
              instead.
            </Text>
          )}
          {nothingLeft && (
            <Text size="sm" c="dimmed">
              There is no removed history left to delete.
            </Text>
          )}
          <Group justify="flex-end">
            <Button variant="default" onClick={() => setOpened(false)}>
              Cancel
            </Button>
            <Button
              color="red"
              disabled={!settled || nothingLeft}
              loading={purge.isPending}
              onClick={() => purge.mutate()}
            >
              Delete permanently
            </Button>
          </Group>
        </Stack>
      </Modal>
    </>
  );
}
