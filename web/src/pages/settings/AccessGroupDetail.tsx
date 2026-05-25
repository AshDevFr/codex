import {
  ActionIcon,
  Alert,
  Badge,
  Box,
  Button,
  Card,
  Group,
  Loader,
  Select,
  Stack,
  Text,
  TextInput,
  Title,
  Tooltip,
} from "@mantine/core";
import { notifications } from "@mantine/notifications";
import {
  IconAlertCircle,
  IconArrowLeft,
  IconLink,
  IconPlus,
  IconShare,
  IconShieldCheck,
  IconTrash,
  IconUsers,
} from "@tabler/icons-react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { Link, useParams } from "react-router-dom";
import { type AccessGroupDetailDto, accessGroupsApi } from "@/api/accessGroups";
import { type AccessMode, sharingTagsApi } from "@/api/sharingTags";
import { usersApi } from "@/api/users";

export function AccessGroupDetail() {
  const { groupId } = useParams<{ groupId: string }>();
  const queryClient = useQueryClient();

  const {
    data: group,
    isLoading,
    error,
  } = useQuery({
    queryKey: ["access-group", groupId],
    queryFn: () => accessGroupsApi.get(groupId!),
    enabled: !!groupId,
  });

  if (isLoading) {
    return (
      <Box py="xl" px="md">
        <Group justify="center" py="xl">
          <Loader />
        </Group>
      </Box>
    );
  }

  if (error || !group) {
    return (
      <Box py="xl" px="md">
        <Alert icon={<IconAlertCircle size={16} />} color="red">
          Failed to load access group. It may have been deleted.
        </Alert>
      </Box>
    );
  }

  return (
    <Box py="xl" px="md">
      <Stack gap="xl">
        {/* Header */}
        <Group justify="space-between">
          <div>
            <Group gap="sm" mb="xs">
              <Button
                variant="subtle"
                size="compact-sm"
                component={Link}
                to="/settings/access-groups"
                leftSection={<IconArrowLeft size={14} />}
              >
                Back
              </Button>
            </Group>
            <Group gap="sm">
              <IconShieldCheck size={24} />
              <Title order={1}>{group.name}</Title>
            </Group>
            {group.description && (
              <Text c="dimmed" size="sm" mt="xs">
                {group.description}
              </Text>
            )}
          </div>
        </Group>

        {/* Members Section */}
        <MembersSection
          groupId={groupId!}
          group={group}
          queryClient={queryClient}
        />

        {/* Grants Section */}
        <GrantsSection
          groupId={groupId!}
          group={group}
          queryClient={queryClient}
        />

        {/* OIDC Mappings Section */}
        <OidcMappingsSection
          groupId={groupId!}
          group={group}
          queryClient={queryClient}
        />
      </Stack>
    </Box>
  );
}

// ==================== Members Section ====================

function MembersSection({
  groupId,
  group,
  queryClient,
}: {
  groupId: string;
  group: AccessGroupDetailDto;
  queryClient: ReturnType<typeof useQueryClient>;
}) {
  const [selectedUserId, setSelectedUserId] = useState<string | null>(null);

  const { data: usersResponse } = useQuery({
    queryKey: ["users", { pageSize: 100 }],
    queryFn: () => usersApi.list({ pageSize: 100 }),
  });

  const addMemberMutation = useMutation({
    mutationFn: (userId: string) =>
      accessGroupsApi.addMembers(groupId, [userId]),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["access-group", groupId] });
      queryClient.invalidateQueries({ queryKey: ["access-groups"] });
      setSelectedUserId(null);
      notifications.show({
        title: "Success",
        message: "Member added",
        color: "green",
      });
    },
    onError: (error: { message?: string }) => {
      notifications.show({
        title: "Error",
        message: error.message || "Failed to add member",
        color: "red",
      });
    },
  });

  const removeMemberMutation = useMutation({
    mutationFn: (userId: string) =>
      accessGroupsApi.removeMember(groupId, userId),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["access-group", groupId] });
      queryClient.invalidateQueries({ queryKey: ["access-groups"] });
      notifications.show({
        title: "Success",
        message: "Member removed",
        color: "green",
      });
    },
    onError: (error: { message?: string }) => {
      notifications.show({
        title: "Error",
        message: error.message || "Failed to remove member",
        color: "red",
      });
    },
  });

  const memberUserIds = new Set(group.members.map((m) => m.userId));
  const availableUsers =
    usersResponse?.data?.filter((u) => !memberUserIds.has(u.id)) || [];
  const userOptions = availableUsers.map((u) => ({
    value: u.id,
    label: `${u.username} (${u.email})`,
  }));

  return (
    <Card withBorder p="md">
      <Stack gap="md">
        <Group gap="sm">
          <IconUsers size={20} />
          <Title order={4}>Members</Title>
          <Badge variant="light" size="sm">
            {group.members.length}
          </Badge>
        </Group>

        {group.members.length > 0 ? (
          <Stack gap="xs">
            {group.members.map((member) => (
              <Group key={member.userId} justify="space-between">
                <Group gap="sm">
                  <Text size="sm" fw={500}>
                    {member.username}
                  </Text>
                  {member.source === "oidc" && (
                    <Badge size="xs" variant="outline" color="violet">
                      OIDC
                    </Badge>
                  )}
                </Group>
                <Tooltip label="Remove member">
                  <ActionIcon
                    size="sm"
                    variant="subtle"
                    color="red"
                    onClick={() => removeMemberMutation.mutate(member.userId)}
                    disabled={removeMemberMutation.isPending}
                  >
                    <IconTrash size={14} />
                  </ActionIcon>
                </Tooltip>
              </Group>
            ))}
          </Stack>
        ) : (
          <Text size="sm" c="dimmed">
            No members yet.
          </Text>
        )}

        {userOptions.length > 0 && (
          <Group gap="sm" align="flex-end">
            <Select
              label="Add member"
              placeholder="Select user..."
              data={userOptions}
              value={selectedUserId}
              onChange={setSelectedUserId}
              searchable
              style={{ flex: 1 }}
            />
            <Tooltip label="Add member">
              <ActionIcon
                variant="filled"
                color="blue"
                size="lg"
                onClick={() =>
                  selectedUserId && addMemberMutation.mutate(selectedUserId)
                }
                disabled={!selectedUserId || addMemberMutation.isPending}
                loading={addMemberMutation.isPending}
              >
                <IconPlus size={16} />
              </ActionIcon>
            </Tooltip>
          </Group>
        )}
      </Stack>
    </Card>
  );
}

// ==================== Grants Section ====================

function GrantsSection({
  groupId,
  group,
  queryClient,
}: {
  groupId: string;
  group: AccessGroupDetailDto;
  queryClient: ReturnType<typeof useQueryClient>;
}) {
  const [selectedTagId, setSelectedTagId] = useState<string | null>(null);
  const [selectedAccessMode, setSelectedAccessMode] =
    useState<AccessMode>("allow");

  const { data: allTags } = useQuery({
    queryKey: ["sharing-tags"],
    queryFn: sharingTagsApi.list,
  });

  const addGrantMutation = useMutation({
    mutationFn: ({
      tagId,
      accessMode,
    }: {
      tagId: string;
      accessMode: AccessMode;
    }) => accessGroupsApi.addGrant(groupId, tagId, accessMode),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["access-group", groupId] });
      queryClient.invalidateQueries({ queryKey: ["access-groups"] });
      setSelectedTagId(null);
      notifications.show({
        title: "Success",
        message: "Grant added",
        color: "green",
      });
    },
    onError: (error: { message?: string }) => {
      notifications.show({
        title: "Error",
        message: error.message || "Failed to add grant",
        color: "red",
      });
    },
  });

  const removeGrantMutation = useMutation({
    mutationFn: (tagId: string) => accessGroupsApi.removeGrant(groupId, tagId),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["access-group", groupId] });
      queryClient.invalidateQueries({ queryKey: ["access-groups"] });
      notifications.show({
        title: "Success",
        message: "Grant removed",
        color: "green",
      });
    },
    onError: (error: { message?: string }) => {
      notifications.show({
        title: "Error",
        message: error.message || "Failed to remove grant",
        color: "red",
      });
    },
  });

  const grantTagIds = new Set(group.grants.map((g) => g.sharingTagId));
  const availableTags =
    allTags?.filter((tag) => !grantTagIds.has(tag.id)) || [];
  const tagOptions = availableTags.map((tag) => ({
    value: tag.id,
    label: tag.name,
  }));

  const accessModeOptions = [
    { value: "allow", label: "Allow" },
    { value: "deny", label: "Deny" },
  ];

  return (
    <Card withBorder p="md">
      <Stack gap="md">
        <Group gap="sm">
          <IconShare size={20} />
          <Title order={4}>Tag Grants</Title>
          <Badge variant="light" size="sm">
            {group.grants.length}
          </Badge>
        </Group>

        <Text size="sm" c="dimmed">
          Members of this group inherit these sharing-tag grants. "Deny" always
          wins over "Allow" across all sources.
        </Text>

        {group.grants.length > 0 ? (
          <Stack gap="xs">
            {group.grants.map((grant) => (
              <Group key={grant.sharingTagId} justify="space-between">
                <Group gap="sm">
                  <Badge
                    variant="light"
                    color={grant.accessMode === "deny" ? "red" : "green"}
                  >
                    {grant.accessMode === "deny" ? "Deny" : "Allow"}
                  </Badge>
                  <Text size="sm">{grant.sharingTagName}</Text>
                </Group>
                <Tooltip label="Remove grant">
                  <ActionIcon
                    size="sm"
                    variant="subtle"
                    color="red"
                    onClick={() =>
                      removeGrantMutation.mutate(grant.sharingTagId)
                    }
                    disabled={removeGrantMutation.isPending}
                  >
                    <IconTrash size={14} />
                  </ActionIcon>
                </Tooltip>
              </Group>
            ))}
          </Stack>
        ) : (
          <Text size="sm" c="dimmed">
            No grants configured.
          </Text>
        )}

        {tagOptions.length > 0 && (
          <Group gap="sm" align="flex-end">
            <Select
              label="Add grant"
              placeholder="Select tag..."
              data={tagOptions}
              value={selectedTagId}
              onChange={setSelectedTagId}
              searchable
              style={{ flex: 1 }}
            />
            <Select
              label="Access"
              data={accessModeOptions}
              value={selectedAccessMode}
              onChange={(value) =>
                setSelectedAccessMode((value as AccessMode) || "allow")
              }
              w={100}
            />
            <Tooltip label="Add grant">
              <ActionIcon
                variant="filled"
                color="blue"
                size="lg"
                onClick={() =>
                  selectedTagId &&
                  addGrantMutation.mutate({
                    tagId: selectedTagId,
                    accessMode: selectedAccessMode,
                  })
                }
                disabled={!selectedTagId || addGrantMutation.isPending}
                loading={addGrantMutation.isPending}
              >
                <IconPlus size={16} />
              </ActionIcon>
            </Tooltip>
          </Group>
        )}
      </Stack>
    </Card>
  );
}

// ==================== OIDC Mappings Section ====================

function OidcMappingsSection({
  groupId,
  group,
  queryClient,
}: {
  groupId: string;
  group: AccessGroupDetailDto;
  queryClient: ReturnType<typeof useQueryClient>;
}) {
  const [oidcGroupName, setOidcGroupName] = useState("");

  const addMappingMutation = useMutation({
    mutationFn: (name: string) => accessGroupsApi.addOidcMapping(groupId, name),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["access-group", groupId] });
      setOidcGroupName("");
      notifications.show({
        title: "Success",
        message: "OIDC mapping added",
        color: "green",
      });
    },
    onError: (error: { message?: string }) => {
      notifications.show({
        title: "Error",
        message: error.message || "Failed to add OIDC mapping",
        color: "red",
      });
    },
  });

  const removeMappingMutation = useMutation({
    mutationFn: (mappingId: string) =>
      accessGroupsApi.removeOidcMapping(groupId, mappingId),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["access-group", groupId] });
      notifications.show({
        title: "Success",
        message: "OIDC mapping removed",
        color: "green",
      });
    },
    onError: (error: { message?: string }) => {
      notifications.show({
        title: "Error",
        message: error.message || "Failed to remove OIDC mapping",
        color: "red",
      });
    },
  });

  return (
    <Card withBorder p="md">
      <Stack gap="md">
        <Group gap="sm">
          <IconLink size={20} />
          <Title order={4}>OIDC Mappings</Title>
          <Badge variant="light" size="sm">
            {group.oidcMappings.length}
          </Badge>
        </Group>

        <Text size="sm" c="dimmed">
          Map IdP group names to this access group. Users with matching OIDC
          group claims will be auto-assigned on login. Case-sensitive match.
        </Text>

        {group.oidcMappings.length > 0 ? (
          <Stack gap="xs">
            {group.oidcMappings.map((mapping) => (
              <Group key={mapping.id} justify="space-between">
                <Group gap="sm">
                  <Badge variant="outline" color="violet">
                    OIDC
                  </Badge>
                  <Text size="sm" ff="monospace">
                    {mapping.oidcGroupName}
                  </Text>
                </Group>
                <Tooltip label="Remove mapping">
                  <ActionIcon
                    size="sm"
                    variant="subtle"
                    color="red"
                    onClick={() => removeMappingMutation.mutate(mapping.id)}
                    disabled={removeMappingMutation.isPending}
                  >
                    <IconTrash size={14} />
                  </ActionIcon>
                </Tooltip>
              </Group>
            ))}
          </Stack>
        ) : (
          <Text size="sm" c="dimmed">
            No OIDC mappings configured.
          </Text>
        )}

        <Group gap="sm" align="flex-end">
          <TextInput
            label="Add OIDC group mapping"
            placeholder="e.g., library-staff"
            value={oidcGroupName}
            onChange={(e) => setOidcGroupName(e.currentTarget.value)}
            style={{ flex: 1 }}
          />
          <Tooltip label="Add mapping">
            <ActionIcon
              variant="filled"
              color="blue"
              size="lg"
              onClick={() =>
                oidcGroupName.trim() &&
                addMappingMutation.mutate(oidcGroupName.trim())
              }
              disabled={!oidcGroupName.trim() || addMappingMutation.isPending}
              loading={addMappingMutation.isPending}
            >
              <IconPlus size={16} />
            </ActionIcon>
          </Tooltip>
        </Group>
      </Stack>
    </Card>
  );
}
