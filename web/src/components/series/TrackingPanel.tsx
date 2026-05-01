import {
  ActionIcon,
  Badge,
  Box,
  Button,
  Card,
  Divider,
  Group,
  NumberInput,
  Select,
  Stack,
  Switch,
  Text,
  TextInput,
  Tooltip,
} from "@mantine/core";
import { IconBellRinging, IconPlus, IconTrash } from "@tabler/icons-react";
import { type FormEvent, useState } from "react";
import {
  useCreateSeriesAlias,
  useDeleteSeriesAlias,
  useSeriesAliases,
  useSeriesTracking,
  useUpdateSeriesTracking,
} from "@/hooks/useSeriesTracking";

interface TrackingPanelProps {
  seriesId: string;
  /** When false, shows read-only state (used for users without SeriesWrite). */
  canEdit: boolean;
}

const STATUS_OPTIONS = [
  { value: "unknown", label: "Unknown" },
  { value: "ongoing", label: "Ongoing" },
  { value: "complete", label: "Complete" },
  { value: "hiatus", label: "Hiatus" },
  { value: "cancelled", label: "Cancelled" },
];

/**
 * Inline panel on the series detail page for release-tracking config.
 *
 * Shows: tracked toggle, status, chapter/volume tracking flags, latest known
 * chapter/volume, and the aliases list. All mutations debounce-free — the
 * surface is small enough that immediate fire-on-blur is fine.
 */
export function TrackingPanel({ seriesId, canEdit }: TrackingPanelProps) {
  const trackingQuery = useSeriesTracking(seriesId);
  const aliasesQuery = useSeriesAliases(seriesId);
  const updateTracking = useUpdateSeriesTracking(seriesId);
  const createAlias = useCreateSeriesAlias(seriesId);
  const deleteAlias = useDeleteSeriesAlias(seriesId);

  const [aliasDraft, setAliasDraft] = useState("");

  const tracking = trackingQuery.data;
  const aliases = aliasesQuery.data ?? [];

  const handleAddAlias = async (e: FormEvent) => {
    e.preventDefault();
    const trimmed = aliasDraft.trim();
    if (!trimmed) return;
    try {
      await createAlias.mutateAsync({ alias: trimmed });
      setAliasDraft("");
    } catch {
      // Notification surfaced inside the hook.
    }
  };

  return (
    <Card withBorder padding="md" radius="md">
      <Stack gap="sm">
        <Group justify="space-between" wrap="nowrap">
          <Group gap="xs">
            <IconBellRinging size={18} />
            <Text fw={600}>Release tracking</Text>
            {tracking?.tracked && (
              <Badge color="green" variant="light" size="sm">
                Tracking
              </Badge>
            )}
          </Group>
          <Switch
            checked={tracking?.tracked ?? false}
            onChange={(event) =>
              updateTracking.mutate({ tracked: event.currentTarget.checked })
            }
            disabled={!canEdit || trackingQuery.isLoading}
            aria-label="Toggle release tracking"
          />
        </Group>

        {tracking?.tracked && (
          <>
            <Group grow align="flex-start">
              <Select
                label="Status"
                value={tracking.trackingStatus}
                onChange={(value) => {
                  if (value) updateTracking.mutate({ trackingStatus: value });
                }}
                data={STATUS_OPTIONS}
                disabled={!canEdit}
              />
              <Stack gap={4}>
                <Text size="sm" fw={500}>
                  Announce
                </Text>
                <Group gap="md">
                  <Switch
                    label="Chapters"
                    checked={tracking.trackChapters}
                    onChange={(e) =>
                      updateTracking.mutate({
                        trackChapters: e.currentTarget.checked,
                      })
                    }
                    disabled={!canEdit}
                  />
                  <Switch
                    label="Volumes"
                    checked={tracking.trackVolumes}
                    onChange={(e) =>
                      updateTracking.mutate({
                        trackVolumes: e.currentTarget.checked,
                      })
                    }
                    disabled={!canEdit}
                  />
                </Group>
              </Stack>
            </Group>

            <Group grow>
              <NumberInput
                label="Latest known chapter"
                placeholder="—"
                value={tracking.latestKnownChapter ?? ""}
                onChange={(value) => {
                  const next =
                    typeof value === "number" && Number.isFinite(value)
                      ? value
                      : null;
                  updateTracking.mutate({ latestKnownChapter: next });
                }}
                allowDecimal
                decimalScale={2}
                step={0.1}
                disabled={!canEdit}
              />
              <NumberInput
                label="Latest known volume"
                placeholder="—"
                value={tracking.latestKnownVolume ?? ""}
                onChange={(value) => {
                  const next =
                    typeof value === "number" &&
                    Number.isFinite(value) &&
                    Number.isInteger(value)
                      ? value
                      : null;
                  updateTracking.mutate({ latestKnownVolume: next });
                }}
                allowDecimal={false}
                step={1}
                disabled={!canEdit}
              />
            </Group>
          </>
        )}

        <Divider my="xs" />

        <Box>
          <Group justify="space-between" mb="xs">
            <Text size="sm" fw={500}>
              Matcher aliases
            </Text>
            <Text size="xs" c="dimmed">
              {aliases.length} alias{aliases.length === 1 ? "" : "es"}
            </Text>
          </Group>
          <Text size="xs" c="dimmed" mb="xs">
            Used by sources that match by title (Nyaa, MangaUpdates without an
            ID).
          </Text>

          {aliases.length === 0 && (
            <Text size="sm" c="dimmed" fs="italic" mb="xs">
              No aliases yet. Add one below or run the metadata backfill task.
            </Text>
          )}

          <Stack gap={4} mb="xs">
            {aliases.map((alias) => (
              <Group
                key={alias.id}
                justify="space-between"
                wrap="nowrap"
                gap="xs"
              >
                <Group gap="xs" wrap="nowrap" style={{ minWidth: 0, flex: 1 }}>
                  <Text size="sm" truncate>
                    {alias.alias}
                  </Text>
                  <Badge
                    color={alias.source === "manual" ? "violet" : "gray"}
                    variant="light"
                    size="xs"
                  >
                    {alias.source}
                  </Badge>
                </Group>
                {canEdit && (
                  <Tooltip label="Remove alias">
                    <ActionIcon
                      size="sm"
                      color="red"
                      variant="subtle"
                      onClick={() => deleteAlias.mutate(alias.id)}
                      loading={
                        deleteAlias.isPending &&
                        deleteAlias.variables === alias.id
                      }
                      aria-label={`Remove alias ${alias.alias}`}
                    >
                      <IconTrash size={14} />
                    </ActionIcon>
                  </Tooltip>
                )}
              </Group>
            ))}
          </Stack>

          {canEdit && (
            <form onSubmit={handleAddAlias}>
              <Group gap="xs" align="flex-end">
                <TextInput
                  placeholder="Add an alias…"
                  value={aliasDraft}
                  onChange={(e) => setAliasDraft(e.currentTarget.value)}
                  style={{ flex: 1 }}
                  disabled={createAlias.isPending}
                />
                <Button
                  type="submit"
                  size="sm"
                  leftSection={<IconPlus size={14} />}
                  loading={createAlias.isPending}
                  disabled={!aliasDraft.trim()}
                >
                  Add
                </Button>
              </Group>
            </form>
          )}
        </Box>
      </Stack>
    </Card>
  );
}
