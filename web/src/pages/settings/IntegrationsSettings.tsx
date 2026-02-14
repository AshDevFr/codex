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
import { useEffect, useRef, useState } from "react";
import type { UserPluginTaskDto } from "@/api/userPlugins";
import { userPluginsApi } from "@/api/userPlugins";
import { useOAuthCallback, useOAuthFlow } from "@/components/plugins/OAuthFlow";
import {
  AvailablePluginCard,
  ConnectedPluginCard,
} from "@/components/plugins/UserPluginCard";
import { UserPluginSettingsModal } from "@/components/plugins/UserPluginSettingsModal";
import { useDocumentTitle } from "@/hooks/useDocumentTitle";

function isTaskActive(task: UserPluginTaskDto | undefined): boolean {
  return task?.status === "pending" || task?.status === "processing";
}

export function IntegrationsSettings() {
  useDocumentTitle("Integrations");

  // Handle OAuth callback query params (e.g., ?oauth=success)
  useOAuthCallback();

  const { startOAuthFlow } = useOAuthFlow();
  const queryClient = useQueryClient();
  const [disconnectTarget, setDisconnectTarget] = useState<string | null>(null);
  const [settingsTarget, setSettingsTarget] = useState<string | null>(null);
  const [liveStatusPluginId, setLiveStatusPluginId] = useState<string | null>(
    null,
  );
  // Track which plugin we're actively polling for sync task status
  const [syncingPluginId, setSyncingPluginId] = useState<string | null>(null);
  const syncTaskNotifiedRef = useRef<string | null>(null);
  const checkedInitialSyncRef = useRef(false);

  // Fetch user's plugins
  const {
    data: pluginData,
    isLoading,
    error,
  } = useQuery({
    queryKey: ["user-plugins"],
    queryFn: userPluginsApi.list,
  });

  // Poll sync task for the syncing plugin until completed/failed
  const { data: syncTask } = useQuery({
    queryKey: ["plugin-sync-task", syncingPluginId],
    queryFn: () =>
      userPluginsApi.getPluginTask(syncingPluginId ?? "", "user_plugin_sync"),
    enabled: syncingPluginId !== null,
    refetchInterval: (query) => {
      const status = query.state.data?.status;
      if (status === "completed" || status === "failed") return false;
      return 3000;
    },
  });

  // Handle sync task completion
  useEffect(() => {
    if (!syncTask || !syncingPluginId) return;
    // Use taskId to deduplicate notifications (same task won't notify twice)
    if (syncTaskNotifiedRef.current === syncTask.taskId) return;

    if (syncTask.status === "completed") {
      syncTaskNotifiedRef.current = syncTask.taskId;
      setSyncingPluginId(null);
      queryClient.invalidateQueries({ queryKey: ["user-plugins"] });
      const result = syncTask.result as Record<string, unknown> | undefined;
      if (result?.skippedReason) {
        notifications.show({
          title: "Sync skipped",
          message: String(result.skippedReason),
          color: "yellow",
        });
      } else if (result) {
        const pulled = result.pulled as number | undefined;
        const pushed = result.pushed as number | undefined;
        const applied = result.applied as number | undefined;
        const pullError = result.pullError as string | undefined;
        const pushError = result.pushError as string | undefined;
        const hasErrors = !!pullError || !!pushError;

        const parts: string[] = [];
        if (pullError) {
          parts.push(`Pull failed: ${pullError}`);
        } else if (pulled != null && pulled > 0) {
          parts.push(`pulled ${pulled}`);
          if (applied != null && applied > 0) parts.push(`${applied} applied`);
        }
        if (pushError) {
          parts.push(`Push failed: ${pushError}`);
        } else if (pushed != null && pushed > 0) {
          parts.push(`pushed ${pushed}`);
        }

        notifications.show({
          title: hasErrors ? "Sync completed with errors" : "Sync completed",
          message:
            parts.join(", ") ||
            (hasErrors
              ? "Sync encountered errors"
              : "Sync finished successfully"),
          color: hasErrors ? "orange" : "green",
        });
      }
    } else if (syncTask.status === "failed") {
      syncTaskNotifiedRef.current = syncTask.taskId;
      setSyncingPluginId(null);
      notifications.show({
        title: "Sync failed",
        message: syncTask.error || "An unknown error occurred",
        color: "red",
      });
    }
  }, [syncTask, syncingPluginId, queryClient]);

  // On-demand live sync status query (only when user clicks refresh)
  const { data: liveStatus, isFetching: fetchingLiveStatus } = useQuery({
    queryKey: ["sync-status", liveStatusPluginId],
    queryFn: () =>
      liveStatusPluginId
        ? userPluginsApi.getSyncStatus(liveStatusPluginId, true)
        : Promise.reject(),
    enabled: liveStatusPluginId !== null,
    staleTime: 30_000,
  });

  // Enable mutation
  const enableMutation = useMutation({
    mutationFn: (pluginId: string) => userPluginsApi.enable(pluginId),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["user-plugins"] });
      notifications.show({
        title: "Integration enabled",
        message: "The integration has been enabled for your account.",
        color: "green",
      });
    },
    onError: (error: Error) => {
      notifications.show({
        title: "Error",
        message: error.message || "Failed to enable integration",
        color: "red",
      });
    },
  });

  // Disable mutation
  const disableMutation = useMutation({
    mutationFn: (pluginId: string) => userPluginsApi.disable(pluginId),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["user-plugins"] });
      notifications.show({
        title: "Integration disabled",
        message: "The integration has been disabled.",
        color: "green",
      });
    },
    onError: (error: Error) => {
      notifications.show({
        title: "Error",
        message: error.message || "Failed to disable integration",
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
        message: "Integration has been disconnected and data removed.",
        color: "green",
      });
    },
    onError: (error: Error) => {
      notifications.show({
        title: "Error",
        message: error.message || "Failed to disconnect integration",
        color: "red",
      });
    },
  });

  // Save token mutation
  const saveTokenMutation = useMutation({
    mutationFn: ({ pluginId, token }: { pluginId: string; token: string }) =>
      userPluginsApi.setCredentials(pluginId, token),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["user-plugins"] });
      notifications.show({
        title: "Token saved",
        message: "Your access token has been saved and encrypted.",
        color: "green",
      });
    },
    onError: (error: Error) => {
      notifications.show({
        title: "Error",
        message: error.message || "Failed to save token",
        color: "red",
      });
    },
  });

  // Sync mutation
  const syncMutation = useMutation({
    mutationFn: (pluginId: string) => userPluginsApi.triggerSync(pluginId),
    onSuccess: (data, pluginId) => {
      syncTaskNotifiedRef.current = null;
      setSyncingPluginId(pluginId);
      // Invalidate any previous cached task data for this plugin
      queryClient.invalidateQueries({
        queryKey: ["plugin-sync-task", pluginId],
      });
      notifications.show({
        title: "Sync started",
        message: data.message,
        color: "blue",
      });
    },
    onError: (error: Error) => {
      notifications.show({
        title: "Error",
        message: error.message || "Failed to start sync",
        color: "red",
      });
    },
  });

  // Detect already-running syncs on page load (runs once)
  useEffect(() => {
    if (!pluginData?.enabled) return;
    if (checkedInitialSyncRef.current) return;
    if (syncingPluginId) return;

    checkedInitialSyncRef.current = true;

    const syncPlugins = pluginData.enabled.filter(
      (p) => p.connected && p.capabilities?.readSync,
    );
    if (syncPlugins.length === 0) return;

    // Check each sync plugin for an in-progress task
    const checkRunning = async () => {
      for (const plugin of syncPlugins) {
        try {
          const task = await userPluginsApi.getPluginTask(
            plugin.pluginId,
            "user_plugin_sync",
          );
          if (isTaskActive(task)) {
            syncTaskNotifiedRef.current = null;
            setSyncingPluginId(plugin.pluginId);
            break;
          }
        } catch {
          // 404 = no task found, which is fine
        }
      }
    };
    checkRunning();
  }, [pluginData?.enabled, syncingPluginId]);

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

  const handleDisable = (pluginId: string) => {
    disableMutation.mutate(pluginId);
  };

  const handleSaveToken = (pluginId: string, token: string) => {
    saveTokenMutation.mutate({ pluginId, token });
  };

  const handleSync = (pluginId: string) => {
    syncMutation.mutate(pluginId);
  };

  const handleSettings = (pluginId: string) => {
    setSettingsTarget(pluginId);
  };

  const handleRefreshStatus = (pluginId: string) => {
    setLiveStatusPluginId(pluginId);
    queryClient.invalidateQueries({ queryKey: ["sync-status", pluginId] });
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
  const settingsPlugin = enabled.find((p) => p.pluginId === settingsTarget);

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
            No integrations have been configured by your administrator. Contact
            your admin to install plugins for sync or recommendations.
          </Alert>
        )}

        {/* Enabled Integrations */}
        {enabled.length > 0 && (
          <Stack gap="md">
            <Title order={3}>Enabled</Title>
            {enabled.map((plugin) => (
              <ConnectedPluginCard
                key={plugin.id}
                plugin={plugin}
                onDisconnect={handleDisconnect}
                onDisable={handleDisable}
                onConnect={handleConnect}
                onSaveToken={handleSaveToken}
                onSync={handleSync}
                onSettings={handleSettings}
                onRefreshStatus={handleRefreshStatus}
                syncStatus={
                  liveStatusPluginId === plugin.pluginId
                    ? (liveStatus ?? null)
                    : null
                }
                syncing={
                  (syncMutation.isPending &&
                    syncMutation.variables === plugin.pluginId) ||
                  (syncingPluginId === plugin.pluginId &&
                    isTaskActive(syncTask ?? undefined))
                }
                disconnecting={
                  disconnectMutation.isPending &&
                  disconnectTarget === plugin.pluginId
                }
                disabling={
                  disableMutation.isPending &&
                  disableMutation.variables === plugin.pluginId
                }
                savingToken={
                  saveTokenMutation.isPending &&
                  saveTokenMutation.variables?.pluginId === plugin.pluginId
                }
                refreshingStatus={
                  fetchingLiveStatus && liveStatusPluginId === plugin.pluginId
                }
              />
            ))}
          </Stack>
        )}

        {/* Available Integrations */}
        {available.length > 0 && (
          <Stack gap="md">
            <Title order={3}>Available</Title>
            <Text size="sm" c="dimmed">
              These integrations are available for you to connect. Enable them
              to sync reading progress, get recommendations, and more.
            </Text>
            {available.map((plugin) => (
              <AvailablePluginCard
                key={plugin.pluginId}
                plugin={plugin}
                onEnable={handleEnable}
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
        title="Disconnect Integration"
        centered
      >
        <Stack gap="md">
          <Text>
            Are you sure you want to disconnect this integration? This will
            remove your credentials and all synced data.
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

      {/* User Plugin Settings Modal */}
      {settingsPlugin && (
        <UserPluginSettingsModal
          plugin={settingsPlugin}
          opened={settingsTarget !== null}
          onClose={() => setSettingsTarget(null)}
        />
      )}
    </Box>
  );
}
