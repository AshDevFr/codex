import {
  ActionIcon,
  Anchor,
  Badge,
  Box,
  Button,
  Card,
  Group,
  Loader,
  MultiSelect,
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
  IconRestore,
  IconTrash,
} from "@tabler/icons-react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { CronExpressionParser } from "cron-parser";
import { toString as cronToString } from "cronstrue";
import { formatDistanceToNow } from "date-fns";
import { type Dispatch, type SetStateAction, useMemo, useState } from "react";
import { pluginsApi } from "@/api/plugins";
import type { ReleaseSource } from "@/api/releases";
import { settingsApi } from "@/api/settings";
import { CronInput } from "@/components/forms/CronInput";
import {
  usePollReleaseSourceNow,
  useReleaseSources,
  useResetReleaseSource,
  useUpdateReleaseSource,
} from "@/hooks/useReleases";
import { useUserPreference } from "@/hooks/useUserPreference";

const SETTING_NOTIFY_LANGUAGES = "release_tracking.notify_languages";
const SETTING_NOTIFY_PLUGINS = "release_tracking.notify_plugins";
const SETTING_DEFAULT_CRON_SCHEDULE = "release_tracking.default_cron_schedule";
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

/**
 * Render a cron expression as a human-readable phrase. Mirrors the logic in
 * `<CronInput>` (5-part → cronstrue normalization). Returns the raw expression
 * as a fallback if parsing fails so we still show *something* meaningful.
 */
function describeCron(expression: string): string {
  const trimmed = expression.trim();
  if (!trimmed) return "";
  try {
    CronExpressionParser.parse(trimmed);
    const parts = trimmed.split(/\s+/);
    const normalized =
      parts.length === 5
        ? parts.map((p) => (p.startsWith("/") ? `*${p}` : p)).join(" ")
        : trimmed;
    return cronToString(normalized, {
      throwExceptionOnParseError: false,
      verbose: false,
    });
  } catch {
    return trimmed;
  }
}

export function ReleaseTrackingSettings() {
  const sourcesQuery = useReleaseSources();
  const update = useUpdateReleaseSource();
  const pollNow = usePollReleaseSourceNow();
  const reset = useResetReleaseSource();

  // The mutation hooks expose a single shared `isPending` flag, which would
  // light up the spinner on every row whenever any one row's request was in
  // flight. Track in-flight `sourceId`s explicitly so each row's spinner
  // reflects only that row's own request, even when multiple are pending
  // concurrently.
  const [pollingIds, setPollingIds] = useState<ReadonlySet<string>>(new Set());
  const [resettingIds, setResettingIds] = useState<ReadonlySet<string>>(
    new Set(),
  );

  const addId = (
    setter: Dispatch<SetStateAction<ReadonlySet<string>>>,
    id: string,
  ) => setter((prev) => new Set(prev).add(id));
  const removeId = (
    setter: Dispatch<SetStateAction<ReadonlySet<string>>>,
    id: string,
  ) =>
    setter((prev) => {
      const next = new Set(prev);
      next.delete(id);
      return next;
    });

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

        <DefaultScheduleCard />

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
                    onCronScheduleChange={(cronSchedule) =>
                      update.mutate({
                        sourceId: source.id,
                        // Send `null` to clear the override and revert to
                        // inheriting the server-wide default.
                        update: { cronSchedule },
                      })
                    }
                    onPollNow={() => {
                      addId(setPollingIds, source.id);
                      pollNow.mutate(source.id, {
                        onSettled: () => removeId(setPollingIds, source.id),
                      });
                    }}
                    pollNowPending={pollingIds.has(source.id)}
                    onReset={() => {
                      if (
                        window.confirm(
                          `Reset "${source.displayName}"?\n\nThis deletes every release ledger row for this source and clears its poll state (etag, last poll time). User-managed settings (enabled, interval, name) are preserved. The next poll will re-record everything as new.\n\nThis cannot be undone.`,
                        )
                      ) {
                        addId(setResettingIds, source.id);
                        reset.mutate(source.id, {
                          onSettled: () => removeId(setResettingIds, source.id),
                        });
                      }
                    }}
                    resetPending={resettingIds.has(source.id)}
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

/**
 * Server-wide default cron schedule for release-source polling. Each
 * `release_sources` row whose `cron_schedule` is NULL inherits this value.
 * The compile-time fallback (`"0 0 * * *"`) only applies if the setting row
 * itself is missing.
 */
function DefaultScheduleCard() {
  const queryClient = useQueryClient();
  const settingQuery = useQuery({
    queryKey: ["admin-setting", SETTING_DEFAULT_CRON_SCHEDULE],
    queryFn: () => settingsApi.get(SETTING_DEFAULT_CRON_SCHEDULE),
  });

  const serverValue = settingQuery.data?.value ?? "";
  const [draft, setDraft] = useState<string>(serverValue);
  // Sync local draft when the server value changes (initial load, refetch).
  // We deliberately don't useEffect: comparing the string each render is
  // cheap, and we only update when the upstream value actually changes.
  if (draft === "" && serverValue !== "" && !settingQuery.isFetching) {
    setDraft(serverValue);
  }

  const updateMutation = useMutation({
    mutationFn: (value: string) =>
      settingsApi.update(SETTING_DEFAULT_CRON_SCHEDULE, { value }),
    onSuccess: () => {
      queryClient.invalidateQueries({
        queryKey: ["admin-setting", SETTING_DEFAULT_CRON_SCHEDULE],
      });
      // Source rows display `effectiveCronSchedule` resolved server-side,
      // so a default change must invalidate the source list to refresh
      // every inheriting row's "(Default)" label.
      queryClient.invalidateQueries({ queryKey: ["release-sources"] });
      notifications.show({
        title: "Default schedule saved",
        message:
          "All sources without a per-row override will use the new schedule.",
        color: "green",
      });
    },
    onError: (err: Error) =>
      notifications.show({
        title: "Failed to save",
        message: err.message ?? "Could not update default schedule.",
        color: "red",
      }),
  });

  const commit = () => {
    const trimmed = draft.trim();
    if (!trimmed || trimmed === serverValue) {
      setDraft(serverValue);
      return;
    }
    updateMutation.mutate(trimmed);
  };

  return (
    <Card withBorder padding="md" radius="md">
      <Stack gap="sm">
        <Group gap="xs">
          <IconClockHour4 size={18} />
          <Text fw={600}>Default schedule</Text>
        </Group>
        <Text size="xs" c="dimmed">
          Server-wide default cron used by every release source that doesn't
          have its own per-row override. Changing this propagates immediately to
          inheriting rows.
        </Text>
        <CronInput
          label="Cron expression"
          description="5-field POSIX cron (minute hour day-of-month month day-of-week)"
          placeholder="0 0 * * *"
          value={draft}
          onChange={setDraft}
          onBlur={commit}
          disabled={settingQuery.isLoading || updateMutation.isPending}
          required
        />
      </Stack>
    </Card>
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
  /** `null` clears the override and reverts to the server-wide default. */
  onCronScheduleChange: (cronSchedule: string | null) => void;
  onPollNow: () => void;
  pollNowPending: boolean;
  onReset: () => void;
  resetPending: boolean;
}

function ReleaseSourceRow({
  source,
  onToggle,
  onCronScheduleChange,
  onPollNow,
  pollNowPending,
  onReset,
  resetPending,
}: RowProps) {
  // Truthy `cronSchedule` means the row has a per-source override; render the
  // editor inline. The server omits the field entirely (rather than sending
  // `null`) when the row is inheriting, so accept both `null` and `undefined`
  // as "no override."
  const hasOverride = Boolean(source.cronSchedule);
  const [isOverriding, setIsOverriding] = useState(hasOverride);
  const [draft, setDraft] = useState<string>(
    source.cronSchedule || source.effectiveCronSchedule,
  );

  const lastPolled = source.lastPolledAt
    ? formatDistanceToNow(new Date(source.lastPolledAt), { addSuffix: true })
    : "—";

  const commitDraft = () => {
    const trimmed = draft.trim();
    if (!trimmed) {
      // Empty editor = revert to inherit.
      if (source.cronSchedule) onCronScheduleChange(null);
      setIsOverriding(false);
      setDraft(source.effectiveCronSchedule);
      return;
    }
    if (trimmed !== source.cronSchedule) {
      onCronScheduleChange(trimmed);
    }
  };

  const resetToDefault = () => {
    if (source.cronSchedule) onCronScheduleChange(null);
    setIsOverriding(false);
    setDraft(source.effectiveCronSchedule);
  };

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
        {isOverriding ? (
          <Stack gap={4}>
            <CronInput
              value={draft}
              onChange={setDraft}
              onBlur={commitDraft}
              showNextRun={false}
              placeholder="0 0 * * *"
              aria-label="Cron schedule override"
            />
            <Anchor
              size="xs"
              component="button"
              type="button"
              onClick={resetToDefault}
            >
              Reset to default
            </Anchor>
          </Stack>
        ) : (
          <Stack gap={2}>
            <Text size="sm">
              {describeCron(source.effectiveCronSchedule)}{" "}
              <Text component="span" size="xs" c="dimmed">
                (Default)
              </Text>
            </Text>
            <Anchor
              size="xs"
              component="button"
              type="button"
              onClick={() => {
                setIsOverriding(true);
                setDraft(source.effectiveCronSchedule);
              }}
            >
              Override
            </Anchor>
          </Stack>
        )}
      </Table.Td>
      <Table.Td>
        <Stack gap={2}>
          <Text size="xs">{lastPolled}</Text>
          {source.lastSummary && (
            <Text size="xs" c="dimmed" lineClamp={2}>
              {source.lastSummary}
            </Text>
          )}
        </Stack>
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
          // Wrap the OK badge in a tooltip carrying `lastSummary` so users
          // can see *why* a poll returned nothing (no tracked series, 304,
          // dropped below threshold, etc.) without grepping logs.
          <Tooltip
            label={source.lastSummary ?? "Last poll completed successfully."}
            multiline
            w={300}
            withArrow
            position="top"
          >
            <Badge color="green" variant="light" size="sm">
              OK
            </Badge>
          </Tooltip>
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
        <Group gap={4} wrap="nowrap">
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
          <Tooltip label="Reset: drop ledger rows and clear poll state">
            <ActionIcon
              variant="subtle"
              color="red"
              onClick={onReset}
              loading={resetPending}
              aria-label="Reset source"
            >
              <IconRestore size={16} />
            </ActionIcon>
          </Tooltip>
        </Group>
      </Table.Td>
    </Table.Tr>
  );
}
