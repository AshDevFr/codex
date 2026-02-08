import {
  Button,
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
  const configFields: ConfigField[] =
    (plugin.userConfigSchema?.fields as ConfigField[] | undefined) ?? [];
  const currentConfig = (plugin.config ?? {}) as Record<string, unknown>;

  // Build initial values from current config + schema defaults
  const initialValues: Record<string, unknown> = {};

  if (isSyncPlugin) {
    initialValues.syncMode =
      typeof currentConfig.syncMode === "string"
        ? currentConfig.syncMode
        : "both";
  }

  for (const field of configFields) {
    initialValues[field.key] = currentConfig[field.key] ?? field.default ?? "";
  }

  const form = useForm({ initialValues });

  const updateMutation = useMutation({
    mutationFn: async () => {
      // Merge form values into a single config object
      const config: Record<string, unknown> = { ...currentConfig };
      for (const [key, value] of Object.entries(form.values)) {
        config[key] = value;
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

  const hasFields = isSyncPlugin || configFields.length > 0;

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
          description="Choose which direction data flows during sync"
          data={[
            { value: "both", label: "Pull & Push" },
            { value: "pull", label: "Pull Only" },
            { value: "push", label: "Push Only" },
          ]}
          {...form.getInputProps("syncMode")}
        />
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
