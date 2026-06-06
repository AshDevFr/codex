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
  hasPermissionableSurface,
  isLibraryScopable,
  isRecommendationProvider,
  isReleaseSource,
  isSyncProvider,
  type PluginConfigForm,
} from "./types";

interface PermissionsTabProps {
  plugin: PluginDto;
  form: PluginConfigForm;
  libraries: { id: string; name: string }[];
}

/** Library scope controls — shared by metadata, sync, and recommendation plugins. */
function LibraryFilter({
  form,
  libraries,
}: {
  form: PluginConfigForm;
  libraries: { id: string; name: string }[];
}) {
  return (
    <>
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
          description="Plugin will only act on series/books in these libraries"
          data={libraries.map((lib) => ({
            value: lib.id,
            label: lib.name,
          }))}
          searchable
          {...form.getInputProps("libraryIds")}
        />
      )}
    </>
  );
}

export function PermissionsTab({
  plugin,
  form,
  libraries,
}: PermissionsTabProps) {
  const permissionable = hasPermissionableSurface(plugin);
  const libraryScopable = isLibraryScopable(plugin);

  // Nothing to configure — no permissions/scopes AND not library-scoped
  // (e.g. release-source-only plugins).
  if (!permissionable && !libraryScopable) {
    const capabilityLabel = isReleaseSource(plugin) ? "Release-source" : null;
    return (
      <Stack gap="md">
        <Alert
          icon={<IconInfoCircle size={16} />}
          color="blue"
          variant="light"
          title="No permission settings for this plugin"
        >
          <Text size="sm">
            {capabilityLabel
              ? `${capabilityLabel} plugins are gated by their manifest capability — they don't write metadata, don't expose scoped UI actions, and aren't library-filtered. There is nothing to configure on this tab.`
              : "This plugin doesn't expose any capability that uses permissions, scopes, or the library filter."}
          </Text>
        </Alert>
      </Stack>
    );
  }

  // Library-scopable but no permissions/scopes (sync / recommendation plugins).
  // Show a short note, then the library filter.
  if (!permissionable) {
    const capabilityLabel = isRecommendationProvider(plugin)
      ? "Recommendation"
      : isSyncProvider(plugin)
        ? "Sync"
        : null;
    return (
      <Stack gap="md">
        <Alert
          icon={<IconInfoCircle size={16} />}
          color="blue"
          variant="light"
          title="No permissions or scopes for this plugin"
        >
          <Text size="sm">
            {capabilityLabel
              ? `${capabilityLabel} plugins are gated by their manifest capability — they don't write metadata or expose scoped UI actions, so there are no permissions or scopes to configure. You can still scope this plugin to specific libraries below.`
              : "This plugin has no permissions or scopes to configure, but you can scope it to specific libraries below."}
          </Text>
        </Alert>

        <LibraryFilter form={form} libraries={libraries} />
      </Stack>
    );
  }

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

      <LibraryFilter form={form} libraries={libraries} />
    </Stack>
  );
}
