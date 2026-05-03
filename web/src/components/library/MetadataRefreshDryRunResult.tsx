import {
  Alert,
  Badge,
  Card,
  Group,
  Modal,
  Paper,
  ScrollArea,
  SimpleGrid,
  Stack,
  Text,
} from "@mantine/core";
import { IconAlertTriangle, IconLock } from "@tabler/icons-react";
import type {
  DryRunResponse,
  DryRunSeriesDelta,
  FieldChange,
} from "@/api/metadataRefresh";

interface MetadataRefreshDryRunResultProps {
  opened: boolean;
  onClose: () => void;
  result: DryRunResponse | null;
  loading?: boolean;
}

/**
 * Render a single field-change row: `field: before → after`. `before` and
 * `after` are arbitrary JSON, so we stringify whatever we get for display.
 */
function fmtValue(value: unknown): string {
  if (value === null || value === undefined) return "(empty)";
  if (typeof value === "string") return value.length > 0 ? value : "(empty)";
  try {
    return JSON.stringify(value);
  } catch {
    return String(value);
  }
}

function ChangeRow({ change }: { change: FieldChange }) {
  return (
    <Group gap="xs" wrap="nowrap" align="flex-start">
      <Text size="xs" fw={500} style={{ minWidth: 120 }}>
        {change.field}
      </Text>
      <Text
        size="xs"
        c="red"
        style={{ textDecoration: "line-through", flexShrink: 0 }}
      >
        {fmtValue(change.before)}
      </Text>
      <Text size="xs" c="dimmed" style={{ flexShrink: 0 }}>
        →
      </Text>
      <Text size="xs" c="green" style={{ wordBreak: "break-word" }}>
        {fmtValue(change.after)}
      </Text>
    </Group>
  );
}

function DeltaCard({ delta }: { delta: DryRunSeriesDelta }) {
  const hasChanges = delta.changes.length > 0;
  const skipped = delta.skipped ?? [];
  const hasOnlySkips = !hasChanges && skipped.length > 0;

  return (
    <Card withBorder p="sm">
      <Stack gap="xs">
        <Group justify="space-between" wrap="nowrap">
          <Text fw={500} size="sm" lineClamp={1}>
            {delta.seriesTitle}
          </Text>
          <Badge size="xs" variant="light" color="gray">
            {delta.provider}
          </Badge>
        </Group>

        {hasChanges && (
          <Stack gap={4}>
            {delta.changes.map((c) => (
              <ChangeRow key={c.field} change={c} />
            ))}
          </Stack>
        )}

        {skipped.length > 0 && (
          <Stack gap={4}>
            {skipped.map((s) => (
              <Group key={s.field} gap="xs" wrap="nowrap">
                <IconLock size={12} style={{ flexShrink: 0, opacity: 0.7 }} />
                <Text size="xs" c="dimmed">
                  <Text span fw={500} inherit>
                    {s.field}
                  </Text>
                  : {s.reason}
                </Text>
              </Group>
            ))}
          </Stack>
        )}

        {hasOnlySkips && (
          <Text size="xs" c="dimmed" fs="italic">
            Nothing would change for this series.
          </Text>
        )}

        {!hasChanges && skipped.length === 0 && (
          <Text size="xs" c="dimmed" fs="italic">
            No deltas reported.
          </Text>
        )}
      </Stack>
    </Card>
  );
}

export function MetadataRefreshDryRunResult({
  opened,
  onClose,
  result,
  loading,
}: MetadataRefreshDryRunResultProps) {
  return (
    <Modal
      opened={opened}
      onClose={onClose}
      title="Dry-run preview"
      size="xl"
      scrollAreaComponent={ScrollArea.Autosize}
    >
      {loading ? (
        <Text c="dimmed">Computing preview…</Text>
      ) : !result ? (
        <Text c="dimmed">No preview available yet.</Text>
      ) : (
        <Stack gap="md">
          <Paper p="sm" withBorder>
            <SimpleGrid cols={{ base: 1, sm: 3 }} spacing="md">
              <Stack gap={2}>
                <Text size="xs" c="dimmed">
                  Eligible (series × provider)
                </Text>
                <Text fw={600}>{result.totalEligible}</Text>
              </Stack>
              <Stack gap={2}>
                <Text size="xs" c="dimmed">
                  Skipped (no external ID)
                </Text>
                <Text fw={600}>{result.estSkippedNoId}</Text>
              </Stack>
              <Stack gap={2}>
                <Text size="xs" c="dimmed">
                  Skipped (recently synced)
                </Text>
                <Text fw={600}>{result.estSkippedRecentlySynced}</Text>
              </Stack>
            </SimpleGrid>
          </Paper>

          {result.unresolvedProviders &&
            result.unresolvedProviders.length > 0 && (
              <Alert
                icon={<IconAlertTriangle size={16} />}
                color="yellow"
                variant="light"
                title="Unresolved providers"
              >
                <Text size="sm">
                  These providers in your config don't match an enabled plugin:
                </Text>
                <Group gap="xs" mt={4}>
                  {result.unresolvedProviders.map((p) => (
                    <Badge key={p} color="yellow" variant="filled" size="sm">
                      {p}
                    </Badge>
                  ))}
                </Group>
              </Alert>
            )}

          {result.sample.length === 0 ? (
            <Alert color="gray" variant="light">
              No series eligible for refresh under the current configuration.
              Try widening the provider list or disabling "use existing source
              IDs only".
            </Alert>
          ) : (
            <Stack gap="xs">
              <Text size="sm" c="dimmed">
                Showing {result.sample.length} of {result.totalEligible}{" "}
                eligible (series × provider) pairs.
              </Text>
              {result.sample.map((delta) => (
                <DeltaCard
                  key={`${delta.seriesId}:${delta.provider}`}
                  delta={delta}
                />
              ))}
            </Stack>
          )}
        </Stack>
      )}
    </Modal>
  );
}
