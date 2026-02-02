import {
  ActionIcon,
  Alert,
  Badge,
  Box,
  Button,
  Card,
  Collapse,
  Divider,
  Group,
  Loader,
  Modal,
  Pagination,
  PasswordInput,
  Select,
  Stack,
  Switch,
  Table,
  Text,
  TextInput,
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
  IconFilter,
  IconTrash,
  IconUser,
  IconUserPlus,
  IconX,
} from "@tabler/icons-react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useState } from "react";
import { useSearchParams } from "react-router-dom";
import { sharingTagsApi } from "@/api/sharingTags";
import { type UserDto, type UserListParams, usersApi } from "@/api/users";
import { PermissionPicker } from "@/components/common";
import { UserSharingTagGrants } from "@/components/users";
import { useAuthStore } from "@/store/authStore";
import { type Permission, ROLE_PERMISSIONS } from "@/types/permissions";

const PAGE_SIZE = 20;

export function UsersSettings() {
  const queryClient = useQueryClient();
  const { user: currentUser } = useAuthStore();
  const [searchParams, setSearchParams] = useSearchParams();
  const [createModalOpened, setCreateModalOpened] = useState(false);
  const [editModalOpened, setEditModalOpened] = useState(false);
  const [deleteModalOpened, setDeleteModalOpened] = useState(false);
  const [selectedUser, setSelectedUser] = useState<UserDto | null>(null);
  const [customPermissions, setCustomPermissions] = useState<Permission[]>([]);
  const [permissionsExpanded, setPermissionsExpanded] = useState(false);

  // Initialize filter state from URL params
  const initialSharingTag = searchParams.get("sharingTag");
  const [filtersOpened, { toggle: toggleFilters }] = useDisclosure(
    // Auto-open filters if there's a sharingTag in URL
    !!initialSharingTag,
  );

  // Filter state - initialized from URL
  const [page, setPage] = useState(0);
  const [roleFilter, setRoleFilter] = useState<string | null>(
    searchParams.get("role"),
  );
  const [sharingTagFilter, setSharingTagFilter] = useState<string | null>(
    initialSharingTag,
  );
  const [sharingTagModeFilter, setSharingTagModeFilter] = useState<
    string | null
  >(searchParams.get("sharingTagMode"));

  // Sync URL when filters change
  useEffect(() => {
    const params = new URLSearchParams();
    if (roleFilter) params.set("role", roleFilter);
    if (sharingTagFilter) params.set("sharingTag", sharingTagFilter);
    if (sharingTagModeFilter)
      params.set("sharingTagMode", sharingTagModeFilter);
    setSearchParams(params, { replace: true });
  }, [roleFilter, sharingTagFilter, sharingTagModeFilter, setSearchParams]);

  // Build query params
  const queryParams: UserListParams = {
    page,
    pageSize: PAGE_SIZE,
    role: roleFilter as UserListParams["role"],
    sharingTag: sharingTagFilter ?? undefined,
    sharingTagMode: sharingTagModeFilter as UserListParams["sharingTagMode"],
  };

  // Fetch users with filters and pagination
  const {
    data: usersResponse,
    isLoading,
    error,
  } = useQuery({
    queryKey: ["users", queryParams],
    queryFn: () => usersApi.list(queryParams),
  });

  // Fetch sharing tags for filter dropdown
  const { data: sharingTags = [] } = useQuery({
    queryKey: ["sharing-tags"],
    queryFn: sharingTagsApi.list,
  });

  // Role options for the select
  const roleOptions = [
    { value: "reader", label: "Reader" },
    { value: "maintainer", label: "Maintainer" },
    { value: "admin", label: "Admin" },
  ];

  // Sharing tag options for filter
  const sharingTagOptions = sharingTags.map((tag) => ({
    value: tag.name,
    label: tag.name,
  }));

  // Access mode options for filter
  const accessModeOptions = [
    { value: "allow", label: "Allow" },
    { value: "deny", label: "Deny" },
  ];

  // Check if any filters are active
  const hasActiveFilters = roleFilter !== null || sharingTagFilter !== null;
  const activeFilterCount = [roleFilter, sharingTagFilter].filter(
    Boolean,
  ).length;

  // Clear all filters
  const clearFilters = () => {
    setRoleFilter(null);
    setSharingTagFilter(null);
    setSharingTagModeFilter(null);
    setPage(0);
  };

  // Handle page change (Mantine Pagination is 1-indexed)
  const handlePageChange = (newPage: number) => {
    setPage(newPage - 1); // Convert to 0-indexed for API
  };

  // Create user form
  const createForm = useForm({
    initialValues: {
      username: "",
      email: "",
      password: "",
      role: "reader" as "reader" | "maintainer" | "admin",
    },
    validate: {
      username: (value) =>
        value.length < 3 ? "Username must be at least 3 characters" : null,
      email: (value) =>
        !/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(value)
          ? "Invalid email address"
          : null,
      password: (value) =>
        value.length < 8 ? "Password must be at least 8 characters" : null,
    },
  });

  // Edit user form
  const editForm = useForm({
    initialValues: {
      username: "",
      email: "",
      password: "",
      role: "reader" as "reader" | "maintainer" | "admin",
      isActive: true,
    },
  });

  // Mutations
  const createUserMutation = useMutation({
    mutationFn: async (data: {
      username: string;
      email: string;
      password: string;
      role: "reader" | "maintainer" | "admin";
    }) => {
      return usersApi.create({
        username: data.username,
        email: data.email,
        password: data.password,
        role: data.role,
      });
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["users"] });
      setCreateModalOpened(false);
      createForm.reset();
      notifications.show({
        title: "Success",
        message: "User created successfully",
        color: "green",
      });
    },
    onError: (error: { message?: string }) => {
      notifications.show({
        title: "Error",
        message: error.message || "Failed to create user",
        color: "red",
      });
    },
  });

  const updateUserMutation = useMutation({
    mutationFn: async ({
      userId,
      data,
    }: {
      userId: string;
      data: {
        username?: string;
        email?: string;
        password?: string;
        role?: "reader" | "maintainer" | "admin";
        isActive?: boolean;
        permissions?: string[];
      };
    }) => {
      return usersApi.update(userId, {
        username: data.username,
        email: data.email,
        password: data.password || undefined,
        role: data.role,
        isActive: data.isActive,
        permissions: data.permissions,
      });
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["users"] });
      setEditModalOpened(false);
      setSelectedUser(null);
      notifications.show({
        title: "Success",
        message: "User updated successfully",
        color: "green",
      });
    },
    onError: (error: { message?: string }) => {
      notifications.show({
        title: "Error",
        message: error.message || "Failed to update user",
        color: "red",
      });
    },
  });

  const deleteUserMutation = useMutation({
    mutationFn: async (userId: string) => {
      return usersApi.delete(userId);
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["users"] });
      setDeleteModalOpened(false);
      setSelectedUser(null);
      notifications.show({
        title: "Success",
        message: "User deleted successfully",
        color: "green",
      });
    },
    onError: (error: { message?: string }) => {
      notifications.show({
        title: "Error",
        message: error.message || "Failed to delete user",
        color: "red",
      });
    },
  });

  const handleEditUser = (user: UserDto) => {
    setSelectedUser(user);
    editForm.setValues({
      username: user.username,
      email: user.email,
      password: "",
      role: user.role,
      isActive: user.isActive,
    });
    // Set custom permissions from user (those not included in role's base permissions)
    const rolePerms = ROLE_PERMISSIONS[user.role] || [];
    const userPerms = (user.permissions || []) as Permission[];
    const customPerms = userPerms.filter((p) => !rolePerms.includes(p));
    setCustomPermissions(customPerms);
    setPermissionsExpanded(customPerms.length > 0);
    setEditModalOpened(true);
  };

  const handleDeleteUser = (user: UserDto) => {
    setSelectedUser(user);
    setDeleteModalOpened(true);
  };

  // Pagination info
  const users = usersResponse?.data ?? [];
  const total = usersResponse?.total ?? 0;
  const totalPages = usersResponse?.totalPages ?? 1;
  const showPagination = total > PAGE_SIZE;

  return (
    <Box py="xl" px="md">
      <Stack gap="xl">
        <Group justify="space-between">
          <Title order={1}>User Management</Title>
          <Group>
            <Button
              variant={hasActiveFilters ? "filled" : "subtle"}
              color={hasActiveFilters ? "blue" : "gray"}
              leftSection={<IconFilter size={16} />}
              onClick={toggleFilters}
              rightSection={
                hasActiveFilters ? (
                  <Badge size="sm" variant="light" color="white">
                    {activeFilterCount}
                  </Badge>
                ) : null
              }
            >
              Filters
            </Button>
            <Button
              leftSection={<IconUserPlus size={16} />}
              onClick={() => setCreateModalOpened(true)}
            >
              Create User
            </Button>
          </Group>
        </Group>

        {/* Filter Panel */}
        <Collapse in={filtersOpened}>
          <Card withBorder p="md">
            <Stack gap="md">
              <Group justify="space-between">
                <Text fw={500}>Filters</Text>
                {hasActiveFilters && (
                  <Button
                    variant="subtle"
                    color="gray"
                    size="xs"
                    leftSection={<IconX size={14} />}
                    onClick={clearFilters}
                  >
                    Clear all
                  </Button>
                )}
              </Group>
              <Group grow>
                <Select
                  label="Role"
                  placeholder="All roles"
                  data={roleOptions}
                  value={roleFilter}
                  onChange={(value) => {
                    setRoleFilter(value);
                    setPage(0);
                  }}
                  clearable
                />
                <Select
                  label="Sharing Tag"
                  placeholder="All users"
                  data={sharingTagOptions}
                  value={sharingTagFilter}
                  onChange={(value) => {
                    setSharingTagFilter(value);
                    if (!value) {
                      setSharingTagModeFilter(null);
                    }
                    setPage(0);
                  }}
                  clearable
                  disabled={sharingTagOptions.length === 0}
                />
                <Select
                  label="Access Mode"
                  placeholder="Any mode"
                  data={accessModeOptions}
                  value={sharingTagModeFilter}
                  onChange={(value) => {
                    setSharingTagModeFilter(value);
                    setPage(0);
                  }}
                  clearable
                  disabled={!sharingTagFilter}
                />
              </Group>
            </Stack>
          </Card>
        </Collapse>

        {isLoading ? (
          <Group justify="center" py="xl">
            <Loader />
          </Group>
        ) : error ? (
          <Alert icon={<IconAlertCircle size={16} />} color="red">
            Failed to load users. Please try again.
          </Alert>
        ) : users.length === 0 ? (
          <Card withBorder p="xl">
            <Stack align="center" gap="sm">
              <Text size="lg" fw={600}>
                No users found
              </Text>
              <Text size="sm" c="dimmed">
                {hasActiveFilters
                  ? "Try adjusting your filters"
                  : "Create your first user to get started"}
              </Text>
            </Stack>
          </Card>
        ) : (
          <>
            {/* Top Pagination */}
            {showPagination && (
              <Group justify="center">
                <Pagination
                  value={page + 1}
                  onChange={handlePageChange}
                  total={totalPages}
                />
              </Group>
            )}

            <Card withBorder>
              <Table>
                <Table.Thead>
                  <Table.Tr>
                    <Table.Th>User</Table.Th>
                    <Table.Th>Email</Table.Th>
                    <Table.Th>Role</Table.Th>
                    <Table.Th>Status</Table.Th>
                    <Table.Th>Created</Table.Th>
                    <Table.Th>Last Login</Table.Th>
                    <Table.Th>Actions</Table.Th>
                  </Table.Tr>
                </Table.Thead>
                <Table.Tbody>
                  {users.map((user: UserDto) => (
                    <Table.Tr key={user.id}>
                      <Table.Td>
                        <Group gap="sm">
                          <IconUser size={20} />
                          <div>
                            <Text fw={500}>{user.username}</Text>
                            {user.id === currentUser?.id && (
                              <Text size="xs" c="dimmed">
                                (You)
                              </Text>
                            )}
                          </div>
                        </Group>
                      </Table.Td>
                      <Table.Td>{user.email}</Table.Td>
                      <Table.Td>
                        <Badge
                          color={
                            user.role === "admin"
                              ? "blue"
                              : user.role === "maintainer"
                                ? "cyan"
                                : "gray"
                          }
                        >
                          {user.role === "admin"
                            ? "Admin"
                            : user.role === "maintainer"
                              ? "Maintainer"
                              : "Reader"}
                        </Badge>
                      </Table.Td>
                      <Table.Td>
                        <Badge color={user.isActive ? "green" : "red"}>
                          {user.isActive ? "Active" : "Inactive"}
                        </Badge>
                      </Table.Td>
                      <Table.Td>
                        {new Date(user.createdAt).toLocaleDateString()}
                      </Table.Td>
                      <Table.Td>
                        {user.lastLoginAt
                          ? new Date(user.lastLoginAt).toLocaleString()
                          : "Never"}
                      </Table.Td>
                      <Table.Td>
                        <Group gap="xs">
                          <Tooltip label="Edit User">
                            <ActionIcon
                              variant="subtle"
                              onClick={() => handleEditUser(user)}
                            >
                              <IconEdit size={16} />
                            </ActionIcon>
                          </Tooltip>
                          <Tooltip label="Delete User">
                            <ActionIcon
                              variant="subtle"
                              color="red"
                              onClick={() => handleDeleteUser(user)}
                              disabled={user.id === currentUser?.id}
                            >
                              <IconTrash size={16} />
                            </ActionIcon>
                          </Tooltip>
                        </Group>
                      </Table.Td>
                    </Table.Tr>
                  ))}
                </Table.Tbody>
              </Table>
            </Card>

            {/* Bottom Pagination */}
            {showPagination && (
              <Group justify="center">
                <Pagination
                  value={page + 1}
                  onChange={handlePageChange}
                  total={totalPages}
                />
              </Group>
            )}

            {/* Results info */}
            <Text size="sm" c="dimmed" ta="center">
              Showing {page * PAGE_SIZE + 1} to{" "}
              {Math.min((page + 1) * PAGE_SIZE, total)} of {total} users
            </Text>
          </>
        )}
      </Stack>

      {/* Create User Modal */}
      <Modal
        opened={createModalOpened}
        onClose={() => {
          setCreateModalOpened(false);
          createForm.reset();
        }}
        title="Create User"
      >
        <form
          onSubmit={createForm.onSubmit((values) =>
            createUserMutation.mutate(values),
          )}
        >
          <Stack gap="md">
            <TextInput
              label="Username"
              placeholder="johndoe"
              {...createForm.getInputProps("username")}
            />
            <TextInput
              label="Email"
              placeholder="john@example.com"
              {...createForm.getInputProps("email")}
            />
            <PasswordInput
              label="Password"
              placeholder="Enter password"
              {...createForm.getInputProps("password")}
            />
            <Select
              label="Role"
              description="Reader: View content. Maintainer: Manage libraries. Admin: Full access."
              data={roleOptions}
              {...createForm.getInputProps("role")}
            />
            <Group justify="flex-end">
              <Button
                variant="subtle"
                onClick={() => setCreateModalOpened(false)}
              >
                Cancel
              </Button>
              <Button type="submit" loading={createUserMutation.isPending}>
                Create User
              </Button>
            </Group>
          </Stack>
        </form>
      </Modal>

      {/* Edit User Modal */}
      <Modal
        opened={editModalOpened}
        onClose={() => {
          setEditModalOpened(false);
          setSelectedUser(null);
          setCustomPermissions([]);
          setPermissionsExpanded(false);
        }}
        title={`Edit User: ${selectedUser?.username}`}
        size="lg"
      >
        <form
          onSubmit={editForm.onSubmit((values) => {
            if (selectedUser) {
              // Combine role permissions with custom permissions
              const rolePerms = ROLE_PERMISSIONS[values.role] || [];
              const allPermissions = [
                ...new Set([...rolePerms, ...customPermissions]),
              ];
              updateUserMutation.mutate({
                userId: selectedUser.id,
                data: {
                  ...values,
                  permissions: allPermissions,
                },
              });
            }
          })}
        >
          <Stack gap="md">
            <TextInput
              label="Username"
              placeholder="johndoe"
              {...editForm.getInputProps("username")}
            />
            <TextInput
              label="Email"
              placeholder="john@example.com"
              {...editForm.getInputProps("email")}
            />
            <PasswordInput
              label="New Password"
              placeholder="Leave blank to keep current password"
              {...editForm.getInputProps("password")}
            />
            <Select
              label="Role"
              description="Reader: View content. Maintainer: Manage libraries. Admin: Full access."
              data={roleOptions}
              {...editForm.getInputProps("role")}
              disabled={selectedUser?.id === currentUser?.id}
            />
            <Switch
              label="Active"
              description="Inactive users cannot log in"
              {...editForm.getInputProps("isActive", { type: "checkbox" })}
              disabled={selectedUser?.id === currentUser?.id}
            />
            {selectedUser?.id === currentUser?.id && (
              <Alert icon={<IconAlertCircle size={16} />} color="yellow">
                You cannot change your own role or deactivate your own account.
              </Alert>
            )}

            {/* Sharing Tag Grants */}
            {selectedUser && <UserSharingTagGrants userId={selectedUser.id} />}

            {/* Custom Permissions */}
            {selectedUser && selectedUser.role !== "admin" && (
              <>
                <Divider />
                <Box>
                  <Button
                    variant="subtle"
                    color="gray"
                    size="sm"
                    leftSection={
                      permissionsExpanded ? (
                        <IconChevronDown size={16} />
                      ) : (
                        <IconChevronRight size={16} />
                      )
                    }
                    onClick={() => setPermissionsExpanded(!permissionsExpanded)}
                    px={0}
                  >
                    Custom Permissions
                    {customPermissions.length > 0 && (
                      <Badge size="sm" ml="xs" variant="light">
                        {customPermissions.length}
                      </Badge>
                    )}
                  </Button>
                  <Text size="xs" c="dimmed" mt={4}>
                    Grant additional permissions beyond the role&apos;s defaults
                  </Text>
                  <Collapse in={permissionsExpanded}>
                    <Box mt="md">
                      <PermissionPicker
                        selectedPermissions={customPermissions}
                        onPermissionsChange={setCustomPermissions}
                        disabledCheckedPermissions={
                          ROLE_PERMISSIONS[editForm.values.role] || []
                        }
                      />
                    </Box>
                  </Collapse>
                </Box>
              </>
            )}

            <Group justify="flex-end">
              <Button
                variant="subtle"
                onClick={() => setEditModalOpened(false)}
              >
                Cancel
              </Button>
              <Button type="submit" loading={updateUserMutation.isPending}>
                Save Changes
              </Button>
            </Group>
          </Stack>
        </form>
      </Modal>

      {/* Delete User Modal */}
      <Modal
        opened={deleteModalOpened}
        onClose={() => {
          setDeleteModalOpened(false);
          setSelectedUser(null);
        }}
        title="Delete User"
      >
        <Stack gap="md">
          <Text>
            Are you sure you want to delete the user{" "}
            <strong>{selectedUser?.username}</strong>?
          </Text>
          <Text size="sm" c="dimmed">
            This action cannot be undone. All data associated with this user
            (reading progress, ratings, preferences) will be permanently
            deleted.
          </Text>
          <Group justify="flex-end">
            <Button
              variant="subtle"
              onClick={() => setDeleteModalOpened(false)}
            >
              Cancel
            </Button>
            <Button
              color="red"
              loading={deleteUserMutation.isPending}
              onClick={() =>
                selectedUser && deleteUserMutation.mutate(selectedUser.id)
              }
            >
              Delete User
            </Button>
          </Group>
        </Stack>
      </Modal>
    </Box>
  );
}
