import {
  ActionIcon,
  Button,
  Group,
  Modal,
  Stack,
  Text,
  TextInput,
  Tooltip,
} from "@mantine/core";
import { notifications } from "@mantine/notifications";
import { IconPlus, IconTrash } from "@tabler/icons-react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useState } from "react";
import { booksApi } from "@/api/books";
import { seriesMetadataApi } from "@/api/seriesMetadata";

interface ExternalIdEntry {
  id?: string;
  source: string;
  externalId: string;
  externalUrl?: string | null;
  lastSyncedAt?: string | null;
  isNew?: boolean;
}

interface ExternalIdEditModalProps {
  entityType: "series" | "book";
  entityId: string;
  opened: boolean;
  onClose: () => void;
}

function formatLastSynced(
  dateString: string | null | undefined,
): string | null {
  if (!dateString) return null;
  const date = new Date(dateString);
  const now = new Date();
  const diffMs = now.getTime() - date.getTime();
  const diffDays = Math.floor(diffMs / (1000 * 60 * 60 * 24));
  if (diffDays === 0) return "Synced today";
  if (diffDays === 1) return "Synced yesterday";
  if (diffDays < 7) return `Synced ${diffDays} days ago`;
  if (diffDays < 30) return `Synced ${Math.floor(diffDays / 7)} weeks ago`;
  return `Synced ${Math.floor(diffDays / 30)} months ago`;
}

export function ExternalIdEditModal({
  entityType,
  entityId,
  opened,
  onClose,
}: ExternalIdEditModalProps) {
  const queryClient = useQueryClient();
  const [entries, setEntries] = useState<ExternalIdEntry[]>([]);
  const [isDirty, setIsDirty] = useState(false);

  const queryKey =
    entityType === "series"
      ? ["series-external-ids", entityId]
      : ["book-external-ids", entityId];

  const { data: existingIds, isLoading } = useQuery({
    queryKey,
    queryFn: async (): Promise<ExternalIdEntry[]> => {
      const ids =
        entityType === "series"
          ? await seriesMetadataApi.listExternalIds(entityId)
          : await booksApi.listExternalIds(entityId);
      return ids.map((ext) => ({
        id: ext.id,
        source: ext.source,
        externalId: ext.externalId,
        externalUrl: ext.externalUrl,
        lastSyncedAt: ext.lastSyncedAt,
      }));
    },
    enabled: opened,
  });

  useEffect(() => {
    if (!opened) return;
    if (existingIds) {
      setEntries([...existingIds]);
      setIsDirty(false);
    } else {
      setEntries([]);
      setIsDirty(false);
    }
  }, [existingIds, opened]);

  const saveMutation = useMutation({
    mutationFn: async () => {
      const existing = existingIds ?? [];
      const existingById = new Map(existing.map((e) => [e.id, e]));

      // Find entries to delete (in existing but not in current entries)
      const currentIds = new Set(entries.filter((e) => e.id).map((e) => e.id));
      const toDelete = existing.filter((e) => !currentIds.has(e.id));

      // Find entries to create/update
      const toSave = entries.filter((e) => {
        if (e.isNew) return true;
        const orig = existingById.get(e.id!);
        if (!orig) return true;
        return (
          orig.source !== e.source ||
          orig.externalId !== e.externalId ||
          (orig.externalUrl ?? "") !== (e.externalUrl ?? "")
        );
      });

      // Execute deletes
      for (const item of toDelete) {
        if (!item.id) continue;
        if (entityType === "series") {
          await seriesMetadataApi.deleteExternalId(entityId, item.id);
        } else {
          await booksApi.deleteExternalId(entityId, item.id);
        }
      }

      // Execute creates/updates (upsert by source)
      for (const item of toSave) {
        if (entityType === "series") {
          await seriesMetadataApi.createExternalId(entityId, {
            source: item.source,
            externalId: item.externalId,
            externalUrl: item.externalUrl || undefined,
          });
        } else {
          await booksApi.createExternalId(entityId, {
            source: item.source,
            externalId: item.externalId,
            externalUrl: item.externalUrl || undefined,
          });
        }
      }
    },
    onSuccess: () => {
      notifications.show({
        title: "External IDs updated",
        message: "Changes have been saved",
        color: "green",
      });
      setIsDirty(false);
      onClose();
      // Invalidate after closing so the refetch doesn't clear entries during the close animation
      queryClient.invalidateQueries({ queryKey });
      if (entityType === "series") {
        queryClient.invalidateQueries({
          queryKey: ["series-metadata", entityId],
        });
        queryClient.invalidateQueries({
          queryKey: ["series", entityId],
        });
      } else {
        queryClient.invalidateQueries({
          queryKey: ["book", entityId],
        });
        queryClient.invalidateQueries({
          queryKey: ["book-external-ids", entityId],
        });
      }
    },
    onError: (error: Error) => {
      notifications.show({
        title: "Failed to update external IDs",
        message: error.message || "An error occurred",
        color: "red",
      });
    },
  });

  const addEntry = () => {
    setEntries([
      ...entries,
      { source: "manual", externalId: "", externalUrl: "", isNew: true },
    ]);
    setIsDirty(true);
  };

  const removeEntry = (index: number) => {
    setEntries(entries.filter((_, i) => i !== index));
    setIsDirty(true);
  };

  const updateEntry = (
    index: number,
    field: keyof ExternalIdEntry,
    value: string,
  ) => {
    setEntries(
      entries.map((entry, i) =>
        i === index ? { ...entry, [field]: value } : entry,
      ),
    );
    setIsDirty(true);
  };

  const hasEmptyFields = entries.some(
    (e) => !e.source.trim() || !e.externalId.trim(),
  );

  const hasDuplicateSources = (() => {
    const sources = entries.map((e) => e.source.trim().toLowerCase());
    return new Set(sources).size !== sources.length;
  })();

  return (
    <Modal
      opened={opened}
      onClose={onClose}
      title="Edit External Source IDs"
      size="lg"
    >
      <Stack gap="md">
        {isLoading ? (
          <Text size="sm" c="dimmed">
            Loading...
          </Text>
        ) : (
          <>
            {entries.length === 0 ? (
              <Text size="sm" c="dimmed">
                No external IDs configured. Click "Add" to create one.
              </Text>
            ) : (
              <Stack gap="xs">
                {entries.map((entry, index) => {
                  const lastSynced = formatLastSynced(entry.lastSyncedAt);
                  const isDuplicateSource =
                    entry.source.trim() !== "" &&
                    entries.some(
                      (other, otherIdx) =>
                        otherIdx !== index &&
                        other.source.trim().toLowerCase() ===
                          entry.source.trim().toLowerCase(),
                    );
                  return (
                    <Group
                      key={entry.id ?? `new-${index}`}
                      gap="xs"
                      align="flex-end"
                      wrap="nowrap"
                    >
                      <TextInput
                        label={index === 0 ? "Source" : undefined}
                        placeholder="e.g. plugin:anilist"
                        value={entry.source}
                        onChange={(e) =>
                          updateEntry(index, "source", e.currentTarget.value)
                        }
                        error={
                          isDuplicateSource ? "Duplicate source" : undefined
                        }
                        style={{ flex: 1 }}
                        size="sm"
                      />
                      <TextInput
                        label={index === 0 ? "External ID" : undefined}
                        placeholder="e.g. 12345"
                        value={entry.externalId}
                        onChange={(e) =>
                          updateEntry(
                            index,
                            "externalId",
                            e.currentTarget.value,
                          )
                        }
                        style={{ flex: 1 }}
                        size="sm"
                      />
                      <TextInput
                        label={index === 0 ? "URL (optional)" : undefined}
                        placeholder="https://..."
                        value={entry.externalUrl ?? ""}
                        onChange={(e) =>
                          updateEntry(
                            index,
                            "externalUrl",
                            e.currentTarget.value,
                          )
                        }
                        style={{ flex: 1.5 }}
                        size="sm"
                      />
                      <Tooltip
                        label={lastSynced ?? "Delete"}
                        position="top"
                        withArrow
                      >
                        <ActionIcon
                          variant="subtle"
                          color="red"
                          onClick={() => removeEntry(index)}
                          size="sm"
                          mb={2}
                        >
                          <IconTrash size={14} />
                        </ActionIcon>
                      </Tooltip>
                    </Group>
                  );
                })}
              </Stack>
            )}

            <Button
              variant="light"
              leftSection={<IconPlus size={14} />}
              onClick={addEntry}
              size="xs"
              w="fit-content"
            >
              Add
            </Button>

            <Group justify="flex-end" mt="md">
              <Button variant="default" onClick={onClose} size="sm">
                Cancel
              </Button>
              <Button
                onClick={() => saveMutation.mutate()}
                loading={saveMutation.isPending}
                disabled={!isDirty || hasEmptyFields || hasDuplicateSources}
                size="sm"
              >
                Save Changes
              </Button>
            </Group>
          </>
        )}
      </Stack>
    </Modal>
  );
}
