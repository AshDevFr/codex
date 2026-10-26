import { Badge, Group, Text } from "@mantine/core";
import { IconCloud } from "@tabler/icons-react";
import type { components } from "@/types/api.generated";

type UserAccessGroup = components["schemas"]["UserAccessGroupDto"];

interface UserAccessGroupBadgesProps {
  groups: UserAccessGroup[];
  /** Admins and maintainers are never restricted, so their groups are shown as inactive */
  unrestricted?: boolean;
}

/** Compact list of the access groups a user belongs to, for table cells. */
export function UserAccessGroupBadges({
  groups,
  unrestricted = false,
}: UserAccessGroupBadgesProps) {
  if (groups.length === 0) {
    return (
      <Text size="sm" c="dimmed">
        None
      </Text>
    );
  }

  return (
    <Group gap={4}>
      {unrestricted && (
        <Text size="xs" c="dimmed" w="100%">
          Not restricted (role)
        </Text>
      )}
      {groups.map((group) => {
        const fromOidc = group.source === "oidc";
        return (
          <Badge
            key={group.id}
            size="sm"
            variant="light"
            color={unrestricted ? "gray" : undefined}
            leftSection={fromOidc ? <IconCloud size={12} /> : undefined}
            aria-label={fromOidc ? `${group.name} (from OIDC)` : undefined}
            title={fromOidc ? "Assigned from an OIDC group" : undefined}
          >
            {group.name}
          </Badge>
        );
      })}
    </Group>
  );
}
