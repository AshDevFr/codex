import {
  ActionIcon,
  Alert,
  Avatar,
  Badge,
  Button,
  Card,
  Divider,
  Group,
  PasswordInput,
  Text,
  Tooltip,
} from "@mantine/core";
import {
  IconCheck,
  IconInfoCircle,
  IconKey,
  IconLink,
  IconLinkOff,
  IconPlayerPause,
  IconPlayerPlay,
  IconRefresh,
  IconSettings,
  IconX,
} from "@tabler/icons-react";
import { useState } from "react";
import type {
  AvailablePluginDto,
  SyncStatusDto,
  UserPluginDto,
} from "@/api/userPlugins";

// =============================================================================
// Helpers
// =============================================================================

function formatTimeAgo(dateString: string): string {
  const date = new Date(dateString);
  const now = new Date();
  const diffMs = now.getTime() - date.getTime();
  const diffMinutes = Math.floor(diffMs / 60000);
  const diffHours = Math.floor(diffMs / 3600000);
  const diffDays = Math.floor(diffMs / 86400000);

  if (diffMinutes < 1) return "just now";
  if (diffMinutes < 60)
    return `${diffMinutes} minute${diffMinutes !== 1 ? "s" : ""} ago`;
  if (diffHours < 24)
    return `${diffHours} hour${diffHours !== 1 ? "s" : ""} ago`;
  if (diffDays < 7) return `${diffDays} day${diffDays !== 1 ? "s" : ""} ago`;
  if (diffDays < 30)
    return `${Math.floor(diffDays / 7)} week${Math.floor(diffDays / 7) !== 1 ? "s" : ""} ago`;
  return date.toLocaleDateString();
}

function healthBadge(status: string): { color: string; label: string } {
  switch (status) {
    case "healthy":
      return { color: "green", label: "Healthy" };
    case "degraded":
      return { color: "yellow", label: "Degraded" };
    case "unhealthy":
      return { color: "red", label: "Unhealthy" };
    default:
      return { color: "gray", label: "Unknown" };
  }
}

function formatSyncResult(result: Record<string, unknown>): string {
  if (result.skippedReason) {
    return `Skipped: ${String(result.skippedReason)}`;
  }
  const parts: string[] = [];
  const pulled = result.pulled as number | undefined;
  const matched = result.matched as number | undefined;
  const applied = result.applied as number | undefined;
  const pushed = result.pushed as number | undefined;
  const pullError = result.pullError as string | undefined;
  const pushError = result.pushError as string | undefined;
  if (pullError) {
    parts.push("Pull failed");
  } else if (pulled != null && pulled > 0) {
    let pullPart = `Pulled ${pulled}`;
    if (matched != null) pullPart += ` (${matched} matched`;
    if (applied != null) pullPart += `, ${applied} applied`;
    if (matched != null) pullPart += ")";
    parts.push(pullPart);
  }
  if (pushError) {
    parts.push("Push failed");
  } else if (pushed != null && pushed > 0) {
    parts.push(`Pushed ${pushed}`);
  }
  return parts.join(", ") || "Sync completed";
}

// =============================================================================
// Connected Plugin Card
// =============================================================================

interface ConnectedPluginCardProps {
  plugin: UserPluginDto;
  onDisconnect: (pluginId: string) => void;
  onDisable: (pluginId: string) => void;
  onSync?: (pluginId: string) => void;
  onSettings?: (pluginId: string) => void;
  onConnect: (pluginId: string) => void;
  onSaveToken?: (pluginId: string, token: string) => void;
  onRefreshStatus?: (pluginId: string) => void;
  syncStatus?: SyncStatusDto | null;
  disconnecting?: boolean;
  disabling?: boolean;
  syncing?: boolean;
  savingToken?: boolean;
  refreshingStatus?: boolean;
}

export function ConnectedPluginCard({
  plugin,
  onDisconnect,
  onDisable,
  onSync,
  onSettings,
  onConnect,
  onSaveToken,
  onRefreshStatus,
  syncStatus,
  disconnecting,
  disabling,
  syncing,
  savingToken,
  refreshingStatus,
}: ConnectedPluginCardProps) {
  const health = healthBadge(plugin.healthStatus);
  const [tokenValue, setTokenValue] = useState("");
  const showTokenInput = !plugin.connected && plugin.requiresOauth;

  return (
    <Card withBorder padding="lg">
      <Group justify="space-between" mb="sm">
        <Group gap="sm">
          {plugin.externalAvatarUrl ? (
            <Avatar src={plugin.externalAvatarUrl} size="md" radius="xl" />
          ) : (
            <Avatar size="md" radius="xl" color="blue">
              {plugin.pluginDisplayName.charAt(0).toUpperCase()}
            </Avatar>
          )}
          <div>
            <Text fw={600} size="lg">
              {plugin.pluginDisplayName}
            </Text>
            {plugin.description && (
              <Text size="sm" c="dimmed">
                {plugin.description}
              </Text>
            )}
          </div>
        </Group>
        <Group gap="xs">
          {plugin.connected ? (
            <Badge
              color="green"
              variant="light"
              leftSection={<IconCheck size={12} />}
            >
              Connected
            </Badge>
          ) : plugin.requiresOauth ? (
            <Badge
              color="yellow"
              variant="light"
              leftSection={<IconX size={12} />}
            >
              Not Connected
            </Badge>
          ) : (
            <Badge
              color="blue"
              variant="light"
              leftSection={<IconCheck size={12} />}
            >
              Enabled
            </Badge>
          )}
          {plugin.connected && (
            <Tooltip label={`Health: ${health.label}`}>
              <Badge color={health.color} variant="dot" size="sm">
                {health.label}
              </Badge>
            </Tooltip>
          )}
        </Group>
      </Group>

      {plugin.connected && plugin.externalUsername && (
        <Text size="sm" c="dimmed" mb="xs">
          Signed in as: <strong>{plugin.externalUsername}</strong>
        </Text>
      )}

      {plugin.lastSyncAt && (
        <Group gap={4} mb={plugin.lastSyncResult ? 0 : "xs"}>
          <Text size="sm" c="dimmed">
            Last sync: {formatTimeAgo(plugin.lastSyncAt)}
          </Text>
          {onRefreshStatus && (
            <Tooltip label="Refresh sync status">
              <ActionIcon
                size="xs"
                variant="subtle"
                color="dimmed"
                onClick={() => onRefreshStatus(plugin.pluginId)}
                loading={refreshingStatus}
              >
                <IconRefresh size={14} />
              </ActionIcon>
            </Tooltip>
          )}
        </Group>
      )}

      {plugin.lastSyncResult != null ? (
        <Text size="xs" c="dimmed" mb="xs">
          {formatSyncResult(plugin.lastSyncResult as Record<string, unknown>)}
        </Text>
      ) : null}

      {plugin.connected && plugin.capabilities?.readSync && (
        <Group gap="md" mb="xs">
          {syncStatus && syncStatus.failureCount > 0 && (
            <Tooltip
              label={
                syncStatus.lastFailureAt
                  ? `Last failure: ${formatTimeAgo(syncStatus.lastFailureAt)}`
                  : "Recent failures detected"
              }
            >
              <Badge color="red" variant="light" size="sm">
                {syncStatus.failureCount} failure
                {syncStatus.failureCount !== 1 ? "s" : ""}
              </Badge>
            </Tooltip>
          )}
          {syncStatus?.externalCount != null && (
            <Text size="xs" c="dimmed">
              {syncStatus.externalCount} external entries
            </Text>
          )}
          {syncStatus?.pendingPull != null && syncStatus.pendingPull > 0 && (
            <Text size="xs" c="dimmed">
              {syncStatus.pendingPull} to pull
            </Text>
          )}
          {syncStatus?.pendingPush != null && syncStatus.pendingPush > 0 && (
            <Text size="xs" c="dimmed">
              {syncStatus.pendingPush} to push
            </Text>
          )}
        </Group>
      )}

      {plugin.userSetupInstructions && !plugin.connected && (
        <Alert
          icon={<IconInfoCircle size={16} />}
          color="blue"
          variant="light"
          mt="xs"
          mb="xs"
        >
          {plugin.userSetupInstructions}
        </Alert>
      )}

      {!plugin.connected && plugin.requiresOauth && plugin.oauthConfigured && (
        <Group mt="xs">
          <Button
            size="xs"
            variant="filled"
            leftSection={<IconLink size={14} />}
            onClick={() => onConnect(plugin.pluginId)}
          >
            Connect with OAuth
          </Button>
        </Group>
      )}

      {showTokenInput && onSaveToken && (
        <>
          {plugin.oauthConfigured && (
            <Divider
              label="or use a personal access token"
              labelPosition="center"
              mt="sm"
              mb="xs"
            />
          )}
          <Group
            gap="xs"
            mt={plugin.oauthConfigured ? undefined : "xs"}
            mb="xs"
            align="end"
          >
            <PasswordInput
              placeholder="Paste your personal access token"
              size="xs"
              style={{ flex: 1 }}
              value={tokenValue}
              onChange={(e) => setTokenValue(e.currentTarget.value)}
            />
            <Button
              size="xs"
              variant="filled"
              leftSection={<IconKey size={14} />}
              loading={savingToken}
              disabled={!tokenValue.trim()}
              onClick={() => {
                onSaveToken(plugin.pluginId, tokenValue.trim());
                setTokenValue("");
              }}
            >
              Save Token
            </Button>
          </Group>
        </>
      )}

      <Group gap="xs" mt="md">
        {plugin.connected && onSync && (
          <Button
            size="xs"
            variant="light"
            leftSection={<IconRefresh size={14} />}
            loading={syncing}
            onClick={() => onSync(plugin.pluginId)}
          >
            Sync Now
          </Button>
        )}
        {onSettings && (
          <Button
            size="xs"
            variant="subtle"
            leftSection={<IconSettings size={14} />}
            onClick={() => onSettings(plugin.pluginId)}
          >
            Settings
          </Button>
        )}
        {plugin.connected && (
          <Tooltip label="Unlink your external account and remove credentials">
            <Button
              size="xs"
              variant="subtle"
              color="red"
              leftSection={<IconLinkOff size={14} />}
              loading={disconnecting}
              onClick={() => onDisconnect(plugin.pluginId)}
            >
              Disconnect
            </Button>
          </Tooltip>
        )}
        {!plugin.connected && plugin.enabled && (
          <Tooltip label="Disable this integration">
            <Button
              size="xs"
              variant="subtle"
              color="orange"
              leftSection={<IconPlayerPause size={14} />}
              loading={disabling}
              onClick={() => onDisable(plugin.pluginId)}
            >
              Disable
            </Button>
          </Tooltip>
        )}
        {!plugin.connected && !plugin.enabled && (
          <Tooltip label="Remove this integration">
            <Button
              size="xs"
              variant="subtle"
              color="red"
              leftSection={<IconLinkOff size={14} />}
              loading={disconnecting}
              onClick={() => onDisconnect(plugin.pluginId)}
            >
              Remove
            </Button>
          </Tooltip>
        )}
      </Group>
    </Card>
  );
}

// =============================================================================
// Available Plugin Card
// =============================================================================

interface AvailablePluginCardProps {
  plugin: AvailablePluginDto;
  onEnable: (pluginId: string) => void;
  enabling?: boolean;
}

export function AvailablePluginCard({
  plugin,
  onEnable,
  enabling,
}: AvailablePluginCardProps) {
  return (
    <Card withBorder padding="lg">
      <Group justify="space-between" mb="sm">
        <Group gap="sm">
          <Avatar size="md" radius="xl" color="gray">
            {plugin.displayName.charAt(0).toUpperCase()}
          </Avatar>
          <div>
            <Text fw={600} size="lg">
              {plugin.displayName}
            </Text>
            {plugin.description && (
              <Text size="sm" c="dimmed">
                {plugin.description}
              </Text>
            )}
          </div>
        </Group>
        <Group gap="xs">
          {plugin.capabilities.readSync && (
            <Badge variant="light" color="blue" size="sm">
              Sync
            </Badge>
          )}
          {plugin.capabilities.userRecommendationProvider && (
            <Badge variant="light" color="grape" size="sm">
              Recommendations
            </Badge>
          )}
        </Group>
      </Group>

      {plugin.userSetupInstructions && (
        <Text size="sm" c="dimmed" mb="xs">
          {plugin.userSetupInstructions}
        </Text>
      )}

      <Group gap="xs" mt="md">
        <Button
          size="xs"
          variant="filled"
          leftSection={<IconPlayerPlay size={14} />}
          loading={enabling}
          onClick={() => onEnable(plugin.pluginId)}
        >
          Enable
        </Button>
      </Group>
    </Card>
  );
}
