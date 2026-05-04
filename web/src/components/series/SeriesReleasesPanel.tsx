import {
  ActionIcon,
  Anchor,
  Badge,
  Card,
  Group,
  Loader,
  Stack,
  Table,
  Text,
  Tooltip,
} from "@mantine/core";
import {
  IconBellOff,
  IconBellRinging,
  IconCheck,
  IconExternalLink,
  IconRss,
  IconX,
} from "@tabler/icons-react";
import { format } from "date-fns";
import { useMemo, useState } from "react";
import type { ReleaseLedgerEntry } from "@/api/releases";
import {
  useDismissRelease,
  useMarkReleaseAcquired,
  useSeriesReleases,
} from "@/hooks/useReleases";
import { useUserPreference } from "@/hooks/useUserPreference";

interface SeriesReleasesPanelProps {
  seriesId: string;
}

const STATE_BADGE: Record<string, { color: string; label: string }> = {
  announced: { color: "blue", label: "New" },
  marked_acquired: { color: "green", label: "Acquired" },
  dismissed: { color: "gray", label: "Dismissed" },
  hidden: { color: "gray", label: "Hidden" },
};

interface GroupedKey {
  chapter: number | null | undefined;
  volume: number | null | undefined;
}

function groupKey(entry: ReleaseLedgerEntry): string {
  return `${entry.chapter ?? "_"}::${entry.volume ?? "_"}`;
}

export function SeriesReleasesPanel({ seriesId }: SeriesReleasesPanelProps) {
  const [showDismissed, setShowDismissed] = useState(false);
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
  const dismiss = useDismissRelease();
  const markAcquired = useMarkReleaseAcquired();

  const groups = useMemo(() => {
    const entries = data?.data ?? [];
    const map = new Map<
      string,
      { key: GroupedKey; entries: ReleaseLedgerEntry[] }
    >();
    for (const entry of entries) {
      const k = groupKey(entry);
      const existing = map.get(k);
      if (existing) {
        existing.entries.push(entry);
      } else {
        map.set(k, {
          key: { chapter: entry.chapter, volume: entry.volume },
          entries: [entry],
        });
      }
    }
    return Array.from(map.values());
  }, [data?.data]);

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
    <Card withBorder padding="md" radius="md">
      <Stack gap="sm">
        <Group justify="space-between" wrap="nowrap" id="releases">
          <Group gap="xs">
            <IconRss size={18} />
            <Text fw={600}>Releases</Text>
            <Badge color="gray" variant="light" size="sm">
              {data?.pagination.total ?? 0}
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
            <Anchor
              component="button"
              type="button"
              size="sm"
              onClick={() => setShowDismissed((prev) => !prev)}
            >
              {showDismissed ? "Hide dismissed" : "Show all states"}
            </Anchor>
          </Group>
        </Group>

        {groups.length === 0 ? (
          <Text size="sm" c="dimmed">
            No releases yet. Once a release source picks this series up, new
            chapters/volumes will land here.
          </Text>
        ) : (
          <Table verticalSpacing="xs" highlightOnHover>
            <Table.Thead>
              <Table.Tr>
                <Table.Th>Ch / Vol</Table.Th>
                <Table.Th>Source / Group</Table.Th>
                <Table.Th>Lang</Table.Th>
                <Table.Th>State</Table.Th>
                <Table.Th>Observed</Table.Th>
                <Table.Th aria-label="Actions" />
              </Table.Tr>
            </Table.Thead>
            <Table.Tbody>
              {groups.map(({ key, entries }) =>
                entries.map((entry, idx) => {
                  const stateInfo = STATE_BADGE[entry.state] ?? {
                    color: "gray",
                    label: entry.state,
                  };
                  const isFirst = idx === 0;
                  return (
                    <Table.Tr key={entry.id}>
                      <Table.Td>
                        {isFirst ? (
                          <Text size="sm" fw={500}>
                            {key.chapter !== null && key.chapter !== undefined
                              ? `Ch ${key.chapter}`
                              : ""}
                            {key.volume !== null && key.volume !== undefined
                              ? key.chapter !== null &&
                                key.chapter !== undefined
                                ? ` · Vol ${key.volume}`
                                : `Vol ${key.volume}`
                              : ""}
                            {!key.chapter && !key.volume ? "—" : ""}
                          </Text>
                        ) : null}
                      </Table.Td>
                      <Table.Td>
                        <Stack gap={2}>
                          {entry.groupOrUploader && (
                            <Text size="sm">{entry.groupOrUploader}</Text>
                          )}
                          <Text size="xs" c="dimmed">
                            source: {entry.sourceId.slice(0, 8)}…
                          </Text>
                        </Stack>
                      </Table.Td>
                      <Table.Td>
                        <Text size="sm">{entry.language ?? "—"}</Text>
                      </Table.Td>
                      <Table.Td>
                        <Badge
                          color={stateInfo.color}
                          variant="light"
                          size="sm"
                        >
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
                          {entry.state === "announced" && (
                            <>
                              <Tooltip label="Mark acquired">
                                <ActionIcon
                                  variant="subtle"
                                  size="sm"
                                  color="green"
                                  loading={markAcquired.isPending}
                                  onClick={() => markAcquired.mutate(entry.id)}
                                  aria-label="Mark acquired"
                                >
                                  <IconCheck size={16} />
                                </ActionIcon>
                              </Tooltip>
                              <Tooltip label="Dismiss">
                                <ActionIcon
                                  variant="subtle"
                                  size="sm"
                                  color="red"
                                  loading={dismiss.isPending}
                                  onClick={() => dismiss.mutate(entry.id)}
                                  aria-label="Dismiss"
                                >
                                  <IconX size={16} />
                                </ActionIcon>
                              </Tooltip>
                            </>
                          )}
                        </Group>
                      </Table.Td>
                    </Table.Tr>
                  );
                }),
              )}
            </Table.Tbody>
          </Table>
        )}
      </Stack>
    </Card>
  );
}
