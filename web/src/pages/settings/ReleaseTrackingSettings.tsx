import {
  ActionIcon,
  Badge,
  Box,
  Card,
  Group,
  Loader,
  NumberInput,
  Stack,
  Switch,
  Table,
  TagsInput,
  Text,
  Title,
  Tooltip,
} from "@mantine/core";
import {
  IconAlertCircle,
  IconBellRinging,
  IconClockHour4,
  IconRefresh,
} from "@tabler/icons-react";
import { formatDistanceToNow } from "date-fns";
import { useState } from "react";
import type { ReleaseSource } from "@/api/releases";
import {
  usePollReleaseSourceNow,
  useReleaseSources,
  useUpdateReleaseSource,
} from "@/hooks/useReleases";
import { useReleaseAnnouncementsStore } from "@/store/releaseAnnouncementsStore";

const PRESETS = [
  { value: 3600, label: "1h" },
  { value: 21600, label: "6h" },
  { value: 43200, label: "12h" },
  { value: 86400, label: "Daily" },
  { value: 604800, label: "Weekly" },
];

function intervalLabel(seconds: number): string {
  const preset = PRESETS.find((p) => p.value === seconds);
  if (preset) return preset.label;
  if (seconds % 3600 === 0) return `${seconds / 3600}h`;
  return `${seconds}s`;
}

export function ReleaseTrackingSettings() {
  const sourcesQuery = useReleaseSources();
  const update = useUpdateReleaseSource();
  const pollNow = usePollReleaseSourceNow();

  return (
    <Box p="md">
      <Stack gap="md">
        <Group gap="sm">
          <IconClockHour4 size={26} />
          <Title order={2}>Release tracking</Title>
        </Group>

        <Text size="sm" c="dimmed">
          Manage release sources. Each row is one logical feed exposed by a
          plugin (e.g. one Nyaa uploader or one MangaUpdates batch). Disabling a
          source pauses its scheduled polls; "Poll now" enqueues an immediate
          fetch.
        </Text>

        <NotificationPreferencesCard />

        {sourcesQuery.isLoading ? (
          <Group>
            <Loader size="sm" />
            <Text>Loading sources…</Text>
          </Group>
        ) : sourcesQuery.error ? (
          <Card withBorder padding="md">
            <Group gap="xs">
              <IconAlertCircle color="red" size={16} />
              <Text c="red" size="sm">
                Failed to load sources.
              </Text>
            </Group>
          </Card>
        ) : (sourcesQuery.data ?? []).length === 0 ? (
          <Card withBorder padding="md" radius="md">
            <Text size="sm" c="dimmed">
              No release sources configured. Install a plugin that declares the
              `release_source` capability and configure at least one source.
            </Text>
          </Card>
        ) : (
          <Card withBorder padding={0} radius="md">
            <Table verticalSpacing="sm">
              <Table.Thead>
                <Table.Tr>
                  <Table.Th>Source</Table.Th>
                  <Table.Th>Plugin</Table.Th>
                  <Table.Th>Interval</Table.Th>
                  <Table.Th>Last poll</Table.Th>
                  <Table.Th>Status</Table.Th>
                  <Table.Th>Enabled</Table.Th>
                  <Table.Th aria-label="Actions" />
                </Table.Tr>
              </Table.Thead>
              <Table.Tbody>
                {(sourcesQuery.data ?? []).map((source) => (
                  <ReleaseSourceRow
                    key={source.id}
                    source={source}
                    onToggle={(enabled) =>
                      update.mutate({
                        sourceId: source.id,
                        update: { enabled },
                      })
                    }
                    onIntervalChange={(seconds) =>
                      update.mutate({
                        sourceId: source.id,
                        update: { pollIntervalS: seconds },
                      })
                    }
                    onPollNow={() => pollNow.mutate(source.id)}
                    pollNowPending={pollNow.isPending}
                  />
                ))}
              </Table.Tbody>
            </Table>
          </Card>
        )}
      </Stack>
    </Box>
  );
}

function NotificationPreferencesCard() {
  const allowedLanguages = useReleaseAnnouncementsStore(
    (s) => s.allowedLanguages,
  );
  const allowedPlugins = useReleaseAnnouncementsStore((s) => s.allowedPlugins);
  const setAllowedLanguages = useReleaseAnnouncementsStore(
    (s) => s.setAllowedLanguages,
  );
  const setAllowedPlugins = useReleaseAnnouncementsStore(
    (s) => s.setAllowedPlugins,
  );

  return (
    <Card withBorder padding="md" radius="md">
      <Stack gap="sm">
        <Group gap="xs">
          <IconBellRinging size={18} />
          <Text fw={600}>Notification preferences</Text>
        </Group>
        <Text size="xs" c="dimmed">
          Filter announcement toasts and the Releases nav badge. Empty means "no
          filter — let everything through." Per-series mute lives on each series
          detail page.
        </Text>
        <TagsInput
          label="Languages"
          description="ISO 639-1 codes (e.g. en, es). Lower-cased automatically."
          placeholder="Add language code…"
          value={Array.from(allowedLanguages)}
          onChange={(values) => setAllowedLanguages(values)}
        />
        <TagsInput
          label="Plugin sources"
          description="Plugin IDs (e.g. release-mangaupdates, release-nyaa)."
          placeholder="Add plugin id…"
          value={Array.from(allowedPlugins)}
          onChange={(values) => setAllowedPlugins(values)}
        />
      </Stack>
    </Card>
  );
}

interface RowProps {
  source: ReleaseSource;
  onToggle: (enabled: boolean) => void;
  onIntervalChange: (seconds: number) => void;
  onPollNow: () => void;
  pollNowPending: boolean;
}

function ReleaseSourceRow({
  source,
  onToggle,
  onIntervalChange,
  onPollNow,
  pollNowPending,
}: RowProps) {
  const [draft, setDraft] = useState<number | null>(source.pollIntervalS);

  const lastPolled = source.lastPolledAt
    ? formatDistanceToNow(new Date(source.lastPolledAt), { addSuffix: true })
    : "—";

  return (
    <Table.Tr>
      <Table.Td>
        <Stack gap={2}>
          <Text size="sm" fw={500}>
            {source.displayName}
          </Text>
          <Text size="xs" c="dimmed">
            {source.sourceKey}
          </Text>
        </Stack>
      </Table.Td>
      <Table.Td>
        <Badge
          variant="light"
          color={source.pluginId === "core" ? "gray" : "blue"}
        >
          {source.pluginId}
        </Badge>
      </Table.Td>
      <Table.Td>
        <Group gap="xs" wrap="nowrap">
          <NumberInput
            value={draft ?? undefined}
            onChange={(value) => {
              if (typeof value === "number") {
                setDraft(value);
              } else if (value === "") {
                setDraft(null);
              }
            }}
            onBlur={() => {
              if (
                draft !== null &&
                draft > 0 &&
                draft !== source.pollIntervalS
              ) {
                onIntervalChange(draft);
              } else {
                setDraft(source.pollIntervalS);
              }
            }}
            min={60}
            max={604800}
            step={60}
            w={120}
            suffix=" s"
            aria-label="Poll interval seconds"
          />
          <Text size="xs" c="dimmed">
            ≈ {intervalLabel(source.pollIntervalS)}
          </Text>
        </Group>
      </Table.Td>
      <Table.Td>
        <Text size="xs">{lastPolled}</Text>
      </Table.Td>
      <Table.Td>
        {source.lastError ? (
          <Tooltip
            label={source.lastError}
            multiline
            w={300}
            withArrow
            position="top"
          >
            <Badge color="red" variant="light" size="sm">
              Errored
            </Badge>
          </Tooltip>
        ) : source.lastPolledAt ? (
          <Badge color="green" variant="light" size="sm">
            OK
          </Badge>
        ) : (
          <Badge color="gray" variant="light" size="sm">
            Never polled
          </Badge>
        )}
      </Table.Td>
      <Table.Td>
        <Switch
          checked={source.enabled}
          onChange={(event) => onToggle(event.currentTarget.checked)}
          aria-label="Enable source"
        />
      </Table.Td>
      <Table.Td>
        <Tooltip label={source.enabled ? "Poll now" : "Enable to poll"}>
          <ActionIcon
            variant="subtle"
            onClick={onPollNow}
            disabled={!source.enabled || pollNowPending}
            loading={pollNowPending}
            aria-label="Poll now"
          >
            <IconRefresh size={16} />
          </ActionIcon>
        </Tooltip>
      </Table.Td>
    </Table.Tr>
  );
}
