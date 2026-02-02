/**
 * Shared permission picker component for selecting permissions.
 * Used in both API key creation and user permission editing.
 */

import { Box, Checkbox, SimpleGrid, Stack, Text } from "@mantine/core";
import {
  PERMISSION_GROUPS,
  type Permission,
  type PermissionGroup as PermissionGroupType,
} from "@/types/permissions";

export interface PermissionPickerProps {
  /** Currently selected permissions */
  selectedPermissions: Permission[];
  /** Callback when permissions change */
  onPermissionsChange: (permissions: Permission[]) => void;
  /**
   * Permissions that are always checked and disabled (e.g., role permissions for users).
   * These are shown as checked but cannot be unchecked.
   */
  disabledCheckedPermissions?: Permission[];
  /**
   * Permissions that are disabled and unchecked (e.g., permissions user doesn't have for tokens).
   * These are shown as unchecked and cannot be checked.
   */
  disabledUncheckedPermissions?: Permission[];
  /** Number of columns for the permission grid */
  columns?: number;
}

/**
 * A permission picker that displays permissions grouped by category.
 *
 * Supports two types of disabled states:
 * - disabledCheckedPermissions: Shown checked but cannot be unchecked (e.g., role-based permissions)
 * - disabledUncheckedPermissions: Shown unchecked and cannot be checked (e.g., unavailable permissions)
 */
export function PermissionPicker({
  selectedPermissions,
  onPermissionsChange,
  disabledCheckedPermissions = [],
  disabledUncheckedPermissions = [],
  columns = 2,
}: PermissionPickerProps) {
  const handlePermissionToggle = (permission: Permission, checked: boolean) => {
    if (checked) {
      onPermissionsChange([...selectedPermissions, permission]);
    } else {
      onPermissionsChange(selectedPermissions.filter((p) => p !== permission));
    }
  };

  // Filter out groups that have no visible permissions
  const visibleGroups = PERMISSION_GROUPS.filter((group) => {
    return group.permissions.some(
      (p) => !disabledUncheckedPermissions.includes(p.value),
    );
  });

  return (
    <Stack gap="md">
      {visibleGroups.map((group) => (
        <PermissionGroupSection
          key={group.label}
          group={group}
          selectedPermissions={selectedPermissions}
          disabledCheckedPermissions={disabledCheckedPermissions}
          disabledUncheckedPermissions={disabledUncheckedPermissions}
          onPermissionToggle={handlePermissionToggle}
          columns={columns}
        />
      ))}
    </Stack>
  );
}

interface PermissionGroupSectionProps {
  group: PermissionGroupType;
  selectedPermissions: Permission[];
  disabledCheckedPermissions: Permission[];
  disabledUncheckedPermissions: Permission[];
  onPermissionToggle: (permission: Permission, checked: boolean) => void;
  columns: number;
}

function PermissionGroupSection({
  group,
  selectedPermissions,
  disabledCheckedPermissions,
  disabledUncheckedPermissions,
  onPermissionToggle,
  columns,
}: PermissionGroupSectionProps) {
  // Filter permissions that should be shown (not in disabledUnchecked)
  const visiblePermissions = group.permissions.filter(
    (p) => !disabledUncheckedPermissions.includes(p.value),
  );

  if (visiblePermissions.length === 0) {
    return null;
  }

  return (
    <Box>
      <Text size="sm" fw={500} mb="xs">
        {group.label}
      </Text>
      <SimpleGrid cols={columns} spacing="xs" verticalSpacing="xs">
        {visiblePermissions.map((perm) => {
          const isDisabledChecked = disabledCheckedPermissions.includes(
            perm.value,
          );
          const isSelected = selectedPermissions.includes(perm.value);
          const isChecked = isDisabledChecked || isSelected;

          return (
            <Checkbox
              key={perm.value}
              label={perm.label}
              description={perm.description}
              checked={isChecked}
              disabled={isDisabledChecked}
              onChange={(e) =>
                onPermissionToggle(perm.value, e.currentTarget.checked)
              }
              styles={{
                root: {
                  alignItems: "flex-start",
                },
                body: {
                  alignItems: "flex-start",
                },
              }}
            />
          );
        })}
      </SimpleGrid>
    </Box>
  );
}
