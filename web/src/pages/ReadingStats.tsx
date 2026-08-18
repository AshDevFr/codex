/**
 * The reading dashboard.
 *
 * One request covers every panel, so no two panels can disagree about which
 * dates they show.
 */

import {
  Alert,
  Button,
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
import { useMemo } from "react";
import { readingStatsApi } from "@/api/readingStats";
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
import {
  buildCalendar,
  groupIntoYears,
  heatThresholds,
  metricValue,
  resolveRange,
  rollUpIntoMonths,
  rollUpIntoWeeks,
  windowFor,
  yearsCovered,
} from "@/components/reading/readingStatsFormat";
import {
  READING_METRICS,
  RELATIVE_RANGES,
  type ReadingMetric,
  rangeKey,
  sortForMetric,
  useReadingStatsPreferencesStore,
} from "@/store/readingStatsPreferencesStore";

/** Bucket labels for the period chart's heading. */
const BAR_LABELS = { day: "day", week: "week", month: "month" } as const;

export function ReadingStats() {
  const metric = useReadingStatsPreferencesStore((state) => state.metric);
  const setMetric = useReadingStatsPreferencesStore((state) => state.setMetric);
  const storedRange = useReadingStatsPreferencesStore((state) => state.range);
  const setRange = useReadingStatsPreferencesStore((state) => state.setRange);

  // Pinned to the day so query keys are stable across re-renders; a `now` that
  // moved every render would refetch continuously.
  const today = useMemo(() => new Date(), []);

  // Window-independent, and changes at most once a day, so it is fetched once
  // and kept far longer than any windowed figure.
  const { data: coverage } = useQuery({
    queryKey: ["readingStats", "coverage"],
    queryFn: () => readingStatsApi.coverage(),
    staleTime: 60 * 60_000,
  });

  const years = useMemo(
    () => yearsCovered(coverage ?? { firstReadAt: null }, today),
    [coverage, today],
  );

  // A stored year can outlive its data, or arrive from another account.
  const range = resolveRange(storedRange, years);

  const { from, to, bars } = useMemo(
    () => windowFor(range, coverage ?? { firstReadAt: null }, today),
    [range, coverage, today],
  );

  // The ranking key is part of the request because the server applies the
  // series limit: ranking by pages here would sort a top-8 chosen by time.
  const sort = sortForMetric(metric);

  const { data, isLoading, error } = useQuery({
    // The window is part of the key, not just the range's name. All-time's
    // window comes from the coverage request, so it changes after the first
    // render; keyed only by "all", the placeholder window's empty result would
    // be served forever.
    queryKey: [
      "readingStats",
      rangeKey(range),
      from.toISOString(),
      to.toISOString(),
      sort,
    ],
    queryFn: () =>
      readingStatsApi.get({
        from,
        to,
        granularity: "day",
        seriesLimit: 8,
        sort,
      }),
    // All-time cannot be asked for until coverage says where history starts.
    // Without this it fetches a one-day placeholder window first and discards
    // it, which shows as a flash of zeroes.
    enabled: range.kind !== "all" || coverage !== undefined,
    staleTime: 60_000,
  });

  const calendar = useMemo(() => {
    if (!data) return [];
    return buildCalendar(data.periods, from, to);
  }, [data, from, to]);

  // All-time spans years, and one grid ten years wide is unreadable at any cell
  // size that fits a screen. Every year shares one scale so a light year cannot
  // be mistaken for a heavy one.
  const calendarYears = useMemo(
    () => (range.kind === "all" ? groupIntoYears(calendar) : []),
    [range, calendar],
  );
  const sharedThresholds = useMemo(
    () => heatThresholds(calendar.map((day) => metricValue(day, metric))),
    [calendar, metric],
  );

  const periodBars = useMemo(() => {
    const periods = data?.periods ?? [];
    const bucketed =
      bars === "month"
        ? rollUpIntoMonths(periods)
        : bars === "week"
          ? rollUpIntoWeeks(periods)
          : periods;
    return bucketed.map((p) => ({
      bucket: p.bucket,
      measuredMs: p.duration.measuredMs,
      inferredMs: p.duration.inferredMs,
      totalMs: p.duration.totalMs,
      pagesRead: p.pagesRead,
      booksFinished: p.booksFinished,
    }));
  }, [data, bars]);

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
  // Provenance is a property of measured time. Under pages or books finished
  // there is no estimate to disclose, so the legend and the caveat about it
  // would be answering a question nobody asked.
  const showingTime = metric === "time";
  const showingPages = metric === "pages";
  const hasInferred = showingTime && summary.duration.inferredMs > 0;

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
          <Group gap="sm" wrap="wrap">
            <SegmentedControl
              value={metric}
              onChange={(value) => setMetric(value as ReadingMetric)}
              data={READING_METRICS.map((m) => ({
                value: m.value,
                label: m.label,
              }))}
              size="sm"
            />
            <SegmentedControl
              value={range.kind === "relative" ? String(range.days) : "custom"}
              onChange={(value) =>
                setRange({
                  kind: "relative",
                  days: Number(value) as 30 | 90 | 365,
                })
              }
              data={RELATIVE_RANGES.map((r) => ({
                value: String(r.days),
                label: r.label,
              }))}
              size="sm"
            />
          </Group>
        </Group>

        {/* Calendar years are not rolling windows, so they get their own row
            rather than being mixed into the relative control above. */}
        {years.length > 0 && (
          <Group gap="xs" wrap="wrap" justify="flex-end">
            <Button
              size="compact-xs"
              variant={range.kind === "all" ? "filled" : "subtle"}
              onClick={() => setRange({ kind: "all" })}
            >
              All time
            </Button>
            {years.map((year) => (
              <Button
                key={year}
                size="compact-xs"
                variant={
                  range.kind === "year" && range.year === year
                    ? "filled"
                    : "subtle"
                }
                onClick={() => setRange({ kind: "year", year })}
              >
                {year}
              </Button>
            ))}
          </Group>
        )}

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
          <StatTile
            label="Books finished"
            value={summary.booksFinished.toLocaleString()}
          />
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

        {showingTime && summary.sessionsWithoutDuration > 0 && (
          <Text size="xs" c="dimmed">
            {summary.sessionsWithoutDuration} of {summary.sessions} sittings
            reported no time at all, so the totals above are a floor rather than
            the full picture.
          </Text>
        )}

        {/* Pages are known only for sittings a reading app measured itself.
            Reading that predates session tracking, and anything synced by an
            app that only saves a position, counts as zero pages rather than as
            missing, so the page views need the same floor caveat time gets. */}
        {showingPages && summary.sessionsWithoutPages > 0 && (
          <Text size="xs" c="dimmed">
            {summary.sessionsWithoutPages} of {summary.sessions} sittings
            reported no page count, so the totals above are a floor rather than
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
            {range.kind === "all" ? (
              <Stack gap="lg">
                {calendarYears.map(({ year, days }, index) => (
                  <Stack gap={4} key={year}>
                    <Text size="xs" c="dimmed" fw={600}>
                      {year}
                    </Text>
                    <ActivityCalendar
                      days={days}
                      metric={metric}
                      thresholds={sharedThresholds}
                      // One legend for the stack, under the last grid, where
                      // it sits in the single-calendar case too.
                      showLegend={index === calendarYears.length - 1}
                    />
                  </Stack>
                ))}
              </Stack>
            ) : (
              <ActivityCalendar days={calendar} metric={metric} />
            )}
          </Stack>
        </Paper>

        <Paper p="md" radius="md" withBorder>
          <Stack gap="sm">
            <Group justify="space-between" wrap="wrap">
              <Title order={4}>
                {READING_METRICS.find((m) => m.value === metric)?.label} per{" "}
                {BAR_LABELS[bars]}
              </Title>
              <Text size="xs" c="dimmed">
                {busiestBucketCaption(periodBars, metric)}
              </Text>
            </Group>
            <PeriodBars periods={periodBars} metric={metric} />
          </Stack>
        </Paper>

        <Group align="flex-start" gap="lg" wrap="wrap">
          <Paper p="md" radius="md" withBorder style={{ flex: "1 1 320px" }}>
            <Stack gap="sm">
              <Title order={4}>Most read</Title>
              <TopSeries series={data.series} metric={metric} />
            </Stack>
          </Paper>

          <Paper p="md" radius="md" withBorder style={{ flex: "1 1 320px" }}>
            <Stack gap="sm">
              <Title order={4}>Devices</Title>
              <DeviceBreakdown devices={data.devices} metric={metric} />
            </Stack>
          </Paper>
        </Group>

        {data.formats.length > 0 && (
          <Paper p="md" radius="md" withBorder>
            <Stack gap="sm">
              <Title order={4}>Formats</Title>
              <FormatBreakdown formats={data.formats} metric={metric} />
            </Stack>
          </Paper>
        )}
      </Stack>
    </Container>
  );
}
