import { Chip, Group, NumberInput, Stack, Switch, Text } from "@mantine/core";
import type { PluginDto } from "@/api/plugins";
import type { MetadataTarget, PluginConfigForm } from "./types";

interface GeneralTabProps {
  plugin: PluginDto;
  form: PluginConfigForm;
}

export function GeneralTab({ plugin, form }: GeneralTabProps) {
  const pluginCapabilities =
    plugin.manifest?.capabilities?.metadataProvider ?? [];
  const canSeries = pluginCapabilities.includes("series");
  const canBook = pluginCapabilities.includes("book");

  return (
    <Stack gap="md">
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

      <NumberInput
        label="Search Results Limit"
        description="Maximum number of results returned by metadata search. Leave empty for plugin default."
        placeholder="Default (plugin decides)"
        min={1}
        max={200}
        allowDecimal={false}
        value={form.values.searchResultsLimit ?? ""}
        onChange={(value) =>
          form.setFieldValue(
            "searchResultsLimit",
            typeof value === "number" ? value : null,
          )
        }
      />
    </Stack>
  );
}
