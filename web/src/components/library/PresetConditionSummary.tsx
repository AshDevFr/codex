import { Badge, Group, Stack, Text } from "@mantine/core";
import type { FilterPresetDto } from "@/api/filterPresets";
import {
  type BookCondition,
  type BookFilterState,
  conditionToBookFilterState,
  conditionToSeriesFilterState,
  type FilterGroupState,
  type SeriesCondition,
  type SeriesFilterState,
  type TriState,
} from "@/types/filters";

const READ_STATUS_LABELS: Record<string, string> = {
  unread: "Unread",
  in_progress: "In Progress",
  read: "Read",
};

const SERIES_STATUS_LABELS: Record<string, string> = {
  ongoing: "Ongoing",
  ended: "Ended",
  hiatus: "Hiatus",
  abandoned: "Abandoned",
  unknown: "Unknown",
};

const BOOK_TYPE_LABELS: Record<string, string> = {
  comic: "Comic",
  manga: "Manga",
  novel: "Novel",
  novella: "Novella",
  anthology: "Anthology",
  artbook: "Artbook",
  oneshot: "Oneshot",
  omnibus: "Omnibus",
  graphic_novel: "Graphic Novel",
  magazine: "Magazine",
};

function labelFor(field: string, value: string): string {
  if (field === "readStatus") return READ_STATUS_LABELS[value] ?? value;
  if (field === "status") return SERIES_STATUS_LABELS[value] ?? value;
  if (field === "bookType") return BOOK_TYPE_LABELS[value] ?? value;
  return value;
}

interface GroupRowProps {
  title: string;
  field: string;
  group: FilterGroupState;
}

function GroupRow({ title, field, group }: GroupRowProps) {
  const entries = Array.from(group.values.entries()).filter(
    ([, state]) => state !== "neutral",
  );
  if (entries.length === 0) return null;
  return (
    <Group gap="xs" wrap="wrap" align="flex-start">
      <Text size="xs" fw={600} c="dimmed" style={{ minWidth: 90 }}>
        {title}
        {group.mode === "allOf" && entries.length > 1 ? " (AND)" : ""}
      </Text>
      <Group gap={4} wrap="wrap" style={{ flex: 1 }}>
        {entries.map(([value, state]) => (
          <Badge
            key={value}
            size="xs"
            variant="light"
            color={state === "include" ? "blue" : "red"}
          >
            {state === "include" ? "+" : "−"} {labelFor(field, value)}
          </Badge>
        ))}
      </Group>
    </Group>
  );
}

interface TriRowProps {
  title: string;
  state: TriState;
  includeLabel?: string;
  excludeLabel?: string;
}

function TriRow({
  title,
  state,
  includeLabel = "Yes",
  excludeLabel = "No",
}: TriRowProps) {
  if (state === "neutral") return null;
  return (
    <Group gap="xs" wrap="nowrap">
      <Text size="xs" fw={600} c="dimmed" style={{ minWidth: 90 }}>
        {title}
      </Text>
      <Badge
        size="xs"
        variant="light"
        color={state === "include" ? "blue" : "red"}
      >
        {state === "include" ? includeLabel : excludeLabel}
      </Badge>
    </Group>
  );
}

function SeriesSummary({ state }: { state: SeriesFilterState }) {
  return (
    <Stack gap={6}>
      <GroupRow
        title="Read Status"
        field="readStatus"
        group={state.readStatus}
      />
      <GroupRow title="Genres" field="genre" group={state.genres} />
      <GroupRow title="Tags" field="tag" group={state.tags} />
      <GroupRow title="Status" field="status" group={state.status} />
      <GroupRow title="Publisher" field="publisher" group={state.publisher} />
      <GroupRow title="Language" field="language" group={state.language} />
      <GroupRow
        title="Sharing Tags"
        field="sharingTag"
        group={state.sharingTags}
      />
      <TriRow title="Completion" state={state.completion} />
      <TriRow
        title="External ID"
        state={state.hasExternalSourceId}
        includeLabel="Has external ID"
        excludeLabel="No external ID"
      />
      <TriRow
        title="My Rating"
        state={state.hasUserRating}
        includeLabel="Has rating"
        excludeLabel="No rating"
      />
      <TriRow
        title="Tracked"
        state={state.isTracked}
        includeLabel="Tracked"
        excludeLabel="Untracked"
      />
    </Stack>
  );
}

function BookSummary({ state }: { state: BookFilterState }) {
  return (
    <Stack gap={6}>
      <GroupRow
        title="Read Status"
        field="readStatus"
        group={state.readStatus}
      />
      <GroupRow title="Genres" field="genre" group={state.genres} />
      <GroupRow title="Tags" field="tag" group={state.tags} />
      <GroupRow title="Book Type" field="bookType" group={state.bookType} />
      <TriRow
        title="Has Error"
        state={state.hasError}
        includeLabel="With errors"
        excludeLabel="No errors"
      />
    </Stack>
  );
}

function hasAnyActive(state: SeriesFilterState | BookFilterState): boolean {
  for (const v of Object.values(state)) {
    if (typeof v === "string") {
      if (v !== "neutral") return true;
    } else if (v && typeof v === "object" && "values" in v) {
      for (const tri of v.values.values()) {
        if (tri !== "neutral") return true;
      }
    }
  }
  return false;
}

export interface PresetConditionSummaryProps {
  preset: FilterPresetDto;
}

/**
 * Read-only renderer for a saved preset's condition. Parses the condition
 * back into the chip UI's flat state and lists each active group or TriState.
 * Falls back to a notice when the condition uses advanced shapes that the
 * chip UI cannot represent (those presets are still applyable from the
 * advanced search page).
 */
export function PresetConditionSummary({
  preset,
}: PresetConditionSummaryProps) {
  const condition = preset.condition as unknown;

  if (preset.target === "series") {
    const state = conditionToSeriesFilterState(
      condition as SeriesCondition | undefined | null,
    );
    if (!state) {
      return <AdvancedNotice />;
    }
    if (!hasAnyActive(state)) {
      return (
        <Text size="xs" c="dimmed">
          No filters in this preset.
        </Text>
      );
    }
    return <SeriesSummary state={state} />;
  }

  if (preset.target === "books") {
    const state = conditionToBookFilterState(
      condition as BookCondition | undefined | null,
    );
    if (!state) {
      return <AdvancedNotice />;
    }
    if (!hasAnyActive(state)) {
      return (
        <Text size="xs" c="dimmed">
          No filters in this preset.
        </Text>
      );
    }
    return <BookSummary state={state} />;
  }

  return <AdvancedNotice />;
}

function AdvancedNotice() {
  return (
    <Text size="xs" c="dimmed" fs="italic">
      Advanced filter. Open this preset in the advanced search page to see the
      full condition.
    </Text>
  );
}
