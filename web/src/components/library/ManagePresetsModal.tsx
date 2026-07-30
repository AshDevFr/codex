import {
  ActionIcon,
  Badge,
  Box,
  Button,
  Card,
  Collapse,
  Divider,
  Group,
  Loader,
  Modal,
  Stack,
  Tabs,
  Text,
  TextInput,
  Tooltip,
} from "@mantine/core";
import { notifications } from "@mantine/notifications";
import {
  IconCheck,
  IconChevronDown,
  IconChevronRight,
  IconLayoutGrid,
  IconPencil,
  IconTrash,
} from "@tabler/icons-react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useMemo, useState } from "react";
import {
  type FilterPresetDto,
  type FilterPresetTarget,
  filterPresetsApi,
} from "@/api/filterPresets";
import { librariesApi } from "@/api/libraries";
import { CollectionFormModal } from "@/components/collections/CollectionFormModal";
import { usePermissions } from "@/hooks/usePermissions";
import type { SeriesCondition } from "@/types/filters";
import { PERMISSIONS } from "@/types/permissions";
import { PresetConditionSummary } from "./PresetConditionSummary";

export interface ManagePresetsModalProps {
  opened: boolean;
  onClose: () => void;
  /** Filter the initial tab to this target; both tabs remain visible. */
  target?: FilterPresetTarget;
}

/**
 * Shared management modal for filter presets.
 *
 * Used from both the library list pages (chip-style filter panels) and the
 * advanced /search page. Lists this user's presets grouped by scope, with
 * rename + delete actions. The condition shape is opaque here — applying a
 * preset is the caller's responsibility (the list pages and SearchPage each
 * own their own apply logic).
 */
export function ManagePresetsModal({
  opened,
  onClose,
  target,
}: ManagePresetsModalProps) {
  const [activeTarget, setActiveTarget] = useState<FilterPresetTarget>(
    target ?? "series",
  );

  return (
    <Modal
      opened={opened}
      onClose={onClose}
      title="Manage filter presets"
      size="lg"
    >
      <Tabs
        value={activeTarget}
        onChange={(v) => {
          if (v === "series" || v === "books") setActiveTarget(v);
        }}
      >
        <Tabs.List>
          <Tabs.Tab value="series">Series</Tabs.Tab>
          <Tabs.Tab value="books">Books</Tabs.Tab>
        </Tabs.List>

        <Tabs.Panel value="series" pt="sm">
          <PresetsList target="series" />
        </Tabs.Panel>
        <Tabs.Panel value="books" pt="sm">
          <PresetsList target="books" />
        </Tabs.Panel>
      </Tabs>
    </Modal>
  );
}

function PresetsList({ target }: { target: FilterPresetTarget }) {
  const qc = useQueryClient();
  const { hasPermission } = usePermissions();
  const canWriteCollections = hasPermission(PERMISSIONS.COLLECTIONS_WRITE);
  // The condition is *copied* into a new collection, not linked. A global
  // collection pointing at one user's private preset would break the moment the
  // preset was deleted, and nobody else could see why it held what it held.
  const [collectionSeed, setCollectionSeed] = useState<
    SeriesCondition | undefined
  >(undefined);
  const startCollectionFromPreset = (preset: FilterPresetDto) => {
    setCollectionSeed(preset.condition as unknown as SeriesCondition);
  };

  const { data: presets, isLoading } = useQuery({
    queryKey: ["filter-presets", "manage", target],
    queryFn: () => filterPresetsApi.list({ target }),
    staleTime: 15_000,
  });

  // Used to resolve library_id -> library name in the expanded preset view.
  const { data: libraries } = useQuery({
    queryKey: ["libraries"],
    queryFn: () => librariesApi.getAll(),
    staleTime: 60_000,
  });
  const libraryNameById = useMemo(() => {
    const map = new Map<string, string>();
    for (const lib of libraries ?? []) {
      map.set(lib.id, lib.name);
    }
    return map;
  }, [libraries]);

  if (isLoading) {
    return (
      <Group justify="center" py="md">
        <Loader size="sm" />
      </Group>
    );
  }
  if (!presets || presets.length === 0) {
    return (
      <Text size="sm" c="dimmed" ta="center" py="md">
        You haven't saved any {target} presets yet.
      </Text>
    );
  }

  const byScope = {
    list: presets.filter((p) => p.scope === "list"),
    search: presets.filter((p) => p.scope === "search"),
  };

  return (
    <Stack gap="md">
      {(["list", "search"] as const).map((scope) => (
        <Box key={scope}>
          <Group gap="xs" mb={6}>
            <Text size="sm" fw={600} tt="capitalize">
              {scope === "list" ? "List pages" : "Advanced search"}
            </Text>
            <Badge size="xs" variant="light">
              {byScope[scope].length}
            </Badge>
          </Group>
          {byScope[scope].length === 0 ? (
            <Text size="xs" c="dimmed">
              None.
            </Text>
          ) : (
            <Stack gap={6}>
              {byScope[scope].map((preset) => (
                <PresetRow
                  key={preset.id}
                  preset={preset}
                  libraryNameById={libraryNameById}
                  onChange={() =>
                    qc.invalidateQueries({
                      queryKey: ["filter-presets"],
                    })
                  }
                  onCreateCollection={
                    canWriteCollections && preset.target === "series"
                      ? startCollectionFromPreset
                      : undefined
                  }
                />
              ))}
            </Stack>
          )}
        </Box>
      ))}

      {/* Mounted outside the rows so it survives the list re-rendering when the
          preset queries refetch. */}
      <CollectionFormModal
        opened={collectionSeed !== undefined}
        onClose={() => setCollectionSeed(undefined)}
        initialCondition={collectionSeed}
      />
    </Stack>
  );
}

function PresetRow({
  preset,
  libraryNameById,
  onChange,
  onCreateCollection,
}: {
  preset: FilterPresetDto;
  libraryNameById: Map<string, string>;
  onChange: () => void;
  /** Offered only for series presets, and only with `collections:write`. */
  onCreateCollection?: (preset: FilterPresetDto) => void;
}) {
  const [renaming, setRenaming] = useState(false);
  const [expanded, setExpanded] = useState(false);
  const [draftName, setDraftName] = useState(preset.name);

  const renameMutation = useMutation({
    mutationFn: () =>
      filterPresetsApi.update(preset.id, {
        name: draftName.trim(),
        condition: preset.condition as never,
        query: preset.query ?? null,
        sort: preset.sort ?? null,
        libraryId: preset.libraryId ?? null,
      }),
    onSuccess: () => {
      notifications.show({
        message: `Renamed to "${draftName.trim()}"`,
        color: "green",
        icon: <IconCheck size={14} />,
      });
      setRenaming(false);
      onChange();
    },
    onError: (err) => {
      notifications.show({
        title: "Could not rename preset",
        message: (err as Error).message ?? "Unknown error",
        color: "red",
      });
    },
  });

  const deleteMutation = useMutation({
    mutationFn: () => filterPresetsApi.delete(preset.id),
    onSuccess: onChange,
  });

  const scopeLabel =
    preset.libraryId == null
      ? "Global"
      : (libraryNameById.get(preset.libraryId) ?? "Library");

  return (
    <Card withBorder p="xs" radius="sm">
      <Group justify="space-between" wrap="nowrap" align="center">
        <Group gap={4} wrap="nowrap" style={{ minWidth: 0, flex: 1 }}>
          {!renaming && (
            <ActionIcon
              variant="subtle"
              size="sm"
              onClick={() => setExpanded((e) => !e)}
              aria-label={
                expanded
                  ? `Collapse ${preset.name}`
                  : `Expand ${preset.name} details`
              }
            >
              {expanded ? (
                <IconChevronDown size={14} />
              ) : (
                <IconChevronRight size={14} />
              )}
            </ActionIcon>
          )}
          <Stack gap={2} style={{ minWidth: 0, flex: 1 }}>
            {renaming ? (
              <Group gap="xs">
                <TextInput
                  size="xs"
                  value={draftName}
                  onChange={(e) => setDraftName(e.currentTarget.value)}
                  style={{ flex: 1 }}
                  autoFocus
                  data-autofocus
                />
                <Button
                  size="compact-xs"
                  onClick={() => renameMutation.mutate()}
                  loading={renameMutation.isPending}
                  disabled={
                    draftName.trim().length === 0 ||
                    draftName.trim() === preset.name
                  }
                >
                  Save
                </Button>
                <Button
                  size="compact-xs"
                  variant="subtle"
                  onClick={() => {
                    setDraftName(preset.name);
                    setRenaming(false);
                  }}
                >
                  Cancel
                </Button>
              </Group>
            ) : (
              <>
                <Group gap={6} wrap="nowrap">
                  <Text size="sm" fw={500} truncate>
                    {preset.name}
                  </Text>
                  <Badge
                    size="xs"
                    variant="light"
                    color={preset.libraryId == null ? "blue" : "gray"}
                  >
                    {scopeLabel}
                  </Badge>
                </Group>
                <Text size="xs" c="dimmed" truncate>
                  {summarize(preset)}
                </Text>
              </>
            )}
          </Stack>
        </Group>
        {!renaming && (
          <Group gap={4} wrap="nowrap">
            {onCreateCollection && (
              <Tooltip label="Create an automatic collection from this preset">
                <ActionIcon
                  variant="subtle"
                  size="sm"
                  onClick={() => onCreateCollection(preset)}
                  aria-label={`Create a collection from ${preset.name}`}
                >
                  <IconLayoutGrid size={14} />
                </ActionIcon>
              </Tooltip>
            )}
            <Tooltip label="Rename">
              <ActionIcon
                variant="subtle"
                size="sm"
                onClick={() => setRenaming(true)}
                aria-label={`Rename ${preset.name}`}
              >
                <IconPencil size={14} />
              </ActionIcon>
            </Tooltip>
            <Tooltip label="Delete">
              <ActionIcon
                variant="subtle"
                color="red"
                size="sm"
                onClick={() => {
                  if (window.confirm(`Delete preset "${preset.name}"?`)) {
                    deleteMutation.mutate();
                  }
                }}
                aria-label={`Delete ${preset.name}`}
              >
                <IconTrash size={14} />
              </ActionIcon>
            </Tooltip>
          </Group>
        )}
      </Group>
      {!renaming && (
        <Collapse in={expanded}>
          <Divider my="xs" />
          <Box pl={26}>
            <PresetConditionSummary preset={preset} />
          </Box>
        </Collapse>
      )}
    </Card>
  );
}

function summarize(preset: FilterPresetDto): string {
  const bits: string[] = [];
  if (preset.query) bits.push(`"${preset.query}"`);
  if (preset.sort) bits.push(`sort: ${preset.sort}`);
  if (
    preset.condition &&
    typeof preset.condition === "object" &&
    Object.keys(preset.condition).length > 0
  ) {
    bits.push("filters");
  }
  return bits.join(" · ") || "(empty)";
}
