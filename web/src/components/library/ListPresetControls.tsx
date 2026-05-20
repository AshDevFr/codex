import {
  ActionIcon,
  Alert,
  Box,
  Button,
  Group,
  Loader,
  Menu,
  Modal,
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

  const { data: presets, isLoading } = useQuery({
    queryKey: ["filter-presets", "list", target, libraryId ?? "global"],
    queryFn: () => filterPresetsApi.list({ scope: "list", target }),
    staleTime: 30_000,
  });

  // Library-scoped presets show only when their library matches; global ones
  // (library_id null) show on every page. Sort: global first, then alpha.
  const visiblePresets = (presets ?? [])
    .filter((p) => p.libraryId === null || p.libraryId === libraryId)
    .sort((a, b) => {
      if (a.libraryId === null && b.libraryId !== null) return -1;
      if (b.libraryId === null && a.libraryId !== null) return 1;
      return a.name.localeCompare(b.name);
    });

  const createMutation = useMutation({
    mutationFn: () => {
      if (!currentCondition) {
        throw new Error("Add at least one filter chip before saving a preset.");
      }
      const targetLibraryId =
        scopeChoice === "library" ? (libraryId ?? null) : null;
      return filterPresetsApi.create({
        name: presetName.trim(),
        scope: "list",
        target,
        condition: currentCondition,
        libraryId: targetLibraryId,
      });
    },
    onSuccess: () => {
      qc.invalidateQueries({
        queryKey: ["filter-presets", "list", target],
      });
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

  const deleteMutation = useMutation({
    mutationFn: (id: string) => filterPresetsApi.delete(id),
    onSuccess: () => {
      qc.invalidateQueries({
        queryKey: ["filter-presets", "list", target],
      });
    },
  });

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
                  {preset.libraryId === null && (
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
              onClick={() => createMutation.mutate()}
              loading={createMutation.isPending}
              disabled={presetName.trim().length === 0 || !canSave}
            >
              Save
            </Button>
          </Group>
        </Stack>
      </Modal>

      <ManagePresetsModal
        opened={manageOpened}
        onClose={manageHandlers.close}
        target={target}
      />
    </Box>
  );
}
