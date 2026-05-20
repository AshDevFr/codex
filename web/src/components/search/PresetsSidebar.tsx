import {
  ActionIcon,
  Alert,
  Button,
  Card,
  Group,
  Loader,
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
  IconCheck,
  IconDeviceFloppy,
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

interface PresetsSidebarProps {
  target: FilterPresetTarget;
  current: {
    query: string;
    sort: string;
    condition?: SeriesCondition | BookCondition;
  };
  onApply: (preset: FilterPresetDto) => void;
}

export function PresetsSidebar({
  target,
  current,
  onApply,
}: PresetsSidebarProps) {
  const qc = useQueryClient();

  const { data: presets, isLoading } = useQuery({
    queryKey: ["filter-presets", "search", target],
    queryFn: () => filterPresetsApi.list({ scope: "search", target }),
    staleTime: 30_000,
  });

  const [saveOpened, saveHandlers] = useDisclosure(false);
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

  return (
    <Stack gap="sm">
      <Group justify="space-between" align="center">
        <Group gap="xs">
          <IconBookmark size={16} />
          <Text size="sm" fw={600}>
            Presets
          </Text>
        </Group>
        <Tooltip
          label={
            canSave
              ? "Save current filters as a preset"
              : "Add a filter, query, or sort first"
          }
        >
          <ActionIcon
            variant="light"
            size="sm"
            onClick={saveHandlers.open}
            disabled={!canSave}
            aria-label="Save preset"
          >
            <IconDeviceFloppy size={14} />
          </ActionIcon>
        </Tooltip>
      </Group>

      {isLoading && <Loader size="sm" />}

      {!isLoading && (presets?.length ?? 0) === 0 && (
        <Text size="xs" c="dimmed">
          No saved presets yet. Build a filter and click the save button.
        </Text>
      )}

      <Stack gap={4}>
        {presets?.map((preset) => (
          <Card
            key={preset.id}
            withBorder
            p="xs"
            radius="sm"
            style={{ cursor: "pointer" }}
            onClick={() => onApply(preset)}
          >
            <Group justify="space-between" wrap="nowrap" align="flex-start">
              <Stack gap={2} style={{ minWidth: 0 }}>
                <Text size="sm" fw={500} truncate>
                  {preset.name}
                </Text>
                <Text size="xs" c="dimmed" truncate>
                  {summarize(preset)}
                </Text>
              </Stack>
              <Tooltip label="Delete">
                <ActionIcon
                  variant="subtle"
                  color="red"
                  size="sm"
                  onClick={(e) => {
                    e.stopPropagation();
                    if (window.confirm(`Delete preset "${preset.name}"?`)) {
                      deleteMutation.mutate(preset.id);
                    }
                  }}
                  aria-label={`Delete preset ${preset.name}`}
                >
                  <IconTrash size={14} />
                </ActionIcon>
              </Tooltip>
            </Group>
          </Card>
        ))}
      </Stack>

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
    </Stack>
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
