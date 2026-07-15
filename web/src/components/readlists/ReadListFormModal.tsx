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
import type { ReadList } from "@/api/readlists";
import { useCreateReadList, useUpdateReadList } from "@/hooks/useReadLists";

interface ReadListFormModalProps {
  opened: boolean;
  onClose: () => void;
  /** When provided, the modal edits this read list instead of creating one. */
  readList?: ReadList;
  /** Called with the created read list (create mode only). */
  onCreated?: (readList: ReadList) => void;
}

export function ReadListFormModal({
  opened,
  onClose,
  readList,
  onCreated,
}: ReadListFormModalProps) {
  const isEdit = Boolean(readList);
  const [name, setName] = useState("");
  const [summary, setSummary] = useState("");
  const [ordered, setOrdered] = useState(true);

  useEffect(() => {
    if (opened) {
      setName(readList?.name ?? "");
      setSummary(readList?.summary ?? "");
      setOrdered(readList?.ordered ?? true);
    }
  }, [opened, readList]);

  const createMutation = useCreateReadList();
  const updateMutation = useUpdateReadList(readList?.id ?? "");
  const pending = createMutation.isPending || updateMutation.isPending;

  const submit = () => {
    const trimmedName = name.trim();
    if (!trimmedName) return;
    const trimmedSummary = summary.trim();
    if (isEdit) {
      updateMutation.mutate(
        { name: trimmedName, summary: trimmedSummary || null, ordered },
        { onSuccess: () => onClose() },
      );
    } else {
      createMutation.mutate(
        { name: trimmedName, summary: trimmedSummary || undefined, ordered },
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
      title={isEdit ? "Edit read list" : "New read list"}
      centered
    >
      <Stack gap="md">
        <TextInput
          label="Name"
          placeholder="e.g. Civil War"
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
          label="Default to manual reading order"
          description="When off, members default to sorting by release date. Either way, every sort (including Manual) stays available on the read list page."
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
