import {
  ActionIcon,
  Alert,
  Button,
  Group,
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
import { ManagePresetsModal } from "@/components/library/ManagePresetsModal";
import type { BookCondition, SeriesCondition } from "@/types/filters";

interface PresetsMenuProps {
  target: FilterPresetTarget;
  current: {
    query: string;
    sort: string;
    condition?: SeriesCondition | BookCondition;
  };
  onApply: (preset: FilterPresetDto) => void;
}

/**
 * Compact dropdown variant of the search-page preset controls. Replaces the
 * old sidebar so the page recovers its full width for results. Same backend
 * surface (`scope='search'`), same save / manage modals as the list pages.
 */
export function PresetsMenu({ target, current, onApply }: PresetsMenuProps) {
  const qc = useQueryClient();

  const { data: presets } = useQuery({
    queryKey: ["filter-presets", "search", target],
    queryFn: () => filterPresetsApi.list({ scope: "search", target }),
    staleTime: 30_000,
  });

  const [saveOpened, saveHandlers] = useDisclosure(false);
  const [manageOpened, manageHandlers] = useDisclosure(false);
  const [presetName, setPresetName] = useState("");

  const createMutation = useMutation({
    mutationFn: () => {
      if (!current.condition && !current.query.trim() && !current.sort) {
        throw new Error(
          "Nothing to save yet — add a filter, search query, or sort first.",
        );
      }
      return filterPresetsApi.create({
        name: presetName.trim(),
        scope: "search",
        target,
        condition: current.condition ?? ({ allOf: [] } as SeriesCondition),
        query: current.query.trim() || null,
        sort: current.sort || null,
      });
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["filter-presets", "search", target] });
      notifications.show({
        message: `Saved preset "${presetName}"`,
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
      qc.invalidateQueries({ queryKey: ["filter-presets", "search", target] });
    },
  });

  const canSave =
    current.condition !== undefined ||
    current.query.trim().length > 0 ||
    current.sort.length > 0;

  const hasPresets = (presets?.length ?? 0) > 0;

  return (
    <Group gap={4} wrap="nowrap">
      <Menu shadow="md" width={280} position="bottom-end" withinPortal>
        <Menu.Target>
          <Button
            size="xs"
            variant="default"
            leftSection={<IconBookmark size={14} />}
            rightSection={<IconChevronDown size={12} />}
          >
            Presets
          </Button>
        </Menu.Target>
        <Menu.Dropdown>
          {hasPresets ? (
            <>
              <Menu.Label>Apply preset</Menu.Label>
              {presets?.map((preset) => (
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
                    <Text size="xs" c="dimmed" lineClamp={1}>
                      {summarize(preset)}
                    </Text>
                  </Stack>
                </Menu.Item>
              ))}
            </>
          ) : (
            <Menu.Label>No saved presets yet</Menu.Label>
          )}
        </Menu.Dropdown>
      </Menu>

      <Tooltip
        label={
          canSave
            ? "Save current filters as a preset"
            : "Add a filter, query, or sort first"
        }
      >
        <ActionIcon
          variant="light"
          size="lg"
          onClick={saveHandlers.open}
          disabled={!canSave}
          aria-label="Save preset"
        >
          <IconBookmarkPlus size={16} />
        </ActionIcon>
      </Tooltip>
      <Tooltip label="Manage presets">
        <ActionIcon
          variant="subtle"
          size="lg"
          onClick={manageHandlers.open}
          disabled={!hasPresets}
          aria-label="Manage presets"
        >
          <IconSettings size={16} />
        </ActionIcon>
      </Tooltip>

      <ManagePresetsModal
        opened={manageOpened}
        onClose={manageHandlers.close}
        target={target}
      />

      <Modal
        opened={saveOpened}
        onClose={saveHandlers.close}
        title="Save preset"
        size="sm"
      >
        <Stack gap="md">
          <TextInput
            label="Preset name"
            value={presetName}
            onChange={(e) => setPresetName(e.currentTarget.value)}
            placeholder="My favourite manga search"
            autoFocus
            data-autofocus
          />
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
              disabled={presetName.trim().length === 0}
            >
              Save
            </Button>
          </Group>
        </Stack>
      </Modal>
    </Group>
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
