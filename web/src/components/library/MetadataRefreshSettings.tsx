import {
  Alert,
  Anchor,
  Badge,
  Button,
  Checkbox,
  Group,
  MultiSelect,
  NumberInput,
  Paper,
  Select,
  Stack,
  Text,
  TextInput,
  Tooltip,
} from "@mantine/core";
import {
  IconInfoCircle,
  IconPlayerPlay,
  IconRefresh,
} from "@tabler/icons-react";
import { useQuery } from "@tanstack/react-query";
import { useEffect, useMemo, useState } from "react";
import { type PluginActionsResponse, pluginsApi } from "@/api/plugins";
import {
  useDryRunMetadataRefresh,
  useFieldGroups,
  useMetadataRefreshConfig,
  useRunMetadataRefreshNow,
  useUpdateMetadataRefreshConfig,
} from "@/hooks/useLibraryMetadataRefresh";
import { useSchedulerTimezone } from "@/hooks/useSchedulerTimezone";
import { useTaskProgress } from "@/hooks/useTaskProgress";
import { CronInput } from "../forms/CronInput";
import { MetadataRefreshDryRunResult } from "./MetadataRefreshDryRunResult";

interface MetadataRefreshSettingsProps {
  libraryId: string;
}

const CRON_PRESETS: { value: string; label: string }[] = [
  { value: "0 * * * *", label: "Hourly (top of the hour)" },
  { value: "0 */6 * * *", label: "Every 6 hours" },
  { value: "0 4 * * *", label: "Daily at 04:00" },
  { value: "0 4 * * 0", label: "Weekly (Sunday 04:00)" },
  { value: "__custom__", label: "Custom cron expression…" },
];

function presetForCron(cron: string): string {
  return CRON_PRESETS.some((p) => p.value === cron && p.value !== "__custom__")
    ? cron
    : "__custom__";
}

export function MetadataRefreshSettings({
  libraryId,
}: MetadataRefreshSettingsProps) {
  const schedulerTimezone = useSchedulerTimezone();
  const configQuery = useMetadataRefreshConfig(libraryId);
  const fieldGroupsQuery = useFieldGroups();
  const updateMutation = useUpdateMetadataRefreshConfig(libraryId);
  const runNowMutation = useRunMetadataRefreshNow(libraryId);
  const dryRunMutation = useDryRunMetadataRefresh(libraryId);

  const pluginsQuery = useQuery<PluginActionsResponse>({
    queryKey: ["plugins", "actions", "series:bulk", libraryId],
    queryFn: () => pluginsApi.getActions("series:bulk", libraryId),
  });

  const { getTask } = useTaskProgress();

  // Local form state (initialized from server, synced on save).
  const [enabled, setEnabled] = useState(false);
  const [cronPreset, setCronPreset] = useState<string>("0 4 * * *");
  const [cronSchedule, setCronSchedule] = useState("0 4 * * *");
  const [timezone, setTimezone] = useState("");
  const [fieldGroups, setFieldGroups] = useState<string[]>([
    "ratings",
    "status",
    "counts",
  ]);
  const [providers, setProviders] = useState<string[]>([]);
  const [existingSourceIdsOnly, setExistingSourceIdsOnly] = useState(true);
  const [skipRecentlySyncedHours, setSkipRecentlySyncedHours] =
    useState<number>(1);
  const [maxConcurrency, setMaxConcurrency] = useState<number>(4);

  const [dryRunOpen, setDryRunOpen] = useState(false);
  const [activeTaskId, setActiveTaskId] = useState<string | null>(null);

  // Hydrate form when the saved config loads.
  useEffect(() => {
    const cfg = configQuery.data;
    if (!cfg) return;
    setEnabled(cfg.enabled);
    setCronSchedule(cfg.cronSchedule);
    setCronPreset(presetForCron(cfg.cronSchedule));
    setTimezone(cfg.timezone ?? "");
    setFieldGroups(cfg.fieldGroups);
    setProviders(cfg.providers);
    setExistingSourceIdsOnly(cfg.existingSourceIdsOnly);
    setSkipRecentlySyncedHours(
      Math.max(0, Math.round((cfg.skipRecentlySyncedWithinS ?? 3600) / 3600)),
    );
    setMaxConcurrency(cfg.maxConcurrency || 4);
  }, [configQuery.data]);

  // Provider options: each entry's value is the wire format `"plugin:<name>"`.
  // Dedupe — the actions endpoint returns one row per (plugin, action) pair.
  const providerOptions = useMemo(() => {
    const seen = new Set<string>();
    const out: { value: string; label: string }[] = [];
    for (const action of pluginsQuery.data?.actions ?? []) {
      const value = `plugin:${action.pluginName}`;
      if (seen.has(value)) continue;
      seen.add(value);
      out.push({ value, label: action.pluginDisplayName || action.pluginName });
    }
    return out;
  }, [pluginsQuery.data]);

  const fieldGroupOptions = useMemo(
    () =>
      (fieldGroupsQuery.data ?? []).map((g) => ({
        value: g.id,
        label: g.label,
      })),
    [fieldGroupsQuery.data],
  );

  const fieldsByGroup = useMemo(() => {
    const map = new Map<string, string[]>();
    for (const g of fieldGroupsQuery.data ?? []) {
      map.set(g.id, g.fields);
    }
    return map;
  }, [fieldGroupsQuery.data]);

  const buildPatch = () => ({
    enabled,
    cronSchedule,
    timezone: timezone.trim() ? timezone.trim() : null,
    fieldGroups,
    providers,
    existingSourceIdsOnly,
    skipRecentlySyncedWithinS: Math.max(0, skipRecentlySyncedHours) * 3600,
    maxConcurrency,
  });

  const handleSave = () => {
    updateMutation.mutate(buildPatch());
  };

  const handlePreview = () => {
    setDryRunOpen(true);
    dryRunMutation.mutate({
      configOverride: {
        ...buildPatch(),
        timezone: timezone.trim() ? timezone.trim() : null,
        extraFields: configQuery.data?.extraFields ?? [],
      },
      sampleSize: 5,
    });
  };

  const handleRunNow = () => {
    runNowMutation.mutate(undefined, {
      onSuccess: (response) => setActiveTaskId(response.taskId),
    });
  };

  const handleCronPresetChange = (value: string | null) => {
    if (!value) return;
    setCronPreset(value);
    if (value !== "__custom__") {
      setCronSchedule(value);
    }
  };

  const activeTask = activeTaskId ? getTask(activeTaskId) : undefined;
  const refreshTaskInFlight =
    !!activeTask &&
    activeTask.status !== "completed" &&
    activeTask.status !== "failed";

  if (configQuery.isLoading) {
    return <Text c="dimmed">Loading schedule…</Text>;
  }
  if (configQuery.isError) {
    return (
      <Alert color="red" variant="light" title="Could not load schedule">
        {(configQuery.error as Error)?.message ?? "Unknown error"}
      </Alert>
    );
  }

  return (
    <Stack gap="md">
      <Alert icon={<IconInfoCircle size={16} />} color="blue" variant="light">
        Configure scheduled metadata refreshes for this library. Field locks
        always win: a locked field is never written, even when its group is
        selected here.
      </Alert>

      <Paper p="md" withBorder>
        <Stack gap="sm">
          <Text fw={500}>Schedule</Text>
          <Checkbox
            label="Enable scheduled refresh"
            checked={enabled}
            onChange={(e) => setEnabled(e.currentTarget.checked)}
          />

          {enabled && (
            <>
              <Select
                label="Cadence"
                data={CRON_PRESETS}
                value={cronPreset}
                onChange={handleCronPresetChange}
                comboboxProps={{ zIndex: 1001 }}
              />
              {cronPreset === "__custom__" && (
                <CronInput
                  label="Custom cron expression"
                  description="Format: minute hour day month weekday"
                  placeholder="0 4 * * *"
                  value={cronSchedule}
                  onChange={setCronSchedule}
                />
              )}
              <TextInput
                label="Timezone"
                description={
                  <>
                    IANA timezone for the schedule. Leave empty to use the
                    server default ({schedulerTimezone}).{" "}
                    <Anchor
                      href="https://en.wikipedia.org/wiki/List_of_tz_database_time_zones"
                      target="_blank"
                      size="xs"
                    >
                      View all timezones
                    </Anchor>
                  </>
                }
                placeholder={schedulerTimezone}
                value={timezone}
                onChange={(e) => setTimezone(e.currentTarget.value)}
              />
            </>
          )}
        </Stack>
      </Paper>

      <Paper p="md" withBorder>
        <Stack gap="sm">
          <Text fw={500}>What to refresh</Text>
          <MultiSelect
            label="Field groups"
            description="Only fields in selected groups will be written. Locked fields are always skipped."
            placeholder="Select groups"
            data={fieldGroupOptions}
            value={fieldGroups}
            onChange={setFieldGroups}
            comboboxProps={{ zIndex: 1001 }}
            renderOption={({ option, checked }) => {
              const fields = fieldsByGroup.get(option.value) ?? [];
              const tooltip = fields.length
                ? `Includes: ${fields.join(", ")}`
                : option.label;
              return (
                <Tooltip label={tooltip} multiline maw={280} withArrow>
                  <Group gap="xs" w="100%">
                    {checked && <Text size="xs">✓</Text>}
                    <Text size="sm">{option.label}</Text>
                  </Group>
                </Tooltip>
              );
            }}
          />

          <MultiSelect
            label="Metadata providers"
            description="Plugins with the metadata-provider capability."
            placeholder={
              providerOptions.length === 0
                ? "No metadata-provider plugins available"
                : "Select providers"
            }
            data={providerOptions}
            value={providers}
            onChange={setProviders}
            comboboxProps={{ zIndex: 1001 }}
            disabled={providerOptions.length === 0}
          />
        </Stack>
      </Paper>

      <Paper p="md" withBorder>
        <Stack gap="sm">
          <Text fw={500}>Safety</Text>
          <Checkbox
            label="Use existing source IDs only"
            description="Skip series that don't already have a stored external ID for the chosen provider. Prevents accidental rematches."
            checked={existingSourceIdsOnly}
            onChange={(e) => setExistingSourceIdsOnly(e.currentTarget.checked)}
          />
          <NumberInput
            label="Skip if synced within (hours)"
            description="Pairs synced more recently than this are skipped."
            min={0}
            max={24 * 30}
            value={skipRecentlySyncedHours}
            onChange={(v) =>
              setSkipRecentlySyncedHours(typeof v === "number" ? v : 0)
            }
          />
          <NumberInput
            label="Max concurrency"
            description="Number of (series × provider) pairs processed in parallel."
            min={1}
            max={16}
            value={maxConcurrency}
            onChange={(v) =>
              setMaxConcurrency(typeof v === "number" && v > 0 ? v : 1)
            }
          />
        </Stack>
      </Paper>

      {refreshTaskInFlight && activeTask && (
        <Alert color="blue" variant="light" icon={<IconRefresh size={16} />}>
          <Group gap="sm" wrap="nowrap">
            <Text size="sm">
              Refresh in progress
              {activeTask.progress
                ? ` — ${activeTask.progress.current} / ${activeTask.progress.total}`
                : ""}
            </Text>
            <Badge size="sm" color="blue" variant="filled">
              {activeTask.status}
            </Badge>
          </Group>
        </Alert>
      )}

      <Group justify="flex-end" gap="sm">
        <Button
          variant="default"
          onClick={handlePreview}
          loading={dryRunMutation.isPending}
          disabled={providers.length === 0 || fieldGroups.length === 0}
        >
          Preview changes
        </Button>
        <Button
          variant="default"
          leftSection={<IconPlayerPlay size={16} />}
          onClick={handleRunNow}
          loading={runNowMutation.isPending}
          disabled={refreshTaskInFlight}
        >
          Run now
        </Button>
        <Button onClick={handleSave} loading={updateMutation.isPending}>
          Save
        </Button>
      </Group>

      <MetadataRefreshDryRunResult
        opened={dryRunOpen}
        onClose={() => setDryRunOpen(false)}
        result={dryRunMutation.data ?? null}
        loading={dryRunMutation.isPending}
      />
    </Stack>
  );
}
