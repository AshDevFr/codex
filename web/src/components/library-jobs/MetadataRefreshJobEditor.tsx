import {
  Alert,
  Anchor,
  Badge,
  Button,
  Checkbox,
  Collapse,
  Divider,
  Group,
  Modal,
  NumberInput,
  Paper,
  Radio,
  Select,
  Stack,
  Switch,
  Text,
  TextInput,
} from "@mantine/core";
import { useDisclosure } from "@mantine/hooks";
import { notifications } from "@mantine/notifications";
import {
  IconAlertTriangle,
  IconChevronDown,
  IconChevronRight,
  IconInfoCircle,
} from "@tabler/icons-react";
import { useQuery } from "@tanstack/react-query";
import { useEffect, useMemo, useState } from "react";
import type {
  CreateLibraryJobRequest,
  LibraryJob,
  LibraryJobConfig,
  RefreshScope,
} from "@/api/libraryJobs";
import { type PluginActionDto, pluginsApi } from "@/api/plugins";
import { CronInput } from "@/components/forms/CronInput";
import {
  useCreateLibraryJob,
  useDryRunLibraryJob,
  useFieldGroups,
  useUpdateLibraryJob,
} from "@/hooks/useLibraryJobs";
import { JobDryRunModal } from "./JobDryRunModal";

interface JobEditorProps {
  libraryId: string;
  opened: boolean;
  onClose: () => void;
  job: LibraryJob | null;
}

const CRON_PRESETS: { value: string; label: string }[] = [
  { value: "0 * * * *", label: "Hourly (top of the hour)" },
  { value: "0 */6 * * *", label: "Every 6 hours" },
  { value: "0 4 * * *", label: "Daily at 04:00" },
  { value: "0 4 * * 0", label: "Weekly (Sunday 04:00)" },
  { value: "custom", label: "Custom" },
];

export function JobEditor({ libraryId, opened, onClose, job }: JobEditorProps) {
  const isEdit = Boolean(job);
  const create = useCreateLibraryJob(libraryId);
  const update = useUpdateLibraryJob(libraryId);
  const dryRun = useDryRunLibraryJob(libraryId);
  const { data: fieldGroups } = useFieldGroups();

  // Plugin list, filtered to those that can act as metadata providers (series
  // or book scope).
  const { data: seriesActions } = useQuery({
    queryKey: ["plugin-actions", "series:bulk"],
    queryFn: () => pluginsApi.getActions("series:bulk"),
    staleTime: 5 * 60 * 1000,
  });
  const { data: bookActions } = useQuery({
    queryKey: ["plugin-actions", "book:bulk"],
    queryFn: () => pluginsApi.getActions("book:bulk"),
    staleTime: 5 * 60 * 1000,
  });

  // Merge actions by pluginId so a plugin appearing in both lists carries
  // capabilities from whichever was last seen (capabilities come from the
  // manifest, so they're identical regardless of scope).
  const allPlugins: PluginActionDto[] = useMemo(() => {
    const map = new Map<string, PluginActionDto>();
    for (const a of seriesActions?.actions ?? []) map.set(a.pluginId, a);
    for (const a of bookActions?.actions ?? []) {
      if (!map.has(a.pluginId)) map.set(a.pluginId, a);
    }
    return Array.from(map.values());
  }, [seriesActions, bookActions]);

  // Form state.
  const [name, setName] = useState("");
  const [enabled, setEnabled] = useState(false);
  const [cronPreset, setCronPreset] = useState("0 4 * * *");
  const [cronCustom, setCronCustom] = useState("0 4 * * *");
  const [timezone, setTimezone] = useState<string>("");
  const [provider, setProvider] = useState<string>(""); // "plugin:<name>"
  const [scope, setScope] = useState<RefreshScope>("series_only");
  const [selectedGroups, setSelectedGroups] = useState<string[]>([
    "ratings",
    "status",
    "counts",
  ]);
  const [extraFields, setExtraFields] = useState<string[]>([]);
  const [existingOnly, setExistingOnly] = useState(true);
  const [skipRecent, setSkipRecent] = useState(3600);
  const [maxConcurrency, setMaxConcurrency] = useState(4);
  const [advancedOpen, advanced] = useDisclosure(false);
  const [dryRunOpen, dryRunModal] = useDisclosure(false);

  // Hydrate when opened on an existing job.
  useEffect(() => {
    if (!opened) return;
    if (job) {
      setName(job.name);
      setEnabled(job.enabled);
      // If saved cron matches a preset, select it; else custom.
      const matchPreset = CRON_PRESETS.find(
        (p) => p.value !== "custom" && p.value === job.cronSchedule,
      );
      setCronPreset(matchPreset ? matchPreset.value : "custom");
      setCronCustom(job.cronSchedule);
      setTimezone(job.timezone ?? "");
      const cfg = job.config;
      setProvider(cfg.provider);
      setScope(cfg.scope ?? "series_only");
      setSelectedGroups(cfg.fieldGroups ?? []);
      setExtraFields(cfg.extraFields ?? []);
      setExistingOnly(cfg.existingSourceIdsOnly ?? true);
      setSkipRecent(cfg.skipRecentlySyncedWithinS ?? 3600);
      setMaxConcurrency(cfg.maxConcurrency ?? 4);
    } else {
      // Reset to defaults for "Add job".
      setName("");
      setEnabled(false);
      setCronPreset("0 4 * * *");
      setCronCustom("0 4 * * *");
      setTimezone("");
      setProvider("");
      setScope("series_only");
      setSelectedGroups(["ratings", "status", "counts"]);
      setExtraFields([]);
      setExistingOnly(true);
      setSkipRecent(3600);
      setMaxConcurrency(4);
    }
  }, [opened, job]);

  const selectedPlugin = allPlugins.find(
    (p) => `plugin:${p.pluginName}` === provider,
  );
  const supportsSeries =
    selectedPlugin?.capabilities?.metadataProvider?.includes("series") ?? false;
  const supportsBooks =
    selectedPlugin?.capabilities?.metadataProvider?.includes("book") ?? false;

  // Auto-correct scope when provider changes and the current scope is no longer valid.
  useEffect(() => {
    if (!selectedPlugin) return;
    if (scope === "series_only" && !supportsSeries) {
      if (supportsBooks) {
        setScope("books_only");
        notifications.show({
          title: "Scope updated",
          message: `${selectedPlugin.pluginDisplayName} only supports book metadata.`,
          color: "blue",
        });
      }
    } else if (scope === "books_only" && !supportsBooks) {
      if (supportsSeries) {
        setScope("series_only");
        notifications.show({
          title: "Scope updated",
          message: `${selectedPlugin.pluginDisplayName} only supports series metadata.`,
          color: "blue",
        });
      }
    } else if (
      scope === "series_and_books" &&
      !(supportsSeries && supportsBooks)
    ) {
      if (supportsSeries) setScope("series_only");
      else if (supportsBooks) setScope("books_only");
    }
  }, [selectedPlugin, supportsBooks, supportsSeries, scope]);

  const cronValue = cronPreset === "custom" ? cronCustom : cronPreset;

  const buildConfig = (): LibraryJobConfig => ({
    type: "metadata_refresh",
    provider,
    scope,
    fieldGroups: selectedGroups,
    extraFields,
    bookFieldGroups: [],
    bookExtraFields: [],
    existingSourceIdsOnly: existingOnly,
    skipRecentlySyncedWithinS: skipRecent,
    maxConcurrency,
  });

  const handleSubmit = async () => {
    if (!provider) {
      notifications.show({
        title: "Pick a provider",
        message: "A provider is required.",
        color: "yellow",
      });
      return;
    }
    const config = buildConfig();
    const body: CreateLibraryJobRequest = {
      name: name.trim() ? name.trim() : undefined,
      enabled,
      cronSchedule: cronValue,
      timezone: timezone || null,
      config,
    };

    if (isEdit && job) {
      await update.mutateAsync({
        jobId: job.id,
        patch: {
          name: body.name ?? undefined,
          enabled: body.enabled,
          cronSchedule: body.cronSchedule,
          timezone: timezone || null,
          config: body.config,
        },
      });
    } else {
      await create.mutateAsync(body);
    }
    onClose();
  };

  const handlePreview = async () => {
    if (!isEdit || !job) {
      notifications.show({
        title: "Save first",
        message: "Preview is available after saving the job at least once.",
        color: "yellow",
      });
      return;
    }
    const config = buildConfig();
    await dryRun.mutateAsync({
      jobId: job.id,
      body: { configOverride: config, sampleSize: 5 },
    });
    dryRunModal.open();
  };

  // Compute which fields will actually be written.
  const previewFields = useMemo(() => {
    const set = new Set<string>();
    for (const g of selectedGroups) {
      const def = fieldGroups?.find((fg) => fg.id === g);
      if (def) {
        for (const f of def.fields) set.add(f);
      }
    }
    for (const f of extraFields) set.add(f);
    return Array.from(set).sort();
  }, [selectedGroups, extraFields, fieldGroups]);

  const allFields = useMemo(() => {
    const set = new Set<string>();
    for (const g of fieldGroups ?? []) {
      for (const f of g.fields) set.add(f);
    }
    return Array.from(set).sort();
  }, [fieldGroups]);

  const fieldFromGroup = (field: string): string | null => {
    for (const g of fieldGroups ?? []) {
      if (selectedGroups.includes(g.id) && g.fields.includes(field))
        return g.label;
    }
    return null;
  };

  return (
    <Modal
      opened={opened}
      onClose={onClose}
      title={isEdit ? "Edit job" : "Add job"}
      size="xl"
    >
      <Stack>
        <TextInput
          label="Name"
          placeholder="Auto-generated from provider + groups"
          value={name}
          onChange={(e) => setName(e.currentTarget.value)}
        />

        <Switch
          label="Enable scheduled runs"
          description="When off, the job exists but its cron does not fire. Run-now still works."
          checked={enabled}
          onChange={(e) => setEnabled(e.currentTarget.checked)}
        />

        <Divider label="Schedule" labelPosition="left" />

        <Select
          label="Cadence"
          data={CRON_PRESETS}
          value={cronPreset}
          onChange={(v) => v && setCronPreset(v)}
        />
        {cronPreset === "custom" && (
          <CronInput value={cronCustom} onChange={setCronCustom} />
        )}
        <TextInput
          label="Timezone (optional, IANA)"
          placeholder="Defaults to server timezone"
          value={timezone}
          onChange={(e) => setTimezone(e.currentTarget.value)}
        />

        <Divider label="Provider & scope" labelPosition="left" />

        <Select
          label="Provider"
          placeholder="Select a metadata provider"
          searchable
          data={allPlugins.map((p) => ({
            value: `plugin:${p.pluginName}`,
            label: p.pluginDisplayName,
          }))}
          value={provider}
          onChange={(v) => v && setProvider(v)}
        />

        {selectedPlugin && (
          <ScopeControl
            scope={scope}
            onChange={setScope}
            supportsSeries={supportsSeries}
            supportsBooks={supportsBooks}
          />
        )}

        <Divider label="What to refresh" labelPosition="left" />

        <Stack gap="xs">
          <Text size="sm" fw={500}>
            Field groups
          </Text>
          <Text size="xs" c="dimmed">
            Select the field groups to refresh. Locked fields are always
            skipped, regardless of selection.
          </Text>
          <Stack gap={2}>
            {(fieldGroups ?? []).map((g) => (
              <Paper key={g.id} withBorder p="xs">
                <Group justify="space-between" wrap="nowrap" align="flex-start">
                  <Stack gap={2} style={{ flex: 1 }}>
                    <Checkbox
                      label={g.label}
                      checked={selectedGroups.includes(g.id)}
                      onChange={(e) => {
                        const checked = e.currentTarget.checked;
                        setSelectedGroups((prev) =>
                          checked
                            ? [...prev, g.id]
                            : prev.filter((id) => id !== g.id),
                        );
                      }}
                    />
                    <Text size="xs" c="dimmed" pl={28}>
                      {g.fields.join(", ")}
                    </Text>
                  </Stack>
                </Group>
              </Paper>
            ))}
          </Stack>
        </Stack>

        <Anchor component="button" onClick={() => advanced.toggle()} size="sm">
          <Group gap={4}>
            {advancedOpen ? (
              <IconChevronDown size={14} />
            ) : (
              <IconChevronRight size={14} />
            )}
            Advanced: individual fields
          </Group>
        </Anchor>
        <Collapse in={advancedOpen}>
          <Paper withBorder p="sm">
            <Stack gap={2}>
              <Text size="xs" c="dimmed">
                Pick individual fields not covered by any selected group. Fields
                already included by a group are disabled with a hint.
              </Text>
              {allFields.map((f) => {
                const includedBy = fieldFromGroup(f);
                const checked = includedBy != null || extraFields.includes(f);
                return (
                  <Checkbox
                    key={f}
                    label={
                      <Group gap={6}>
                        <Text>{f}</Text>
                        {includedBy && (
                          <Badge size="xs" variant="light">
                            via {includedBy}
                          </Badge>
                        )}
                      </Group>
                    }
                    checked={checked}
                    disabled={includedBy != null}
                    onChange={(e) => {
                      const v = e.currentTarget.checked;
                      setExtraFields((prev) =>
                        v ? [...prev, f] : prev.filter((x) => x !== f),
                      );
                    }}
                  />
                );
              })}
            </Stack>
          </Paper>
        </Collapse>

        <Alert variant="light" icon={<IconInfoCircle size={16} />}>
          <Text size="sm">
            Will write {previewFields.length} field
            {previewFields.length === 1 ? "" : "s"}:{" "}
            {previewFields.length === 0 ? (
              <em>
                none (all fields would be applied — same as omitting filter)
              </em>
            ) : (
              <code>{previewFields.join(", ")}</code>
            )}
          </Text>
        </Alert>

        <Divider label="Safety" labelPosition="left" />

        <Switch
          label="Use existing source IDs only"
          description="Skip series that don't already have a stored external ID for the chosen provider. Prevents accidental rematches."
          checked={existingOnly}
          onChange={(e) => setExistingOnly(e.currentTarget.checked)}
        />
        <NumberInput
          label="Skip if synced within (seconds)"
          description="Skip series synced more recently than this. 0 disables the guard."
          value={skipRecent}
          onChange={(v) => setSkipRecent(typeof v === "number" ? v : 3600)}
          min={0}
        />
        <NumberInput
          label="Max concurrency"
          description="Series processed in parallel."
          value={maxConcurrency}
          onChange={(v) =>
            setMaxConcurrency(
              typeof v === "number" ? Math.min(16, Math.max(1, v)) : 4,
            )
          }
          min={1}
          max={16}
        />

        <Group justify="flex-end" mt="md">
          <Button variant="default" onClick={onClose}>
            Cancel
          </Button>
          <Button
            variant="default"
            onClick={handlePreview}
            loading={dryRun.isPending}
            disabled={!isEdit}
          >
            Preview changes
          </Button>
          <Button
            onClick={handleSubmit}
            loading={create.isPending || update.isPending}
          >
            {isEdit ? "Save changes" : "Create job"}
          </Button>
        </Group>
      </Stack>

      <JobDryRunModal
        opened={dryRunOpen}
        onClose={dryRunModal.close}
        result={dryRun.data}
      />
    </Modal>
  );
}

function ScopeControl({
  scope,
  onChange,
  supportsSeries,
  supportsBooks,
}: {
  scope: RefreshScope;
  onChange: (s: RefreshScope) => void;
  supportsSeries: boolean;
  supportsBooks: boolean;
}) {
  const onlyOne = supportsSeries !== supportsBooks;
  const lockedLabel = supportsSeries ? "Series only" : "Books only";

  if (onlyOne) {
    return (
      <Alert variant="light" color="blue" icon={<IconInfoCircle size={16} />}>
        <Text size="sm">
          <strong>Scope:</strong> {lockedLabel}{" "}
          <Text component="span" size="xs" c="dimmed">
            (this provider only supports one content type)
          </Text>
        </Text>
      </Alert>
    );
  }

  return (
    <Stack gap="xs">
      <Text size="sm" fw={500}>
        Scope
      </Text>
      <Radio.Group value={scope} onChange={(v) => onChange(v as RefreshScope)}>
        <Stack gap={4}>
          <Radio
            value="series_only"
            label="Series only"
            disabled={!supportsSeries}
          />
          <Radio
            value="books_only"
            label={
              <Group gap={6}>
                Books only
                <Badge size="xs" variant="light" color="yellow">
                  coming soon
                </Badge>
              </Group>
            }
            disabled
          />
          <Radio
            value="series_and_books"
            label={
              <Group gap={6}>
                Series & books
                <Badge size="xs" variant="light" color="yellow">
                  coming soon
                </Badge>
              </Group>
            }
            disabled
          />
        </Stack>
      </Radio.Group>
      {scope !== "series_only" && (
        <Alert
          variant="light"
          color="yellow"
          icon={<IconAlertTriangle size={16} />}
        >
          <Text size="sm">
            Book-scope refresh isn't implemented yet. Saving with this scope
            will be rejected.
          </Text>
        </Alert>
      )}
    </Stack>
  );
}
