import { Button, Group, Modal, Stack, Text } from "@mantine/core";

interface ReleasesBulkDeleteModalProps {
  opened: boolean;
  onClose: () => void;
  onConfirm: () => void;
  count: number;
  isPending: boolean;
}

/** Confirmation modal for bulk-deleting ledger entries.
 *  Hard-deletes are reversible by the upstream re-poll, so we surface that
 *  caveat in the body — users typically want Dismiss, not Delete. */
export function ReleasesBulkDeleteModal({
  opened,
  onClose,
  onConfirm,
  count,
  isPending,
}: ReleasesBulkDeleteModalProps) {
  const noun = count === 1 ? "release" : "releases";
  return (
    <Modal opened={opened} onClose={onClose} title="Delete releases?" centered>
      <Stack gap="md">
        <Text size="sm">
          This will hard-delete {count} {noun} from the ledger and clear the
          affected sources' cache so they re-fetch on the next poll. The
          releases will reappear if the upstream still lists them.
        </Text>
        <Group justify="flex-end" gap="xs">
          <Button variant="subtle" onClick={onClose}>
            Cancel
          </Button>
          <Button color="red" loading={isPending} onClick={onConfirm}>
            Delete {count} {noun}
          </Button>
        </Group>
      </Stack>
    </Modal>
  );
}
