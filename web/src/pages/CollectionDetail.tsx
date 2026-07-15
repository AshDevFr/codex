import {
  ActionIcon,
  Badge,
  Button,
  Center,
  Container,
  Group,
  Modal,
  SegmentedControl,
  Skeleton,
  Stack,
  Text,
  Title,
  Tooltip,
} from "@mantine/core";
import {
  IconEdit,
  IconLock,
  IconLockOpen,
  IconTrash,
} from "@tabler/icons-react";
import { useState } from "react";
import { useNavigate, useParams } from "react-router-dom";
import type { CollectionSeriesSort } from "@/api/collections";
import { CollectionFormModal } from "@/components/collections/CollectionFormModal";
import { MediaGrid, type MediaGridItem } from "@/components/library/MediaGrid";
import {
  useCollection,
  useCollectionSeries,
  useDeleteCollection,
  useRemoveSeriesFromCollection,
  useReorderCollection,
} from "@/hooks/useCollections";
import { usePermissions } from "@/hooks/usePermissions";
import type { Series } from "@/types";
import { PERMISSIONS } from "@/types/permissions";

export function CollectionDetail() {
  const { collectionId } = useParams<{ collectionId: string }>();
  const navigate = useNavigate();
  const { hasPermission } = usePermissions();
  const canWrite = hasPermission(PERMISSIONS.COLLECTIONS_WRITE);
  const canDelete = hasPermission(PERMISSIONS.COLLECTIONS_DELETE);

  // No override sends no sort param: the server then applies the collection's
  // default order (manual when `ordered`, title otherwise).
  const [sortOverride, setSortOverride] = useState<CollectionSeriesSort | null>(
    null,
  );
  const { data: collection, isLoading } = useCollection(collectionId);
  const { data: series } = useCollectionSeries(
    collectionId,
    sortOverride ?? undefined,
  );
  const sort: CollectionSeriesSort =
    sortOverride ?? (collection?.ordered ? "manual" : "title");

  const removeMutation = useRemoveSeriesFromCollection(collectionId ?? "");
  const reorderMutation = useReorderCollection(collectionId ?? "");
  const deleteMutation = useDeleteCollection();

  const [editOpen, setEditOpen] = useState(false);
  const [deleteOpen, setDeleteOpen] = useState(false);
  // Reordering rewrites the manual order with no undo, so it stays locked
  // until explicitly enabled to keep stray drags harmless.
  const [reorderUnlocked, setReorderUnlocked] = useState(false);

  const members: Series[] = series ?? [];
  // Dragging edits the shared manual order, so it is only offered in the
  // Manual view (any collection maintains positions, ordered or not).
  const canReorder = canWrite && sort === "manual";
  const items: MediaGridItem[] = members.map((s) => ({
    id: s.id,
    type: "series",
    data: s,
  }));

  if (isLoading) {
    return (
      <Container fluid py="md">
        <Skeleton height={32} width={240} mb="lg" />
        <MediaGrid items={[]} loading />
      </Container>
    );
  }

  if (!collection) {
    return (
      <Container fluid py="md">
        <Center mih={240}>
          <Text c="dimmed">Collection not found.</Text>
        </Center>
      </Container>
    );
  }

  return (
    <Container fluid py="md">
      <Group justify="space-between" align="flex-start" mb="lg" wrap="nowrap">
        <Stack gap={4} style={{ minWidth: 0 }}>
          <Group gap="sm" align="center">
            <Title order={2} style={{ wordBreak: "break-word" }}>
              {collection.name}
            </Title>
            <Badge variant="light">{collection.seriesCount} series</Badge>
            {collection.ordered && (
              <Badge variant="outline" color="gray">
                Ordered
              </Badge>
            )}
          </Group>
          {collection.summary && (
            <Text c="dimmed" size="sm">
              {collection.summary}
            </Text>
          )}
        </Stack>
        <Group gap="xs" wrap="nowrap">
          {members.length > 1 && (
            <SegmentedControl
              value={sort}
              onChange={(value) =>
                setSortOverride(value as CollectionSeriesSort)
              }
              data={[
                { label: "Title", value: "title" },
                { label: "Date added", value: "added" },
                { label: "Year", value: "year" },
                { label: "Manual", value: "manual" },
              ]}
              aria-label="Sort series"
            />
          )}
          {canReorder && members.length > 1 && (
            <Tooltip
              label={
                reorderUnlocked
                  ? "Lock reordering"
                  : "Unlock reordering (drag & drop)"
              }
            >
              <ActionIcon
                variant={reorderUnlocked ? "filled" : "default"}
                size="lg"
                onClick={() => setReorderUnlocked((v) => !v)}
                aria-label={
                  reorderUnlocked ? "Lock reordering" : "Unlock reordering"
                }
              >
                {reorderUnlocked ? (
                  <IconLockOpen size={16} />
                ) : (
                  <IconLock size={16} />
                )}
              </ActionIcon>
            </Tooltip>
          )}
          {canWrite && (
            <Button
              variant="default"
              leftSection={<IconEdit size={16} />}
              onClick={() => setEditOpen(true)}
            >
              Edit
            </Button>
          )}
          {canDelete && (
            <Button
              color="red"
              variant="light"
              leftSection={<IconTrash size={16} />}
              onClick={() => setDeleteOpen(true)}
            >
              Delete
            </Button>
          )}
        </Group>
      </Group>

      {members.length === 0 ? (
        <Center mih={200}>
          <Stack align="center" gap="xs">
            <Text c="dimmed">This collection has no series yet.</Text>
            {canWrite && (
              <Text c="dimmed" size="sm">
                Open a series and use "Add to collection".
              </Text>
            )}
          </Stack>
        </Center>
      ) : (
        <MediaGrid
          items={items}
          onRemove={
            canWrite ? (item) => removeMutation.mutate(item.id) : undefined
          }
          removeLabel="Remove from collection"
          removingId={
            removeMutation.isPending ? removeMutation.variables : undefined
          }
          reorderable={canReorder && reorderUnlocked}
          onReorder={(ids) => reorderMutation.mutate(ids)}
          reorderPending={reorderMutation.isPending}
        />
      )}

      <CollectionFormModal
        opened={editOpen}
        onClose={() => setEditOpen(false)}
        collection={collection}
      />

      <Modal
        opened={deleteOpen}
        onClose={() => setDeleteOpen(false)}
        title="Delete collection"
        centered
      >
        <Stack gap="md">
          <Text>
            Delete <strong>{collection.name}</strong>? The series themselves are
            not affected.
          </Text>
          <Group justify="flex-end">
            <Button variant="subtle" onClick={() => setDeleteOpen(false)}>
              Cancel
            </Button>
            <Button
              color="red"
              loading={deleteMutation.isPending}
              onClick={() =>
                deleteMutation.mutate(collection.id, {
                  onSuccess: () => navigate("/collections"),
                })
              }
            >
              Delete
            </Button>
          </Group>
        </Stack>
      </Modal>
    </Container>
  );
}
