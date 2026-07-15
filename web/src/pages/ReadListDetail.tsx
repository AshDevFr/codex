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

  // No override sends no sort param: the server then applies the read list's
  // default order (manual when `ordered`, release date otherwise).
  const [sortOverride, setSortOverride] = useState<ReadListBookSort | null>(
    null,
  );
  const { data: readList, isLoading } = useReadList(readListId);
  const { data: books } = useReadListBooks(
    readListId,
    sortOverride ?? undefined,
  );
  const sort: ReadListBookSort =
    sortOverride ?? (readList?.ordered ? "manual" : "release");

  const removeMutation = useRemoveBookFromReadList(readListId ?? "");
  const reorderMutation = useReorderReadList(readListId ?? "");
  const deleteMutation = useDeleteReadList();

  const [editOpen, setEditOpen] = useState(false);
  const [deleteOpen, setDeleteOpen] = useState(false);
  // Reordering rewrites the manual reading order with no undo, so it stays
  // locked until explicitly enabled to keep stray drags harmless.
  const [reorderUnlocked, setReorderUnlocked] = useState(false);

  const members: Book[] = books ?? [];
  // Dragging edits the shared reading order, so it is only offered in the
  // Manual view (any read list maintains positions, ordered or not).
  const canReorder = canWrite && sort === "manual";
  const items: MediaGridItem[] = members.map((b) => ({
    id: b.id,
    type: "book",
    data: b,
  }));

  if (isLoading) {
    return (
      <Container fluid py="md">
        <Skeleton height={32} width={240} mb="lg" />
        <MediaGrid items={[]} loading />
      </Container>
    );
  }

  if (!readList) {
    return (
      <Container fluid py="md">
        <Center mih={240}>
          <Text c="dimmed">Read list not found.</Text>
        </Center>
      </Container>
    );
  }

  return (
    <Container fluid py="md">
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
          {members.length > 1 && (
            <SegmentedControl
              value={sort}
              onChange={(value) => setSortOverride(value as ReadListBookSort)}
              data={[
                { label: "Release", value: "release" },
                { label: "Title", value: "title" },
                { label: "Date added", value: "added" },
                { label: "Manual", value: "manual" },
              ]}
              aria-label="Sort books"
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
          reorderable={canReorder && reorderUnlocked}
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
