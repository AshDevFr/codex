import {
  Button,
  Divider,
  Group,
  Modal,
  NumberInput,
  Select,
  Stack,
  Switch,
  Text,
  TextInput,
} from "@mantine/core";
import { useForm } from "@mantine/form";
import { notifications } from "@mantine/notifications";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import type { UserPluginDto } from "@/api/userPlugins";
import { userPluginsApi } from "@/api/userPlugins";

// =============================================================================
// Types
// =============================================================================

interface ConfigField {
  key: string;
  label: string;
  description?: string | null;
  type: string;
  required?: boolean | null;
  default?: unknown;
}

interface CodexSyncValues {
  includeCompleted: boolean;
  includeInProgress: boolean;
  countPartialProgress: boolean;
  syncRatings: boolean;
}

interface CodexRecommendationValues {
  maxRecommendations: number;
  maxSeeds: number;
  dropThreshold: number;
}

// =============================================================================
// Inner content component (keyed by plugin.id for clean remounts)
// =============================================================================

function UserPluginSettingsContent({
  plugin,
  onClose,
}: {
  plugin: UserPluginDto;
  onClose: () => void;
}) {
  const queryClient = useQueryClient();
  const isSyncPlugin = plugin.capabilities?.readSync === true;
  const isRecPlugin = plugin.capabilities?.userRecommendationProvider === true;
  const wantsFullMetadata = plugin.capabilities?.wantsFullMetadata === true;
  const configFields: ConfigField[] =
    (plugin.userConfigSchema?.fields as ConfigField[] | undefined) ?? [];
  const currentConfig = (plugin.config ?? {}) as Record<string, unknown>;
  const codexConfig = (currentConfig._codex ?? {}) as Record<string, unknown>;

  // Build initial values from current config + schema defaults
  const initialValues: Record<string, unknown> = {};

  if (isSyncPlugin) {
    initialValues.syncMode =
      typeof currentConfig.syncMode === "string"
        ? currentConfig.syncMode
        : "both";

    // Codex generic sync settings (stored in config._codex.*)
    initialValues._codex_includeCompleted =
      typeof codexConfig.includeCompleted === "boolean"
        ? codexConfig.includeCompleted
        : true;
    initialValues._codex_includeInProgress =
      typeof codexConfig.includeInProgress === "boolean"
        ? codexConfig.includeInProgress
        : true;
    initialValues._codex_countPartialProgress =
      typeof codexConfig.countPartialProgress === "boolean"
        ? codexConfig.countPartialProgress
        : false;
    initialValues._codex_syncRatings =
      typeof codexConfig.syncRatings === "boolean"
        ? codexConfig.syncRatings
        : true;
  }

  if (isRecPlugin) {
    initialValues._codex_maxRecommendations =
      typeof codexConfig.maxRecommendations === "number"
        ? codexConfig.maxRecommendations
        : 20;
    initialValues._codex_maxSeeds =
      typeof codexConfig.maxSeeds === "number" ? codexConfig.maxSeeds : 10;
    initialValues._codex_dropThreshold =
      typeof codexConfig.dropThreshold === "number"
        ? codexConfig.dropThreshold
        : 0;
  }

  // Metadata-enrichment opt-ins (stored in config._codex.send*), shown only when
  // the plugin declares the wantsFullMetadata capability. All default false.
  if (wantsFullMetadata) {
    initialValues._codex_sendTags =
      typeof codexConfig.sendTags === "boolean" ? codexConfig.sendTags : false;
    initialValues._codex_sendGenres =
      typeof codexConfig.sendGenres === "boolean"
        ? codexConfig.sendGenres
        : false;
    initialValues._codex_sendMetadata =
      typeof codexConfig.sendMetadata === "boolean"
        ? codexConfig.sendMetadata
        : false;
    initialValues._codex_sendCustomMetadata =
      typeof codexConfig.sendCustomMetadata === "boolean"
        ? codexConfig.sendCustomMetadata
        : false;
  }

  for (const field of configFields) {
    initialValues[field.key] = currentConfig[field.key] ?? field.default ?? "";
  }

  const form = useForm({ initialValues });

  const updateMutation = useMutation({
    mutationFn: async () => {
      // Build config object: preserve existing keys, update from form
      const config: Record<string, unknown> = { ...currentConfig };

      // Extract Codex settings into _codex namespace
      const codex: Record<string, unknown> = {
        ...((currentConfig._codex as Record<string, unknown>) ?? {}),
      };

      if (isSyncPlugin) {
        const syncValues: CodexSyncValues = {
          includeCompleted: !!form.values._codex_includeCompleted,
          includeInProgress: !!form.values._codex_includeInProgress,
          countPartialProgress: !!form.values._codex_countPartialProgress,
          syncRatings: !!form.values._codex_syncRatings,
        };
        Object.assign(codex, syncValues);
      }

      if (isRecPlugin) {
        const recValues: CodexRecommendationValues = {
          maxRecommendations: Number(
            form.values._codex_maxRecommendations ?? 20,
          ),
          maxSeeds: Number(form.values._codex_maxSeeds ?? 10),
          dropThreshold: Number(form.values._codex_dropThreshold ?? 0),
        };
        Object.assign(codex, recValues);
      }

      if (wantsFullMetadata) {
        // tags/genres only meaningfully apply to sync (recommendation entries
        // always carry them); metadata/custom apply to both.
        Object.assign(codex, {
          sendMetadata: !!form.values._codex_sendMetadata,
          sendCustomMetadata: !!form.values._codex_sendCustomMetadata,
        });
        if (isSyncPlugin) {
          Object.assign(codex, {
            sendTags: !!form.values._codex_sendTags,
            sendGenres: !!form.values._codex_sendGenres,
          });
        }
      }

      if (isSyncPlugin || isRecPlugin || wantsFullMetadata) {
        config._codex = codex;
      }

      // Write non-codex form values at top level
      for (const [key, value] of Object.entries(form.values)) {
        if (!key.startsWith("_codex_")) {
          config[key] = value;
        }
      }

      return userPluginsApi.updateConfig(plugin.pluginId, config);
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["user-plugins"] });
      notifications.show({
        title: "Settings saved",
        message: "Your plugin settings have been updated.",
        color: "green",
      });
      onClose();
    },
    onError: (error: Error) => {
      notifications.show({
        title: "Error",
        message: error.message || "Failed to save settings",
        color: "red",
      });
    },
  });

  const hasFields =
    isSyncPlugin || isRecPlugin || wantsFullMetadata || configFields.length > 0;

  return (
    <Stack gap="md">
      {!hasFields && (
        <Text size="sm" c="dimmed">
          This plugin has no configurable settings.
        </Text>
      )}

      {isSyncPlugin && (
        <Select
          label="Sync Mode"
          description="Choose which direction data flows during sync. In Pull & Push mode, remote progress is imported first (additive only — books are never un-read), then local progress is exported. The highest progress always wins."
          data={[
            {
              value: "both",
              label: "Pull & Push (recommended)",
            },
            {
              value: "pull",
              label: "Pull Only",
            },
            {
              value: "push",
              label: "Push Only",
            },
          ]}
          {...form.getInputProps("syncMode")}
        />
      )}

      {isSyncPlugin && (
        <>
          <Divider label="Sync Settings" labelPosition="left" mt="xs" />
          <Switch
            label="Include completed series"
            description="Push series where all local books are marked as read"
            checked={!!form.values._codex_includeCompleted}
            onChange={(e) =>
              form.setFieldValue(
                "_codex_includeCompleted",
                e.currentTarget.checked,
              )
            }
          />
          <Switch
            label="Include in-progress series"
            description="Push series where at least one book has been started"
            checked={!!form.values._codex_includeInProgress}
            onChange={(e) =>
              form.setFieldValue(
                "_codex_includeInProgress",
                e.currentTarget.checked,
              )
            }
          />
          <Switch
            label="Count partially-read books"
            description="Include partially-read books in the progress count (otherwise only fully-read books are counted)"
            checked={!!form.values._codex_countPartialProgress}
            onChange={(e) =>
              form.setFieldValue(
                "_codex_countPartialProgress",
                e.currentTarget.checked,
              )
            }
          />
          <Switch
            label="Sync ratings & notes"
            description="Include user ratings and notes in sync. When off, only reading progress is synced."
            checked={!!form.values._codex_syncRatings}
            onChange={(e) =>
              form.setFieldValue("_codex_syncRatings", e.currentTarget.checked)
            }
          />
        </>
      )}

      {isRecPlugin && (
        <>
          <Divider
            label="Recommendation Settings"
            labelPosition="left"
            mt="xs"
          />
          <NumberInput
            label="Maximum recommendations"
            description="How many recommendations to generate per refresh (1–100)"
            min={1}
            max={100}
            {...form.getInputProps("_codex_maxRecommendations")}
          />
          <NumberInput
            label="Maximum seed titles"
            description="How many of your highest-rated series to use as seeds (1–25)"
            min={1}
            max={25}
            {...form.getInputProps("_codex_maxSeeds")}
          />
          <NumberInput
            label="Minimum rating threshold"
            description="Series rated below this are not used as seeds (0 = no filter)"
            min={0}
            max={10}
            step={0.5}
            decimalScale={1}
            {...form.getInputProps("_codex_dropThreshold")}
          />
        </>
      )}

      {wantsFullMetadata && (
        <>
          <Divider label="Metadata Enrichment" labelPosition="left" mt="xs" />
          <Text size="xs" c="dimmed">
            Send extra series data to this plugin. Everything here is off by
            default; enable only what the plugin needs.
          </Text>
          {isSyncPlugin && (
            <>
              <Switch
                label="Send tags"
                description="Include each series' tags so the plugin can apply tag-based rules. Small payload."
                checked={!!form.values._codex_sendTags}
                onChange={(e) =>
                  form.setFieldValue("_codex_sendTags", e.currentTarget.checked)
                }
              />
              <Switch
                label="Send genres"
                description="Include each series' genres. Small payload."
                checked={!!form.values._codex_sendGenres}
                onChange={(e) =>
                  form.setFieldValue(
                    "_codex_sendGenres",
                    e.currentTarget.checked,
                  )
                }
              />
            </>
          )}
          <Switch
            label="Send metadata"
            description="Include summary, authors, publisher, age rating, language, and reading direction. This is the heaviest option (summaries can be large)."
            checked={!!form.values._codex_sendMetadata}
            onChange={(e) =>
              form.setFieldValue("_codex_sendMetadata", e.currentTarget.checked)
            }
          />
          <Switch
            label="Send custom metadata"
            description="Include your user-defined custom metadata fields. These can hold private notes — only enable for plugins you trust."
            checked={!!form.values._codex_sendCustomMetadata}
            onChange={(e) =>
              form.setFieldValue(
                "_codex_sendCustomMetadata",
                e.currentTarget.checked,
              )
            }
          />
        </>
      )}

      {(isSyncPlugin || isRecPlugin || wantsFullMetadata) &&
        configFields.length > 0 && (
          <Divider label="Plugin Settings" labelPosition="left" mt="xs" />
        )}

      {configFields.map((field) => {
        const props = form.getInputProps(field.key);
        switch (field.type) {
          case "boolean":
            return (
              <Switch
                key={field.key}
                label={field.label}
                description={field.description}
                checked={!!props.value}
                onChange={(e) =>
                  form.setFieldValue(field.key, e.currentTarget.checked)
                }
              />
            );
          case "number":
            return (
              <NumberInput
                key={field.key}
                label={field.label}
                description={field.description}
                {...props}
              />
            );
          default:
            return (
              <TextInput
                key={field.key}
                label={field.label}
                description={field.description}
                {...props}
              />
            );
        }
      })}

      {hasFields && (
        <Group justify="flex-end" mt="md">
          <Button variant="subtle" onClick={onClose}>
            Cancel
          </Button>
          <Button
            onClick={() => updateMutation.mutate()}
            loading={updateMutation.isPending}
          >
            Save
          </Button>
        </Group>
      )}

      {!hasFields && (
        <Group justify="flex-end">
          <Button variant="subtle" onClick={onClose}>
            Close
          </Button>
        </Group>
      )}
    </Stack>
  );
}

// =============================================================================
// Exported modal component
// =============================================================================

interface UserPluginSettingsModalProps {
  plugin: UserPluginDto;
  opened: boolean;
  onClose: () => void;
}

export function UserPluginSettingsModal({
  plugin,
  opened,
  onClose,
}: UserPluginSettingsModalProps) {
  return (
    <Modal
      opened={opened}
      onClose={onClose}
      title={`Settings: ${plugin.pluginDisplayName}`}
      centered
    >
      {/* Key forces remount when plugin changes, resetting form state */}
      <UserPluginSettingsContent
        key={plugin.id}
        plugin={plugin}
        onClose={onClose}
      />
    </Modal>
  );
}
