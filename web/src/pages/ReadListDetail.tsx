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
import type { ReadListBookSort } from "@/api/readlists";
import { MediaGrid, type MediaGridItem } from "@/components/library/MediaGrid";
import { ReadListFormModal } from "@/components/readlists/ReadListFormModal";
import { usePermissions } from "@/hooks/usePermissions";
import {
  useDeleteReadList,
  useReadList,
  useReadListBooks,
  useRemoveBookFromReadList,
  useReorderReadList,
} from "@/hooks/useReadLists";
import type { Book } from "@/types";
import { PERMISSIONS } from "@/types/permissions";

export function ReadListDetail() {
  const { readListId } = useParams<{ readListId: string }>();
  const navigate = useNavigate();
  const { hasPermission } = usePermissions();
  const canWrite = hasPermission(PERMISSIONS.READLISTS_WRITE);
  const canDelete = hasPermission(PERMISSIONS.READLISTS_DELETE);

  const [sort, setSort] = useState<ReadListBookSort>("release");
  const { data: readList, isLoading } = useReadList(readListId);
  // The server ignores sort for manually ordered read lists; skip the param
  // there so the query cache doesn't fragment per sort.
  const { data: books } = useReadListBooks(
    readListId,
    readList?.ordered ? undefined : sort,
  );

  const removeMutation = useRemoveBookFromReadList(readListId ?? "");
  const reorderMutation = useReorderReadList(readListId ?? "");
  const deleteMutation = useDeleteReadList();

  const [editOpen, setEditOpen] = useState(false);
  const [deleteOpen, setDeleteOpen] = useState(false);

  const members: Book[] = books ?? [];
  const reorderable = canWrite && Boolean(readList?.ordered);
  const items: MediaGridItem[] = members.map((b) => ({
    id: b.id,
    type: "book",
    data: b,
  }));

  if (isLoading) {
    return (
      <Container size="xl" py="md">
        <Skeleton height={32} width={240} mb="lg" />
        <MediaGrid items={[]} loading />
      </Container>
    );
  }

  if (!readList) {
    return (
      <Container size="xl" py="md">
        <Center mih={240}>
          <Text c="dimmed">Read list not found.</Text>
        </Center>
      </Container>
    );
  }

  return (
    <Container size="xl" py="md">
      <Group justify="space-between" align="flex-start" mb="md" wrap="nowrap">
        <Stack gap={4} style={{ minWidth: 0 }}>
          <Group gap="sm" align="center">
            <Title order={2} style={{ wordBreak: "break-word" }}>
              {readList.name}
            </Title>
            <Badge variant="light">{readList.bookCount} books</Badge>
            {readList.ordered && (
              <Badge variant="outline" color="gray">
                Ordered
              </Badge>
            )}
          </Group>
          {readList.summary && (
            <Text c="dimmed" size="sm">
              {readList.summary}
            </Text>
          )}
        </Stack>
        <Group gap="xs" wrap="nowrap">
          {!readList.ordered && members.length > 1 && (
            <SegmentedControl
              value={sort}
              onChange={(value) => setSort(value as ReadListBookSort)}
              data={[
                { label: "Release", value: "release" },
                { label: "Title", value: "title" },
                { label: "Date added", value: "added" },
              ]}
              aria-label="Sort books"
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
            <Text c="dimmed">This read list has no books yet.</Text>
            {canWrite && (
              <Text c="dimmed" size="sm">
                Open a book and use "Add to read list".
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
          removeLabel="Remove from read list"
          removingId={
            removeMutation.isPending ? removeMutation.variables : undefined
          }
          reorderable={reorderable}
          onReorder={(ids) => reorderMutation.mutate(ids)}
          reorderPending={reorderMutation.isPending}
        />
      )}

      <ReadListFormModal
        opened={editOpen}
        onClose={() => setEditOpen(false)}
        readList={readList}
      />

      <Modal
        opened={deleteOpen}
        onClose={() => setDeleteOpen(false)}
        title="Delete read list"
        centered
      >
        <Stack gap="md">
          <Text>
            Delete <strong>{readList.name}</strong>? The books themselves are
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
                deleteMutation.mutate(readList.id, {
                  onSuccess: () => navigate("/readlists"),
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
