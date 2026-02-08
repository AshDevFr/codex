import {
  Avatar,
  Badge,
  Button,
  Card,
  Group,
  Text,
  Tooltip,
} from "@mantine/core";
import {
  IconCheck,
  IconLink,
  IconLinkOff,
  IconPlayerPlay,
  IconRefresh,
  IconSettings,
  IconX,
} from "@tabler/icons-react";
import type { AvailablePluginDto, UserPluginDto } from "@/api/userPlugins";

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

// =============================================================================
// Connected Plugin Card
// =============================================================================

interface ConnectedPluginCardProps {
  plugin: UserPluginDto;
  onDisconnect: (pluginId: string) => void;
  onSync?: (pluginId: string) => void;
  onSettings?: (pluginId: string) => void;
  onConnect: (pluginId: string) => void;
  disconnecting?: boolean;
  syncing?: boolean;
}

export function ConnectedPluginCard({
  plugin,
  onDisconnect,
  onSync,
  onSettings,
  onConnect,
  disconnecting,
  syncing,
}: ConnectedPluginCardProps) {
  const health = healthBadge(plugin.healthStatus);

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
        <Text size="sm" c="dimmed" mb="xs">
          Last sync: {formatTimeAgo(plugin.lastSyncAt)}
        </Text>
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
        {!plugin.connected && plugin.requiresOauth && (
          <Button
            size="xs"
            variant="filled"
            leftSection={<IconLink size={14} />}
            onClick={() => onConnect(plugin.pluginId)}
          >
            Connect
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
  onConnect: (pluginId: string) => void;
  enabling?: boolean;
}

export function AvailablePluginCard({
  plugin,
  onEnable,
  onConnect,
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

      <Group gap="xs" mt="md">
        {plugin.requiresOauth ? (
          <Button
            size="xs"
            variant="filled"
            leftSection={<IconLink size={14} />}
            loading={enabling}
            onClick={() => onConnect(plugin.pluginId)}
          >
            Connect with {plugin.displayName}
          </Button>
        ) : (
          <Button
            size="xs"
            variant="filled"
            leftSection={<IconPlayerPlay size={14} />}
            loading={enabling}
            onClick={() => onEnable(plugin.pluginId)}
          >
            Enable
          </Button>
        )}
      </Group>
    </Card>
  );
}
