/**
 * The visual pieces of the reading dashboard.
 *
 * Every chart here is inline SVG or CSS rather than a charting library: the app
 * has no chart dependency, and none of these forms is complicated enough to
 * justify adding one.
 *
 * Measured and reconstructed time are drawn as two steps of a single hue, not
 * two colours. They are the same measure at different confidence, and two hues
 * would say they were different things.
 */

import { Anchor, Box, Group, Paper, Stack, Text, Tooltip } from "@mantine/core";
import { Link } from "react-router-dom";
import type {
  ReadingByDeviceDto,
  ReadingByFormatDto,
  ReadingBySeriesDto,
} from "@/api/readingStats";
import type { ReadingMetric } from "@/store/readingStatsPreferencesStore";
import classes from "./ReadingStatsCharts.module.css";
import {
  type CalendarDay,
  formatDayLabel,
  formatDuration,
  formatMetric,
  formatMetricShort,
  groupIntoWeeks,
  heatLevel,
  heatThresholds,
  metricValue,
} from "./readingStatsFormat";

/** A headline figure. Not a chart: one number needs no axes. */
export function StatTile({
  label,
  value,
  hint,
}: {
  label: string;
  value: string;
  hint?: string;
}) {
  return (
    <Paper p="md" radius="md" withBorder style={{ flex: "1 1 160px" }}>
      <Text size="xs" c="dimmed" tt="uppercase" fw={600}>
        {label}
      </Text>
      <Text
        fz={28}
        fw={700}
        lh={1.2}
        style={{ fontVariantNumeric: "tabular-nums" }}
      >
        {value}
      </Text>
      {hint && (
        <Text size="xs" c="dimmed" mt={2}>
          {hint}
        </Text>
      )}
    </Paper>
  );
}

/**
 * Legend for the provenance split.
 *
 * Always present when both figures are shown, so identity is never carried by
 * colour alone.
 */
export function ProvenanceLegend({ inferredMs }: { inferredMs: number }) {
  return (
    <Group gap="md" wrap="wrap">
      <Group gap={6}>
        <span className={`${classes.swatch} ${classes.measuredSwatch}`} />
        <Text size="xs" c="dimmed">
          Measured
        </Text>
      </Group>
      <Group gap={6}>
        <span className={`${classes.swatch} ${classes.inferredSwatch}`} />
        <Text size="xs" c="dimmed">
          Estimated ({formatDuration(inferredMs)})
        </Text>
      </Group>
    </Group>
  );
}

const CELL = 12;
const CELL_GAP = 3;
/** Only alternate rows are labelled; the blanks still need a stable key. */
const WEEKDAYS = [
  { id: "mon", label: "Mon" },
  { id: "tue", label: "" },
  { id: "wed", label: "Wed" },
  { id: "thu", label: "" },
  { id: "fri", label: "Fri" },
  { id: "sat", label: "" },
  { id: "sun", label: "Sun" },
];

/**
 * Calendar heatmap of daily reading.
 *
 * Sequential single hue, light to dark, with a distinct empty step so "did not
 * read" is visibly different from "read a little" rather than merely paler.
 * Levels are relative to the busiest day so the calendar is legible whether
 * someone reads twenty minutes a day or six hours.
 */
export function ActivityCalendar({
  days,
  metric = "time",
}: {
  days: CalendarDay[];
  metric?: ReadingMetric;
}) {
  const weeks = groupIntoWeeks(days);
  const values = days.map((day) => metricValue(day, metric));
  const thresholds = heatThresholds(values);
  const busiest = values.reduce((max, value) => Math.max(max, value), 0);

  // A window is empty when nothing was read in it, not when the API returned
  // no rows. Every window has days once the gaps are filled, so drawing on
  // row count alone renders a silent year as a full grid of empty cells.
  if (weeks.length === 0 || busiest === 0) {
    return (
      <Text size="sm" c="dimmed">
        No reading recorded in this period.
      </Text>
    );
  }

  const width = weeks.length * (CELL + CELL_GAP);
  const height = 7 * (CELL + CELL_GAP);

  return (
    <Stack gap="xs">
      <Box className={classes.scroller}>
        <Group gap="xs" wrap="nowrap" align="flex-start">
          <Stack gap={CELL_GAP} style={{ paddingTop: 1 }}>
            {WEEKDAYS.map((weekday) => (
              <Text
                key={weekday.id}
                size="10px"
                c="dimmed"
                style={{ height: CELL, lineHeight: `${CELL}px`, width: 24 }}
              >
                {weekday.label}
              </Text>
            ))}
          </Stack>
          <svg
            width={width}
            height={height}
            role="img"
            aria-label={`Daily reading activity, ${days.length} days`}
          >
            {weeks.map((week, weekIndex) =>
              week.map((day, dayIndex) => {
                if (!day) return null;
                const value = metricValue(day, metric);
                const level = heatLevel(value, thresholds);
                return (
                  <Tooltip
                    key={day.date}
                    label={
                      value > 0
                        ? `${formatDayLabel(day.date)}: ${formatMetric(value, metric)}`
                        : `${formatDayLabel(day.date)}: no reading`
                    }
                    withArrow
                  >
                    <rect
                      className={classes.cell}
                      x={weekIndex * (CELL + CELL_GAP)}
                      y={dayIndex * (CELL + CELL_GAP)}
                      width={CELL}
                      height={CELL}
                      rx={3}
                      fill={`var(--heat-${level})`}
                    />
                  </Tooltip>
                );
              }),
            )}
          </svg>
        </Group>
      </Box>
      <Group gap={6} justify="flex-end">
        <Text size="10px" c="dimmed">
          Less
        </Text>
        {[0, 1, 2, 3, 4, 5].map((level) => (
          <span
            key={level}
            className={classes.swatch}
            style={{ background: `var(--heat-${level})` }}
          />
        ))}
        <Text size="10px" c="dimmed">
          More
        </Text>
      </Group>
    </Stack>
  );
}

/**
 * Reading per bucket, as a stacked column chart.
 *
 * A 2px gap between the two segments keeps them from reading as one block, and
 * the data end is rounded while the baseline end is square, so bars stay
 * anchored to the axis.
 */
export function PeriodBars({
  periods,
  metric = "time",
}: {
  periods: {
    bucket: string;
    measuredMs: number;
    inferredMs: number;
    totalMs: number;
    pagesRead: number;
    booksFinished: number;
  }[];
  metric?: ReadingMetric;
}) {
  if (periods.length === 0) {
    return (
      <Text size="sm" c="dimmed">
        No reading recorded in this period.
      </Text>
    );
  }

  const max =
    periods.reduce((m, p) => Math.max(m, metricValue(p, metric)), 0) || 1;
  const barWidth = 14;
  const gap = 6;
  const height = 140;
  const width = periods.length * (barWidth + gap);

  return (
    <Box className={classes.scroller}>
      <svg
        width={Math.max(width, 200)}
        height={height + 20}
        role="img"
        aria-label="Reading per period"
      >
        {periods.map((p, i) => {
          const value = metricValue(p, metric);
          const totalH = Math.round((value / max) * height);
          // The measured/estimated split is a statement about where a *time*
          // came from. Under pages or books finished there is no provenance to
          // report, so the bar is one piece.
          const measuredH =
            metric === "time"
              ? Math.round((p.measuredMs / max) * height)
              : totalH;
          const inferredH = Math.max(0, totalH - measuredH);
          const x = i * (barWidth + gap);
          return (
            <Tooltip
              key={p.bucket}
              label={`${p.bucket}: ${formatMetric(value, metric)}`}
              withArrow
            >
              <g className={classes.bar}>
                {/* Estimated time sits on top, visually subordinate. */}
                {inferredH > 0 && (
                  <rect
                    x={x}
                    y={height - totalH}
                    width={barWidth}
                    height={inferredH}
                    rx={3}
                    fill="var(--inferred)"
                  />
                )}
                {measuredH > 0 && (
                  <rect
                    x={x}
                    // 2px surface gap between the segments.
                    y={height - measuredH}
                    width={barWidth}
                    height={measuredH}
                    rx={3}
                    fill="var(--measured)"
                  />
                )}
                {totalH === 0 && (
                  <rect
                    x={x}
                    y={height - 2}
                    width={barWidth}
                    height={2}
                    rx={1}
                    fill="var(--grid)"
                  />
                )}
              </g>
            </Tooltip>
          );
        })}
      </svg>
    </Box>
  );
}

/** A breakdown row, in the shape the metric helpers want. */
function rowValue(
  row: {
    duration: { totalMs: number };
    pagesRead: number;
    booksFinished: number;
  },
  metric: ReadingMetric,
): number {
  return metricValue(
    {
      totalMs: row.duration.totalMs,
      pagesRead: row.pagesRead,
      booksFinished: row.booksFinished,
    },
    metric,
  );
}

/**
 * Rows worth nothing under the metric on screen say nothing, so they are not
 * drawn.
 *
 * Deliberately not "has no data": the device the backfill calls `legacy` has no
 * time at all and thousands of sittings. Under time it is silent and drops out;
 * under books finished it is most of the reader's history.
 */
function contributed(
  row: {
    duration: { totalMs: number };
    pagesRead: number;
    booksFinished: number;
  },
  metric: ReadingMetric,
): boolean {
  return rowValue(row, metric) > 0;
}

/**
 * The device id the session backfill coins for reading that predates time
 * tracking. It is bookkeeping, not a device the reader owns, so it never
 * appears verbatim.
 */
const LEGACY_DEVICE_ID = "legacy";

function deviceLabel(device: ReadingByDeviceDto): string {
  if (device.deviceId === LEGACY_DEVICE_ID) return "Before time tracking";
  return device.deviceName ?? device.deviceId;
}

/** A labelled row with a proportional bar. Used for series, devices, formats. */
function RankedRow({
  label,
  href,
  sublabel,
  measuredMs,
  inferredMs,
  value,
  metric,
  max,
}: {
  label: string;
  href?: string;
  sublabel?: string;
  measuredMs: number;
  inferredMs: number;
  value: number;
  metric: ReadingMetric;
  max: number;
}) {
  // Only time has a provenance split to draw; the other metrics are one bar.
  const measuredPct =
    max > 0 ? ((metric === "time" ? measuredMs : value) / max) * 100 : 0;
  const inferredPct =
    max > 0 && metric === "time" ? (inferredMs / max) * 100 : 0;

  return (
    <Stack gap={4}>
      <Group justify="space-between" wrap="nowrap" gap="sm">
        {href ? (
          <Anchor
            component={Link}
            to={href}
            size="sm"
            truncate
            c="inherit"
            underline="hover"
            style={{ minWidth: 0 }}
          >
            {label}
          </Anchor>
        ) : (
          <Text size="sm" truncate style={{ minWidth: 0 }}>
            {label}
          </Text>
        )}
        <Text
          size="sm"
          c="dimmed"
          style={{ fontVariantNumeric: "tabular-nums", flex: "none" }}
        >
          {formatMetric(value, metric)}
        </Text>
      </Group>
      <div className={classes.track}>
        <div
          className={classes.trackMeasured}
          style={{ width: `${measuredPct}%` }}
        />
        <div
          className={classes.trackInferred}
          style={{ width: `${inferredPct}%` }}
        />
      </div>
      {sublabel && (
        <Text size="xs" c="dimmed">
          {sublabel}
        </Text>
      )}
    </Stack>
  );
}

export function TopSeries({
  series,
  metric = "time",
}: {
  series: ReadingBySeriesDto[];
  metric?: ReadingMetric;
}) {
  const read = series.filter((s) => contributed(s, metric));

  if (read.length === 0) {
    return (
      <Text size="sm" c="dimmed">
        No series read in this period.
      </Text>
    );
  }
  const max = read.reduce((m, s) => Math.max(m, rowValue(s, metric)), 0);

  return (
    <Stack gap="md">
      {read.map((s) => (
        <RankedRow
          key={s.seriesId}
          label={s.seriesName}
          href={`/series/${s.seriesId}`}
          sublabel={`${s.pagesRead} pages across ${s.books} ${s.books === 1 ? "book" : "books"}`}
          measuredMs={s.duration.measuredMs}
          inferredMs={s.duration.inferredMs}
          value={rowValue(s, metric)}
          metric={metric}
          max={max}
        />
      ))}
    </Stack>
  );
}

export function DeviceBreakdown({
  devices,
  metric = "time",
}: {
  devices: ReadingByDeviceDto[];
  metric?: ReadingMetric;
}) {
  const used = devices.filter((d) => contributed(d, metric));

  if (used.length === 0) {
    return (
      <Text size="sm" c="dimmed">
        No devices recorded in this period.
      </Text>
    );
  }
  const max = used.reduce((m, d) => Math.max(m, rowValue(d, metric)), 0);

  return (
    <Stack gap="md">
      {used.map((d) => (
        <RankedRow
          key={d.deviceId}
          label={deviceLabel(d)}
          sublabel={`${d.sessions} ${d.sessions === 1 ? "sitting" : "sittings"}, last read ${new Date(d.lastReadAt).toLocaleDateString()}`}
          measuredMs={d.duration.measuredMs}
          inferredMs={d.duration.inferredMs}
          value={rowValue(d, metric)}
          metric={metric}
          max={max}
        />
      ))}
    </Stack>
  );
}

export function FormatBreakdown({
  formats,
  metric = "time",
}: {
  formats: ReadingByFormatDto[];
  metric?: ReadingMetric;
}) {
  const read = formats.filter((f) => contributed(f, metric));

  if (read.length === 0) return null;
  const max = read.reduce((m, f) => Math.max(m, rowValue(f, metric)), 0);

  return (
    <Group gap="lg" wrap="wrap">
      {read.map((f) => (
        <Stack key={f.format} gap={2} style={{ minWidth: 90 }}>
          <Text size="xs" c="dimmed" tt="uppercase" fw={600}>
            {f.format}
          </Text>
          <Text fw={600} style={{ fontVariantNumeric: "tabular-nums" }}>
            {formatMetric(rowValue(f, metric), metric)}
          </Text>
          <div className={classes.track}>
            <div
              className={classes.trackMeasured}
              style={{
                width: `${max > 0 ? (rowValue(f, metric) / max) * 100 : 0}%`,
              }}
            />
          </div>
        </Stack>
      ))}
    </Group>
  );
}

/** Axis-free summary of the busiest bucket, for the period chart's caption. */
export function busiestBucketCaption(
  periods: {
    bucket: string;
    totalMs: number;
    pagesRead: number;
    booksFinished: number;
  }[],
  metric: ReadingMetric = "time",
): string | null {
  if (periods.length === 0) return null;
  const busiest = periods.reduce((best, p) =>
    metricValue(p, metric) > metricValue(best, metric) ? p : best,
  );
  const value = metricValue(busiest, metric);
  if (value <= 0) return null;
  return `Busiest: ${busiest.bucket}, ${formatMetricShort(value, metric)}`;
}

/** Re-exported so the page and its tests share one formatter. */
export { formatDuration };
