import { Button, Card, Group, Modal, Stack, Switch, Text } from "@mantine/core";
import { useDisclosure } from "@mantine/hooks";
import { notifications } from "@mantine/notifications";
import { IconUserPlus } from "@tabler/icons-react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { type AccessGroupDetailDto, accessGroupsApi } from "@/api/accessGroups";

interface DefaultGroupCardProps {
  group: AccessGroupDetailDto;
}

/**
 * Default-group controls for an access group: the "new users join this group"
 * switch, and a one-off action to add everyone who already has an account.
 */
export function DefaultGroupCard({ group }: DefaultGroupCardProps) {
  const queryClient = useQueryClient();
  const [confirmOpen, { open: openConfirm, close: closeConfirm }] =
    useDisclosure(false);

  const invalidate = () => {
    queryClient.invalidateQueries({ queryKey: ["access-group", group.id] });
    queryClient.invalidateQueries({ queryKey: ["access-groups"] });
    queryClient.invalidateQueries({ queryKey: ["users"] });
  };

  const setDefaultMutation = useMutation({
    mutationFn: (isDefault: boolean) =>
      accessGroupsApi.update(group.id, { isDefault }),
    onSuccess: (_, isDefault) => {
      invalidate();
      notifications.show({
        title: "Success",
        message: isDefault
          ? `New users will join ${group.name}`
          : `${group.name} is no longer the default group`,
        color: "green",
      });
    },
    onError: (error: { message?: string }) => {
      notifications.show({
        title: "Error",
        message: error.message || "Failed to update the default group",
        color: "red",
      });
    },
  });

  const addAllMutation = useMutation({
    mutationFn: () => accessGroupsApi.addAllUsers(group.id),
    onSuccess: ({ added }) => {
      invalidate();
      closeConfirm();
      notifications.show({
        title: "Success",
        message: `Added ${added} ${added === 1 ? "user" : "users"}`,
        color: "green",
      });
    },
    onError: (error: { message?: string }) => {
      notifications.show({
        title: "Error",
        message: error.message || "Failed to add users",
        color: "red",
      });
    },
  });

  return (
    <Card withBorder p="md">
      <Stack gap="md">
        <Switch
          label="Default group for new users"
          description="New accounts join this group when they are created, whether by an admin, by registration or on first OIDC login. They can be removed later like any member. Turning this on replaces the current default group, if there is one."
          checked={group.isDefault}
          onChange={(event) =>
            setDefaultMutation.mutate(event.currentTarget.checked)
          }
          disabled={setDefaultMutation.isPending}
        />

        <Group justify="space-between" align="center" wrap="nowrap">
          <Text size="sm" c="dimmed">
            The default only applies to accounts created from now on. To bring
            existing users in, add them all at once.
          </Text>
          <Button
            variant="light"
            leftSection={<IconUserPlus size={16} />}
            onClick={openConfirm}
            style={{ flexShrink: 0 }}
          >
            Add all existing users
          </Button>
        </Group>
      </Stack>

      <Modal
        opened={confirmOpen}
        onClose={closeConfirm}
        title={`Add all users to ${group.name}?`}
      >
        <Stack gap="md">
          <Text size="sm">
            Every user who is not already a member will be added to this group,
            including administrators. Existing memberships are left as they are.
          </Text>
          <Group justify="flex-end">
            <Button variant="default" onClick={closeConfirm}>
              Cancel
            </Button>
            <Button
              onClick={() => addAllMutation.mutate()}
              loading={addAllMutation.isPending}
            >
              Add all users
            </Button>
          </Group>
        </Stack>
      </Modal>
    </Card>
  );
}
