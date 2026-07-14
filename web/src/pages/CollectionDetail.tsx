import {
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
} from "@mantine/core";
import { IconEdit, IconTrash } from "@tabler/icons-react";
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

  const [sort, setSort] = useState<CollectionSeriesSort>("title");
  const { data: collection, isLoading } = useCollection(collectionId);
  // The server ignores sort for manually ordered collections; skip the param
  // there so the query cache doesn't fragment per sort.
  const { data: series } = useCollectionSeries(
    collectionId,
    collection?.ordered ? undefined : sort,
  );

  const removeMutation = useRemoveSeriesFromCollection(collectionId ?? "");
  const reorderMutation = useReorderCollection(collectionId ?? "");
  const deleteMutation = useDeleteCollection();

  const [editOpen, setEditOpen] = useState(false);
  const [deleteOpen, setDeleteOpen] = useState(false);

  const members: Series[] = series ?? [];
  const reorderable = canWrite && Boolean(collection?.ordered);
  const items: MediaGridItem[] = members.map((s) => ({
    id: s.id,
    type: "series",
    data: s,
  }));

  if (isLoading) {
    return (
      <Container size="xl" py="md">
        <Skeleton height={32} width={240} mb="lg" />
        <MediaGrid items={[]} loading />
      </Container>
    );
  }

  if (!collection) {
    return (
      <Container size="xl" py="md">
        <Center mih={240}>
          <Text c="dimmed">Collection not found.</Text>
        </Center>
      </Container>
    );
  }

  return (
    <Container size="xl" py="md">
      <Group justify="space-between" align="center" mb="lg" wrap="nowrap">
        <Group gap="sm" align="center" style={{ minWidth: 0 }}>
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
        <Group gap="xs" wrap="nowrap">
          {!collection.ordered && members.length > 1 && (
            <SegmentedControl
              value={sort}
              onChange={(value) => setSort(value as CollectionSeriesSort)}
              data={[
                { label: "Title", value: "title" },
                { label: "Date added", value: "added" },
                { label: "Year", value: "year" },
              ]}
              aria-label="Sort series"
            />
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
          reorderable={reorderable}
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
