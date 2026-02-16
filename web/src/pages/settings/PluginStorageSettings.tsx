import {
  ActionIcon,
  Alert,
  Box,
  Button,
  Card,
  Group,
  Loader,
  Modal,
  SimpleGrid,
  Stack,
  Table,
  Text,
  Title,
} from "@mantine/core";
import { notifications } from "@mantine/notifications";
import {
  IconAlertCircle,
  IconDatabase,
  IconFile,
  IconPlugConnected,
  IconRefresh,
  IconTrash,
} from "@tabler/icons-react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import type {
  AllPluginStorageStatsDto,
  PluginCleanupResultDto,
  PluginStorageStatsDto,
} from "@/api/pluginStorage";
import { pluginStorageApi } from "@/api/pluginStorage";

function formatBytes(bytes: number): string {
  if (bytes === 0) return "0 B";
  const k = 1024;
  const sizes = ["B", "KB", "MB", "GB", "TB"];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return `${Number.parseFloat((bytes / k ** i).toFixed(2))} ${sizes[i]}`;
}

// Stat card component
function StatCard({
  title,
  value,
  color,
  icon,
}: {
  title: string;
  value: string | number;
  color: string;
  icon: React.ReactNode;
}) {
  return (
    <Card withBorder padding="md">
      <Group justify="space-between">
        <div>
          <Text size="xs" c="dimmed" tt="uppercase" fw={700}>
            {title}
          </Text>
          <Text size="xl" fw={700}>
            {typeof value === "number" ? value.toLocaleString() : value}
          </Text>
        </div>
        <Box c={color}>{icon}</Box>
      </Group>
    </Card>
  );
}

export function PluginStorageSettings() {
  const queryClient = useQueryClient();
  const [cleanupTarget, setCleanupTarget] =
    useState<PluginStorageStatsDto | null>(null);

  // Fetch plugin storage stats
  const {
    data: stats,
    isLoading,
    refetch,
  } = useQuery<AllPluginStorageStatsDto>({
    queryKey: ["plugin-storage-stats"],
    queryFn: () => pluginStorageApi.getStats(),
  });

  // Cleanup mutation
  const cleanupMutation = useMutation<PluginCleanupResultDto, Error, string>({
    mutationFn: (name: string) => pluginStorageApi.cleanupPlugin(name),
    onSuccess: (data, name) => {
      setCleanupTarget(null);
      queryClient.invalidateQueries({ queryKey: ["plugin-storage-stats"] });

      if (data.filesDeleted > 0) {
        notifications.show({
          title: "Storage Cleaned Up",
          message: `Deleted ${data.filesDeleted.toLocaleString()} files from "${name}", freed ${formatBytes(data.bytesFreed)}`,
          color: "green",
        });
      } else {
        notifications.show({
          title: "Storage Cleaned Up",
          message: `Plugin "${name}" had no files to delete`,
          color: "blue",
        });
      }

      if (data.failures > 0) {
        notifications.show({
          title: "Some Files Failed to Delete",
          message: `${data.failures} files could not be deleted`,
          color: "orange",
        });
      }
    },
    onError: (_error, name) => {
      notifications.show({
        title: "Error",
        message: `Failed to clean up storage for "${name}"`,
        color: "red",
      });
    },
  });

  const hasPlugins = (stats?.plugins.length || 0) > 0;

  if (isLoading) {
    return (
      <Box py="xl" px="md">
        <Stack gap="xl" align="center">
          <Loader size="lg" />
          <Text c="dimmed">Loading plugin storage statistics...</Text>
        </Stack>
      </Box>
    );
  }

  return (
    <Box py="xl" px="md">
      <Stack gap="xl">
        {/* Header */}
        <Group justify="space-between">
          <div>
            <Title order={1}>Plugin Storage</Title>
            <Text c="dimmed" size="sm">
              Manage plugin file storage
            </Text>
          </div>
          <Button
            variant="light"
            leftSection={<IconRefresh size={16} />}
            onClick={() => refetch()}
          >
            Refresh
          </Button>
        </Group>

        {/* Info Alert */}
        <Alert
          icon={<IconAlertCircle size={16} />}
          color="blue"
          title="About Plugin Storage"
        >
          Plugins store file-based data (databases, caches) in isolated
          directories. This page shows disk usage per plugin and lets you clean
          up storage.
        </Alert>

        {/* Summary Stat Cards */}
        <SimpleGrid cols={{ base: 1, sm: 3 }} spacing="md">
          <StatCard
            title="Plugins with Storage"
            value={stats?.plugins.length || 0}
            color={hasPlugins ? "blue" : "gray"}
            icon={<IconPlugConnected size={32} />}
          />
          <StatCard
            title="Total Files"
            value={stats?.totalFileCount || 0}
            color={hasPlugins ? "blue" : "gray"}
            icon={<IconFile size={32} />}
          />
          <StatCard
            title="Total Size"
            value={formatBytes(stats?.totalBytes || 0)}
            color={hasPlugins ? "blue" : "gray"}
            icon={<IconDatabase size={32} />}
          />
        </SimpleGrid>

        {/* Per-Plugin Table */}
        <Card withBorder>
          <Stack gap="md">
            <Title order={4}>Per-Plugin Storage</Title>
            {hasPlugins ? (
              <Table striped highlightOnHover>
                <Table.Thead>
                  <Table.Tr>
                    <Table.Th>Plugin Name</Table.Th>
                    <Table.Th>File Count</Table.Th>
                    <Table.Th>Size</Table.Th>
                    <Table.Th>Actions</Table.Th>
                  </Table.Tr>
                </Table.Thead>
                <Table.Tbody>
                  {stats?.plugins.map((plugin) => (
                    <Table.Tr key={plugin.pluginName}>
                      <Table.Td>
                        <Text fw={500}>{plugin.pluginName}</Text>
                      </Table.Td>
                      <Table.Td>{plugin.fileCount.toLocaleString()}</Table.Td>
                      <Table.Td>{formatBytes(plugin.totalBytes)}</Table.Td>
                      <Table.Td>
                        <ActionIcon
                          variant="subtle"
                          color="red"
                          onClick={() => setCleanupTarget(plugin)}
                          aria-label={`Delete storage for ${plugin.pluginName}`}
                        >
                          <IconTrash size={16} />
                        </ActionIcon>
                      </Table.Td>
                    </Table.Tr>
                  ))}
                </Table.Tbody>
              </Table>
            ) : (
              <Text c="dimmed">No plugins have stored any files yet.</Text>
            )}
          </Stack>
        </Card>

        {/* Cleanup Confirmation Modal */}
        <Modal
          opened={cleanupTarget !== null}
          onClose={() => setCleanupTarget(null)}
          title="Delete Plugin Storage"
          centered
        >
          <Stack gap="md">
            <Text>
              Delete all storage for{" "}
              <strong>{cleanupTarget?.pluginName}</strong>? This removes all
              cached data and cannot be undone. The plugin will recreate files
              as needed.
            </Text>
            <Text size="sm" c="dimmed">
              {cleanupTarget?.fileCount.toLocaleString()} files,{" "}
              {formatBytes(cleanupTarget?.totalBytes || 0)}
            </Text>
            <Group justify="flex-end">
              <Button variant="subtle" onClick={() => setCleanupTarget(null)}>
                Cancel
              </Button>
              <Button
                color="red"
                loading={cleanupMutation.isPending}
                onClick={() => {
                  if (cleanupTarget) {
                    cleanupMutation.mutate(cleanupTarget.pluginName);
                  }
                }}
              >
                Delete
              </Button>
            </Group>
          </Stack>
        </Modal>
      </Stack>
    </Box>
  );
}
