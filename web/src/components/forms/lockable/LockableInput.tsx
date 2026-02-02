import type { TextInputProps } from "@mantine/core";
import { ActionIcon, Group, TextInput, Tooltip } from "@mantine/core";
import { IconLock, IconLockOpen } from "@tabler/icons-react";

export interface LockableInputProps
  extends Omit<TextInputProps, "value" | "onChange"> {
  /** Current value */
  value: string;
  /** Callback when value changes */
  onChange: (value: string) => void;
  /** Whether the field is locked */
  locked: boolean;
  /** Callback when lock state changes */
  onLockChange: (locked: boolean) => void;
  /** Original value (for auto-lock detection) */
  originalValue?: string;
  /** Whether to auto-lock when value differs from original */
  autoLock?: boolean;
}

/**
 * A text input with a lock icon to indicate if the field is protected
 * from automatic updates (e.g., scanner re-analysis).
 *
 * When locked, the field value will not be overwritten during automatic
 * metadata updates.
 */
export function LockableInput({
  value,
  onChange,
  locked,
  onLockChange,
  originalValue,
  autoLock = true,
  ...props
}: LockableInputProps) {
  const handleChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const newValue = e.target.value;
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
      <TextInput
        value={value}
        onChange={handleChange}
        style={{ flex: 1 }}
        {...props}
      />
    </Group>
  );
}
