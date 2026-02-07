import {
  Alert,
  Box,
  Button,
  Group,
  Loader,
  Modal,
  Stack,
  Text,
  Title,
} from "@mantine/core";
import { notifications } from "@mantine/notifications";
import { IconAlertCircle, IconPlugConnected } from "@tabler/icons-react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { userPluginsApi } from "@/api/userPlugins";
import { useOAuthCallback, useOAuthFlow } from "@/components/plugins/OAuthFlow";
import {
  AvailablePluginCard,
  ConnectedPluginCard,
} from "@/components/plugins/UserPluginCard";
import { useDocumentTitle } from "@/hooks/useDocumentTitle";

export function IntegrationsSettings() {
  useDocumentTitle("Integrations");

  // Handle OAuth callback query params (e.g., ?oauth=success)
  useOAuthCallback();

  const { startOAuthFlow } = useOAuthFlow();
  const queryClient = useQueryClient();
  const [disconnectTarget, setDisconnectTarget] = useState<string | null>(null);

  // Fetch user's plugins
  const {
    data: pluginData,
    isLoading,
    error,
  } = useQuery({
    queryKey: ["user-plugins"],
    queryFn: userPluginsApi.list,
  });

  // Enable mutation
  const enableMutation = useMutation({
    mutationFn: (pluginId: string) => userPluginsApi.enable(pluginId),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["user-plugins"] });
      notifications.show({
        title: "Plugin enabled",
        message: "The plugin has been enabled for your account.",
        color: "green",
      });
    },
    onError: (error: Error) => {
      notifications.show({
        title: "Error",
        message: error.message || "Failed to enable plugin",
        color: "red",
      });
    },
  });

  // Disconnect mutation
  const disconnectMutation = useMutation({
    mutationFn: (pluginId: string) => userPluginsApi.disconnect(pluginId),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["user-plugins"] });
      setDisconnectTarget(null);
      notifications.show({
        title: "Disconnected",
        message: "Plugin has been disconnected and data removed.",
        color: "green",
      });
    },
    onError: (error: Error) => {
      notifications.show({
        title: "Error",
        message: error.message || "Failed to disconnect plugin",
        color: "red",
      });
    },
  });

  const handleDisconnect = (pluginId: string) => {
    setDisconnectTarget(pluginId);
  };

  const confirmDisconnect = () => {
    if (disconnectTarget) {
      disconnectMutation.mutate(disconnectTarget);
    }
  };

  const handleConnect = (pluginId: string) => {
    startOAuthFlow(pluginId);
  };

  const handleEnable = (pluginId: string) => {
    enableMutation.mutate(pluginId);
  };

  if (isLoading) {
    return (
      <Box py="xl" px="md">
        <Stack align="center" gap="md" py="xl">
          <Loader />
          <Text c="dimmed">Loading integrations...</Text>
        </Stack>
      </Box>
    );
  }

  if (error) {
    return (
      <Box py="xl" px="md">
        <Alert
          icon={<IconAlertCircle size={16} />}
          title="Error loading integrations"
          color="red"
        >
          {error instanceof Error
            ? error.message
            : "An unexpected error occurred"}
        </Alert>
      </Box>
    );
  }

  const { enabled = [], available = [] } = pluginData ?? {};
  const hasNoPlugins = enabled.length === 0 && available.length === 0;

  return (
    <Box py="xl" px="md">
      <Stack gap="xl">
        <Title order={1}>Integrations</Title>

        {hasNoPlugins && (
          <Alert
            icon={<IconPlugConnected size={16} />}
            title="No integrations available"
            color="blue"
            variant="light"
          >
            No user plugins have been installed by your administrator. Contact
            your admin to install sync or recommendation plugins.
          </Alert>
        )}

        {/* Connected Services */}
        {enabled.length > 0 && (
          <Stack gap="md">
            <Title order={3}>Connected Services</Title>
            {enabled.map((plugin) => (
              <ConnectedPluginCard
                key={plugin.id}
                plugin={plugin}
                onDisconnect={handleDisconnect}
                onConnect={handleConnect}
                disconnecting={
                  disconnectMutation.isPending &&
                  disconnectTarget === plugin.pluginId
                }
              />
            ))}
          </Stack>
        )}

        {/* Available Plugins */}
        {available.length > 0 && (
          <Stack gap="md">
            <Title order={3}>Available Plugins</Title>
            <Text size="sm" c="dimmed">
              These plugins are available for you to connect. Enable them to
              sync reading progress, get recommendations, and more.
            </Text>
            {available.map((plugin) => (
              <AvailablePluginCard
                key={plugin.pluginId}
                plugin={plugin}
                onEnable={handleEnable}
                onConnect={handleConnect}
                enabling={
                  enableMutation.isPending &&
                  enableMutation.variables === plugin.pluginId
                }
              />
            ))}
          </Stack>
        )}
      </Stack>

      {/* Disconnect Confirmation Modal */}
      <Modal
        opened={disconnectTarget !== null}
        onClose={() => setDisconnectTarget(null)}
        title="Disconnect Plugin"
        centered
      >
        <Stack gap="md">
          <Text>
            Are you sure you want to disconnect this plugin? This will remove
            your credentials and all plugin data.
          </Text>
          <Group justify="flex-end">
            <Button variant="subtle" onClick={() => setDisconnectTarget(null)}>
              Cancel
            </Button>
            <Button
              color="red"
              onClick={confirmDisconnect}
              loading={disconnectMutation.isPending}
            >
              Disconnect
            </Button>
          </Group>
        </Stack>
      </Modal>
    </Box>
  );
}
