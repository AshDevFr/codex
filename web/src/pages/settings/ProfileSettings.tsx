import {
  ActionIcon,
  Alert,
  Badge,
  Box,
  Button,
  Card,
  CopyButton,
  Group,
  Modal,
  PasswordInput,
  SegmentedControl,
  Select,
  Stack,
  Switch,
  Table,
  Tabs,
  Text,
  TextInput,
  Title,
  Tooltip,
} from "@mantine/core";
import { useForm } from "@mantine/form";
import { notifications } from "@mantine/notifications";
import {
  IconCheck,
  IconCopy,
  IconKey,
  IconPalette,
  IconPlus,
  IconTrash,
  IconUser,
} from "@tabler/icons-react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { api } from "@/api/client";
import { userPreferencesApi } from "@/api/userPreferences";
import { PermissionPicker } from "@/components/common";
import { useAppName } from "@/hooks/useAppName";
import { useDocumentTitle } from "@/hooks/useDocumentTitle";
import { useAuthStore } from "@/store/authStore";
import { useUserPreferencesStore } from "@/store/userPreferencesStore";
import type { components } from "@/types/api.generated";
import {
  ALL_PERMISSIONS,
  getPermissionsForRole,
  PERMISSION_PRESETS,
  type Permission,
  type PermissionPreset,
  parsePermissions,
} from "@/types/permissions";
import type { PreferenceKey, TypedPreferences } from "@/types/preferences";
import { PREFERENCE_DEFAULTS } from "@/types/preferences";

type ApiKeyDto = components["schemas"]["ApiKeyDto"];

export function ProfileSettings() {
  const appName = useAppName();
  const { user } = useAuthStore();
  const queryClient = useQueryClient();
  const { getPreference, setPreference } = useUserPreferencesStore();
  const [createKeyModalOpened, setCreateKeyModalOpened] = useState(false);
  const [newApiKey, setNewApiKey] = useState<string | null>(null);
  const [permissionPreset, setPermissionPreset] =
    useState<PermissionPreset>("full");
  const [selectedPermissions, setSelectedPermissions] = useState<Permission[]>(
    [],
  );

  useDocumentTitle("Profile Settings");

  // Fetch user preferences
  const { data: preferences } = useQuery({
    queryKey: ["user-preferences"],
    queryFn: userPreferencesApi.getAll,
  });

  // Fetch API keys
  const { data: apiKeys, isLoading: apiKeysLoading } = useQuery({
    queryKey: ["api-keys"],
    queryFn: async () => {
      const response = await api.get<ApiKeyDto[]>("/api-keys");
      // Handle both array and object with data property
      const data = response.data;
      if (Array.isArray(data)) {
        return data;
      }
      // If it's an object with a data property (paginated response)
      if (data && typeof data === "object" && "data" in data) {
        return (data as { data: ApiKeyDto[] }).data;
      }
      return [];
    },
  });

  // Password change form
  const passwordForm = useForm({
    initialValues: {
      currentPassword: "",
      newPassword: "",
      confirmPassword: "",
    },
    validate: {
      newPassword: (value) =>
        value.length < 8 ? "Password must be at least 8 characters" : null,
      confirmPassword: (value, values) =>
        value !== values.newPassword ? "Passwords do not match" : null,
    },
  });

  // Create API key form
  const apiKeyForm = useForm({
    initialValues: {
      name: "",
      expiresInDays: 30,
    },
    validate: {
      name: (value) => (value.length < 1 ? "Name is required" : null),
    },
  });

  // Mutations
  const updatePreferenceMutation = useMutation({
    mutationFn: async ({
      key,
      value,
    }: {
      key: PreferenceKey;
      value: TypedPreferences[PreferenceKey];
    }) => {
      return userPreferencesApi.set(key, value as never);
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["user-preferences"] });
    },
    onError: () => {
      notifications.show({
        title: "Error",
        message: "Failed to update preference",
        color: "red",
      });
    },
  });

  const changePasswordMutation = useMutation({
    mutationFn: async (data: {
      currentPassword: string;
      newPassword: string;
    }) => {
      await api.post("/auth/change-password", data);
    },
    onSuccess: () => {
      notifications.show({
        title: "Success",
        message: "Password changed successfully",
        color: "green",
      });
      passwordForm.reset();
    },
    onError: () => {
      notifications.show({
        title: "Error",
        message:
          "Failed to change password. Please check your current password.",
        color: "red",
      });
    },
  });

  const createApiKeyMutation = useMutation({
    mutationFn: async (data: {
      name: string;
      expiresInDays: number;
      permissions?: Permission[];
    }) => {
      const response = await api.post<{ apiKey: ApiKeyDto; key: string }>(
        "/api-keys",
        {
          name: data.name,
          expiresAt: data.expiresInDays
            ? new Date(
                Date.now() + data.expiresInDays * 24 * 60 * 60 * 1000,
              ).toISOString()
            : null,
          // Only send permissions if not using full access (let backend use defaults)
          permissions: data.permissions,
        },
      );
      return response.data;
    },
    onSuccess: (data) => {
      setNewApiKey(data.key);
      queryClient.invalidateQueries({ queryKey: ["api-keys"] });
      apiKeyForm.reset();
      setPermissionPreset("full");
      setSelectedPermissions([]);
    },
    onError: () => {
      notifications.show({
        title: "Error",
        message: "Failed to create API key",
        color: "red",
      });
    },
  });

  const deleteApiKeyMutation = useMutation({
    mutationFn: async (keyId: string) => {
      await api.delete(`/api-keys/${keyId}`);
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["api-keys"] });
      notifications.show({
        title: "Success",
        message: "API key deleted",
        color: "green",
      });
    },
    onError: () => {
      notifications.show({
        title: "Error",
        message: "Failed to delete API key",
        color: "red",
      });
    },
  });

  // Helper to get preference value with default
  const getPref = <K extends PreferenceKey>(key: K): TypedPreferences[K] => {
    const cached = getPreference(key);
    if (cached !== undefined) return cached;

    const serverPref = preferences?.find((p) => p.key === key);
    if (serverPref) {
      return serverPref.value as TypedPreferences[K];
    }
    return PREFERENCE_DEFAULTS[key];
  };

  // Helper to update preference
  const updatePref = <K extends PreferenceKey>(
    key: K,
    value: TypedPreferences[K],
  ) => {
    setPreference(key, value);
    updatePreferenceMutation.mutate({ key, value });
  };

  return (
    <Box py="xl" px="md">
      <Stack gap="xl">
        <Title order={1}>Profile Settings</Title>

        <Tabs defaultValue="account">
          <Tabs.List>
            <Tabs.Tab value="account" leftSection={<IconUser size={16} />}>
              Account
            </Tabs.Tab>
            <Tabs.Tab
              value="preferences"
              leftSection={<IconPalette size={16} />}
            >
              Preferences
            </Tabs.Tab>
            <Tabs.Tab value="api-keys" leftSection={<IconKey size={16} />}>
              API Keys
            </Tabs.Tab>
          </Tabs.List>

          {/* Account Tab */}
          <Tabs.Panel value="account" pt="md">
            <Stack gap="lg">
              <Card withBorder>
                <Stack gap="md">
                  <Title order={3}>Account Information</Title>
                  <Group>
                    <Text fw={500}>Username:</Text>
                    <Text>{user?.username}</Text>
                  </Group>
                  <Group>
                    <Text fw={500}>Email:</Text>
                    <Text>{user?.email}</Text>
                  </Group>
                  <Group>
                    <Text fw={500}>Role:</Text>
                    <Badge color={user?.role === "admin" ? "blue" : "gray"}>
                      {user?.role === "admin"
                        ? "Admin"
                        : user?.role === "maintainer"
                          ? "Maintainer"
                          : "User"}
                    </Badge>
                  </Group>
                </Stack>
              </Card>

              <Card withBorder>
                <form
                  onSubmit={passwordForm.onSubmit((values) =>
                    changePasswordMutation.mutate({
                      currentPassword: values.currentPassword,
                      newPassword: values.newPassword,
                    }),
                  )}
                >
                  <Stack gap="md">
                    <Title order={3}>Change Password</Title>
                    <PasswordInput
                      label="Current Password"
                      placeholder="Enter current password"
                      {...passwordForm.getInputProps("currentPassword")}
                    />
                    <PasswordInput
                      label="New Password"
                      placeholder="Enter new password"
                      {...passwordForm.getInputProps("newPassword")}
                    />
                    <PasswordInput
                      label="Confirm New Password"
                      placeholder="Confirm new password"
                      {...passwordForm.getInputProps("confirmPassword")}
                    />
                    <Group>
                      <Button
                        type="submit"
                        loading={changePasswordMutation.isPending}
                      >
                        Change Password
                      </Button>
                    </Group>
                  </Stack>
                </form>
              </Card>
            </Stack>
          </Tabs.Panel>

          {/* Preferences Tab */}
          <Tabs.Panel value="preferences" pt="md">
            <Stack gap="lg">
              <Card withBorder>
                <Stack gap="md">
                  <Title order={3}>Appearance</Title>
                  <Group justify="space-between">
                    <div>
                      <Text fw={500}>Theme</Text>
                      <Text size="sm" c="dimmed">
                        Choose your preferred color theme
                      </Text>
                    </div>
                    <SegmentedControl
                      value={getPref("ui.theme")}
                      onChange={(value) =>
                        updatePref(
                          "ui.theme",
                          value as "light" | "dark" | "system",
                        )
                      }
                      data={[
                        { label: "Light", value: "light" },
                        { label: "Dark", value: "dark" },
                        { label: "System", value: "system" },
                      ]}
                    />
                  </Group>
                </Stack>
              </Card>

              <Card withBorder>
                <Stack gap="md">
                  <Title order={3}>Library Display</Title>
                  <Group justify="space-between">
                    <div>
                      <Text fw={500}>Show Deleted Books</Text>
                      <Text size="sm" c="dimmed">
                        Display soft-deleted books in the library
                      </Text>
                    </div>
                    <Switch
                      checked={getPref("library.show_deleted_books")}
                      onChange={(e) =>
                        updatePref(
                          "library.show_deleted_books",
                          e.currentTarget.checked,
                        )
                      }
                    />
                  </Group>
                </Stack>
              </Card>
            </Stack>
          </Tabs.Panel>

          {/* API Keys Tab */}
          <Tabs.Panel value="api-keys" pt="md">
            <Stack gap="lg">
              <Card withBorder>
                <Stack gap="md">
                  <Group justify="space-between">
                    <Title order={3}>API Keys</Title>
                    <Button
                      leftSection={<IconPlus size={16} />}
                      onClick={() => setCreateKeyModalOpened(true)}
                    >
                      Create Key
                    </Button>
                  </Group>
                  <Text size="sm" c="dimmed">
                    API keys allow external applications to access your{" "}
                    {appName} account. Keep them secure and never share them
                    publicly.
                  </Text>
                  {apiKeysLoading ? (
                    <Text>Loading API keys...</Text>
                  ) : Array.isArray(apiKeys) && apiKeys.length > 0 ? (
                    <Table>
                      <Table.Thead>
                        <Table.Tr>
                          <Table.Th>Name</Table.Th>
                          <Table.Th>Permissions</Table.Th>
                          <Table.Th>Created</Table.Th>
                          <Table.Th>Expires</Table.Th>
                          <Table.Th>Last Used</Table.Th>
                          <Table.Th>Actions</Table.Th>
                        </Table.Tr>
                      </Table.Thead>
                      <Table.Tbody>
                        {apiKeys.map((key: ApiKeyDto) => {
                          const permissions = parsePermissions(key.permissions);
                          const userPerms = user?.role
                            ? PERMISSION_PRESETS.find(
                                (p) => p.value === "full",
                              )?.getPermissions(user.role) || []
                            : [];
                          const isFullAccess =
                            userPerms.length > 0 &&
                            permissions.length === userPerms.length;
                          return (
                            <Table.Tr key={key.id}>
                              <Table.Td>
                                <Text fw={500}>{key.name}</Text>
                              </Table.Td>
                              <Table.Td>
                                <Tooltip
                                  label={
                                    permissions.length > 0
                                      ? permissions.join(", ")
                                      : "No permissions"
                                  }
                                  multiline
                                  w={300}
                                >
                                  <Badge
                                    variant="light"
                                    color={
                                      isFullAccess
                                        ? "blue"
                                        : permissions.length > 0
                                          ? "cyan"
                                          : "gray"
                                    }
                                  >
                                    {isFullAccess
                                      ? "Full Access"
                                      : `${permissions.length} permissions`}
                                  </Badge>
                                </Tooltip>
                              </Table.Td>
                              <Table.Td>
                                {new Date(key.createdAt).toLocaleDateString()}
                              </Table.Td>
                              <Table.Td>
                                {key.expiresAt
                                  ? new Date(key.expiresAt).toLocaleDateString()
                                  : "Never"}
                              </Table.Td>
                              <Table.Td>
                                {key.lastUsedAt
                                  ? new Date(key.lastUsedAt).toLocaleString()
                                  : "Never"}
                              </Table.Td>
                              <Table.Td>
                                <ActionIcon
                                  color="red"
                                  variant="light"
                                  onClick={() =>
                                    deleteApiKeyMutation.mutate(key.id)
                                  }
                                  loading={deleteApiKeyMutation.isPending}
                                >
                                  <IconTrash size={16} />
                                </ActionIcon>
                              </Table.Td>
                            </Table.Tr>
                          );
                        })}
                      </Table.Tbody>
                    </Table>
                  ) : (
                    <Text c="dimmed">No API keys created yet.</Text>
                  )}
                </Stack>
              </Card>
            </Stack>
          </Tabs.Panel>
        </Tabs>
      </Stack>

      {/* Create API Key Modal */}
      <Modal
        opened={createKeyModalOpened}
        onClose={() => {
          setCreateKeyModalOpened(false);
          setNewApiKey(null);
          apiKeyForm.reset();
          setPermissionPreset("full");
          setSelectedPermissions([]);
        }}
        title="Create API Key"
        size="lg"
      >
        {newApiKey ? (
          <Stack gap="md">
            <Alert icon={<IconCheck size={16} />} color="green">
              API key created successfully!
            </Alert>
            <Text size="sm" c="dimmed">
              Copy this key now. You won't be able to see it again.
            </Text>
            <Group>
              <TextInput
                value={newApiKey}
                readOnly
                style={{ flex: 1, fontFamily: "monospace" }}
              />
              <CopyButton value={newApiKey}>
                {({ copied, copy }) => (
                  <Tooltip label={copied ? "Copied" : "Copy"}>
                    <ActionIcon
                      color={copied ? "green" : "gray"}
                      onClick={copy}
                    >
                      {copied ? (
                        <IconCheck size={16} />
                      ) : (
                        <IconCopy size={16} />
                      )}
                    </ActionIcon>
                  </Tooltip>
                )}
              </CopyButton>
            </Group>
            <Button
              onClick={() => {
                setCreateKeyModalOpened(false);
                setNewApiKey(null);
              }}
            >
              Done
            </Button>
          </Stack>
        ) : (
          <form
            onSubmit={apiKeyForm.onSubmit((values) => {
              // Determine permissions based on preset
              let permissions: Permission[] | undefined;
              if (permissionPreset === "custom") {
                permissions = selectedPermissions;
              } else if (permissionPreset !== "full") {
                const preset = PERMISSION_PRESETS.find(
                  (p) => p.value === permissionPreset,
                );
                permissions = preset?.getPermissions(user?.role || "reader");
              }
              // For "full" preset, don't send permissions (backend uses user's full permissions)
              createApiKeyMutation.mutate({
                ...values,
                permissions,
              });
            })}
          >
            <Stack gap="md">
              <TextInput
                label="Key Name"
                placeholder="My API Key"
                description="A name to identify this key"
                {...apiKeyForm.getInputProps("name")}
              />
              <Select
                label="Expiration"
                description="When this key should expire"
                data={[
                  { label: "7 days", value: "7" },
                  { label: "30 days", value: "30" },
                  { label: "90 days", value: "90" },
                  { label: "1 year", value: "365" },
                  { label: "Never", value: "0" },
                ]}
                value={String(apiKeyForm.values.expiresInDays)}
                onChange={(value) =>
                  apiKeyForm.setFieldValue(
                    "expiresInDays",
                    Number.parseInt(value || "30", 10),
                  )
                }
              />
              <Select
                label="Permissions"
                description="What this key can access"
                data={PERMISSION_PRESETS.map((preset) => ({
                  value: preset.value,
                  label: preset.label,
                  description: preset.description,
                }))}
                value={permissionPreset}
                onChange={(value) => {
                  setPermissionPreset(value as PermissionPreset);
                  if (value !== "custom") {
                    setSelectedPermissions([]);
                  }
                }}
              />
              {permissionPreset === "custom" && (
                <Card withBorder p="md">
                  <Stack gap="md">
                    <div>
                      <Text size="sm" fw={500}>
                        Select Permissions
                      </Text>
                      <Text size="xs" c="dimmed">
                        You can only grant permissions your role has
                      </Text>
                    </div>
                    <PermissionPicker
                      selectedPermissions={selectedPermissions}
                      onPermissionsChange={setSelectedPermissions}
                      disabledUncheckedPermissions={
                        ALL_PERMISSIONS.filter((p) => {
                          // User can select from their role's permissions + custom permissions
                          const rolePerms = getPermissionsForRole(
                            user?.role || "reader",
                          );
                          const customPerms = parsePermissions(
                            user?.permissions || [],
                          );
                          const allUserPerms = [
                            ...new Set([...rolePerms, ...customPerms]),
                          ];
                          return !allUserPerms.includes(p);
                        }) as Permission[]
                      }
                    />
                    {selectedPermissions.length === 0 && (
                      <Text size="xs" c="red">
                        Select at least one permission
                      </Text>
                    )}
                  </Stack>
                </Card>
              )}
              <Group justify="flex-end">
                <Button
                  variant="subtle"
                  onClick={() => setCreateKeyModalOpened(false)}
                >
                  Cancel
                </Button>
                <Button
                  type="submit"
                  loading={createApiKeyMutation.isPending}
                  disabled={
                    permissionPreset === "custom" &&
                    selectedPermissions.length === 0
                  }
                >
                  Create Key
                </Button>
              </Group>
            </Stack>
          </form>
        )}
      </Modal>
    </Box>
  );
}
