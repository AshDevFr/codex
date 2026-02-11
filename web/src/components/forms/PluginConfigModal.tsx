import { Button, Group, Modal, Stack, Tabs } from "@mantine/core";
import { useForm } from "@mantine/form";
import { notifications } from "@mantine/notifications";
import {
  IconCode,
  IconKey,
  IconSearch,
  IconSettings,
  IconShield,
} from "@tabler/icons-react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import type { PluginDto } from "@/api/plugins";
import { pluginsApi } from "@/api/plugins";
import type { AutoMatchConditions } from "./ConditionsEditor";
import type { PreprocessingRule } from "./PreprocessingRulesEditor";
import {
  ConditionsTab,
  isMetadataProvider,
  isOAuthPlugin,
  type MetadataTarget,
  OAuthTab,
  PermissionsTab,
  type PluginConfigFormValues,
  PreprocessingTab,
  TemplateTab,
} from "./plugin-config";

// =============================================================================
// Inner content component (keyed by plugin.id for clean remounts)
// =============================================================================

function PluginConfigContent({
  plugin,
  onClose,
  libraries,
}: {
  plugin: PluginDto;
  onClose: () => void;
  libraries: { id: string; name: string }[];
}) {
  const queryClient = useQueryClient();
  const isMeta = isMetadataProvider(plugin);
  const isOAuth = isOAuthPlugin(plugin);
  const [activeTab, setActiveTab] = useState<string | null>("permissions");

  // Parse initial preprocessing rules from plugin
  const initialPreprocessingRules: PreprocessingRule[] =
    plugin.searchPreprocessingRules &&
    Array.isArray(plugin.searchPreprocessingRules)
      ? (plugin.searchPreprocessingRules as PreprocessingRule[])
      : [];

  // Parse initial auto-match conditions from plugin
  const initialAutoMatchConditions: AutoMatchConditions | null =
    plugin.autoMatchConditions && typeof plugin.autoMatchConditions === "object"
      ? (plugin.autoMatchConditions as AutoMatchConditions)
      : null;

  // State for the complex editors
  const [preprocessingRules, setPreprocessingRules] = useState<
    PreprocessingRule[]
  >(initialPreprocessingRules);
  const [autoMatchConditions, setAutoMatchConditions] =
    useState<AutoMatchConditions | null>(initialAutoMatchConditions);
  const [testTitle, setTestTitle] = useState("");

  // Determine which targets the plugin's manifest supports
  const pluginCapabilities =
    plugin.manifest?.capabilities?.metadataProvider ?? [];
  const canSeries = pluginCapabilities.includes("series");
  const canBook = pluginCapabilities.includes("book");

  // Parse initial metadata targets from plugin
  const initialMetadataTargets: MetadataTarget[] = plugin.metadataTargets
    ? (plugin.metadataTargets.filter(
        (t): t is MetadataTarget => t === "series" || t === "book",
      ) as MetadataTarget[])
    : (["series", "book"].filter((t) =>
        t === "series" ? canSeries : canBook,
      ) as MetadataTarget[]);

  // Extract OAuth config from plugin.config JSON
  const pluginConfig = plugin.config as Record<string, unknown> | null;
  const initialOAuthClientId =
    typeof pluginConfig?.oauth_client_id === "string"
      ? pluginConfig.oauth_client_id
      : "";
  const initialOAuthClientSecret =
    typeof pluginConfig?.oauth_client_secret === "string"
      ? pluginConfig.oauth_client_secret
      : "";

  // Form for all fields
  const form = useForm<PluginConfigFormValues>({
    initialValues: {
      permissions: plugin.permissions,
      scopes: plugin.scopes,
      allLibraries: plugin.libraryIds.length === 0,
      libraryIds: plugin.libraryIds,
      searchQueryTemplate: plugin.searchQueryTemplate ?? "",
      searchResultsLimit: plugin.internalConfig?.searchResultsLimit ?? null,
      useExistingExternalId: plugin.useExistingExternalId ?? true,
      metadataTargets: initialMetadataTargets,
      oauthClientId: initialOAuthClientId,
      oauthClientSecret: initialOAuthClientSecret,
    },
  });

  const updateMutation = useMutation({
    mutationFn: async () => {
      const payload: Record<string, unknown> = {
        permissions: form.values.permissions,
        scopes: form.values.scopes,
        libraryIds: form.values.allLibraries ? [] : form.values.libraryIds,
      };

      if (isMeta) {
        payload.searchQueryTemplate =
          form.values.searchQueryTemplate.trim() || null;
        payload.searchPreprocessingRules = preprocessingRules;
        payload.autoMatchConditions = autoMatchConditions;
        payload.useExistingExternalId = form.values.useExistingExternalId;
        payload.metadataTargets = form.values.metadataTargets;
        payload.internalConfig = {
          searchResultsLimit: form.values.searchResultsLimit || null,
        };
      }

      if (isOAuth) {
        const existingConfig = (plugin.config as Record<string, unknown>) ?? {};
        const config: Record<string, unknown> = { ...existingConfig };
        if (form.values.oauthClientId.trim()) {
          config.oauth_client_id = form.values.oauthClientId.trim();
        } else {
          delete config.oauth_client_id;
        }
        if (form.values.oauthClientSecret.trim()) {
          config.oauth_client_secret = form.values.oauthClientSecret.trim();
        } else {
          delete config.oauth_client_secret;
        }
        payload.config = config;
      }

      return pluginsApi.update(plugin.id, payload);
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["plugins"] });
      queryClient.invalidateQueries({ queryKey: ["plugin-actions"] });
      notifications.show({
        title: "Success",
        message: "Plugin configuration updated successfully",
        color: "green",
      });
      onClose();
    },
    onError: (error: Error) => {
      notifications.show({
        title: "Error",
        message: error.message || "Failed to update plugin configuration",
        color: "red",
      });
    },
  });

  return (
    <>
      <Tabs value={activeTab} onChange={setActiveTab}>
        <Tabs.List>
          <Tabs.Tab value="permissions" leftSection={<IconShield size={14} />}>
            Permissions
          </Tabs.Tab>
          {isOAuth && (
            <Tabs.Tab value="oauth" leftSection={<IconKey size={14} />}>
              OAuth
            </Tabs.Tab>
          )}
          {isMeta && (
            <>
              <Tabs.Tab value="template" leftSection={<IconCode size={14} />}>
                Template
              </Tabs.Tab>
              <Tabs.Tab
                value="preprocessing"
                leftSection={<IconSettings size={14} />}
              >
                Preprocessing
              </Tabs.Tab>
              <Tabs.Tab
                value="conditions"
                leftSection={<IconSearch size={14} />}
              >
                Conditions
              </Tabs.Tab>
            </>
          )}
        </Tabs.List>

        <Stack gap="md" mt="md">
          <Tabs.Panel value="permissions">
            <PermissionsTab plugin={plugin} form={form} libraries={libraries} />
          </Tabs.Panel>

          {isOAuth && (
            <Tabs.Panel value="oauth">
              <OAuthTab plugin={plugin} form={form} />
            </Tabs.Panel>
          )}

          {isMeta && (
            <>
              <Tabs.Panel value="template">
                <TemplateTab form={form} />
              </Tabs.Panel>

              <Tabs.Panel value="preprocessing">
                <PreprocessingTab
                  preprocessingRules={preprocessingRules}
                  onPreprocessingRulesChange={setPreprocessingRules}
                  testTitle={testTitle}
                  onTestTitleChange={setTestTitle}
                />
              </Tabs.Panel>

              <Tabs.Panel value="conditions">
                <ConditionsTab
                  plugin={plugin}
                  form={form}
                  autoMatchConditions={autoMatchConditions}
                  onAutoMatchConditionsChange={setAutoMatchConditions}
                />
              </Tabs.Panel>
            </>
          )}
        </Stack>
      </Tabs>

      <Group justify="flex-end" mt="xl">
        <Button variant="subtle" onClick={onClose}>
          Cancel
        </Button>
        <Button
          onClick={() => updateMutation.mutate()}
          loading={updateMutation.isPending}
        >
          Save Changes
        </Button>
      </Group>
    </>
  );
}

// =============================================================================
// Exported modal component
// =============================================================================

interface PluginConfigModalProps {
  plugin: PluginDto;
  opened: boolean;
  onClose: () => void;
  libraries: { id: string; name: string }[];
}

/**
 * Modal for configuring plugin permissions, scopes, library access,
 * and (for metadata providers) search settings.
 *
 * Shows capability-aware tabs based on the plugin's manifest.
 */
export function PluginConfigModal({
  plugin,
  opened,
  onClose,
  libraries,
}: PluginConfigModalProps) {
  return (
    <Modal
      opened={opened}
      onClose={onClose}
      title={`Configure: ${plugin.displayName}`}
      size="lg"
      centered
    >
      {/* Key forces remount when plugin changes, resetting all form state */}
      <PluginConfigContent
        key={plugin.id}
        plugin={plugin}
        onClose={onClose}
        libraries={libraries}
      />
    </Modal>
  );
}
