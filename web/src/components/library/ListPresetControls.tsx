import {
  ActionIcon,
  Alert,
  Badge,
  Box,
  Button,
  Card,
  Divider,
  Group,
  Loader,
  Menu,
  Modal,
  SimpleGrid,
  Stack,
  Text,
  TextInput,
  Tooltip,
} from "@mantine/core";
import { useDisclosure } from "@mantine/hooks";
import { notifications } from "@mantine/notifications";
import {
  IconBookmark,
  IconBookmarkPlus,
  IconCheck,
  IconChevronDown,
  IconSettings,
  IconTrash,
} from "@tabler/icons-react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import {
  type FilterPresetDto,
  type FilterPresetTarget,
  filterPresetsApi,
} from "@/api/filterPresets";
import type { BookCondition, SeriesCondition } from "@/types/filters";
import { ManagePresetsModal } from "./ManagePresetsModal";
import { PresetConditionSummary } from "./PresetConditionSummary";

export interface ListPresetControlsProps {
  target: FilterPresetTarget;
  /** UUID of the library being filtered, or null for "all libraries" / global. */
  libraryId: string | null;
  /** Current condition; undefined when no filters are active. */
  currentCondition: SeriesCondition | BookCondition | undefined;
  /** Whether the current draft has any active chips. */
  hasActiveFilters: boolean;
  /**
   * Called when a preset is selected. Receives the preset so the panel can
   * convert its condition back into chip state and apply it.
   */
  onApply: (preset: FilterPresetDto) => void;
}

/**
 * Save / load / manage controls for filter presets on the library list page.
 *
 * Mounted inside the filter drawer; lists `scope='list'` presets for the
 * active target. Library-scoped presets only show when the user is browsing
 * the same library; global presets (library_id = null) show everywhere.
 */
export function ListPresetControls({
  target,
  libraryId,
  currentCondition,
  hasActiveFilters,
  onApply,
}: ListPresetControlsProps) {
  const qc = useQueryClient();
  const [saveOpened, saveHandlers] = useDisclosure(false);
  const [manageOpened, manageHandlers] = useDisclosure(false);
  const [presetName, setPresetName] = useState("");
  const [scopeChoice, setScopeChoice] = useState<"library" | "global">(
    libraryId ? "library" : "global",
  );
  // Set when the user clicks Save and an existing preset in the same scope
  // and library already uses the requested name. Drives the Replace/Cancel
  // confirmation modal.
  const [duplicate, setDuplicate] = useState<FilterPresetDto | null>(null);

  const { data: presets, isLoading } = useQuery({
    queryKey: ["filter-presets", "list", target, libraryId ?? "global"],
    queryFn: () => filterPresetsApi.list({ scope: "list", target }),
    staleTime: 30_000,
  });

  // Library-scoped presets show only when their library matches; global ones
  // (library_id null) show on every page. Use loose equality so an absent
  // field (legacy responses) is treated the same as an explicit null.
  // Sort: global first, then alpha.
  const visiblePresets = (presets ?? [])
    .filter((p) => p.libraryId == null || p.libraryId === libraryId)
    .sort((a, b) => {
      const aGlobal = a.libraryId == null;
      const bGlobal = b.libraryId == null;
      if (aGlobal && !bGlobal) return -1;
      if (bGlobal && !aGlobal) return 1;
      return a.name.localeCompare(b.name);
    });

  const targetLibraryId =
    scopeChoice === "library" ? (libraryId ?? null) : null;

  const createMutation = useMutation({
    mutationFn: () => {
      if (!currentCondition) {
        throw new Error("Add at least one filter chip before saving a preset.");
      }
      return filterPresetsApi.create({
        name: presetName.trim(),
        scope: "list",
        target,
        condition: currentCondition,
        libraryId: targetLibraryId,
      });
    },
    onSuccess: () => {
      // Invalidate every filter-presets query (sidebar + Manage modal) so the
      // new preset shows up everywhere it might be listed.
      qc.invalidateQueries({ queryKey: ["filter-presets"] });
      notifications.show({
        message: `Saved preset "${presetName.trim()}"`,
        color: "green",
        icon: <IconCheck size={14} />,
      });
      setPresetName("");
      saveHandlers.close();
    },
    onError: (err) => {
      notifications.show({
        title: "Could not save preset",
        message: (err as Error).message ?? "Unknown error",
        color: "red",
      });
    },
  });

  const replaceMutation = useMutation({
    mutationFn: (existing: FilterPresetDto) => {
      if (!currentCondition) {
        throw new Error("Add at least one filter chip before saving a preset.");
      }
      return filterPresetsApi.update(existing.id, {
        name: existing.name,
        condition: currentCondition,
        query: existing.query ?? null,
        sort: existing.sort ?? null,
        libraryId: existing.libraryId ?? null,
      });
    },
    onSuccess: (_data, existing) => {
      qc.invalidateQueries({ queryKey: ["filter-presets"] });
      notifications.show({
        message: `Replaced preset "${existing.name}"`,
        color: "green",
        icon: <IconCheck size={14} />,
      });
      setPresetName("");
      setDuplicate(null);
      saveHandlers.close();
    },
    onError: (err) => {
      notifications.show({
        title: "Could not replace preset",
        message: (err as Error).message ?? "Unknown error",
        color: "red",
      });
    },
  });

  const deleteMutation = useMutation({
    mutationFn: (id: string) => filterPresetsApi.delete(id),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["filter-presets"] });
    },
  });

  const handleSaveClick = () => {
    const trimmed = presetName.trim();
    if (trimmed.length === 0 || !currentCondition) return;
    // Backend enforces uniqueness on (user, scope, target, library_id, name);
    // mirror that here so we can offer Replace instead of failing later.
    const existing = (presets ?? []).find(
      (p) =>
        p.scope === "list" &&
        p.target === target &&
        (p.libraryId ?? null) === targetLibraryId &&
        p.name === trimmed,
    );
    if (existing) {
      setDuplicate(existing);
      return;
    }
    createMutation.mutate();
  };

  const canSave = hasActiveFilters && currentCondition !== undefined;
  const isAllLibraries = libraryId === null || libraryId === "all";

  return (
    <Box>
      <Group justify="space-between" align="center" mb="xs">
        <Group gap={6}>
          <IconBookmark size={14} />
          <Text size="sm" fw={600}>
            Presets
          </Text>
        </Group>
        <Group gap={4}>
          <Tooltip
            label={
              canSave
                ? "Save current filters as a preset"
                : "Add at least one chip first"
            }
          >
            <ActionIcon
              variant="light"
              size="sm"
              onClick={saveHandlers.open}
              disabled={!canSave}
              aria-label="Save preset"
            >
              <IconBookmarkPlus size={14} />
            </ActionIcon>
          </Tooltip>
          <Tooltip label="Manage presets">
            <ActionIcon
              variant="subtle"
              size="sm"
              onClick={manageHandlers.open}
              disabled={(presets?.length ?? 0) === 0}
              aria-label="Manage presets"
            >
              <IconSettings size={14} />
            </ActionIcon>
          </Tooltip>
        </Group>
      </Group>

      {isLoading && <Loader size="xs" />}

      {!isLoading && visiblePresets.length === 0 && (
        <Text size="xs" c="dimmed">
          No saved presets for this page yet.
        </Text>
      )}

      {visiblePresets.length > 0 && (
        <Menu shadow="md" width={260} position="bottom-start" withinPortal>
          <Menu.Target>
            <Button
              variant="default"
              size="xs"
              rightSection={<IconChevronDown size={12} />}
              fullWidth
              justify="space-between"
            >
              Apply preset…
            </Button>
          </Menu.Target>
          <Menu.Dropdown>
            <Menu.Label>This user's presets</Menu.Label>
            {visiblePresets.map((preset) => (
              <Menu.Item
                key={preset.id}
                onClick={() => onApply(preset)}
                rightSection={
                  <ActionIcon
                    component="span"
                    variant="subtle"
                    color="red"
                    size="sm"
                    aria-label={`Delete preset ${preset.name}`}
                    onClick={(e) => {
                      e.stopPropagation();
                      if (window.confirm(`Delete preset "${preset.name}"?`)) {
                        deleteMutation.mutate(preset.id);
                      }
                    }}
                  >
                    <IconTrash size={12} />
                  </ActionIcon>
                }
              >
                <Stack gap={0}>
                  <Text size="sm">{preset.name}</Text>
                  {preset.libraryId == null && (
                    <Text size="xs" c="dimmed">
                      Global
                    </Text>
                  )}
                </Stack>
              </Menu.Item>
            ))}
          </Menu.Dropdown>
        </Menu>
      )}

      <Modal
        opened={saveOpened}
        onClose={saveHandlers.close}
        title="Save filter preset"
        size="sm"
      >
        <Stack gap="md">
          <TextInput
            label="Preset name"
            value={presetName}
            onChange={(e) => setPresetName(e.currentTarget.value)}
            placeholder="My favourite books filter"
            autoFocus
            data-autofocus
          />
          {!isAllLibraries && (
            <Stack gap={4}>
              <Text size="sm" fw={500}>
                Availability
              </Text>
              <Group gap="xs">
                <Button
                  size="xs"
                  variant={scopeChoice === "library" ? "filled" : "default"}
                  onClick={() => setScopeChoice("library")}
                >
                  This library
                </Button>
                <Button
                  size="xs"
                  variant={scopeChoice === "global" ? "filled" : "default"}
                  onClick={() => setScopeChoice("global")}
                >
                  All libraries
                </Button>
              </Group>
              <Text size="xs" c="dimmed">
                Global presets appear on every library's filter panel.
              </Text>
            </Stack>
          )}
          {createMutation.error && (
            <Alert color="red">{(createMutation.error as Error).message}</Alert>
          )}
          <Group justify="flex-end">
            <Button variant="subtle" onClick={saveHandlers.close}>
              Cancel
            </Button>
            <Button
              onClick={handleSaveClick}
              loading={createMutation.isPending}
              disabled={presetName.trim().length === 0 || !canSave}
            >
              Save
            </Button>
          </Group>
        </Stack>
      </Modal>

      <DuplicatePresetModal
        existing={duplicate}
        target={target}
        targetLibraryId={targetLibraryId}
        newCondition={currentCondition}
        onCancel={() => setDuplicate(null)}
        onReplace={() => {
          if (duplicate) replaceMutation.mutate(duplicate);
        }}
        replacePending={replaceMutation.isPending}
      />

      <ManagePresetsModal
        opened={manageOpened}
        onClose={manageHandlers.close}
        target={target}
      />
    </Box>
  );
}

interface DuplicatePresetModalProps {
  existing: FilterPresetDto | null;
  target: FilterPresetTarget;
  targetLibraryId: string | null;
  newCondition: SeriesCondition | BookCondition | undefined;
  onCancel: () => void;
  onReplace: () => void;
  replacePending: boolean;
}

/**
 * Confirmation modal shown when the user tries to save a preset whose name
 * already exists in the same scope/library. Renders both the existing and
 * the new preset's filter summary so the user can compare before deciding
 * to Replace or Cancel.
 */
function DuplicatePresetModal({
  existing,
  target,
  targetLibraryId,
  newCondition,
  onCancel,
  onReplace,
  replacePending,
}: DuplicatePresetModalProps) {
  if (!existing || !newCondition) {
    return (
      <Modal opened={existing !== null} onClose={onCancel} size="lg" title="" />
    );
  }
  // Synthetic dto-like wrapper so PresetConditionSummary can render the
  // in-progress draft the same way it renders saved presets.
  const draftPreview: FilterPresetDto = {
    id: "__draft__",
    name: existing.name,
    scope: "list",
    target,
    condition: newCondition as unknown as Record<string, never>,
    query: null,
    sort: null,
    libraryId: targetLibraryId,
    createdAt: new Date().toISOString(),
    updatedAt: new Date().toISOString(),
  };
  return (
    <Modal
      opened={existing !== null}
      onClose={onCancel}
      title={`Replace preset "${existing.name}"?`}
      size="xl"
    >
      <Stack gap="md">
        <Text size="sm" c="dimmed">
          A preset with this name already exists in the same scope. Replace its
          filters with your current selection?
        </Text>
        <SimpleGrid cols={{ base: 1, sm: 2 }} spacing="md">
          <Card withBorder p="sm" radius="sm">
            <Stack gap={6}>
              <Group justify="space-between" wrap="nowrap">
                <Text size="sm" fw={600}>
                  Existing
                </Text>
                <Badge size="xs" variant="light" color="gray">
                  {existing.libraryId == null ? "Global" : "This library"}
                </Badge>
              </Group>
              <Divider />
              <PresetConditionSummary preset={existing} />
            </Stack>
          </Card>
          <Card withBorder p="sm" radius="sm">
            <Stack gap={6}>
              <Group justify="space-between" wrap="nowrap">
                <Text size="sm" fw={600}>
                  New
                </Text>
                <Badge size="xs" variant="light" color="blue">
                  {targetLibraryId == null ? "Global" : "This library"}
                </Badge>
              </Group>
              <Divider />
              <PresetConditionSummary preset={draftPreview} />
            </Stack>
          </Card>
        </SimpleGrid>
        <Group justify="flex-end">
          <Button variant="subtle" onClick={onCancel}>
            Cancel
          </Button>
          <Button color="red" onClick={onReplace} loading={replacePending}>
            Replace
          </Button>
        </Group>
      </Stack>
    </Modal>
  );
}
