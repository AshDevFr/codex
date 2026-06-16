import {
  ActionIcon,
  Badge,
  Button,
  Center,
  Container,
  Group,
  Modal,
  SimpleGrid,
  Skeleton,
  Stack,
  Text,
  Title,
  Tooltip,
} from "@mantine/core";
import {
  IconChevronDown,
  IconChevronUp,
  IconEdit,
  IconTrash,
  IconX,
} from "@tabler/icons-react";
import { useState } from "react";
import { useNavigate, useParams } from "react-router-dom";
import { CollectionFormModal } from "@/components/collections/CollectionFormModal";
import { MediaCard } from "@/components/library/MediaCard";
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

  const { data: collection, isLoading } = useCollection(collectionId);
  const { data: series } = useCollectionSeries(collectionId);

  const removeMutation = useRemoveSeriesFromCollection(collectionId ?? "");
  const reorderMutation = useReorderCollection(collectionId ?? "");
  const deleteMutation = useDeleteCollection();

  const [editOpen, setEditOpen] = useState(false);
  const [deleteOpen, setDeleteOpen] = useState(false);

  const members: Series[] = series ?? [];
  const reorderable = canWrite && Boolean(collection?.ordered);

  const move = (index: number, delta: number) => {
    const target = index + delta;
    if (target < 0 || target >= members.length) return;
    const ids = members.map((s) => s.id);
    [ids[index], ids[target]] = [ids[target], ids[index]];
    reorderMutation.mutate(ids);
  };

  if (isLoading) {
    return (
      <Container size="xl" py="md">
        <Skeleton height={32} width={240} mb="lg" />
        <SimpleGrid cols={{ base: 2, sm: 3, md: 4, lg: 6 }} spacing="md">
          {Array.from({ length: 6 }).map((_, i) => (
            // biome-ignore lint/suspicious/noArrayIndexKey: static skeletons
            <Skeleton key={i} height={300} radius="md" />
          ))}
        </SimpleGrid>
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
        {(canWrite || canDelete) && (
          <Group gap="xs" wrap="nowrap">
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
        )}
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
        <SimpleGrid cols={{ base: 2, sm: 3, md: 4, lg: 6 }} spacing="md">
          {members.map((s, index) => (
            <Stack key={s.id} gap={4}>
              <MediaCard type="series" data={s} />
              {canWrite && (
                <Group gap={4} justify="center">
                  {reorderable && (
                    <>
                      <Tooltip label="Move up">
                        <ActionIcon
                          variant="subtle"
                          size="sm"
                          disabled={index === 0 || reorderMutation.isPending}
                          onClick={() => move(index, -1)}
                          aria-label="Move up"
                        >
                          <IconChevronUp size={16} />
                        </ActionIcon>
                      </Tooltip>
                      <Tooltip label="Move down">
                        <ActionIcon
                          variant="subtle"
                          size="sm"
                          disabled={
                            index === members.length - 1 ||
                            reorderMutation.isPending
                          }
                          onClick={() => move(index, 1)}
                          aria-label="Move down"
                        >
                          <IconChevronDown size={16} />
                        </ActionIcon>
                      </Tooltip>
                    </>
                  )}
                  <Tooltip label="Remove from collection">
                    <ActionIcon
                      variant="subtle"
                      color="red"
                      size="sm"
                      loading={
                        removeMutation.isPending &&
                        removeMutation.variables === s.id
                      }
                      onClick={() => removeMutation.mutate(s.id)}
                      aria-label="Remove from collection"
                    >
                      <IconX size={16} />
                    </ActionIcon>
                  </Tooltip>
                </Group>
              )}
            </Stack>
          ))}
        </SimpleGrid>
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
