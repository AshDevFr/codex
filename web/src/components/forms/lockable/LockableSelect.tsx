import type { SelectProps } from "@mantine/core";
import { ActionIcon, Group, Select, Tooltip } from "@mantine/core";
import { IconLock, IconLockOpen } from "@tabler/icons-react";

export interface LockableSelectProps
  extends Omit<SelectProps, "value" | "onChange"> {
  /** Current value */
  value: string | null;
  /** Callback when value changes */
  onChange: (value: string | null) => void;
  /** Whether the field is locked */
  locked: boolean;
  /** Callback when lock state changes */
  onLockChange: (locked: boolean) => void;
  /** Original value (for auto-lock detection) */
  originalValue?: string | null;
  /** Whether to auto-lock when value differs from original */
  autoLock?: boolean;
}

/**
 * A select dropdown with a lock icon to indicate if the field is protected
 * from automatic updates (e.g., scanner re-analysis).
 *
 * When locked, the field value will not be overwritten during automatic
 * metadata updates.
 */
export function LockableSelect({
  value,
  onChange,
  locked,
  onLockChange,
  originalValue,
  autoLock = true,
  ...props
}: LockableSelectProps) {
  const handleChange = (newValue: string | null) => {
    onChange(newValue);

    // Auto-lock when value differs from original
    if (autoLock && originalValue !== undefined && !locked) {
      if (newValue !== originalValue) {
        onLockChange(true);
      }
    }
  };

  const toggleLock = () => {
    onLockChange(!locked);
  };

  return (
    <Group gap="xs" wrap="nowrap" align="flex-end">
      <Tooltip
        label={
          locked
            ? "Locked: Protected from automatic updates"
            : "Unlocked: Can be updated automatically"
        }
        position="left"
      >
        <ActionIcon
          variant="subtle"
          color={locked ? "orange" : "gray"}
          onClick={toggleLock}
          aria-label={locked ? "Unlock field" : "Lock field"}
        >
          {locked ? <IconLock size={18} /> : <IconLockOpen size={18} />}
        </ActionIcon>
      </Tooltip>
      <Select
        value={value}
        onChange={handleChange}
        style={{ flex: 1 }}
        comboboxProps={{ zIndex: 1100, ...props.comboboxProps }}
        {...props}
      />
    </Group>
  );
}
