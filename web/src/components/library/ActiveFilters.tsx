import { ActionIcon, Badge, Group, Text } from "@mantine/core";
import { IconX } from "@tabler/icons-react";
import { useSeriesFilterState } from "@/hooks/useSeriesFilterState";
import type { FilterGroupState, SeriesFilterState, TriState } from "@/types";
import styles from "./ActiveFilters.module.css";

/**
 * Displays active filter chips with the ability to remove individual filters.
 *
 * Shows chips for each active filter value, grouped by category.
 * Include filters are shown in blue, exclude filters in red.
 * Clicking the X on a chip removes that filter.
 */
export function ActiveFilters() {
  const {
    filters,
    hasActiveFilters,
    setGenreState,
    setTagState,
    setStatusState,
    setReadStatusState,
    setPublisherState,
    setLanguageState,
    setSharingTagState,
    setCompletionState,
    setHasExternalSourceIdState,
    setHasUserRatingState,
    setIsTrackedState,
    clearAll,
  } = useSeriesFilterState();

  if (!hasActiveFilters) {
    return null;
  }

  // Helper to create chips for a filter group
  const renderGroupChips = (
    group: FilterGroupState,
    groupName: keyof SeriesFilterState,
    label: string,
    onRemove: (value: string, state: TriState) => void,
  ) => {
    const chips: React.ReactNode[] = [];

    for (const [value, state] of group.values) {
      if (state === "neutral") continue;

      const isExclude = state === "exclude";
      chips.push(
        <Badge
          key={`${groupName}-${value}`}
          variant="filled"
          color={isExclude ? "red" : "blue"}
          size="md"
          className={styles.chip}
          rightSection={
            <ActionIcon
              size="xs"
              variant="transparent"
              color="white"
              onClick={() => onRemove(value, "neutral")}
              aria-label={`Remove ${value} filter`}
            >
              <IconX size={12} />
            </ActionIcon>
          }
        >
          {isExclude ? "NOT " : ""}
          {label}: {value}
        </Badge>,
      );
    }

    return chips;
  };

  const allChips = [
    ...renderGroupChips(filters.genres, "genres", "Genre", setGenreState),
    ...renderGroupChips(filters.tags, "tags", "Tag", setTagState),
    ...renderGroupChips(filters.status, "status", "Status", setStatusState),
    ...renderGroupChips(
      filters.readStatus,
      "readStatus",
      "Read Status",
      setReadStatusState,
    ),
    ...renderGroupChips(
      filters.publisher,
      "publisher",
      "Publisher",
      setPublisherState,
    ),
    ...renderGroupChips(
      filters.language,
      "language",
      "Language",
      setLanguageState,
    ),
    ...renderGroupChips(
      filters.sharingTags,
      "sharingTags",
      "Sharing Tag",
      setSharingTagState,
    ),
  ];

  // Single-value (TriState) filters. Driven by one descriptor list so the chip
  // display can't drift from the set of filters the panel exposes: adding a new
  // boolean filter here is all it takes for it to appear (and be removable).
  const triStateChips: {
    key: "completion" | "hasExternalSourceId" | "hasUserRating" | "isTracked";
    label: string;
    ariaLabel: string;
    onRemove: () => void;
  }[] = [
    {
      key: "completion",
      label: "Complete",
      ariaLabel: "Remove complete filter",
      onRemove: () => setCompletionState("neutral"),
    },
    {
      key: "hasExternalSourceId",
      label: "Has External ID",
      ariaLabel: "Remove external ID filter",
      onRemove: () => setHasExternalSourceIdState("neutral"),
    },
    {
      key: "hasUserRating",
      label: "Has Rating",
      ariaLabel: "Remove rating filter",
      onRemove: () => setHasUserRatingState("neutral"),
    },
    {
      key: "isTracked",
      label: "Tracked",
      ariaLabel: "Remove tracking filter",
      onRemove: () => setIsTrackedState("neutral"),
    },
  ];

  for (const { key, label, ariaLabel, onRemove } of triStateChips) {
    const state = filters[key];
    if (state === "neutral") continue;

    const isExclude = state === "exclude";
    allChips.push(
      <Badge
        key={key}
        variant="filled"
        color={isExclude ? "red" : "blue"}
        size="md"
        className={styles.chip}
        rightSection={
          <ActionIcon
            size="xs"
            variant="transparent"
            color="white"
            onClick={onRemove}
            aria-label={ariaLabel}
          >
            <IconX size={12} />
          </ActionIcon>
        }
      >
        {isExclude ? "NOT " : ""}
        {label}
      </Badge>,
    );
  }

  return (
    <Group gap="xs" className={styles.container}>
      <Text size="sm" c="dimmed" fw={500}>
        Filters:
      </Text>
      {allChips}
      <Badge
        variant="outline"
        color="gray"
        size="md"
        className={styles.clearButton}
        onClick={clearAll}
        style={{ cursor: "pointer" }}
      >
        Clear all
      </Badge>
    </Group>
  );
}
