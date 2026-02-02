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

  // Add completion chip if active
  if (filters.completion !== "neutral") {
    const isExclude = filters.completion === "exclude";
    allChips.push(
      <Badge
        key="completion"
        variant="filled"
        color={isExclude ? "red" : "blue"}
        size="md"
        className={styles.chip}
        rightSection={
          <ActionIcon
            size="xs"
            variant="transparent"
            color="white"
            onClick={() => setCompletionState("neutral")}
            aria-label="Remove complete filter"
          >
            <IconX size={12} />
          </ActionIcon>
        }
      >
        {isExclude ? "NOT " : ""}Complete
      </Badge>,
    );
  }

  // Add hasExternalSourceId chip if active
  if (filters.hasExternalSourceId !== "neutral") {
    const isExclude = filters.hasExternalSourceId === "exclude";
    allChips.push(
      <Badge
        key="hasExternalSourceId"
        variant="filled"
        color={isExclude ? "red" : "blue"}
        size="md"
        className={styles.chip}
        rightSection={
          <ActionIcon
            size="xs"
            variant="transparent"
            color="white"
            onClick={() => setHasExternalSourceIdState("neutral")}
            aria-label="Remove external ID filter"
          >
            <IconX size={12} />
          </ActionIcon>
        }
      >
        {isExclude ? "NOT " : ""}Has External ID
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
