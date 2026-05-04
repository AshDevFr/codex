import {
  ActionIcon,
  Badge,
  Box,
  Button,
  Card,
  Group,
  Loader,
  MultiSelect,
  NumberInput,
  Stack,
  Switch,
  Table,
  TagsInput,
  Text,
  Title,
  Tooltip,
} from "@mantine/core";
import { notifications } from "@mantine/notifications";
import {
  IconAlertCircle,
  IconBellRinging,
  IconClockHour4,
  IconRefresh,
  IconTrash,
} from "@tabler/icons-react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { formatDistanceToNow } from "date-fns";
import { useMemo, useState } from "react";
import { pluginsApi } from "@/api/plugins";
import type { ReleaseSource } from "@/api/releases";
import { settingsApi } from "@/api/settings";
import {
  usePollReleaseSourceNow,
  useReleaseSources,
  useUpdateReleaseSource,
} from "@/hooks/useReleases";
import { useUserPreference } from "@/hooks/useUserPreference";

const SETTING_NOTIFY_LANGUAGES = "release_tracking.notify_languages";
const SETTING_NOTIFY_PLUGINS = "release_tracking.notify_plugins";
const PREF_MUTED_SERIES = "release_tracking.muted_series_ids";

/** Parse a settings-table JSON-array value back to a string list. */
function parseArraySetting(value: string | undefined | null): string[] {
  if (!value) return [];
  try {
    const parsed = JSON.parse(value);
    return Array.isArray(parsed)
      ? parsed.filter((v): v is string => typeof v === "string")
      : [];
  } catch {
    return [];
  }
}

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
  const queryClient = useQueryClient();

  // Server-wide notify allowlists (admin-managed, persisted in `settings`).
  const notifyLanguagesQuery = useQuery({
    queryKey: ["admin-setting", SETTING_NOTIFY_LANGUAGES],
    queryFn: () => settingsApi.get(SETTING_NOTIFY_LANGUAGES),
  });
  const notifyPluginsQuery = useQuery({
    queryKey: ["admin-setting", SETTING_NOTIFY_PLUGINS],
    queryFn: () => settingsApi.get(SETTING_NOTIFY_PLUGINS),
  });

  // Per-user mute list (persisted in user_preferences via the user-prefs
  // store, with localStorage caching + debounced server sync). Used here
  // only for the count display + "Clear all mutes" action; per-series
  // toggle lives on each series detail page.
  const [mutedSeriesIds, setMutedSeriesIds] =
    useUserPreference(PREF_MUTED_SERIES);

  // Pull every registered plugin so we can show release-source ones in the
  // dropdown. Stale entries (in the allowlist but no longer installed) keep
  // their slot in the option list so admins can see + remove them.
  const pluginsQuery = useQuery({
    queryKey: ["plugins"],
    queryFn: pluginsApi.getAll,
  });

  const allowedLanguages = useMemo(
    () => parseArraySetting(notifyLanguagesQuery.data?.value),
    [notifyLanguagesQuery.data],
  );
  const allowedPlugins = useMemo(
    () => parseArraySetting(notifyPluginsQuery.data?.value),
    [notifyPluginsQuery.data],
  );

  const pluginOptions = useMemo(() => {
    const registered = (pluginsQuery.data?.plugins ?? []).filter(
      (p) => p.manifest?.capabilities?.releaseSource === true,
    );
    const seen = new Set<string>();
    const opts: { value: string; label: string }[] = [];
    for (const p of registered) {
      seen.add(p.name);
      opts.push({
        value: p.name,
        label: p.manifest?.displayName ?? p.name,
      });
    }
    for (const id of allowedPlugins) {
      if (!seen.has(id)) {
        opts.push({ value: id, label: `${id} (not installed)` });
      }
    }
    return opts;
  }, [pluginsQuery.data, allowedPlugins]);

  // Persist a setting back to the server. Lower-cases language codes so the
  // backend filter (`shouldNotify`) doesn't need to re-normalize.
  const updateSettingMutation = useMutation({
    mutationFn: ({ key, values }: { key: string; values: string[] }) =>
      settingsApi.update(key, { value: JSON.stringify(values) }),
    onSuccess: (_data, vars) => {
      queryClient.invalidateQueries({
        queryKey: ["admin-setting", vars.key],
      });
    },
    onError: (err: Error) =>
      notifications.show({
        title: "Failed to save",
        message: err.message ?? "Could not update notification preferences.",
        color: "red",
      }),
  });

  const clearMutes = () => {
    setMutedSeriesIds([]);
    notifications.show({
      title: "Mutes cleared",
      message: "All per-series mutes have been removed.",
      color: "green",
    });
  };

  const setAllowedLanguages = (values: string[]) =>
    updateSettingMutation.mutate({
      key: SETTING_NOTIFY_LANGUAGES,
      values: values
        .map((v) => v.trim().toLowerCase())
        .filter((v) => v.length > 0),
    });
  const setAllowedPlugins = (values: string[]) =>
    updateSettingMutation.mutate({
      key: SETTING_NOTIFY_PLUGINS,
      values,
    });

  return (
    <Card withBorder padding="md" radius="md">
      <Stack gap="sm">
        <Group gap="xs">
          <IconBellRinging size={18} />
          <Text fw={600}>Notification preferences</Text>
        </Group>
        <Text size="xs" c="dimmed">
          Filter announcement toasts and the Releases nav badge. Empty means "no
          filter — let everything through." Server-wide for languages and plugin
          sources; per-series mute is per-user (toggle on each series detail
          page).
        </Text>
        <TagsInput
          label="Languages"
          description="ISO 639-1 codes (e.g. en, es). Lower-cased automatically. Server-wide."
          placeholder="Add language code…"
          value={allowedLanguages}
          onChange={setAllowedLanguages}
          disabled={notifyLanguagesQuery.isLoading}
        />
        <MultiSelect
          label="Plugin sources"
          description="Pick the release-source plugins to receive notifications from. Empty = all installed sources are allowed. Server-wide."
          placeholder={
            allowedPlugins.length === 0
              ? "All release-source plugins"
              : undefined
          }
          data={pluginOptions}
          value={allowedPlugins}
          onChange={setAllowedPlugins}
          searchable
          clearable
          nothingFoundMessage={
            pluginsQuery.isLoading
              ? "Loading plugins…"
              : "No release-source plugins installed"
          }
          disabled={notifyPluginsQuery.isLoading}
        />
        <Group justify="space-between" mt="xs" wrap="nowrap">
          <Box>
            <Text size="sm" fw={500}>
              Muted series
            </Text>
            <Text size="xs" c="dimmed">
              {mutedSeriesIds.length === 0
                ? "No series muted for your account."
                : `${mutedSeriesIds.length} series muted for your account.`}
            </Text>
          </Box>
          <Button
            size="xs"
            variant="light"
            color="red"
            leftSection={<IconTrash size={14} />}
            onClick={clearMutes}
            disabled={mutedSeriesIds.length === 0}
          >
            Clear all mutes
          </Button>
        </Group>
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
