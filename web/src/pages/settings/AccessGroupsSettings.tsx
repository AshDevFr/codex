import {
  ActionIcon,
  Alert,
  Badge,
  Box,
  Button,
  Card,
  Group,
  Modal,
  Stack,
  Text,
  Textarea,
  TextInput,
  Title,
  Tooltip,
} from "@mantine/core";
import { useForm } from "@mantine/form";
import { notifications } from "@mantine/notifications";
import {
  IconAlertCircle,
  IconEdit,
  IconEye,
  IconPlus,
  IconShieldCheck,
  IconTrash,
} from "@tabler/icons-react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { Link } from "react-router-dom";
import { type AccessGroupDto, accessGroupsApi } from "@/api/accessGroups";
import { TableSkeleton } from "@/components/skeletons";
import { ResponsiveTable, type ResponsiveTableColumn } from "@/components/ui";
import { useShowSkeleton } from "@/lib/motion/useShowSkeleton";

export function AccessGroupsSettings() {
  const queryClient = useQueryClient();
  const [createModalOpened, setCreateModalOpened] = useState(false);
  const [editModalOpened, setEditModalOpened] = useState(false);
  const [deleteModalOpened, setDeleteModalOpened] = useState(false);
  const [selectedGroup, setSelectedGroup] = useState<AccessGroupDto | null>(
    null,
  );

  const {
    data: accessGroups,
    isLoading,
    error,
  } = useQuery({
    queryKey: ["access-groups"],
    queryFn: accessGroupsApi.list,
  });
  const showSkeleton = useShowSkeleton(isLoading);

  const createForm = useForm({
    initialValues: { name: "", description: "" },
    validate: {
      name: (value) => (value.trim().length < 1 ? "Name is required" : null),
    },
  });

  const editForm = useForm({
    initialValues: { name: "", description: "" },
    validate: {
      name: (value) => (value.trim().length < 1 ? "Name is required" : null),
    },
  });

  const createMutation = useMutation({
    mutationFn: async (data: { name: string; description: string }) => {
      return accessGroupsApi.create({
        name: data.name.trim(),
        description: data.description.trim() || null,
      });
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["access-groups"] });
      setCreateModalOpened(false);
      createForm.reset();
      notifications.show({
        title: "Success",
        message: "Access group created successfully",
        color: "green",
      });
    },
    onError: (error: { message?: string }) => {
      notifications.show({
        title: "Error",
        message: error.message || "Failed to create access group",
        color: "red",
      });
    },
  });

  const updateMutation = useMutation({
    mutationFn: async ({
      groupId,
      data,
    }: {
      groupId: string;
      data: { name: string; description: string };
    }) => {
      return accessGroupsApi.update(groupId, {
        name: data.name.trim(),
        description: data.description.trim() || null,
      });
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["access-groups"] });
      setEditModalOpened(false);
      setSelectedGroup(null);
      notifications.show({
        title: "Success",
        message: "Access group updated successfully",
        color: "green",
      });
    },
    onError: (error: { message?: string }) => {
      notifications.show({
        title: "Error",
        message: error.message || "Failed to update access group",
        color: "red",
      });
    },
  });

  const deleteMutation = useMutation({
    mutationFn: async (groupId: string) => {
      return accessGroupsApi.delete(groupId);
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["access-groups"] });
      setDeleteModalOpened(false);
      setSelectedGroup(null);
      notifications.show({
        title: "Success",
        message: "Access group deleted successfully",
        color: "green",
      });
    },
    onError: (error: { message?: string }) => {
      notifications.show({
        title: "Error",
        message: error.message || "Failed to delete access group",
        color: "red",
      });
    },
  });

  const handleEditGroup = (group: AccessGroupDto) => {
    setSelectedGroup(group);
    editForm.setValues({
      name: group.name,
      description: group.description || "",
    });
    setEditModalOpened(true);
  };

  const handleDeleteGroup = (group: AccessGroupDto) => {
    setSelectedGroup(group);
    setDeleteModalOpened(true);
  };

  const columns: ResponsiveTableColumn<AccessGroupDto>[] = [
    {
      key: "name",
      header: "Group",
      mobilePrimary: true,
      accessor: (group) => (
        <Group gap="sm" wrap="nowrap">
          <IconShieldCheck size={20} />
          <Text
            fw={500}
            component={Link}
            to={`/settings/access-groups/${group.id}`}
            c="blue"
            td="none"
            style={{ cursor: "pointer" }}
          >
            {group.name}
          </Text>
        </Group>
      ),
    },
    {
      key: "description",
      header: "Description",
      mobileFullWidth: true,
      accessor: (group) => (
        <Text size="sm" c={group.description ? undefined : "dimmed"}>
          {group.description || "No description"}
        </Text>
      ),
    },
    {
      key: "members",
      header: "Members",
      accessor: (group) => (
        <Badge variant="light" color="blue">
          {group.memberCount} {group.memberCount === 1 ? "member" : "members"}
        </Badge>
      ),
    },
    {
      key: "grants",
      header: "Grants",
      accessor: (group) => (
        <Badge variant="light" color="grape">
          {group.grantCount} {group.grantCount === 1 ? "grant" : "grants"}
        </Badge>
      ),
    },
  ];

  return (
    <Box py="xl" px="md">
      <Stack gap="xl">
        <Group justify="space-between">
          <div>
            <Title order={1}>Access Groups</Title>
            <Text c="dimmed" size="sm" mt="xs">
              Manage groups of sharing-tag grants that can be assigned to
              multiple users
            </Text>
          </div>
          <Button
            leftSection={<IconPlus size={16} />}
            onClick={() => setCreateModalOpened(true)}
          >
            Create Group
          </Button>
        </Group>

        {isLoading ? (
          showSkeleton ? (
            <TableSkeleton
              rows={4}
              columnLabels={["Group", "Description", "Members", "Grants"]}
              withMobilePrimary
            />
          ) : null
        ) : error ? (
          <Alert icon={<IconAlertCircle size={16} />} color="red">
            Failed to load access groups. Please try again.
          </Alert>
        ) : accessGroups && accessGroups.length > 0 ? (
          <Card withBorder p={{ base: 0, xs: "md" }}>
            <ResponsiveTable
              data={accessGroups}
              columns={columns}
              getRowKey={(group) => group.id}
              rowActions={(group) => (
                <>
                  <Tooltip label="View Details">
                    <ActionIcon
                      variant="subtle"
                      component={Link}
                      to={`/settings/access-groups/${group.id}`}
                      aria-label={`View ${group.name}`}
                    >
                      <IconEye size={16} />
                    </ActionIcon>
                  </Tooltip>
                  <Tooltip label="Edit Group">
                    <ActionIcon
                      variant="subtle"
                      onClick={() => handleEditGroup(group)}
                      aria-label={`Edit ${group.name}`}
                    >
                      <IconEdit size={16} />
                    </ActionIcon>
                  </Tooltip>
                  <Tooltip label="Delete Group">
                    <ActionIcon
                      variant="subtle"
                      color="red"
                      onClick={() => handleDeleteGroup(group)}
                      aria-label={`Delete ${group.name}`}
                    >
                      <IconTrash size={16} />
                    </ActionIcon>
                  </Tooltip>
                </>
              )}
            />
          </Card>
        ) : (
          <Alert
            icon={<IconShieldCheck size={16} />}
            color="gray"
            variant="light"
          >
            <Text fw={500}>No access groups yet</Text>
            <Text size="sm" mt="xs">
              Create access groups to bundle sharing-tag grants and assign them
              to multiple users at once. Users inherit all grants from their
              groups, with per-user overrides taking precedence.
            </Text>
          </Alert>
        )}
      </Stack>

      {/* Create Group Modal */}
      <Modal
        opened={createModalOpened}
        onClose={() => {
          setCreateModalOpened(false);
          createForm.reset();
        }}
        title="Create Access Group"
      >
        <form
          onSubmit={createForm.onSubmit((values) =>
            createMutation.mutate(values),
          )}
        >
          <Stack gap="md">
            <TextInput
              label="Name"
              placeholder="e.g., Manga Readers, Library Staff"
              description="A unique name for this group"
              required
              {...createForm.getInputProps("name")}
            />
            <Textarea
              label="Description"
              placeholder="Optional description for this group"
              description="Help admins understand the purpose of this group"
              rows={3}
              {...createForm.getInputProps("description")}
            />
            <Group justify="flex-end">
              <Button
                variant="subtle"
                onClick={() => {
                  setCreateModalOpened(false);
                  createForm.reset();
                }}
              >
                Cancel
              </Button>
              <Button type="submit" loading={createMutation.isPending}>
                Create Group
              </Button>
            </Group>
          </Stack>
        </form>
      </Modal>

      {/* Edit Group Modal */}
      <Modal
        opened={editModalOpened}
        onClose={() => {
          setEditModalOpened(false);
          setSelectedGroup(null);
        }}
        title={`Edit Group: ${selectedGroup?.name}`}
      >
        <form
          onSubmit={editForm.onSubmit((values) => {
            if (selectedGroup) {
              updateMutation.mutate({
                groupId: selectedGroup.id,
                data: values,
              });
            }
          })}
        >
          <Stack gap="md">
            <TextInput
              label="Name"
              placeholder="e.g., Manga Readers"
              required
              {...editForm.getInputProps("name")}
            />
            <Textarea
              label="Description"
              placeholder="Optional description"
              rows={3}
              {...editForm.getInputProps("description")}
            />
            <Group justify="flex-end">
              <Button
                variant="subtle"
                onClick={() => {
                  setEditModalOpened(false);
                  setSelectedGroup(null);
                }}
              >
                Cancel
              </Button>
              <Button type="submit" loading={updateMutation.isPending}>
                Save Changes
              </Button>
            </Group>
          </Stack>
        </form>
      </Modal>

      {/* Delete Group Modal */}
      <Modal
        opened={deleteModalOpened}
        onClose={() => {
          setDeleteModalOpened(false);
          setSelectedGroup(null);
        }}
        title="Delete Access Group"
      >
        <Stack gap="md">
          <Text>
            Are you sure you want to delete the access group{" "}
            <strong>{selectedGroup?.name}</strong>?
          </Text>
          {selectedGroup &&
            (selectedGroup.memberCount > 0 || selectedGroup.grantCount > 0) && (
              <Alert icon={<IconAlertCircle size={16} />} color="yellow">
                This group has {selectedGroup.memberCount}{" "}
                {selectedGroup.memberCount === 1 ? "member" : "members"} and{" "}
                {selectedGroup.grantCount}{" "}
                {selectedGroup.grantCount === 1 ? "grant" : "grants"}. Deleting
                it will remove all memberships and grants.
              </Alert>
            )}
          <Text size="sm" c="dimmed">
            This action cannot be undone.
          </Text>
          <Group justify="flex-end">
            <Button
              variant="subtle"
              onClick={() => {
                setDeleteModalOpened(false);
                setSelectedGroup(null);
              }}
            >
              Cancel
            </Button>
            <Button
              color="red"
              loading={deleteMutation.isPending}
              onClick={() =>
                selectedGroup && deleteMutation.mutate(selectedGroup.id)
              }
            >
              Delete Group
            </Button>
          </Group>
        </Stack>
      </Modal>
    </Box>
  );
}
