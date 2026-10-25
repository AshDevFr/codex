import { Badge, Group, Text } from "@mantine/core";
import { IconCloud } from "@tabler/icons-react";
import type { components } from "@/types/api.generated";

type UserAccessGroup = components["schemas"]["UserAccessGroupDto"];

interface UserAccessGroupBadgesProps {
  groups: UserAccessGroup[];
}

/** Compact list of the access groups a user belongs to, for table cells. */
export function UserAccessGroupBadges({ groups }: UserAccessGroupBadgesProps) {
  if (groups.length === 0) {
    return (
      <Text size="sm" c="dimmed">
        None
      </Text>
    );
  }

  return (
    <Group gap={4}>
      {groups.map((group) => {
        const fromOidc = group.source === "oidc";
        return (
          <Badge
            key={group.id}
            size="sm"
            variant="light"
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
