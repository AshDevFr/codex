import {
  ActionIcon,
  Badge,
  Box,
  Button,
  Card,
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
import { IconCheck, IconPencil, IconTrash } from "@tabler/icons-react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import {
  type FilterPresetDto,
  type FilterPresetTarget,
  filterPresetsApi,
} from "@/api/filterPresets";

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
  const { data: presets, isLoading } = useQuery({
    queryKey: ["filter-presets", "manage", target],
    queryFn: () => filterPresetsApi.list({ target }),
    staleTime: 15_000,
  });

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
                  onChange={() =>
                    qc.invalidateQueries({
                      queryKey: ["filter-presets"],
                    })
                  }
                />
              ))}
            </Stack>
          )}
        </Box>
      ))}
    </Stack>
  );
}

function PresetRow({
  preset,
  onChange,
}: {
  preset: FilterPresetDto;
  onChange: () => void;
}) {
  const [renaming, setRenaming] = useState(false);
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

  return (
    <Card withBorder p="xs" radius="sm">
      <Group justify="space-between" wrap="nowrap" align="center">
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
              <Text size="sm" fw={500} truncate>
                {preset.name}
              </Text>
              <Text size="xs" c="dimmed" truncate>
                {summarize(preset)}
              </Text>
            </>
          )}
        </Stack>
        {!renaming && (
          <Group gap={4} wrap="nowrap">
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
    </Card>
  );
}

function summarize(preset: FilterPresetDto): string {
  const bits: string[] = [];
  if (preset.libraryId === null) bits.push("Global");
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
