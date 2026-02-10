import {
  Alert,
  Divider,
  MultiSelect,
  Stack,
  Switch,
  Text,
} from "@mantine/core";
import { IconInfoCircle } from "@tabler/icons-react";
import type { PluginDto } from "@/api/plugins";
import {
  getPermissionData,
  getScopeData,
  type PluginConfigForm,
} from "./types";

interface PermissionsTabProps {
  plugin: PluginDto;
  form: PluginConfigForm;
  libraries: { id: string; name: string }[];
}

export function PermissionsTab({
  plugin,
  form,
  libraries,
}: PermissionsTabProps) {
  const permissionInfo = getPermissionData(plugin);
  const scopeData = getScopeData(plugin);

  return (
    <Stack gap="md">
      {permissionInfo.showNoManifestWarning && (
        <Alert
          icon={<IconInfoCircle size={16} />}
          color="yellow"
          variant="light"
        >
          <Text size="sm">
            This plugin has not been tested yet. Test the connection to discover
            its capabilities. All permissions are shown below.
          </Text>
        </Alert>
      )}

      <MultiSelect
        label="Permissions"
        placeholder="Select permissions"
        description="RBAC permissions controlling what the plugin can write"
        data={permissionInfo.data}
        searchable
        {...form.getInputProps("permissions")}
      />

      <MultiSelect
        label="Scopes"
        placeholder="Select scopes"
        description="Where the plugin actions will be available in the UI"
        data={scopeData}
        searchable
        {...form.getInputProps("scopes")}
      />

      <Divider label="Library Filter" labelPosition="center" />

      <Switch
        label="All Libraries"
        description="When enabled, plugin applies to all libraries. Disable to select specific libraries."
        {...form.getInputProps("allLibraries", { type: "checkbox" })}
      />
      {!form.values.allLibraries && (
        <MultiSelect
          label="Libraries"
          placeholder="Select libraries"
          description="Plugin will only be available for series/books in these libraries"
          data={libraries.map((lib) => ({
            value: lib.id,
            label: lib.name,
          }))}
          searchable
          {...form.getInputProps("libraryIds")}
        />
      )}
    </Stack>
  );
}
