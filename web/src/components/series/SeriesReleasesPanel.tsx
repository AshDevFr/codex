import {
  ActionIcon,
  Anchor,
  Badge,
  Box,
  Card,
  Collapse,
  Group,
  Loader,
  Stack,
  Text,
  Tooltip,
} from "@mantine/core";
import { useDisclosure } from "@mantine/hooks";
import {
  IconBellOff,
  IconBellRinging,
  IconChevronDown,
  IconChevronRight,
  IconRss,
} from "@tabler/icons-react";
import { useEffect, useMemo, useState } from "react";
import type { BulkReleaseAction, ReleaseSource } from "@/api/releases";
import { ReleasesBulkActionBar } from "@/components/releases/ReleasesBulkActionBar";
import { ReleasesBulkDeleteModal } from "@/components/releases/ReleasesBulkDeleteModal";
import { ReleasesTable } from "@/components/releases/ReleasesTable";
import {
  useBulkReleaseAction,
  useDeleteRelease,
  useDismissRelease,
  useMarkReleaseAcquired,
  useReleaseSources,
  useSeriesReleases,
} from "@/hooks/useReleases";
import { useUserPreference } from "@/hooks/useUserPreference";

interface SeriesReleasesPanelProps {
  seriesId: string;
}

export function SeriesReleasesPanel({ seriesId }: SeriesReleasesPanelProps) {
  const [showDismissed, setShowDismissed] = useState(false);
  // Releases panel collapses by default — series detail is the user's main
  // landing point and the panel can grow long. They open it deliberately.
  const [opened, { toggle }] = useDisclosure(false);
  const stateFilter = showDismissed ? undefined : "announced";

  // Per-user mute. Persisted via the user_preferences store with localStorage
  // caching + debounced server sync.
  const [mutedSeriesIds, setMutedSeriesIds] = useUserPreference(
    "release_tracking.muted_series_ids",
  );
  const isMuted = mutedSeriesIds.includes(seriesId);
  const toggleMute = () => {
    if (isMuted) {
      setMutedSeriesIds(mutedSeriesIds.filter((id) => id !== seriesId));
    } else {
      setMutedSeriesIds([...mutedSeriesIds, seriesId]);
    }
  };

  const { data, isLoading } = useSeriesReleases(seriesId, {
    state: stateFilter,
    pageSize: 100,
  });
  const { data: sources } = useReleaseSources();
  const dismiss = useDismissRelease();
  const markAcquired = useMarkReleaseAcquired();
  const deleteRelease = useDeleteRelease();
  const bulk = useBulkReleaseAction();

  const entries = data?.data ?? [];
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [confirmBulkDelete, { open: openBulkDelete, close: closeBulkDelete }] =
    useDisclosure(false);
  // Drop selections when the visible set changes — IDs that fell off screen
  // shouldn't quietly remain selected for the next bulk action.
  // biome-ignore lint/correctness/useExhaustiveDependencies: deps are change-triggers
  useEffect(() => {
    setSelected(new Set());
  }, [showDismissed, seriesId]);
  const toggleAll = () => {
    setSelected((prev) => {
      const allSelected =
        entries.length > 0 && entries.every((e) => prev.has(e.id));
      const next = new Set(prev);
      if (allSelected) {
        for (const e of entries) next.delete(e.id);
      } else {
        for (const e of entries) next.add(e.id);
      }
      return next;
    });
  };
  const toggleOne = (id: string) => {
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  };
  const runBulk = (action: BulkReleaseAction) => {
    const ids = Array.from(selected);
    if (ids.length === 0) return;
    bulk.mutate({ ids, action }, { onSuccess: () => setSelected(new Set()) });
  };

  // Same client-side join the inbox uses: keep the ledger DTO lean while
  // showing a human label instead of a UUID prefix.
  const sourceById = useMemo(() => {
    const map = new Map<string, ReleaseSource>();
    for (const s of sources ?? []) map.set(s.id, s);
    return map;
  }, [sources]);

  if (isLoading) {
    return (
      <Card withBorder padding="md" radius="md">
        <Group>
          <Loader size="sm" />
          <Text size="sm">Loading releases…</Text>
        </Group>
      </Card>
    );
  }

  return (
    <>
      <Card withBorder padding="md" radius="md">
        <Stack gap="sm">
          <Group justify="space-between" wrap="nowrap" id="releases">
            <Group
              gap="xs"
              onClick={toggle}
              style={{ cursor: "pointer", flex: 1, minWidth: 0 }}
              role="button"
              aria-expanded={opened}
              aria-label={opened ? "Collapse releases" : "Expand releases"}
            >
              {opened ? (
                <IconChevronDown size={16} />
              ) : (
                <IconChevronRight size={16} />
              )}
              <IconRss size={18} />
              <Text fw={600}>Releases</Text>
              <Badge color="gray" variant="light" size="sm">
                {data?.total ?? 0}
              </Badge>
              {isMuted && (
                <Badge color="orange" variant="light" size="sm">
                  Muted
                </Badge>
              )}
            </Group>
            <Group gap="xs">
              <Tooltip
                label={
                  isMuted
                    ? "Re-enable announcement toasts and badge for this series"
                    : "Stop announcement toasts and badge for this series (your account only)"
                }
              >
                <ActionIcon
                  variant="subtle"
                  color={isMuted ? "orange" : "gray"}
                  onClick={toggleMute}
                  aria-label={isMuted ? "Unmute releases" : "Mute releases"}
                >
                  {isMuted ? (
                    <IconBellOff size={16} />
                  ) : (
                    <IconBellRinging size={16} />
                  )}
                </ActionIcon>
              </Tooltip>
              {opened && (
                <Anchor
                  component="button"
                  type="button"
                  size="sm"
                  onClick={() => setShowDismissed((prev) => !prev)}
                >
                  {showDismissed ? "Hide dismissed" : "Show all states"}
                </Anchor>
              )}
            </Group>
          </Group>

          <Collapse in={opened}>
            {selected.size > 0 && (
              <Box mb="xs">
                <ReleasesBulkActionBar
                  count={selected.size}
                  isPending={bulk.isPending}
                  onAction={runBulk}
                  onClear={() => setSelected(new Set())}
                  onDeleteClick={openBulkDelete}
                />
              </Box>
            )}
            {entries.length === 0 ? (
              <Text size="sm" c="dimmed">
                No releases yet. Once a release source picks this series up, new
                chapters/volumes will land here.
              </Text>
            ) : (
              <ReleasesTable
                entries={entries}
                sourceById={sourceById}
                selected={selected}
                onToggleOne={toggleOne}
                onToggleAll={toggleAll}
                onDismiss={(id) => dismiss.mutate(id)}
                onMarkAcquired={(id) => markAcquired.mutate(id)}
                onDelete={(id) => deleteRelease.mutate(id)}
                isDismissPending={dismiss.isPending}
                isMarkAcquiredPending={markAcquired.isPending}
                isDeletePending={deleteRelease.isPending}
                verticalSpacing="xs"
              />
            )}
          </Collapse>
        </Stack>
      </Card>
      <ReleasesBulkDeleteModal
        opened={confirmBulkDelete}
        onClose={closeBulkDelete}
        onConfirm={() => {
          runBulk("delete");
          closeBulkDelete();
        }}
        count={selected.size}
        isPending={bulk.isPending}
      />
    </>
  );
}
