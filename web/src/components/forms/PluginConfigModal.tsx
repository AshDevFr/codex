import {
  Alert,
  Badge,
  Button,
  Card,
  Chip,
  Code,
  CopyButton,
  Divider,
  Group,
  Modal,
  MultiSelect,
  Paper,
  PasswordInput,
  Stack,
  Switch,
  Tabs,
  Text,
  Textarea,
  TextInput,
  Tooltip,
} from "@mantine/core";
import { useForm } from "@mantine/form";
import { notifications } from "@mantine/notifications";
import {
  IconCode,
  IconCopy,
  IconInfoCircle,
  IconKey,
  IconSearch,
  IconSettings,
  IconShield,
} from "@tabler/icons-react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useMemo, useState } from "react";
import type { PluginDto } from "@/api/plugins";
import {
  AVAILABLE_PERMISSIONS,
  AVAILABLE_SCOPES,
  pluginsApi,
} from "@/api/plugins";
import { SAMPLE_SERIES_CONTEXT } from "@/utils/templateUtils";
import { type AutoMatchConditions, ConditionsEditor } from "./ConditionsEditor";
import {
  type PreprocessingRule,
  PreprocessingRulesEditor,
} from "./PreprocessingRulesEditor";

// =============================================================================
// Capability detection helpers
// =============================================================================

function isMetadataProvider(plugin: PluginDto): boolean {
  return (plugin.manifest?.capabilities?.metadataProvider?.length ?? 0) > 0;
}

function isSyncProvider(plugin: PluginDto): boolean {
  return plugin.manifest?.capabilities?.userReadSync === true;
}

function isOAuthPlugin(plugin: PluginDto): boolean {
  return plugin.manifest?.oauth != null;
}

function hasManifest(plugin: PluginDto): boolean {
  return plugin.manifest != null;
}

// =============================================================================
// Permission grouping by capability
// =============================================================================

const METADATA_PERMISSION_VALUES = new Set(
  AVAILABLE_PERMISSIONS.filter((p) => p.value.startsWith("metadata:")).map(
    (p) => p.value,
  ),
);

const LIBRARY_PERMISSION_VALUES = new Set(
  AVAILABLE_PERMISSIONS.filter((p) => p.value.startsWith("library:")).map(
    (p) => p.value,
  ),
);

function getPermissionData(plugin: PluginDto) {
  const isMeta = isMetadataProvider(plugin);
  const isSync = isSyncProvider(plugin);
  const noManifest = !hasManifest(plugin);

  if (noManifest) {
    // No manifest: show all with a note
    return {
      data: [
        {
          group: "Metadata",
          items: AVAILABLE_PERMISSIONS.filter((p) =>
            METADATA_PERMISSION_VALUES.has(p.value),
          ).map((p) => ({ value: p.value, label: p.label })),
        },
        {
          group: "Library",
          items: AVAILABLE_PERMISSIONS.filter((p) =>
            LIBRARY_PERMISSION_VALUES.has(p.value),
          ).map((p) => ({ value: p.value, label: p.label })),
        },
      ],
      showNoManifestWarning: true,
    };
  }

  const groups: { group: string; items: { value: string; label: string }[] }[] =
    [];

  if (isMeta) {
    groups.push({
      group: "Metadata",
      items: AVAILABLE_PERMISSIONS.filter((p) =>
        METADATA_PERMISSION_VALUES.has(p.value),
      ).map((p) => ({ value: p.value, label: p.label })),
    });
  }

  if (isSync || isMeta) {
    // Library read is useful for both sync and metadata providers
    groups.push({
      group: "Library",
      items: AVAILABLE_PERMISSIONS.filter((p) =>
        LIBRARY_PERMISSION_VALUES.has(p.value),
      ).map((p) => ({ value: p.value, label: p.label })),
    });
  }

  return { data: groups, showNoManifestWarning: false };
}

// =============================================================================
// Scope filtering by capability
// =============================================================================

// Mirrors backend PluginScope::series_scopes()
const SERIES_SCOPES = new Set([
  "series:detail",
  "series:bulk",
  "library:detail",
  "library:scan",
]);

// Mirrors backend PluginScope::book_scopes()
const BOOK_SCOPES = new Set([
  "book:detail",
  "book:bulk",
  "library:detail",
  "library:scan",
]);

// Sync providers operate at series/library level
const SYNC_SCOPES = new Set([
  "series:detail",
  "library:detail",
  "library:scan",
]);

function getScopeData(plugin: PluginDto) {
  const noManifest = !hasManifest(plugin);

  if (noManifest) {
    return AVAILABLE_SCOPES.map((s) => ({ value: s.value, label: s.label }));
  }

  const metadataTargets = plugin.manifest?.capabilities?.metadataProvider ?? [];
  const canSeries = metadataTargets.includes("series");
  const canBook = metadataTargets.includes("book");
  const isSync = isSyncProvider(plugin);

  const allowed = new Set<string>();
  if (canSeries) for (const s of SERIES_SCOPES) allowed.add(s);
  if (canBook) for (const s of BOOK_SCOPES) allowed.add(s);
  if (isSync) for (const s of SYNC_SCOPES) allowed.add(s);

  return AVAILABLE_SCOPES.filter((s) => allowed.has(s.value)).map((s) => ({
    value: s.value,
    label: s.label,
  }));
}

// =============================================================================
// Template helpers (preserved from SearchConfigModal)
// =============================================================================

const TEMPLATE_HELPERS = [
  {
    name: "clean",
    example: "{{clean metadata.title}}",
    description: "Remove noise (Digital, year, etc.)",
  },
  {
    name: "truncate",
    example: "{{truncate metadata.title 50}}",
    description: "Limit to N characters",
  },
  {
    name: "first_word",
    example: "{{first_word metadata.title}}",
    description: "First word only",
  },
  {
    name: "lowercase",
    example: "{{lowercase metadata.title}}",
    description: "Convert to lowercase",
  },
] as const;

function renderTemplatePreview(template: string): string {
  if (!template.trim()) return "(default: series title)";

  let preview = template;
  const ctx = SAMPLE_SERIES_CONTEXT;
  const meta = ctx.metadata;

  preview = preview.replace(/\{\{bookCount\}\}/g, String(ctx.bookCount ?? 0));
  preview = preview.replace(/\{\{seriesId\}\}/g, ctx.seriesId ?? "");

  preview = preview.replace(/\{\{metadata\.title\}\}/g, meta?.title ?? "");
  preview = preview.replace(
    /\{\{metadata\.titleSort\}\}/g,
    meta?.titleSort ?? "",
  );
  preview = preview.replace(
    /\{\{metadata\.year\}\}/g,
    String(meta?.year ?? ""),
  );
  preview = preview.replace(
    /\{\{metadata\.publisher\}\}/g,
    meta?.publisher ?? "",
  );
  preview = preview.replace(
    /\{\{metadata\.language\}\}/g,
    meta?.language ?? "",
  );
  preview = preview.replace(/\{\{metadata\.status\}\}/g, meta?.status ?? "");
  preview = preview.replace(
    /\{\{metadata\.ageRating\}\}/g,
    String(meta?.ageRating ?? ""),
  );
  preview = preview.replace(
    /\{\{metadata\.genres\}\}/g,
    meta?.genres?.join(", ") ?? "",
  );
  preview = preview.replace(
    /\{\{metadata\.tags\}\}/g,
    meta?.tags?.join(", ") ?? "",
  );

  preview = preview.replace(/\{\{clean metadata\.title\}\}/g, "One Piece");
  preview = preview.replace(
    /\{\{truncate metadata\.title \d+\}\}/g,
    "One Piece (D...",
  );
  preview = preview.replace(/\{\{first_word metadata\.title\}\}/g, "One");
  preview = preview.replace(
    /\{\{lowercase metadata\.title\}\}/g,
    "one piece (digital)",
  );

  preview = preview.replace(/\{\{#if [\w.]+\}\}(.*?)\{\{\/if\}\}/g, "$1");
  preview = preview.replace(/\{\{#unless [\w.]+\}\}(.*?)\{\{\/unless\}\}/g, "");

  return preview || "(empty)";
}

// =============================================================================
// Form types
// =============================================================================

type MetadataTarget = "series" | "book";

interface PluginConfigFormValues {
  // Permissions & Access
  permissions: string[];
  scopes: string[];
  allLibraries: boolean;
  libraryIds: string[];
  // Search config (metadata providers only)
  searchQueryTemplate: string;
  useExistingExternalId: boolean;
  metadataTargets: MetadataTarget[];
  // OAuth config (OAuth plugins only)
  oauthClientId: string;
  oauthClientSecret: string;
}

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
      // Permissions & Access
      permissions: plugin.permissions,
      scopes: plugin.scopes,
      allLibraries: plugin.libraryIds.length === 0,
      libraryIds: plugin.libraryIds,
      // Search config
      searchQueryTemplate: plugin.searchQueryTemplate ?? "",
      useExistingExternalId: plugin.useExistingExternalId ?? true,
      metadataTargets: initialMetadataTargets,
      // OAuth config
      oauthClientId: initialOAuthClientId,
      oauthClientSecret: initialOAuthClientSecret,
    },
  });

  // Live preview of the template
  const templatePreview = useMemo(
    () => renderTemplatePreview(form.values.searchQueryTemplate),
    [form.values.searchQueryTemplate],
  );

  // Permission and scope data filtered by capability
  const permissionInfo = getPermissionData(plugin);
  const scopeData = getScopeData(plugin);

  const updateMutation = useMutation({
    mutationFn: async () => {
      // Always send permissions & access fields
      const payload: Record<string, unknown> = {
        permissions: form.values.permissions,
        scopes: form.values.scopes,
        libraryIds: form.values.allLibraries ? [] : form.values.libraryIds,
      };

      // Only include search config fields for metadata providers
      if (isMeta) {
        payload.searchQueryTemplate =
          form.values.searchQueryTemplate.trim() || null;
        payload.searchPreprocessingRules = preprocessingRules;
        payload.autoMatchConditions = autoMatchConditions;
        payload.useExistingExternalId = form.values.useExistingExternalId;
        payload.metadataTargets = form.values.metadataTargets;
      }

      // Merge OAuth config into plugin.config JSON
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

  const handleSubmit = () => {
    updateMutation.mutate();
  };

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
          {/* ============================================================= */}
          {/* Permissions & Access Tab                                       */}
          {/* ============================================================= */}
          <Tabs.Panel value="permissions">
            <Stack gap="md">
              {permissionInfo.showNoManifestWarning && (
                <Alert
                  icon={<IconInfoCircle size={16} />}
                  color="yellow"
                  variant="light"
                >
                  <Text size="sm">
                    This plugin has not been tested yet. Test the connection to
                    discover its capabilities. All permissions are shown below.
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
          </Tabs.Panel>

          {/* ============================================================= */}
          {/* OAuth Tab (OAuth plugins only)                                */}
          {/* ============================================================= */}
          {isOAuth && (
            <Tabs.Panel value="oauth">
              <Stack gap="md">
                {plugin.manifest?.adminSetupInstructions && (
                  <Alert
                    icon={<IconInfoCircle size={16} />}
                    color="blue"
                    variant="light"
                  >
                    <Text size="sm" style={{ whiteSpace: "pre-line" }}>
                      {plugin.manifest.adminSetupInstructions}
                    </Text>
                  </Alert>
                )}

                <Paper p="sm" withBorder>
                  <Stack gap="xs">
                    <Text size="xs" c="dimmed" tt="uppercase" fw={600}>
                      OAuth Callback URL
                    </Text>
                    <Text size="xs" c="dimmed">
                      Set this as the redirect URL in your OAuth provider
                      settings.
                    </Text>
                    <Group gap="xs">
                      <Code style={{ fontSize: 12, flex: 1 }}>
                        {`${window.location.origin}/api/v1/user/plugins/oauth/callback`}
                      </Code>
                      <CopyButton
                        value={`${window.location.origin}/api/v1/user/plugins/oauth/callback`}
                      >
                        {({ copied, copy }) => (
                          <Tooltip label={copied ? "Copied" : "Copy"} withArrow>
                            <Button
                              size="compact-xs"
                              variant="subtle"
                              onClick={copy}
                              leftSection={<IconCopy size={14} />}
                            >
                              {copied ? "Copied" : "Copy"}
                            </Button>
                          </Tooltip>
                        )}
                      </CopyButton>
                    </Group>
                  </Stack>
                </Paper>

                <TextInput
                  label="OAuth Client ID"
                  placeholder="Enter the client ID from your OAuth provider"
                  description="Required for OAuth flow. Users cannot connect via OAuth without this."
                  {...form.getInputProps("oauthClientId")}
                />

                <PasswordInput
                  label="OAuth Client Secret"
                  placeholder="Enter the client secret (optional for some providers)"
                  description="Some providers require a client secret for token exchange."
                  {...form.getInputProps("oauthClientSecret")}
                />

                {plugin.manifest?.oauth && (
                  <Paper p="sm" withBorder bg="var(--mantine-color-dark-7)">
                    <Stack gap="xs">
                      <Text size="xs" fw={500}>
                        OAuth Endpoints (from manifest)
                      </Text>
                      <Group gap="xl">
                        <div>
                          <Text size="xs" c="dimmed">
                            Authorization URL
                          </Text>
                          <Text size="xs" ff="monospace">
                            {plugin.manifest.oauth.authorizationUrl}
                          </Text>
                        </div>
                        <div>
                          <Text size="xs" c="dimmed">
                            Token URL
                          </Text>
                          <Text size="xs" ff="monospace">
                            {plugin.manifest.oauth.tokenUrl}
                          </Text>
                        </div>
                      </Group>
                      {plugin.manifest.oauth.scopes &&
                        plugin.manifest.oauth.scopes.length > 0 && (
                          <div>
                            <Text size="xs" c="dimmed">
                              Scopes
                            </Text>
                            <Text size="xs" ff="monospace">
                              {plugin.manifest.oauth.scopes.join(", ")}
                            </Text>
                          </div>
                        )}
                    </Stack>
                  </Paper>
                )}
              </Stack>
            </Tabs.Panel>
          )}

          {/* ============================================================= */}
          {/* Search Template Tab (metadata providers only)                  */}
          {/* ============================================================= */}
          {isMeta && (
            <Tabs.Panel value="template">
              <Stack gap="md">
                <Alert
                  icon={<IconInfoCircle size={16} />}
                  color="blue"
                  variant="light"
                >
                  <Text size="sm">
                    Customize the search query using Handlebars syntax. The
                    template has access to series context data shown below.
                  </Text>
                </Alert>

                <Stack gap="xs">
                  <Text fw={500} size="sm">
                    Search Query Template
                  </Text>
                  <Textarea
                    placeholder="{{metadata.title}}"
                    rows={2}
                    styles={{ input: { fontFamily: "monospace" } }}
                    {...form.getInputProps("searchQueryTemplate")}
                  />

                  <Group gap="xs" align="center">
                    <Text size="xs" c="dimmed">
                      Helpers:
                    </Text>
                    {TEMPLATE_HELPERS.map((helper) => (
                      <Tooltip
                        key={helper.name}
                        label={`${helper.description} — ${helper.example}`}
                      >
                        <Badge
                          size="xs"
                          variant="light"
                          color="blue"
                          style={{ cursor: "help", textTransform: "none" }}
                        >
                          {helper.name}
                        </Badge>
                      </Tooltip>
                    ))}
                  </Group>

                  <Paper p="xs" withBorder bg="var(--mantine-color-dark-7)">
                    <Group gap="xs">
                      <Text size="xs" c="dimmed">
                        Result:
                      </Text>
                      <Text size="xs" ff="monospace">
                        {templatePreview}
                      </Text>
                    </Group>
                  </Paper>
                </Stack>

                <Card padding="sm" withBorder bg="var(--mantine-color-dark-7)">
                  <Stack gap="xs">
                    <Group justify="space-between" align="center">
                      <Text size="xs" fw={500}>
                        Available Context
                      </Text>
                      <Text size="xs" c="dimmed">
                        Access fields using dot notation, e.g.,{" "}
                        <Code style={{ fontSize: 10 }}>
                          {"{{metadata.title}}"}
                        </Code>
                      </Text>
                    </Group>
                    <Textarea
                      size="xs"
                      value={JSON.stringify(SAMPLE_SERIES_CONTEXT, null, 2)}
                      readOnly
                      rows={10}
                      styles={{
                        input: { fontFamily: "monospace", fontSize: "11px" },
                      }}
                    />
                  </Stack>
                </Card>
              </Stack>
            </Tabs.Panel>
          )}

          {/* ============================================================= */}
          {/* Preprocessing Tab (metadata providers only)                    */}
          {/* ============================================================= */}
          {isMeta && (
            <Tabs.Panel value="preprocessing">
              <Stack gap="md">
                <Alert
                  icon={<IconInfoCircle size={16} />}
                  color="blue"
                  variant="light"
                >
                  <Text size="sm">
                    Transform series titles before metadata search. Rules are
                    applied in order, before the search query template.
                  </Text>
                </Alert>

                <PreprocessingRulesEditor
                  value={preprocessingRules}
                  onChange={setPreprocessingRules}
                  testInput={testTitle}
                  onTestInputChange={setTestTitle}
                  label="Title Preprocessing Rules"
                  description="Transform series titles before metadata search. Rules are applied in order."
                />
              </Stack>
            </Tabs.Panel>
          )}

          {/* ============================================================= */}
          {/* Conditions Tab (metadata providers only)                       */}
          {/* ============================================================= */}
          {isMeta && (
            <Tabs.Panel value="conditions">
              <Stack gap="md">
                <Alert
                  icon={<IconInfoCircle size={16} />}
                  color="blue"
                  variant="light"
                >
                  <Text size="sm">
                    Define conditions that control when auto-matching runs for
                    this plugin. Without conditions, auto-matching will run for
                    all series.
                  </Text>
                </Alert>

                <Stack gap={4}>
                  <Text fw={500} size="sm">
                    Metadata Targets
                  </Text>
                  <Text size="xs" c="dimmed">
                    Which resource types should this plugin auto-match against?
                    Options are limited to the plugin&apos;s capabilities.
                  </Text>
                  <Chip.Group
                    multiple
                    value={form.values.metadataTargets}
                    onChange={(value) =>
                      form.setFieldValue(
                        "metadataTargets",
                        value as MetadataTarget[],
                      )
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
                      <Chip
                        value="book"
                        disabled={!canBook}
                        size="sm"
                        variant="outline"
                      >
                        Books
                      </Chip>
                    </Group>
                  </Chip.Group>
                  {!canSeries && !canBook && (
                    <Text size="xs" c="yellow">
                      This plugin has no manifest yet. Test the connection to
                      discover its capabilities.
                    </Text>
                  )}
                </Stack>

                <Switch
                  label="Use Existing External ID"
                  description="Skip search when series already has an external ID from this plugin"
                  checked={form.values.useExistingExternalId}
                  onChange={(e) =>
                    form.setFieldValue(
                      "useExistingExternalId",
                      e.currentTarget.checked,
                    )
                  }
                />

                <ConditionsEditor
                  value={autoMatchConditions}
                  onChange={setAutoMatchConditions}
                  label="Auto-Match Conditions"
                  description="Define conditions that must be met for auto-matching to run."
                />
              </Stack>
            </Tabs.Panel>
          )}
        </Stack>
      </Tabs>

      <Group justify="flex-end" mt="xl">
        <Button variant="subtle" onClick={onClose}>
          Cancel
        </Button>
        <Button onClick={handleSubmit} loading={updateMutation.isPending}>
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
