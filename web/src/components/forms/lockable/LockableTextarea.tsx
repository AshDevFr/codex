import type { TextareaProps } from "@mantine/core";
import { ActionIcon, Group, Textarea, Tooltip } from "@mantine/core";
import { IconLock, IconLockOpen } from "@tabler/icons-react";

export interface LockableTextareaProps
  extends Omit<TextareaProps, "value" | "onChange"> {
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
 * A textarea with a lock icon to indicate if the field is protected
 * from automatic updates (e.g., scanner re-analysis).
 *
 * When locked, the field value will not be overwritten during automatic
 * metadata updates.
 */
export function LockableTextarea({
  value,
  onChange,
  locked,
  onLockChange,
  originalValue,
  autoLock = true,
  ...props
}: LockableTextareaProps) {
  const handleChange = (e: React.ChangeEvent<HTMLTextAreaElement>) => {
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
    <Group gap="xs" wrap="nowrap" align="flex-start">
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
          mt={props.label ? 24 : 0}
        >
          {locked ? <IconLock size={18} /> : <IconLockOpen size={18} />}
        </ActionIcon>
      </Tooltip>
      <Textarea
        value={value}
        onChange={handleChange}
        style={{ flex: 1 }}
        {...props}
      />
    </Group>
  );
}
