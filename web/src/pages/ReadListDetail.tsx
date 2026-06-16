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
import { MediaCard } from "@/components/library/MediaCard";
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

  const { data: readList, isLoading } = useReadList(readListId);
  const { data: books } = useReadListBooks(readListId);

  const removeMutation = useRemoveBookFromReadList(readListId ?? "");
  const reorderMutation = useReorderReadList(readListId ?? "");
  const deleteMutation = useDeleteReadList();

  const [editOpen, setEditOpen] = useState(false);
  const [deleteOpen, setDeleteOpen] = useState(false);

  const members: Book[] = books ?? [];
  const reorderable = canWrite && Boolean(readList?.ordered);

  const move = (index: number, delta: number) => {
    const target = index + delta;
    if (target < 0 || target >= members.length) return;
    const ids = members.map((b) => b.id);
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
            <Text c="dimmed">This read list has no books yet.</Text>
            {canWrite && (
              <Text c="dimmed" size="sm">
                Open a book and use "Add to read list".
              </Text>
            )}
          </Stack>
        </Center>
      ) : (
        <SimpleGrid cols={{ base: 2, sm: 3, md: 4, lg: 6 }} spacing="md">
          {members.map((b, index) => (
            <Stack key={b.id} gap={4}>
              <MediaCard type="book" data={b} />
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
                  <Tooltip label="Remove from read list">
                    <ActionIcon
                      variant="subtle"
                      color="red"
                      size="sm"
                      loading={
                        removeMutation.isPending &&
                        removeMutation.variables === b.id
                      }
                      onClick={() => removeMutation.mutate(b.id)}
                      aria-label="Remove from read list"
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
