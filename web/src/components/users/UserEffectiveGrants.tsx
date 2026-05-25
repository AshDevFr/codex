import { Badge, Card, Group, Loader, Stack, Text, Title } from "@mantine/core";
import { IconShieldCheck } from "@tabler/icons-react";
import { useQuery } from "@tanstack/react-query";
import { accessGroupsApi } from "@/api/accessGroups";

interface UserEffectiveGrantsProps {
  userId: string;
}

export function UserEffectiveGrants({ userId }: UserEffectiveGrantsProps) {
  const { data: effectiveGrants, isLoading } = useQuery({
    queryKey: ["user-effective-grants", userId],
    queryFn: () => accessGroupsApi.getEffectiveGrants(userId),
  });

  if (isLoading) {
    return (
      <Card withBorder p="md">
        <Stack gap="md">
          <Group gap="sm">
            <IconShieldCheck size={20} />
            <Title order={5}>Effective Grants</Title>
          </Group>
          <Group justify="center">
            <Loader size="sm" />
          </Group>
        </Stack>
      </Card>
    );
  }

  const grants = effectiveGrants?.grants || [];

  return (
    <Card withBorder p="md">
      <Stack gap="md">
        <Group gap="sm">
          <IconShieldCheck size={20} />
          <Title order={5}>Effective Grants</Title>
          {grants.length > 0 && (
            <Badge variant="light" size="sm">
              {grants.length}
            </Badge>
          )}
        </Group>

        <Text size="sm" c="dimmed">
          Combined view of all sharing-tag grants from both per-user overrides
          and access group memberships. User overrides are shown in bold.
        </Text>

        {grants.length > 0 ? (
          <Stack gap="xs">
            {grants.map((grant) => {
              const isUserSource = grant.sources.some((s) => s.kind === "user");
              const groupSources = grant.sources.filter(
                (s) => s.kind === "group",
              );

              return (
                <Group
                  key={`${grant.sharingTagId}-${grant.accessMode}`}
                  justify="space-between"
                  wrap="nowrap"
                >
                  <Group gap="sm" wrap="nowrap">
                    <Badge
                      variant="light"
                      color={grant.accessMode === "deny" ? "red" : "green"}
                    >
                      {grant.accessMode === "deny" ? "Deny" : "Allow"}
                    </Badge>
                    <Text size="sm" fw={isUserSource ? 700 : 400}>
                      {grant.sharingTagName}
                    </Text>
                  </Group>
                  <Group gap={4} wrap="nowrap">
                    {isUserSource && (
                      <Badge size="xs" variant="filled" color="blue">
                        user
                      </Badge>
                    )}
                    {groupSources.map((source) => (
                      <Badge
                        key={source.groupId}
                        size="xs"
                        variant="outline"
                        color="grape"
                      >
                        {source.groupName}
                      </Badge>
                    ))}
                  </Group>
                </Group>
              );
            })}
          </Stack>
        ) : (
          <Text size="sm" c="dimmed">
            No grants configured. User can see all content.
          </Text>
        )}
      </Stack>
    </Card>
  );
}
