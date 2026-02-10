import { Alert, Chip, Group, Stack, Switch, Text } from "@mantine/core";
import { IconInfoCircle } from "@tabler/icons-react";
import type { PluginDto } from "@/api/plugins";
import {
  type AutoMatchConditions,
  ConditionsEditor,
} from "../ConditionsEditor";
import type { MetadataTarget, PluginConfigForm } from "./types";

interface ConditionsTabProps {
  plugin: PluginDto;
  form: PluginConfigForm;
  autoMatchConditions: AutoMatchConditions | null;
  onAutoMatchConditionsChange: (conditions: AutoMatchConditions | null) => void;
}

export function ConditionsTab({
  plugin,
  form,
  autoMatchConditions,
  onAutoMatchConditionsChange,
}: ConditionsTabProps) {
  const pluginCapabilities =
    plugin.manifest?.capabilities?.metadataProvider ?? [];
  const canSeries = pluginCapabilities.includes("series");
  const canBook = pluginCapabilities.includes("book");

  return (
    <Stack gap="md">
      <Alert icon={<IconInfoCircle size={16} />} color="blue" variant="light">
        <Text size="sm">
          Define conditions that control when auto-matching runs for this
          plugin. Without conditions, auto-matching will run for all series.
        </Text>
      </Alert>

      <Stack gap={4}>
        <Text fw={500} size="sm">
          Metadata Targets
        </Text>
        <Text size="xs" c="dimmed">
          Which resource types should this plugin auto-match against? Options
          are limited to the plugin&apos;s capabilities.
        </Text>
        <Chip.Group
          multiple
          value={form.values.metadataTargets}
          onChange={(value) =>
            form.setFieldValue("metadataTargets", value as MetadataTarget[])
          }
        >
          <Group gap="xs" mt={4}>
            <Chip
              value="series"
              disabled={!canSeries}
              size="sm"
              variant="outline"
            >
              Series
            </Chip>
            <Chip value="book" disabled={!canBook} size="sm" variant="outline">
              Books
            </Chip>
          </Group>
        </Chip.Group>
        {!canSeries && !canBook && (
          <Text size="xs" c="yellow">
            This plugin has no manifest yet. Test the connection to discover its
            capabilities.
          </Text>
        )}
      </Stack>

      <Switch
        label="Use Existing External ID"
        description="Skip search when series already has an external ID from this plugin"
        checked={form.values.useExistingExternalId}
        onChange={(e) =>
          form.setFieldValue("useExistingExternalId", e.currentTarget.checked)
        }
      />

      <ConditionsEditor
        value={autoMatchConditions}
        onChange={onAutoMatchConditionsChange}
        label="Auto-Match Conditions"
        description="Define conditions that must be met for auto-matching to run."
      />
    </Stack>
  );
}
