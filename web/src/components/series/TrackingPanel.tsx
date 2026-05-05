import {
  ActionIcon,
  Badge,
  Box,
  Button,
  Card,
  Collapse,
  Divider,
  Group,
  NumberInput,
  Stack,
  Switch,
  Text,
  TextInput,
  Tooltip,
  UnstyledButton,
} from "@mantine/core";
import {
  IconBellRinging,
  IconChevronDown,
  IconChevronRight,
  IconPlus,
  IconTrash,
} from "@tabler/icons-react";
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
  // Default collapsed so the panel is a thin one-liner unless the user
  // explicitly wants to fiddle. The summary in the header carries the
  // load-bearing info (tracking on/off, last-known marks, alias count).
  const [expanded, setExpanded] = useState(false);

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

  // Build a compact one-line summary that conveys "what is this series's
  // tracking state right now" without expanding. Examples:
  //   "Tracking · ch 142 · vol 15 · 3 aliases"
  //   "Tracking · ch 142 · 0 aliases"
  //   "Not tracked"
  // Untracked summary keeps the panel minimal — the toggle is the only
  // actionable control until tracking is on.
  const summary = (() => {
    if (!tracking?.tracked) return "Not tracked";
    const parts: string[] = ["Tracking"];
    if (tracking.trackChapters && tracking.latestKnownChapter != null) {
      parts.push(`ch ${tracking.latestKnownChapter}`);
    }
    if (tracking.trackVolumes && tracking.latestKnownVolume != null) {
      parts.push(`vol ${tracking.latestKnownVolume}`);
    }
    parts.push(`${aliases.length} alias${aliases.length === 1 ? "" : "es"}`);
    return parts.join(" · ");
  })();

  return (
    <Card withBorder padding="md" radius="md">
      <Stack gap="sm">
        <Group justify="space-between" wrap="nowrap">
          <UnstyledButton
            onClick={() => setExpanded((v) => !v)}
            aria-expanded={expanded}
            aria-label={
              expanded ? "Collapse release tracking" : "Expand release tracking"
            }
            style={{ flex: 1, minWidth: 0 }}
          >
            <Group gap="xs" wrap="nowrap">
              {expanded ? (
                <IconChevronDown size={16} />
              ) : (
                <IconChevronRight size={16} />
              )}
              <IconBellRinging size={18} />
              <Text fw={600}>Release tracking</Text>
              {tracking?.tracked && (
                <Badge color="green" variant="light" size="sm">
                  TRACKING
                </Badge>
              )}
              <Text size="sm" c="dimmed" truncate>
                {summary}
              </Text>
            </Group>
          </UnstyledButton>
          <Switch
            checked={tracking?.tracked ?? false}
            onChange={(event) =>
              updateTracking.mutate({ tracked: event.currentTarget.checked })
            }
            disabled={!canEdit || trackingQuery.isLoading}
            aria-label="Toggle release tracking"
          />
        </Group>

        <Collapse in={expanded}>
          <Stack gap="sm" mt="xs">
            {tracking?.tracked && (
              <>
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
                Used by sources that match by title (Nyaa, MangaUpdates without
                an ID).
              </Text>

              {aliases.length === 0 && (
                <Text size="sm" c="dimmed" fs="italic" mb="xs">
                  No aliases yet. Add one below or run the metadata backfill
                  task.
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
                    <Group
                      gap="xs"
                      wrap="nowrap"
                      style={{ minWidth: 0, flex: 1 }}
                    >
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
        </Collapse>
      </Stack>
    </Card>
  );
}
