import {
  Alert,
  Badge,
  Code,
  Divider,
  Grid,
  Group,
  Stack,
  Text,
} from "@mantine/core";
import { IconAlertCircle, IconUsers } from "@tabler/icons-react";
import type { PluginDto } from "@/api/plugins";
import { PluginFailureHistory } from "./PluginFailures";
import { ConfigSchemaHelp } from "./PluginForm";

// Metadata provider configuration details (left column)
function MetadataConfigDetails({ plugin }: { plugin: PluginDto }) {
  const isMetadataProvider =
    plugin.manifest?.capabilities?.metadataProvider &&
    plugin.manifest.capabilities.metadataProvider.length > 0;
  if (!isMetadataProvider) return null;

  const preprocessingRulesCount = Array.isArray(plugin.searchPreprocessingRules)
    ? (plugin.searchPreprocessingRules as unknown[]).length
    : 0;

  const autoMatchConditionsCount =
    plugin.autoMatchConditions &&
    typeof plugin.autoMatchConditions === "object" &&
    "rules" in (plugin.autoMatchConditions as Record<string, unknown>) &&
    Array.isArray((plugin.autoMatchConditions as Record<string, unknown>).rules)
      ? (
          (plugin.autoMatchConditions as Record<string, unknown>)
            .rules as unknown[]
        ).length
      : 0;

  return (
    <Group gap="xl">
      <div>
        <Text size="xs" c="dimmed" tt="uppercase" fw={600}>
          External ID
        </Text>
        <Badge
          variant="light"
          size="sm"
          color={plugin.useExistingExternalId ? "violet" : "gray"}
          mt={4}
        >
          {plugin.useExistingExternalId ? "Prioritized" : "Not prioritized"}
        </Badge>
      </div>
      <div>
        <Text size="xs" c="dimmed" tt="uppercase" fw={600}>
          Search Template
        </Text>
        <Badge
          variant="light"
          size="sm"
          color={plugin.searchQueryTemplate ? "blue" : "gray"}
          mt={4}
        >
          {plugin.searchQueryTemplate ? "Custom" : "Default"}
        </Badge>
      </div>
      <div>
        <Text size="xs" c="dimmed" tt="uppercase" fw={600}>
          Preprocessing Rules
        </Text>
        <Text size="sm" mt={4}>
          {preprocessingRulesCount}{" "}
          {preprocessingRulesCount === 1 ? "rule" : "rules"}
        </Text>
      </div>
      <div>
        <Text size="xs" c="dimmed" tt="uppercase" fw={600}>
          Auto-Match Conditions
        </Text>
        <Text size="sm" mt={4}>
          {autoMatchConditionsCount}{" "}
          {autoMatchConditionsCount === 1 ? "condition" : "conditions"}
        </Text>
      </div>
    </Group>
  );
}

// Metadata provider manifest details (right column)
function MetadataManifestDetails({ plugin }: { plugin: PluginDto }) {
  const metadataProvider = plugin.manifest?.capabilities?.metadataProvider;
  if (!metadataProvider || metadataProvider.length === 0) return null;

  return (
    <div>
      <Text size="xs" c="dimmed" tt="uppercase" fw={600}>
        Metadata Targets
      </Text>
      <Group gap="xs" mt={4}>
        {(["series", "book"] as const).map((target) => {
          const pluginSupports = metadataProvider.includes(target);
          if (!pluginSupports) return null;
          const isActive = plugin.metadataTargets
            ? plugin.metadataTargets.includes(target)
            : true; // null = auto (all capabilities active)
          return (
            <Badge
              key={target}
              variant={isActive ? "light" : "outline"}
              color={isActive ? "teal" : "gray"}
              size="sm"
            >
              {target === "series" ? "Series" : "Books"}
            </Badge>
          );
        })}
      </Group>
    </div>
  );
}

// External ID source manifest details (right column)
// Shown for any plugin that declares an external ID source (sync, recommendations, etc.)
function ExternalIdSourceDetails({ plugin }: { plugin: PluginDto }) {
  const externalIdSource = plugin.manifest?.capabilities?.externalIdSource;

  if (!externalIdSource) return null;

  return (
    <div>
      <Text size="xs" c="dimmed" tt="uppercase" fw={600}>
        External ID Source
      </Text>
      <Badge variant="light" size="sm" color="violet" mt={4}>
        {externalIdSource}
      </Badge>
    </div>
  );
}

// OAuth manifest details (right column)
function OAuthManifestDetails({ plugin }: { plugin: PluginDto }) {
  if (!plugin.manifest?.oauth) return null;

  const { oauth } = plugin.manifest;
  const pluginConfig = plugin.config as Record<string, unknown> | null;
  const hasClientId =
    typeof pluginConfig?.oauth_client_id === "string" &&
    pluginConfig.oauth_client_id !== "";

  return (
    <Stack gap="xs">
      <Text size="xs" c="dimmed" tt="uppercase" fw={600}>
        OAuth Configuration
      </Text>
      <Group gap="xs">
        <Badge
          variant="light"
          size="sm"
          color={hasClientId ? "green" : "yellow"}
        >
          {hasClientId ? "Client ID configured" : "Client ID not set"}
        </Badge>
        {oauth.pkce && (
          <Badge variant="light" size="sm" color="blue">
            PKCE
          </Badge>
        )}
      </Group>
      <Group gap="xl">
        <div>
          <Text size="xs" c="dimmed">
            Auth URL
          </Text>
          <Code style={{ fontSize: 11 }}>{oauth.authorizationUrl}</Code>
        </div>
        <div>
          <Text size="xs" c="dimmed">
            Token URL
          </Text>
          <Code style={{ fontSize: 11 }}>{oauth.tokenUrl}</Code>
        </div>
      </Group>
      {plugin.manifest.adminSetupInstructions && (
        <Alert variant="light" color="blue" p="xs">
          <Text size="xs" style={{ whiteSpace: "pre-line" }}>
            {plugin.manifest.adminSetupInstructions}
          </Text>
        </Alert>
      )}
    </Stack>
  );
}

export function PluginDetails({
  plugin,
  libraries,
}: {
  plugin: PluginDto;
  libraries: { id: string; name: string }[];
}) {
  return (
    <Grid gutter="xl">
      {/* Left column: plugin configuration */}
      <Grid.Col span={{ base: 12, md: 6 }}>
        <Stack gap="sm">
          <Divider label="Configuration" labelPosition="left" />
          <Group gap="xl">
            <div>
              <Text size="xs" c="dimmed" tt="uppercase" fw={600}>
                Description
              </Text>
              <Text size="sm">{plugin.description || "No description"}</Text>
            </div>
            <div>
              <Text size="xs" c="dimmed" tt="uppercase" fw={600}>
                Credentials
              </Text>
              <Text size="sm">
                {plugin.hasCredentials ? "Configured" : "Not configured"}
              </Text>
            </div>
            <div>
              <Text size="xs" c="dimmed" tt="uppercase" fw={600}>
                Delivery Method
              </Text>
              <Text size="sm">{plugin.credentialDelivery}</Text>
            </div>
            <div>
              <Text size="xs" c="dimmed" tt="uppercase" fw={600}>
                Rate Limit
              </Text>
              <Text size="sm">
                {plugin.rateLimitRequestsPerMinute != null
                  ? `${plugin.rateLimitRequestsPerMinute} req/min`
                  : "No limit"}
              </Text>
            </div>
            {plugin.userCount != null && (
              <div>
                <Text size="xs" c="dimmed" tt="uppercase" fw={600}>
                  Users
                </Text>
                <Group gap={6} mt={4}>
                  <IconUsers size={14} color="var(--mantine-color-dimmed)" />
                  <Text size="sm">
                    {plugin.userCount}{" "}
                    {plugin.userCount === 1 ? "user" : "users"}
                  </Text>
                </Group>
              </div>
            )}
          </Group>

          {plugin.args.length > 0 && (
            <div>
              <Text size="xs" c="dimmed" tt="uppercase" fw={600}>
                Arguments
              </Text>
              <Code block>{plugin.args.join("\n")}</Code>
            </div>
          )}

          <Group gap="xl">
            <div>
              <Text size="xs" c="dimmed" tt="uppercase" fw={600}>
                Permissions
              </Text>
              <Group gap="xs" mt={4}>
                {plugin.permissions.length > 0 ? (
                  plugin.permissions.map((perm) => (
                    <Badge key={perm} variant="outline" size="sm">
                      {perm}
                    </Badge>
                  ))
                ) : (
                  <Text size="sm" c="dimmed">
                    None
                  </Text>
                )}
              </Group>
            </div>
            <div>
              <Text size="xs" c="dimmed" tt="uppercase" fw={600}>
                Scopes
              </Text>
              <Group gap="xs" mt={4}>
                {plugin.scopes.length > 0 ? (
                  plugin.scopes.map((scope) => (
                    <Badge key={scope} variant="outline" size="sm" color="blue">
                      {scope}
                    </Badge>
                  ))
                ) : (
                  <Text size="sm" c="dimmed">
                    None
                  </Text>
                )}
              </Group>
            </div>
            <div>
              <Text size="xs" c="dimmed" tt="uppercase" fw={600}>
                Libraries
              </Text>
              <Group gap="xs" mt={4}>
                {plugin.libraryIds.length === 0 ? (
                  <Badge variant="light" size="sm" color="gray">
                    All Libraries
                  </Badge>
                ) : (
                  plugin.libraryIds.map((libId) => {
                    const lib = libraries.find((l) => l.id === libId);
                    return (
                      <Badge
                        key={libId}
                        variant="outline"
                        size="sm"
                        color="cyan"
                      >
                        {lib?.name || libId}
                      </Badge>
                    );
                  })
                )}
              </Group>
            </div>
          </Group>

          <MetadataConfigDetails plugin={plugin} />

          {plugin.disabledReason && (
            <Alert
              icon={<IconAlertCircle size={16} />}
              color="red"
              variant="outline"
            >
              <Text fw={500} c="red.4">
                Disabled Reason
              </Text>
              <Text size="sm" c="dimmed">
                {plugin.disabledReason}
              </Text>
            </Alert>
          )}
        </Stack>
      </Grid.Col>

      {/* Right column: manifest & config schema */}
      {plugin.manifest && (
        <Grid.Col span={{ base: 12, md: 6 }}>
          <Stack gap="sm">
            <Divider label="Manifest" labelPosition="left" />
            <Group gap="xl">
              <div>
                <Text size="xs" c="dimmed" tt="uppercase" fw={600}>
                  Version
                </Text>
                <Text size="sm">{plugin.manifest.version}</Text>
              </div>
              <div>
                <Text size="xs" c="dimmed" tt="uppercase" fw={600}>
                  Protocol
                </Text>
                <Text size="sm">v{plugin.manifest.protocolVersion}</Text>
              </div>
              {plugin.manifest.author && (
                <div>
                  <Text size="xs" c="dimmed" tt="uppercase" fw={600}>
                    Author
                  </Text>
                  <Text size="sm">{plugin.manifest.author}</Text>
                </div>
              )}
            </Group>
            <Group gap="xs">
              {plugin.manifest.capabilities.metadataProvider &&
                plugin.manifest.capabilities.metadataProvider.length > 0 && (
                  <Badge color="teal" variant="light">
                    Metadata Provider
                  </Badge>
                )}
              {plugin.manifest.capabilities.userReadSync && (
                <Badge color="violet" variant="light">
                  Reading Sync
                </Badge>
              )}
              {plugin.manifest.capabilities.userRecommendationProvider && (
                <Badge color="orange" variant="light">
                  Recommendation Provider
                </Badge>
              )}
            </Group>

            <MetadataManifestDetails plugin={plugin} />
            <ExternalIdSourceDetails plugin={plugin} />
            <OAuthManifestDetails plugin={plugin} />

            {plugin.manifest.configSchema && (
              <ConfigSchemaHelp schema={plugin.manifest.configSchema} />
            )}
          </Stack>
        </Grid.Col>
      )}

      {/* Failure history spans full width */}
      <Grid.Col span={12}>
        <PluginFailureHistory pluginId={plugin.id} />
      </Grid.Col>
    </Grid>
  );
}
