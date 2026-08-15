/**
 * The reading dashboard.
 *
 * One request covers every panel, so no two panels can disagree about which
 * dates they show.
 */

import {
  Alert,
  Center,
  Container,
  Group,
  Loader,
  Paper,
  SegmentedControl,
  Stack,
  Text,
  Title,
} from "@mantine/core";
import { IconInfoCircle } from "@tabler/icons-react";
import { useQuery } from "@tanstack/react-query";
import { useMemo, useState } from "react";
import {
  type ReadingStatsGranularity,
  readingStatsApi,
} from "@/api/readingStats";
import classes from "@/components/reading/ReadingStatsCharts.module.css";
import {
  ActivityCalendar,
  busiestBucketCaption,
  DeviceBreakdown,
  FormatBreakdown,
  formatDuration,
  PeriodBars,
  ProvenanceLegend,
  StatTile,
  TopSeries,
} from "@/components/reading/ReadingStatsPanels";
import { buildCalendar } from "@/components/reading/readingStatsFormat";

const RANGES = [
  { value: "30", label: "30 days" },
  { value: "90", label: "90 days" },
  { value: "365", label: "1 year" },
] as const;

/** Days of history a range shows, and the bucket size that suits it. */
function granularityFor(days: number): ReadingStatsGranularity {
  if (days <= 30) return "day";
  if (days <= 90) return "day";
  return "week";
}

export function ReadingStats() {
  const [rangeDays, setRangeDays] = useState("90");
  const days = Number(rangeDays);

  // Pinned to the day so the query key is stable across re-renders; a `now`
  // that moves every render would refetch continuously.
  const { from, to } = useMemo(() => {
    const end = new Date();
    end.setUTCHours(23, 59, 59, 999);
    const start = new Date(end);
    start.setUTCDate(start.getUTCDate() - (days - 1));
    start.setUTCHours(0, 0, 0, 0);
    return { from: start, to: end };
  }, [days]);

  const granularity = granularityFor(days);

  const { data, isLoading, error } = useQuery({
    queryKey: ["readingStats", rangeDays, granularity],
    queryFn: () =>
      readingStatsApi.get({ from, to, granularity, seriesLimit: 8 }),
    staleTime: 60_000,
  });

  const calendar = useMemo(() => {
    if (!data) return [];
    // The calendar always wants daily grain, whatever the chart above shows.
    return buildCalendar(granularity === "day" ? data.periods : [], from, to);
  }, [data, granularity, from, to]);

  const periodBars = useMemo(
    () =>
      (data?.periods ?? []).map((p) => ({
        bucket: p.bucket,
        measuredMs: p.duration.measuredMs,
        inferredMs: p.duration.inferredMs,
        totalMs: p.duration.totalMs,
      })),
    [data],
  );

  if (isLoading) {
    return (
      <Center h={300}>
        <Loader />
      </Center>
    );
  }

  if (error || !data) {
    return (
      <Container size="lg" py="md">
        <Alert color="red" title="Could not load reading statistics">
          {error instanceof Error ? error.message : "Please try again."}
        </Alert>
      </Container>
    );
  }

  const { summary } = data;
  const hasInferred = summary.duration.inferredMs > 0;

  return (
    <Container size="lg" py="md" className={classes.viz}>
      <Stack gap="lg">
        <Group justify="space-between" align="flex-end" wrap="wrap">
          <div>
            <Title order={2}>Reading</Title>
            <Text size="sm" c="dimmed">
              How much you have read, and where.
            </Text>
          </div>
          <SegmentedControl
            value={rangeDays}
            onChange={setRangeDays}
            data={RANGES.map((r) => ({ value: r.value, label: r.label }))}
            size="sm"
          />
        </Group>

        <Group gap="md" wrap="wrap" align="stretch">
          <StatTile
            label="Time read"
            value={formatDuration(summary.duration.totalMs)}
            hint={
              hasInferred
                ? `${formatDuration(summary.duration.measuredMs)} measured`
                : undefined
            }
          />
          <StatTile label="Pages" value={summary.pagesRead.toLocaleString()} />
          <StatTile
            label="Sittings"
            value={summary.sessions.toLocaleString()}
          />
          <StatTile label="Books" value={summary.books.toLocaleString()} />
        </Group>

        {hasInferred && (
          <Alert
            variant="light"
            color="blue"
            icon={<IconInfoCircle size={16} />}
            title="Some of this time is estimated"
          >
            {formatDuration(summary.duration.inferredMs)} came from apps that
            cannot report reading time, so it was reconstructed from how often
            they saved your place. That estimate only ever undercounts, and it
            cannot see reading done from an already-downloaded book at all.
          </Alert>
        )}

        {summary.sessionsWithoutDuration > 0 && (
          <Text size="xs" c="dimmed">
            {summary.sessionsWithoutDuration} of {summary.sessions} sittings
            reported no time at all, so the totals above are a floor rather than
            the full picture.
          </Text>
        )}

        <Paper p="md" radius="md" withBorder>
          <Stack gap="sm">
            <Group justify="space-between" wrap="wrap">
              <Title order={4}>Daily activity</Title>
              {hasInferred && (
                <ProvenanceLegend inferredMs={summary.duration.inferredMs} />
              )}
            </Group>
            <ActivityCalendar days={calendar} />
          </Stack>
        </Paper>

        <Paper p="md" radius="md" withBorder>
          <Stack gap="sm">
            <Group justify="space-between" wrap="wrap">
              <Title order={4}>
                Time per {granularity === "week" ? "week" : "day"}
              </Title>
              <Text size="xs" c="dimmed">
                {busiestBucketCaption(periodBars)}
              </Text>
            </Group>
            <PeriodBars periods={periodBars} />
          </Stack>
        </Paper>

        <Group align="flex-start" gap="lg" wrap="wrap">
          <Paper p="md" radius="md" withBorder style={{ flex: "1 1 320px" }}>
            <Stack gap="sm">
              <Title order={4}>Most read</Title>
              <TopSeries series={data.series} />
            </Stack>
          </Paper>

          <Paper p="md" radius="md" withBorder style={{ flex: "1 1 320px" }}>
            <Stack gap="sm">
              <Title order={4}>Devices</Title>
              <DeviceBreakdown devices={data.devices} />
            </Stack>
          </Paper>
        </Group>

        {data.formats.length > 0 && (
          <Paper p="md" radius="md" withBorder>
            <Stack gap="sm">
              <Title order={4}>Formats</Title>
              <FormatBreakdown formats={data.formats} />
            </Stack>
          </Paper>
        )}
      </Stack>
    </Container>
  );
}
