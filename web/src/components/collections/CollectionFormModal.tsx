import {
  Button,
  Checkbox,
  Group,
  Modal,
  Stack,
  Textarea,
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
  const [summary, setSummary] = useState("");
  const [ordered, setOrdered] = useState(false);

  // Seed fields when (re)opening.
  useEffect(() => {
    if (opened) {
      setName(collection?.name ?? "");
      setSummary(collection?.summary ?? "");
      setOrdered(collection?.ordered ?? false);
    }
  }, [opened, collection]);

  const createMutation = useCreateCollection();
  const updateMutation = useUpdateCollection(collection?.id ?? "");
  const pending = createMutation.isPending || updateMutation.isPending;

  const submit = () => {
    const trimmed = name.trim();
    if (!trimmed) return;
    const trimmedSummary = summary.trim();
    if (isEdit) {
      updateMutation.mutate(
        { name: trimmed, summary: trimmedSummary || null, ordered },
        { onSuccess: () => onClose() },
      );
    } else {
      createMutation.mutate(
        { name: trimmed, summary: trimmedSummary || undefined, ordered },
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
        <Textarea
          label="Summary"
          placeholder="Optional description"
          value={summary}
          onChange={(e) => setSummary(e.currentTarget.value)}
          autosize
          minRows={2}
        />
        <Checkbox
          label="Default to manual order"
          description="When off, members default to sorting by title. Either way, every sort (including Manual) stays available on the collection page."
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
