import {
  ActionIcon,
  Alert,
  Badge,
  Button,
  Card,
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
  IconInfoCircle,
  IconLock,
  IconLockOpen,
  IconSortAscending,
  IconSortDescending,
  IconTrash,
  IconUser,
  IconWand,
} from "@tabler/icons-react";
import { useState } from "react";
import { useNavigate, useParams } from "react-router-dom";
import type { CollectionSeriesSort, SortDirection } from "@/api/collections";
import { CollectionFormModal } from "@/components/collections/CollectionFormModal";
import { MediaGrid, type MediaGridItem } from "@/components/library/MediaGrid";
import { ConditionSummary } from "@/components/library/PresetConditionSummary";
import {
  useCollection,
  useCollectionSeries,
  useDeleteCollection,
  useRemoveSeriesFromCollection,
  useReorderCollection,
} from "@/hooks/useCollections";
import { usePermissions } from "@/hooks/usePermissions";
import { useListSortPreferencesStore } from "@/store/listSortPreferencesStore";
import type { Series } from "@/types";
import type { SeriesCondition } from "@/types/filters";
import { PERMISSIONS } from "@/types/permissions";
import { describesPersonalData } from "@/utils/collectionRules";

export function CollectionDetail() {
  const { collectionId } = useParams<{ collectionId: string }>();
  const navigate = useNavigate();
  const { hasPermission } = usePermissions();
  const canWrite = hasPermission(PERMISSIONS.COLLECTIONS_WRITE);
  const canDelete = hasPermission(PERMISSIONS.COLLECTIONS_DELETE);

  // The per-list choice persists in localStorage; "no explicit choice" sends
  // no sort param and the server applies the collection's default (manual
  // when `ordered`, title otherwise).
  const stored = useListSortPreferencesStore(
    (state) => state.collections[collectionId ?? ""],
  );
  const setCollectionSort = useListSortPreferencesStore(
    (state) => state.setCollectionSort,
  );
  const sortOverride = stored?.sort ?? null;
  const direction: SortDirection = stored?.direction ?? "asc";
  const setSortOverride = (sort: CollectionSeriesSort) =>
    setCollectionSort(collectionId ?? "", { sort });
  const setDirection = (direction: SortDirection) =>
    setCollectionSort(collectionId ?? "", { direction });
  const { data: collection, isLoading } = useCollection(collectionId);
  const { data: series } = useCollectionSeries(
    collectionId,
    sortOverride ?? undefined,
    direction === "desc" ? "desc" : undefined,
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
  const isAutomatic = collection?.automatic ?? false;
  const rule = collection?.condition as SeriesCondition | null | undefined;
  const personalized = isAutomatic && describesPersonalData(rule);
  // Hand-editing an automatic collection is refused by the API (409), so none
  // of the affordances for it are offered.
  const canEditMembers = canWrite && !isAutomatic;
  // Dragging edits the shared manual order, so it is only offered in the
  // Manual view (any collection maintains positions, ordered or not).
  const canReorder = canEditMembers && sort === "manual";
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
      <Group justify="space-between" align="flex-start" mb="lg">
        <Stack gap={4} style={{ minWidth: 0 }}>
          <Group gap="sm" align="center">
            <Title order={2} style={{ wordBreak: "break-word" }}>
              {collection.name}
            </Title>
            {/* An automatic collection has no seriesCount from the API, but the
                member list is already loaded here, so the count is free. */}
            <Badge variant="light">
              {(collection.seriesCount ?? members.length) === 1
                ? "1 series"
                : `${collection.seriesCount ?? members.length} series`}
            </Badge>
            {isAutomatic && (
              <Tooltip label="Members come from a rule and stay up to date automatically">
                <Badge variant="light" leftSection={<IconWand size={11} />}>
                  Automatic
                </Badge>
              </Tooltip>
            )}
            {personalized && (
              <Tooltip label="This rule uses personal ratings or reading progress">
                <Badge
                  variant="light"
                  color="yellow"
                  leftSection={<IconUser size={11} />}
                >
                  Personal
                </Badge>
              </Tooltip>
            )}
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
        <Group gap="xs">
          {members.length > 1 && (
            <SegmentedControl
              value={sort}
              onChange={(value) =>
                setSortOverride(value as CollectionSeriesSort)
              }
              data={[
                { label: "Title", value: "title" },
                { label: "Date added", value: "added" },
                // Series only carry a release year; label matches the read
                // list page so the two selectors read as one control.
                { label: "Release", value: "year" },
                // Manual order needs someone to have arranged it, which never
                // happens for a rule-backed collection.
                ...(isAutomatic ? [] : [{ label: "Manual", value: "manual" }]),
              ]}
              aria-label="Sort series"
            />
          )}
          {sort !== "manual" && members.length > 1 && (
            <Tooltip label={direction === "asc" ? "Ascending" : "Descending"}>
              <ActionIcon
                variant="default"
                size="lg"
                onClick={() =>
                  setDirection(direction === "asc" ? "desc" : "asc")
                }
                aria-label={
                  direction === "asc"
                    ? "Sort ascending (click for descending)"
                    : "Sort descending (click for ascending)"
                }
              >
                {direction === "asc" ? (
                  <IconSortAscending size={16} />
                ) : (
                  <IconSortDescending size={16} />
                )}
              </ActionIcon>
            </Tooltip>
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

      {isAutomatic && rule && (
        <Card withBorder radius="md" p="sm" mb="md">
          <Stack gap="xs">
            <Group gap={6}>
              <IconWand size={14} />
              <Text size="sm" fw={600}>
                Membership rule
              </Text>
            </Group>
            <ConditionSummary condition={rule} />
            {personalized && (
              <Alert
                variant="light"
                color="yellow"
                icon={<IconInfoCircle size={16} />}
                p="xs"
              >
                This rule uses your own ratings or reading progress, so other
                people see a different set of series here.
              </Alert>
            )}
            <Text size="xs" c="dimmed">
              Series matching this rule belong automatically. To change what is
              here, edit the rule or correct the series' metadata.
            </Text>
          </Stack>
        </Card>
      )}

      {members.length === 0 ? (
        <Center mih={200}>
          <Stack align="center" gap="xs">
            <Text c="dimmed">
              {isAutomatic
                ? "No series match this rule yet."
                : "This collection has no series yet."}
            </Text>
            {canWrite && (
              <Text c="dimmed" size="sm">
                {isAutomatic
                  ? "Edit the rule to widen it, or tag some series so they match."
                  : 'Open a series and use "Add to collection".'}
              </Text>
            )}
          </Stack>
        </Center>
      ) : (
        <MediaGrid
          items={items}
          onRemove={
            canEditMembers
              ? (item) => removeMutation.mutate(item.id)
              : undefined
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
