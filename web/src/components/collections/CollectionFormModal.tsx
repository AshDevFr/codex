import {
  Button,
  Checkbox,
  Group,
  Modal,
  Stack,
  TextInput,
} from "@mantine/core";
import { useEffect, useState } from "react";
import type { Collection } from "@/api/collections";
import {
  useCreateCollection,
  useUpdateCollection,
} from "@/hooks/useCollections";

interface CollectionFormModalProps {
  opened: boolean;
  onClose: () => void;
  /** When provided, the modal edits this collection instead of creating one. */
  collection?: Collection;
  /** Called with the created collection (create mode only). */
  onCreated?: (collection: Collection) => void;
}

export function CollectionFormModal({
  opened,
  onClose,
  collection,
  onCreated,
}: CollectionFormModalProps) {
  const isEdit = Boolean(collection);
  const [name, setName] = useState("");
  const [ordered, setOrdered] = useState(false);

  // Seed fields when (re)opening.
  useEffect(() => {
    if (opened) {
      setName(collection?.name ?? "");
      setOrdered(collection?.ordered ?? false);
    }
  }, [opened, collection]);

  const createMutation = useCreateCollection();
  const updateMutation = useUpdateCollection(collection?.id ?? "");
  const pending = createMutation.isPending || updateMutation.isPending;

  const submit = () => {
    const trimmed = name.trim();
    if (!trimmed) return;
    if (isEdit) {
      updateMutation.mutate(
        { name: trimmed, ordered },
        { onSuccess: () => onClose() },
      );
    } else {
      createMutation.mutate(
        { name: trimmed, ordered },
        {
          onSuccess: (created) => {
            onCreated?.(created);
            onClose();
          },
        },
      );
    }
  };

  return (
    <Modal
      opened={opened}
      onClose={onClose}
      title={isEdit ? "Edit collection" : "New collection"}
      centered
    >
      <Stack gap="md">
        <TextInput
          label="Name"
          placeholder="e.g. Batman"
          value={name}
          onChange={(e) => setName(e.currentTarget.value)}
          data-autofocus
          required
        />
        <Checkbox
          label="Keep series in manual order"
          description="When off, members are sorted by title."
          checked={ordered}
          onChange={(e) => setOrdered(e.currentTarget.checked)}
        />
        <Group justify="flex-end">
          <Button variant="subtle" onClick={onClose}>
            Cancel
          </Button>
          <Button onClick={submit} loading={pending} disabled={!name.trim()}>
            {isEdit ? "Save" : "Create"}
          </Button>
        </Group>
      </Stack>
    </Modal>
  );
}
