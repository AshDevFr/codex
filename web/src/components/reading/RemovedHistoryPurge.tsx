/**
 * The way out of the "removed from library" row.
 *
 * Reading of a book that has since been deleted from the server keeps counting
 * towards every statistic. This lets the reader decide it should not, and says
 * exactly what that costs before doing it.
 *
 * The figures in the dialog come from their own request rather than from the
 * row: the row covers only the dates on screen, while a purge removes every
 * such session regardless of date, so the row would understate what is lost.
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
import { IconAlertTriangle } from "@tabler/icons-react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { readingStatsApi } from "@/api/readingStats";
import { formatDuration } from "./readingStatsFormat";

function plural(count: number, one: string, many: string): string {
  return `${count} ${count === 1 ? one : many}`;
}

export function RemovedHistoryPurge() {
  const [opened, setOpened] = useState(false);
  const queryClient = useQueryClient();

  const totals = useQuery({
    queryKey: ["readingStats", "orphaned"],
    queryFn: () => readingStatsApi.orphaned(),
    enabled: opened,
    // Always re-read on open: the dialog's whole job is to be accurate.
    staleTime: 0,
  });

  const purge = useMutation({
    mutationFn: () => readingStatsApi.purgeOrphaned(),
    onSuccess: (result) => {
      setOpened(false);
      // Every total on the page included this history, and coverage may have
      // moved too, so everything under the prefix is stale.
      queryClient.invalidateQueries({ queryKey: ["readingStats"] });
      notifications.show({
        title: "Removed history deleted",
        message: `${plural(result.sessionsRemoved, "sitting", "sittings")} and ${plural(
          result.completionsRemoved,
          "finished read",
          "finished reads",
        )} no longer count.`,
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
  const nothingToDelete = data !== undefined && data.sessions === 0;

  return (
    <>
      <Button
        variant="subtle"
        color="red"
        size="compact-xs"
        onClick={() => setOpened(true)}
      >
        Delete
      </Button>
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
          {totals.isLoading && (
            <Group justify="center">
              <Loader size="sm" />
            </Group>
          )}
          {totals.error && (
            <Alert color="red" icon={<IconAlertTriangle size={16} />}>
              Could not total the removed history, so nothing can be deleted
              right now.
            </Alert>
          )}
          {data && !nothingToDelete && (
            <Alert color="red" icon={<IconAlertTriangle size={16} />}>
              This permanently deletes{" "}
              {plural(data.sessions, "sitting", "sittings")} (
              {formatDuration(data.duration.totalMs)} of reading) and{" "}
              {plural(data.completions, "finished read", "finished reads")},
              across all dates, not only the ones shown. It cannot be undone.
            </Alert>
          )}
          {nothingToDelete && (
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
              disabled={!data || nothingToDelete}
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
