import {
  ActionIcon,
  Box,
  Button,
  Group,
  Stack,
  TextInput,
  Tooltip,
} from "@mantine/core";
import {
  IconLock,
  IconLockOpen,
  IconPlus,
  IconTrash,
} from "@tabler/icons-react";
import { useCallback } from "react";

export interface ListItem {
  id: string;
  values: Record<string, string>;
  locked: boolean;
}

export interface FieldConfig {
  key: string;
  label: string;
  placeholder?: string;
  /** Flex value for width distribution (default: 1) */
  flex?: number;
}

export interface LockableListEditorProps {
  /** List of items */
  items: ListItem[];
  /** Callback when items change */
  onChange: (items: ListItem[]) => void;
  /** Field configuration for each column */
  fields: FieldConfig[];
  /** Original items (for auto-lock detection) */
  originalItems?: ListItem[];
  /** Whether to auto-lock when value differs from original */
  autoLock?: boolean;
  /** Label for the add button */
  addButtonLabel?: string;
  /** Generate a new unique ID for items */
  generateId?: () => string;
  /**
   * Optional callback to derive values for other fields when one field changes.
   * Returns a partial values object to merge into the item, or undefined to skip.
   * Example: auto-fill site name from a URL.
   */
  deriveValues?: (
    fieldKey: string,
    value: string,
    currentValues: Record<string, string>,
  ) => Record<string, string> | undefined;
}

/**
 * An editable list with per-row lock icons.
 * Each row has a lock, configurable fields, and a delete button.
 *
 * When a row is locked, it will not be overwritten during automatic
 * metadata updates.
 */
export function LockableListEditor({
  items,
  onChange,
  fields,
  originalItems,
  autoLock = true,
  addButtonLabel = "Add",
  generateId = () => crypto.randomUUID(),
  deriveValues,
}: LockableListEditorProps) {
  const findOriginalItem = useCallback(
    (id: string): ListItem | undefined => {
      return originalItems?.find((item) => item.id === id);
    },
    [originalItems],
  );

  const itemDiffersFromOriginal = useCallback(
    (item: ListItem): boolean => {
      const original = findOriginalItem(item.id);
      if (!original) return true; // New item always differs

      // Check if any field value differs
      return fields.some(
        (field) => item.values[field.key] !== original.values[field.key],
      );
    },
    [findOriginalItem, fields],
  );

  const handleFieldChange = (
    index: number,
    fieldKey: string,
    value: string,
  ) => {
    const newItems = [...items];
    const item = { ...newItems[index] };
    item.values = { ...item.values, [fieldKey]: value };

    // Derive other field values if callback provided
    if (deriveValues) {
      const derived = deriveValues(fieldKey, value, item.values);
      if (derived) {
        item.values = { ...item.values, ...derived };
      }
    }

    // Auto-lock when value differs from original
    if (autoLock && !item.locked && originalItems) {
      if (itemDiffersFromOriginal(item)) {
        item.locked = true;
      }
    }

    newItems[index] = item;
    onChange(newItems);
  };

  const handleLockToggle = (index: number) => {
    const newItems = [...items];
    newItems[index] = {
      ...newItems[index],
      locked: !newItems[index].locked,
    };
    onChange(newItems);
  };

  const handleDelete = (index: number) => {
    const newItems = items.filter((_, i) => i !== index);
    onChange(newItems);
  };

  const handleAdd = () => {
    const newItem: ListItem = {
      id: generateId(),
      values: fields.reduce(
        (acc, field) => {
          acc[field.key] = "";
          return acc;
        },
        {} as Record<string, string>,
      ),
      locked: false,
    };
    onChange([...items, newItem]);
  };

  return (
    <Stack gap="xs">
      {items.map((item, index) => (
        <Group key={item.id} gap="xs" wrap="nowrap" align="flex-end">
          <Tooltip
            label={
              item.locked
                ? "Locked: Protected from automatic updates"
                : "Unlocked: Can be updated automatically"
            }
            position="left"
          >
            <ActionIcon
              variant="subtle"
              color={item.locked ? "orange" : "gray"}
              onClick={() => handleLockToggle(index)}
              aria-label={item.locked ? "Unlock row" : "Lock row"}
            >
              {item.locked ? (
                <IconLock size={18} />
              ) : (
                <IconLockOpen size={18} />
              )}
            </ActionIcon>
          </Tooltip>

          {fields.map((field) => (
            <TextInput
              key={field.key}
              label={index === 0 ? field.label : undefined}
              placeholder={field.placeholder}
              value={item.values[field.key] || ""}
              onChange={(e) =>
                handleFieldChange(index, field.key, e.target.value)
              }
              style={{ flex: field.flex ?? 1 }}
            />
          ))}

          <ActionIcon
            variant="subtle"
            color="red"
            onClick={() => handleDelete(index)}
            aria-label="Delete row"
          >
            <IconTrash size={18} />
          </ActionIcon>
        </Group>
      ))}

      <Box>
        <Button
          variant="subtle"
          leftSection={<IconPlus size={16} />}
          onClick={handleAdd}
          size="sm"
        >
          {addButtonLabel}
        </Button>
      </Box>
    </Stack>
  );
}
