import {
  ActionIcon,
  Anchor,
  Badge,
  Box,
  Button,
  Card,
  Collapse,
  Group,
  Loader,
  MultiSelect,
  Stack,
  Switch,
  TagsInput,
  Text,
  Title,
  Tooltip,
} from "@mantine/core";
import { useDisclosure } from "@mantine/hooks";
import { notifications } from "@mantine/notifications";
import {
  IconAlertCircle,
  IconBellRinging,
  IconChevronDown,
  IconChevronRight,
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
import { ResponsiveTable } from "@/components/ui";
import {
  usePollAllReleaseSourcesNow,
  usePollReleaseSourceNow,
  useReleaseSources,
  useResetAllReleaseSources,
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
  const pollAll = usePollAllReleaseSourcesNow();
  const reset = useResetReleaseSource();
  const resetAll = useResetAllReleaseSources();

  const sources = sourcesQuery.data ?? [];

  // The "Poll all" button is disabled when no enabled source exists:
  // sending the request would just no-op server-side and waste a round
  // trip. It also goes disabled while a fan-out is already in flight.
  const enabledCount = sources.filter((s) => s.enabled).length;
  const pollAllDisabled = enabledCount === 0 || pollAll.isPending;
  // "Reset all" includes disabled sources by design (see backend handler
  // doc), so the disable rule is just "no rows at all."
  const resetAllDisabled = sources.length === 0 || resetAll.isPending;

  /**
   * Confirm and fire the global reset. Destructive — wipes the ledger
   * across every source, including disabled ones. The confirm string
   * mirrors the per-source reset prompt's structure but emphasizes the
   * blast radius up front and includes the row count so the user knows
   * exactly what they're about to nuke.
   */
  const handleResetAll = () => {
    const sourceCount = sources.length;
    const message =
      `Reset ALL ${sourceCount} release source(s)?\n\n` +
      `This deletes every release ledger row across every source ` +
      `(including disabled ones) and clears each source's poll state ` +
      `(etag, last poll time). User-managed settings (enabled, interval, ` +
      `name) are preserved. The next poll for each enabled source will ` +
      `re-record everything as new.\n\n` +
      `This cannot be undone.`;
    if (window.confirm(message)) {
      resetAll.mutate();
    }
  };

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
        <Group gap="sm" justify="space-between" wrap="nowrap">
          <Group gap="sm">
            <IconClockHour4 size={26} />
            <Title order={2}>Release tracking</Title>
          </Group>
          <Group gap="xs" wrap="nowrap">
            <Button
              leftSection={<IconRefresh size={16} />}
              variant="default"
              size="sm"
              onClick={() => pollAll.mutate()}
              disabled={pollAllDisabled}
              loading={pollAll.isPending}
              aria-label="Poll all enabled release sources now"
            >
              Poll all now
            </Button>
            {/* "Reset all" sits next to "Poll all" but uses a `light`
                + red palette so the destructive action is visually
                distinct from the safe one. Confirm dialog gates the
                actual call. */}
            <Button
              leftSection={<IconRestore size={16} />}
              variant="light"
              color="red"
              size="sm"
              onClick={handleResetAll}
              disabled={resetAllDisabled}
              loading={resetAll.isPending}
              aria-label="Reset every release source"
            >
              Reset all
            </Button>
          </Group>
        </Group>

        <Text size="sm" c="dimmed">
          Manage release sources. Each row is one logical feed exposed by a
          plugin (e.g. one Nyaa uploader or one MangaUpdates batch). Disabling a
          source pauses its scheduled polls; "Poll now" enqueues an immediate
          fetch. "Poll all now" fans the request out across every enabled
          source.
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
          <Card withBorder p={{ base: 0, xs: 0 }} radius="md">
            <ResponsiveTable<ReleaseSource>
              data={sourcesQuery.data ?? []}
              columns={[
                {
                  key: "source",
                  header: "Source",
                  mobilePrimary: true,
                  accessor: (source) => <SourceCell source={source} />,
                },
                {
                  key: "plugin",
                  header: "Plugin",
                  accessor: (source) => <PluginCell source={source} />,
                },
                {
                  key: "interval",
                  header: "Interval",
                  mobileFullWidth: true,
                  accessor: (source) => (
                    <CronCell
                      source={source}
                      onCronScheduleChange={(cronSchedule) =>
                        update.mutate({
                          sourceId: source.id,
                          update: { cronSchedule },
                        })
                      }
                    />
                  ),
                },
                {
                  key: "lastPoll",
                  header: "Last poll",
                  mobileFullWidth: true,
                  accessor: (source) => <LastPollCell source={source} />,
                },
                {
                  key: "status",
                  header: "Status",
                  accessor: (source) => <StatusCell source={source} />,
                },
                {
                  key: "enabled",
                  header: "Enabled",
                  accessor: (source) => (
                    <Switch
                      checked={source.enabled}
                      onChange={(event) =>
                        update.mutate({
                          sourceId: source.id,
                          update: {
                            enabled: event.currentTarget.checked,
                          },
                        })
                      }
                      aria-label="Enable source"
                    />
                  ),
                },
              ]}
              getRowKey={(source) => source.id}
              tableProps={{ verticalSpacing: "sm" }}
              rowActions={(source) => (
                <>
                  <Tooltip
                    label={source.enabled ? "Poll now" : "Enable to poll"}
                  >
                    <ActionIcon
                      variant="subtle"
                      onClick={() => {
                        addId(setPollingIds, source.id);
                        pollNow.mutate(source.id, {
                          onSettled: () => removeId(setPollingIds, source.id),
                        });
                      }}
                      disabled={!source.enabled || pollingIds.has(source.id)}
                      loading={pollingIds.has(source.id)}
                      aria-label="Poll now"
                    >
                      <IconRefresh size={16} />
                    </ActionIcon>
                  </Tooltip>
                  <Tooltip label="Reset: drop ledger rows and clear poll state">
                    <ActionIcon
                      variant="subtle"
                      color="red"
                      onClick={() => {
                        if (
                          window.confirm(
                            `Reset "${source.displayName}"?\n\nThis deletes every release ledger row for this source and clears its poll state (etag, last poll time). User-managed settings (enabled, interval, name) are preserved. The next poll will re-record everything as new.\n\nThis cannot be undone.`,
                          )
                        ) {
                          addId(setResettingIds, source.id);
                          reset.mutate(source.id, {
                            onSettled: () =>
                              removeId(setResettingIds, source.id),
                          });
                        }
                      }}
                      loading={resettingIds.has(source.id)}
                      aria-label="Reset source"
                    >
                      <IconRestore size={16} />
                    </ActionIcon>
                  </Tooltip>
                </>
              )}
              rowActionsHeader=""
            />
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
  const [opened, { toggle }] = useDisclosure(false);
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
        <Group
          gap="xs"
          onClick={toggle}
          style={{ cursor: "pointer" }}
          role="button"
          aria-expanded={opened}
          aria-label={
            opened ? "Collapse default schedule" : "Expand default schedule"
          }
        >
          {opened ? (
            <IconChevronDown size={16} />
          ) : (
            <IconChevronRight size={16} />
          )}
          <IconClockHour4 size={18} />
          <Text fw={600}>Default schedule</Text>
          {!opened && draft && (
            <Text size="xs" c="dimmed">
              {draft}
            </Text>
          )}
        </Group>
        <Collapse in={opened}>
          <Stack gap="sm">
            <Text size="xs" c="dimmed">
              Server-wide default cron used by every release source that doesn't
              have its own per-row override. Changing this propagates
              immediately to inheriting rows.
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
        </Collapse>
      </Stack>
    </Card>
  );
}

function NotificationPreferencesCard() {
  const [opened, { toggle }] = useDisclosure(false);
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

  const summaryParts: string[] = [];
  if (allowedLanguages.length > 0) {
    summaryParts.push(
      `${allowedLanguages.length} ${allowedLanguages.length === 1 ? "language" : "languages"}`,
    );
  }
  if (allowedPlugins.length > 0) {
    summaryParts.push(
      `${allowedPlugins.length} ${allowedPlugins.length === 1 ? "source" : "sources"}`,
    );
  }
  if (mutedSeriesIds.length > 0) {
    summaryParts.push(
      `${mutedSeriesIds.length} muted ${mutedSeriesIds.length === 1 ? "series" : "series"}`,
    );
  }
  const summary = summaryParts.length > 0 ? summaryParts.join(" · ") : null;

  return (
    <Card withBorder padding="md" radius="md">
      <Stack gap="sm">
        <Group
          gap="xs"
          onClick={toggle}
          style={{ cursor: "pointer" }}
          role="button"
          aria-expanded={opened}
          aria-label={
            opened
              ? "Collapse notification preferences"
              : "Expand notification preferences"
          }
        >
          {opened ? (
            <IconChevronDown size={16} />
          ) : (
            <IconChevronRight size={16} />
          )}
          <IconBellRinging size={18} />
          <Text fw={600}>Notification preferences</Text>
          {!opened && summary && (
            <Text size="xs" c="dimmed">
              {summary}
            </Text>
          )}
        </Group>
        <Collapse in={opened}>
          <Stack gap="sm">
            <Text size="xs" c="dimmed">
              Filter announcement toasts and the Releases nav badge. Empty means
              "no filter — let everything through." Server-wide for languages
              and plugin sources; per-series mute is per-user (toggle on each
              series detail page).
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
        </Collapse>
      </Stack>
    </Card>
  );
}

function SourceCell({ source }: { source: ReleaseSource }) {
  return (
    <Stack gap={2}>
      <Text size="sm" fw={500}>
        {source.displayName}
      </Text>
      <Text size="xs" c="dimmed">
        {source.sourceKey}
      </Text>
    </Stack>
  );
}

function PluginCell({ source }: { source: ReleaseSource }) {
  return (
    <Badge variant="light" color={source.pluginId === "core" ? "gray" : "blue"}>
      {source.pluginId}
    </Badge>
  );
}

function LastPollCell({ source }: { source: ReleaseSource }) {
  const lastPolled = source.lastPolledAt
    ? formatDistanceToNow(new Date(source.lastPolledAt), { addSuffix: true })
    : "—";
  return (
    <Stack gap={2}>
      <Text size="xs">{lastPolled}</Text>
      {source.lastSummary && (
        <Text size="xs" c="dimmed" lineClamp={2}>
          {source.lastSummary}
        </Text>
      )}
    </Stack>
  );
}

function StatusCell({ source }: { source: ReleaseSource }) {
  if (source.lastError) {
    return (
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
    );
  }
  if (source.lastPolledAt) {
    // Wrap the OK badge in a tooltip carrying `lastSummary` so users can
    // see *why* a poll returned nothing (no tracked series, 304, dropped
    // below threshold, etc.) without grepping logs.
    return (
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
    );
  }
  return (
    <Badge color="gray" variant="light" size="sm">
      Never polled
    </Badge>
  );
}

function CronCell({
  source,
  onCronScheduleChange,
}: {
  source: ReleaseSource;
  /** `null` clears the override and reverts to the server-wide default. */
  onCronScheduleChange: (cronSchedule: string | null) => void;
}) {
  // Truthy `cronSchedule` means the row has a per-source override; render the
  // editor inline. The server omits the field entirely (rather than sending
  // `null`) when the row is inheriting, so accept both `null` and `undefined`
  // as "no override."
  const hasOverride = Boolean(source.cronSchedule);
  const [isOverriding, setIsOverriding] = useState(hasOverride);
  const [draft, setDraft] = useState<string>(
    source.cronSchedule || source.effectiveCronSchedule,
  );

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

  if (isOverriding) {
    return (
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
    );
  }

  return (
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
  );
}
