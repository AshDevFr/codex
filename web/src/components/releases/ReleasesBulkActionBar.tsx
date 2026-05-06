import { Button, Card, Group, Text } from "@mantine/core";
import {
  IconCheck,
  IconEyeOff,
  IconRefresh,
  IconTrash,
  IconX,
} from "@tabler/icons-react";
import type { BulkReleaseAction } from "@/api/releases";

interface ReleasesBulkActionBarProps {
  count: number;
  isPending: boolean;
  onAction: (action: BulkReleaseAction) => void;
  onClear: () => void;
  /** Show the Delete button. The inbox routes Delete through a confirm modal,
   *  which it wires up itself; the series panel currently doesn't expose
   *  bulk-delete (use per-row delete instead). */
  onDeleteClick?: () => void;
  /** When true, render as a sticky banner (page-level inbox). Off for the
   *  embedded series panel where the parent card already provides framing. */
  sticky?: boolean;
}

export function ReleasesBulkActionBar({
  count,
  isPending,
  onAction,
  onClear,
  onDeleteClick,
  sticky = false,
}: ReleasesBulkActionBarProps) {
  return (
    <Card
      withBorder
      padding="sm"
      radius="md"
      style={sticky ? { position: "sticky", top: 0, zIndex: 2 } : undefined}
    >
      <Group justify="space-between" wrap="wrap">
        <Text size="sm" fw={500}>
          {count} selected
        </Text>
        <Group gap="xs">
          <Button
            size="xs"
            variant="light"
            color="green"
            leftSection={<IconCheck size={14} />}
            loading={isPending}
            onClick={() => onAction("mark-acquired")}
          >
            Mark acquired
          </Button>
          <Button
            size="xs"
            variant="light"
            color="gray"
            leftSection={<IconX size={14} />}
            loading={isPending}
            onClick={() => onAction("dismiss")}
          >
            Dismiss
          </Button>
          <Button
            size="xs"
            variant="light"
            color="gray"
            leftSection={<IconEyeOff size={14} />}
            loading={isPending}
            onClick={() => onAction("ignore")}
          >
            Ignore
          </Button>
          <Button
            size="xs"
            variant="light"
            color="blue"
            leftSection={<IconRefresh size={14} />}
            loading={isPending}
            onClick={() => onAction("reset")}
          >
            Reset
          </Button>
          {onDeleteClick && (
            <Button
              size="xs"
              variant="light"
              color="red"
              leftSection={<IconTrash size={14} />}
              loading={isPending}
              onClick={onDeleteClick}
            >
              Delete
            </Button>
          )}
          <Button size="xs" variant="subtle" onClick={onClear}>
            Clear
          </Button>
        </Group>
      </Group>
    </Card>
  );
}
