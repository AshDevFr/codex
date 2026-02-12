import {
  ActionIcon,
  Alert,
  Badge,
  Box,
  Button,
  Card,
  Code,
  Collapse,
  Group,
  Loader,
  Modal,
  ScrollArea,
  Stack,
  Switch,
  Table,
  Text,
  Title,
  Tooltip,
} from "@mantine/core";
import { useForm } from "@mantine/form";
import { useDisclosure } from "@mantine/hooks";
import { notifications } from "@mantine/notifications";
import {
  IconAlertCircle,
  IconChevronDown,
  IconChevronRight,
  IconEdit,
  IconPlayerPlay,
  IconPlugConnected,
  IconPlus,
  IconRefresh,
  IconSettings,
  IconTrash,
} from "@tabler/icons-react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Fragment, useState } from "react";
import { librariesApi } from "@/api/libraries";
import {
  type CreatePluginRequest,
  type PluginDto,
  type PluginHealthStatus,
  pluginsApi,
} from "@/api/plugins";
import { PluginConfigModal } from "@/components/forms/PluginConfigModal";
import {
  type OfficialPlugin,
  OfficialPlugins,
} from "./plugins/OfficialPlugins";
import { PluginDetails } from "./plugins/PluginDetails";
import {
  defaultFormValues,
  PluginForm,
  type PluginFormValues,
  safeJsonParse,
} from "./plugins/PluginForm";
import { healthStatusColors } from "./plugins/types";

export function PluginsSettings() {
  const queryClient = useQueryClient();
  const [
    createModalOpened,
    { open: openCreateModal, close: closeCreateModal },
  ] = useDisclosure(false);
  const [editModalOpened, { open: openEditModal, close: closeEditModal }] =
    useDisclosure(false);
  const [
    deleteModalOpened,
    { open: openDeleteModal, close: closeDeleteModal },
  ] = useDisclosure(false);
  const [selectedPlugin, setSelectedPlugin] = useState<PluginDto | null>(null);
  const [expandedRows, setExpandedRows] = useState<Set<string>>(new Set());
  const [configPlugin, setConfigPlugin] = useState<PluginDto | null>(null);

  // Fetch plugins
  const {
    data: pluginsResponse,
    isLoading,
    error,
  } = useQuery({
    queryKey: ["plugins"],
    queryFn: pluginsApi.getAll,
  });

  const plugins = pluginsResponse?.plugins ?? [];

  // Fetch libraries for the library filter dropdown
  const { data: libraries = [] } = useQuery({
    queryKey: ["libraries"],
    queryFn: librariesApi.getAll,
  });

  // Create form
  const createForm = useForm<PluginFormValues>({
    initialValues: defaultFormValues,
    validate: {
      name: (value) => {
        if (!value.trim()) return "Name is required";
        if (!/^[a-z0-9-]+$/.test(value)) {
          return "Name must be lowercase alphanumeric with hyphens only";
        }
        return null;
      },
      displayName: (value) =>
        !value.trim() ? "Display name is required" : null,
      command: (value) => (!value.trim() ? "Command is required" : null),
    },
  });

  // Edit form
  const editForm = useForm<PluginFormValues>({
    initialValues: defaultFormValues,
    validate: {
      displayName: (value) =>
        !value.trim() ? "Display name is required" : null,
      command: (value) => (!value.trim() ? "Command is required" : null),
    },
  });

  // Mutations
  const createMutation = useMutation({
    mutationFn: async (values: PluginFormValues) => {
      const request: CreatePluginRequest = {
        name: values.name.trim(),
        displayName: values.displayName.trim(),
        description: values.description.trim() || undefined,
        command: values.command.trim(),
        args: values.args
          .split("\n")
          .map((a) => a.trim())
          .filter(Boolean),
        env: values.envVars.filter((e) => e.key.trim()),
        workingDirectory: values.workingDirectory.trim() || undefined,
        credentialDelivery: values.credentialDelivery,
        credentials: values.credentials.trim()
          ? safeJsonParse(values.credentials, "credentials")
          : undefined,
        config: values.config.trim()
          ? safeJsonParse(values.config, "config")
          : undefined,
        enabled: values.enabled,
        rateLimitRequestsPerMinute: values.rateLimitEnabled
          ? values.rateLimitRequestsPerMinute
          : null,
      };
      return pluginsApi.create(request);
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["plugins"] });
      queryClient.invalidateQueries({ queryKey: ["plugin-actions"] });
      closeCreateModal();
      createForm.reset();
      notifications.show({
        title: "Success",
        message: "Plugin created successfully",
        color: "green",
      });
    },
    onError: (error: Error) => {
      notifications.show({
        title: "Error",
        message: error.message || "Failed to create plugin",
        color: "red",
      });
    },
  });

  const updateMutation = useMutation({
    mutationFn: async ({
      id,
      values,
    }: {
      id: string;
      values: PluginFormValues;
    }) => {
      return pluginsApi.update(id, {
        displayName: values.displayName.trim(),
        description: values.description.trim() || null,
        command: values.command.trim(),
        args: values.args
          .split("\n")
          .map((a) => a.trim())
          .filter(Boolean),
        env: values.envVars.filter((e) => e.key.trim()),
        workingDirectory: values.workingDirectory.trim() || null,
        credentialDelivery: values.credentialDelivery,
        credentials: values.credentials.trim()
          ? safeJsonParse(values.credentials, "credentials")
          : undefined,
        config: values.config.trim()
          ? safeJsonParse(values.config, "config")
          : undefined,
        rateLimitRequestsPerMinute: values.rateLimitEnabled
          ? values.rateLimitRequestsPerMinute
          : null,
      });
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["plugins"] });
      queryClient.invalidateQueries({ queryKey: ["plugin-actions"] });
      closeEditModal();
      setSelectedPlugin(null);
      notifications.show({
        title: "Success",
        message: "Plugin updated successfully",
        color: "green",
      });
    },
    onError: (error: Error) => {
      notifications.show({
        title: "Error",
        message: error.message || "Failed to update plugin",
        color: "red",
      });
    },
  });

  const deleteMutation = useMutation({
    mutationFn: pluginsApi.delete,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["plugins"] });
      queryClient.invalidateQueries({ queryKey: ["plugin-actions"] });
      closeDeleteModal();
      setSelectedPlugin(null);
      notifications.show({
        title: "Success",
        message: "Plugin deleted successfully",
        color: "green",
      });
    },
    onError: (error: Error) => {
      notifications.show({
        title: "Error",
        message: error.message || "Failed to delete plugin",
        color: "red",
      });
    },
  });

  const enableMutation = useMutation({
    mutationFn: pluginsApi.enable,
    onSuccess: (data) => {
      queryClient.invalidateQueries({ queryKey: ["plugins"] });
      queryClient.invalidateQueries({ queryKey: ["plugin-actions"] });
      notifications.show({
        title: "Success",
        message: data.message,
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

  const disableMutation = useMutation({
    mutationFn: pluginsApi.disable,
    onSuccess: (data) => {
      queryClient.invalidateQueries({ queryKey: ["plugins"] });
      queryClient.invalidateQueries({ queryKey: ["plugin-actions"] });
      notifications.show({
        title: "Success",
        message: data.message,
        color: "green",
      });
    },
    onError: (error: Error) => {
      notifications.show({
        title: "Error",
        message: error.message || "Failed to disable plugin",
        color: "red",
      });
    },
  });

  const testMutation = useMutation({
    mutationFn: pluginsApi.test,
    onSuccess: (data) => {
      if (data.success) {
        notifications.show({
          title: "Connection Successful",
          message: `${data.message}${data.latencyMs ? ` (${data.latencyMs}ms)` : ""}`,
          color: "green",
        });
      } else {
        notifications.show({
          title: "Connection Failed",
          message: data.message,
          color: "red",
        });
      }
      queryClient.invalidateQueries({ queryKey: ["plugins"] });
    },
    onError: (error: Error) => {
      notifications.show({
        title: "Test Failed",
        message: error.message || "Failed to test plugin connection",
        color: "red",
      });
    },
  });

  const resetFailuresMutation = useMutation({
    mutationFn: pluginsApi.resetFailures,
    onSuccess: (data) => {
      queryClient.invalidateQueries({ queryKey: ["plugins"] });
      queryClient.invalidateQueries({ queryKey: ["plugin-actions"] });
      notifications.show({
        title: "Success",
        message: data.message,
        color: "green",
      });
    },
    onError: (error: Error) => {
      notifications.show({
        title: "Error",
        message: error.message || "Failed to reset failures",
        color: "red",
      });
    },
  });

  const handleAddOfficialPlugin = (official: OfficialPlugin) => {
    createForm.setValues({
      ...defaultFormValues,
      name: official.name,
      displayName: official.displayName,
      description: official.description,
      command: official.formDefaults.command,
      args: official.formDefaults.args,
      credentialDelivery: official.formDefaults.credentialDelivery,
      credentials: official.formDefaults.credentials ?? "",
      enabled: true,
    });
    openCreateModal();
  };

  const handleEditPlugin = (plugin: PluginDto) => {
    setSelectedPlugin(plugin);
    editForm.setValues({
      name: plugin.name,
      displayName: plugin.displayName,
      description: plugin.description || "",
      command: plugin.command,
      args: plugin.args.join("\n"),
      envVars:
        typeof plugin.env === "object" && plugin.env !== null
          ? Object.entries(plugin.env as Record<string, string>).map(
              ([key, value]) => ({ key, value }),
            )
          : [],
      workingDirectory: plugin.workingDirectory || "",
      credentialDelivery: plugin.credentialDelivery,
      credentials: "",
      config:
        plugin.config && Object.keys(plugin.config as object).length > 0
          ? JSON.stringify(plugin.config, null, 2)
          : "",
      enabled: plugin.enabled,
      rateLimitEnabled: plugin.rateLimitRequestsPerMinute != null,
      rateLimitRequestsPerMinute: plugin.rateLimitRequestsPerMinute ?? 60,
    });
    openEditModal();
  };

  const handleDeletePlugin = (plugin: PluginDto) => {
    setSelectedPlugin(plugin);
    openDeleteModal();
  };

  const toggleRowExpansion = (id: string) => {
    setExpandedRows((prev) => {
      const next = new Set(prev);
      if (next.has(id)) {
        next.delete(id);
      } else {
        next.add(id);
      }
      return next;
    });
  };

  return (
    <Box py="xl" px="md">
      <Stack gap="xl">
        <Group justify="space-between">
          <div>
            <Title order={1}>Plugins</Title>
            <Text c="dimmed" size="sm" mt="xs">
              Manage external plugin processes for metadata fetching and other
              integrations
            </Text>
          </div>
          <Button
            leftSection={<IconPlus size={16} />}
            onClick={openCreateModal}
          >
            Add Plugin
          </Button>
        </Group>

        <OfficialPlugins
          installedPlugins={plugins}
          onAdd={handleAddOfficialPlugin}
        />

        {isLoading ? (
          <Group justify="center" py="xl">
            <Loader />
          </Group>
        ) : error ? (
          <Alert icon={<IconAlertCircle size={16} />} color="red">
            Failed to load plugins. Please try again.
          </Alert>
        ) : plugins.length > 0 ? (
          <Card withBorder p={0}>
            <ScrollArea>
              <Table>
                <Table.Thead>
                  <Table.Tr>
                    <Table.Th w={40} />
                    <Table.Th>Plugin</Table.Th>
                    <Table.Th>Command</Table.Th>
                    <Table.Th>Status</Table.Th>
                    <Table.Th>Health</Table.Th>
                    <Table.Th>Actions</Table.Th>
                  </Table.Tr>
                </Table.Thead>
                <Table.Tbody>
                  {plugins.map((plugin) => (
                    <Fragment key={plugin.id}>
                      <Table.Tr>
                        <Table.Td>
                          <ActionIcon
                            variant="subtle"
                            size="sm"
                            onClick={() => toggleRowExpansion(plugin.id)}
                          >
                            {expandedRows.has(plugin.id) ? (
                              <IconChevronDown size={16} />
                            ) : (
                              <IconChevronRight size={16} />
                            )}
                          </ActionIcon>
                        </Table.Td>
                        <Table.Td>
                          <Group gap="sm">
                            <IconPlugConnected size={20} />
                            <div>
                              <Text fw={500}>{plugin.displayName}</Text>
                              <Text size="xs" c="dimmed">
                                {plugin.name}
                              </Text>
                            </div>
                          </Group>
                        </Table.Td>
                        <Table.Td>
                          <Code>{plugin.command}</Code>
                        </Table.Td>
                        <Table.Td>
                          <Switch
                            checked={plugin.enabled}
                            onChange={() =>
                              plugin.enabled
                                ? disableMutation.mutate(plugin.id)
                                : enableMutation.mutate(plugin.id)
                            }
                            disabled={
                              enableMutation.isPending ||
                              disableMutation.isPending
                            }
                          />
                        </Table.Td>
                        <Table.Td>
                          <Group gap="xs">
                            <Badge
                              color={
                                healthStatusColors[
                                  plugin.healthStatus as PluginHealthStatus
                                ] || "gray"
                              }
                              variant="light"
                            >
                              {plugin.healthStatus}
                            </Badge>
                            {plugin.failureCount > 0 && (
                              <Tooltip
                                label={`${plugin.failureCount} failures${plugin.lastFailureAt ? ` (last: ${new Date(plugin.lastFailureAt).toLocaleString()})` : ""}`}
                              >
                                <Badge color="red" variant="outline" size="sm">
                                  {plugin.failureCount}
                                </Badge>
                              </Tooltip>
                            )}
                          </Group>
                        </Table.Td>
                        <Table.Td>
                          <Group gap="xs">
                            <Tooltip label="Test Connection">
                              <ActionIcon
                                variant="subtle"
                                onClick={() => testMutation.mutate(plugin.id)}
                                loading={
                                  testMutation.isPending &&
                                  testMutation.variables === plugin.id
                                }
                              >
                                <IconPlayerPlay size={16} />
                              </ActionIcon>
                            </Tooltip>
                            {plugin.failureCount > 0 && (
                              <Tooltip label="Reset Failures">
                                <ActionIcon
                                  variant="subtle"
                                  color="yellow"
                                  onClick={() =>
                                    resetFailuresMutation.mutate(plugin.id)
                                  }
                                  loading={
                                    resetFailuresMutation.isPending &&
                                    resetFailuresMutation.variables ===
                                      plugin.id
                                  }
                                >
                                  <IconRefresh size={16} />
                                </ActionIcon>
                              </Tooltip>
                            )}
                            <Tooltip label="Configure Plugin">
                              <ActionIcon
                                variant="subtle"
                                color="blue"
                                onClick={() => setConfigPlugin(plugin)}
                              >
                                <IconSettings size={16} />
                              </ActionIcon>
                            </Tooltip>
                            <Tooltip label="Edit Plugin">
                              <ActionIcon
                                variant="subtle"
                                onClick={() => handleEditPlugin(plugin)}
                              >
                                <IconEdit size={16} />
                              </ActionIcon>
                            </Tooltip>
                            <Tooltip label="Delete Plugin">
                              <ActionIcon
                                variant="subtle"
                                color="red"
                                onClick={() => handleDeletePlugin(plugin)}
                              >
                                <IconTrash size={16} />
                              </ActionIcon>
                            </Tooltip>
                          </Group>
                        </Table.Td>
                      </Table.Tr>
                      <Table.Tr key={`${plugin.id}-details`}>
                        <Table.Td colSpan={6} p={0}>
                          <Collapse in={expandedRows.has(plugin.id)}>
                            <Box
                              p="md"
                              bg="var(--mantine-color-dark-6)"
                              style={{
                                borderTop:
                                  "1px solid var(--mantine-color-dark-4)",
                              }}
                            >
                              <PluginDetails
                                plugin={plugin}
                                libraries={libraries}
                              />
                            </Box>
                          </Collapse>
                        </Table.Td>
                      </Table.Tr>
                    </Fragment>
                  ))}
                </Table.Tbody>
              </Table>
            </ScrollArea>
          </Card>
        ) : (
          <Alert
            icon={<IconPlugConnected size={16} />}
            color="gray"
            variant="light"
          >
            <Text fw={500}>No plugins configured</Text>
            <Text size="sm" mt="xs">
              Add plugins to enable metadata fetching from external sources like
              MangaBaka, AniList, or other providers.
            </Text>
          </Alert>
        )}
      </Stack>

      {/* Create Plugin Modal */}
      <Modal
        opened={createModalOpened}
        onClose={() => {
          closeCreateModal();
          createForm.reset();
        }}
        title="Add Plugin"
        size="lg"
      >
        <PluginForm
          form={createForm}
          onSubmit={(values) => createMutation.mutate(values)}
          isLoading={createMutation.isPending}
          onCancel={() => {
            closeCreateModal();
            createForm.reset();
          }}
          isCreate
        />
      </Modal>

      {/* Edit Plugin Modal */}
      <Modal
        opened={editModalOpened}
        onClose={() => {
          closeEditModal();
          setSelectedPlugin(null);
        }}
        title={`Edit Plugin: ${selectedPlugin?.displayName}`}
        size="lg"
      >
        <PluginForm
          form={editForm}
          onSubmit={(values) =>
            selectedPlugin &&
            updateMutation.mutate({ id: selectedPlugin.id, values })
          }
          isLoading={updateMutation.isPending}
          onCancel={() => {
            closeEditModal();
            setSelectedPlugin(null);
          }}
          manifest={selectedPlugin?.manifest}
        />
      </Modal>

      {/* Delete Plugin Modal */}
      <Modal
        opened={deleteModalOpened}
        onClose={() => {
          closeDeleteModal();
          setSelectedPlugin(null);
        }}
        title="Delete Plugin"
      >
        <Stack gap="md">
          <Text>
            Are you sure you want to delete the plugin{" "}
            <strong>{selectedPlugin?.displayName}</strong>?
          </Text>
          <Text size="sm" c="dimmed">
            This action cannot be undone.
          </Text>
          <Group justify="flex-end">
            <Button
              variant="subtle"
              onClick={() => {
                closeDeleteModal();
                setSelectedPlugin(null);
              }}
            >
              Cancel
            </Button>
            <Button
              color="red"
              loading={deleteMutation.isPending}
              onClick={() =>
                selectedPlugin && deleteMutation.mutate(selectedPlugin.id)
              }
            >
              Delete Plugin
            </Button>
          </Group>
        </Stack>
      </Modal>

      {/* Plugin Config Modal */}
      {configPlugin && (
        <PluginConfigModal
          plugin={configPlugin}
          opened={!!configPlugin}
          onClose={() => setConfigPlugin(null)}
          libraries={libraries}
        />
      )}
    </Box>
  );
}
