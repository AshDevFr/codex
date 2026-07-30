import { Badge, Group, Stack, Text } from "@mantine/core";
import { useQuery } from "@tanstack/react-query";
import type { FilterPresetDto } from "@/api/filterPresets";
import { librariesApi } from "@/api/libraries";
import { storageToDisplayRating } from "@/api/ratings";
import {
  type BookCondition,
  type BookFilterState,
  conditionToBookFilterState,
  conditionToSeriesFilterState,
  type FilterGroupState,
  type NumberOperator,
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

const RATING_FIELD_LABELS: Record<string, string> = {
  userRating: "My Rating",
  communityRating: "Community Rating",
};

function labelFor(
  field: string,
  value: string,
  names?: Map<string, string>,
): string {
  if (field === "readStatus") return READ_STATUS_LABELS[value] ?? value;
  if (field === "status") return SERIES_STATUS_LABELS[value] ?? value;
  if (field === "bookType") return BOOK_TYPE_LABELS[value] ?? value;
  // Libraries are stored by UUID. Fall back to a short prefix rather than
  // dumping a full UUID at the user when the name isn't loaded (or the library
  // has since been deleted).
  if (field === "libraryId") {
    return names?.get(value) ?? `${value.slice(0, 8)}…`;
  }
  return value;
}

/**
 * Render a `NumberOperator` over a rating on the 1-10 display scale.
 *
 * Conditions store the 1-100 scale, so every bound goes through
 * `storageToDisplayRating`. Showing the raw stored value would read as "my
 * rating is at least 85" on a 10-point scale.
 */
function describeRating(operator: NumberOperator): string {
  const show = (stored: number) => storageToDisplayRating(stored).toFixed(1);
  switch (operator.operator) {
    case "eq":
      return `is ${show(operator.value)}`;
    case "ne":
      return `is not ${show(operator.value)}`;
    case "gt":
      return `> ${show(operator.value)}`;
    case "gte":
      return `≥ ${show(operator.value)}`;
    case "lt":
      return `< ${show(operator.value)}`;
    case "lte":
      return `≤ ${show(operator.value)}`;
    case "between": {
      const { min, max } = operator;
      if (typeof min === "number" && typeof max === "number") {
        return `${show(min)} to ${show(max)}`;
      }
      if (typeof min === "number") return `≥ ${show(min)}`;
      if (typeof max === "number") return `≤ ${show(max)}`;
      return "any";
    }
    case "isNull":
      return "not rated";
    case "isNotNull":
      return "rated";
  }
}

interface RatingLeaf {
  field: "userRating" | "communityRating";
  operator: NumberOperator;
}

function asRatingLeaf(condition: unknown): RatingLeaf | null {
  if (typeof condition !== "object" || condition === null) return null;
  const keys = Object.keys(condition as Record<string, unknown>);
  if (keys.length !== 1) return null;
  const field = keys[0];
  if (field !== "userRating" && field !== "communityRating") return null;
  const operator = (condition as Record<string, NumberOperator>)[field];
  if (!operator || typeof operator.operator !== "string") return null;
  return { field, operator };
}

/**
 * Pull rating leaves out of a condition so they can be described directly.
 *
 * Rating filters are only buildable in the advanced filter builder, and the
 * chip-based state this summary otherwise renders has no numeric slot. Without
 * this split, any preset containing a rating would fall through to the
 * "advanced filter" notice and tell the user nothing about it.
 */
function extractRatingLeaves(condition: SeriesCondition | null | undefined): {
  ratings: RatingLeaf[];
  rest: SeriesCondition | null;
} {
  if (!condition) return { ratings: [], rest: null };

  const direct = asRatingLeaf(condition);
  if (direct) return { ratings: [direct], rest: null };

  const record = condition as Record<string, unknown>;
  if (!Array.isArray(record.allOf)) return { ratings: [], rest: condition };

  const ratings: RatingLeaf[] = [];
  const others: SeriesCondition[] = [];
  for (const item of record.allOf as SeriesCondition[]) {
    const leaf = asRatingLeaf(item);
    if (leaf) ratings.push(leaf);
    else others.push(item);
  }

  if (ratings.length === 0) return { ratings: [], rest: condition };
  if (others.length === 0) return { ratings, rest: null };
  if (others.length === 1) return { ratings, rest: others[0] };
  return { ratings, rest: { allOf: others } };
}

function RatingRow({ leaf }: { leaf: RatingLeaf }) {
  return (
    <Group gap="xs" wrap="nowrap">
      <Text size="xs" fw={600} c="dimmed" style={{ minWidth: 90 }}>
        {RATING_FIELD_LABELS[leaf.field]}
      </Text>
      <Badge size="xs" variant="light" color="blue">
        {describeRating(leaf.operator)}
      </Badge>
    </Group>
  );
}

interface GroupRowProps {
  title: string;
  field: string;
  group: FilterGroupState;
  /** UUID to display-name map, for fields whose values are IDs. */
  names?: Map<string, string>;
}

function GroupRow({ title, field, group, names }: GroupRowProps) {
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
            {state === "include" ? "+" : "−"} {labelFor(field, value, names)}
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

function SeriesSummary({
  state,
  libraryNames,
}: {
  state: SeriesFilterState;
  libraryNames?: Map<string, string>;
}) {
  return (
    <Stack gap={6}>
      <GroupRow
        title="Libraries"
        field="libraryId"
        group={state.libraries}
        names={libraryNames}
      />
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

export interface ConditionSummaryProps {
  condition: unknown;
  /** Which grammar the condition speaks. Defaults to `series`. */
  target?: "series" | "books";
  /** Wording for the "nothing here" case. */
  emptyLabel?: string;
}

/**
 * Read-only renderer for a filter condition. Parses it back into the chip UI's
 * flat state and lists each active group or TriState. Falls back to a notice
 * when the condition uses advanced shapes the chip UI cannot represent (those
 * are still applyable from the advanced search page).
 *
 * Rating leaves are described separately, before that parse: they're only
 * buildable in the advanced builder and the chip state has no numeric slot, so
 * folding them in would hide them behind the advanced-filter notice.
 *
 * Used both for saved presets and for an automatic collection's membership rule,
 * which is the same grammar rendered in a different place.
 */
export function ConditionSummary({
  condition,
  target = "series",
  emptyLabel = "No filters in this preset.",
}: ConditionSummaryProps) {
  // Library chips hold UUIDs; resolve display names where we can.
  const { data: libraries } = useQuery({
    queryKey: ["libraries"],
    queryFn: () => librariesApi.getAll(),
    staleTime: 5 * 60 * 1000,
    enabled: target === "series",
  });
  const libraryNames = libraries
    ? new Map(libraries.map((l) => [l.id, l.name]))
    : undefined;

  if (target === "series") {
    const { ratings, rest } = extractRatingLeaves(
      condition as SeriesCondition | undefined | null,
    );
    const state = conditionToSeriesFilterState(rest);
    const ratingRows = ratings.map((leaf) => (
      <RatingRow key={leaf.field} leaf={leaf} />
    ));

    // The non-rating remainder is outside the chip grammar. Still show the
    // ratings we could read, then say the rest needs the advanced page.
    if (!state) {
      return (
        <Stack gap={6}>
          {ratingRows}
          <AdvancedNotice />
        </Stack>
      );
    }

    if (!hasAnyActive(state)) {
      if (ratings.length > 0) return <Stack gap={6}>{ratingRows}</Stack>;
      return (
        <Text size="xs" c="dimmed">
          {emptyLabel}
        </Text>
      );
    }

    return (
      <Stack gap={6}>
        {ratingRows}
        <SeriesSummary state={state} libraryNames={libraryNames} />
      </Stack>
    );
  }

  if (target === "books") {
    const state = conditionToBookFilterState(
      condition as BookCondition | undefined | null,
    );
    if (!state) {
      return <AdvancedNotice />;
    }
    if (!hasAnyActive(state)) {
      return (
        <Text size="xs" c="dimmed">
          {emptyLabel}
        </Text>
      );
    }
    return <BookSummary state={state} />;
  }

  return <AdvancedNotice />;
}

export interface PresetConditionSummaryProps {
  preset: FilterPresetDto;
}

/** [`ConditionSummary`] for a saved preset, reading the target off the preset. */
export function PresetConditionSummary({
  preset,
}: PresetConditionSummaryProps) {
  // A target outside the two known grammars can't be summarized at all, so it
  // falls through to the advanced notice rather than being parsed as series.
  if (preset.target !== "series" && preset.target !== "books") {
    return <AdvancedNotice />;
  }
  return (
    <ConditionSummary
      condition={preset.condition as unknown}
      target={preset.target}
    />
  );
}

function AdvancedNotice() {
  return (
    <Text size="xs" c="dimmed" fs="italic">
      Advanced filter. Open this preset in the advanced search page to see the
      full condition.
    </Text>
  );
}
