import {
  ActionIcon,
  Box,
  CloseButton,
  Group,
  SegmentedControl,
  Stack,
  Text,
  TextInput,
  Tooltip,
} from "@mantine/core";
import { IconSearch, IconX } from "@tabler/icons-react";
import { useMemo, useState } from "react";
import type { FilterGroupState, FilterMode, TriState } from "@/types";
import classes from "./FilterGroup.module.css";
import { TriStateChip, type TriStateChipVariant } from "./TriStateChip";

interface FilterOption {
  value: string;
  label: string;
  count?: number;
}

interface FilterGroupProps {
  /** Group title (e.g., "Genres", "Tags") */
  title: string;
  /** Available options to filter by */
  options: FilterOption[];
  /** Current filter state */
  state: FilterGroupState;
  /** Callback when a value's state changes */
  onValueChange: (value: string, state: TriState) => void;
  /** Callback when mode changes */
  onModeChange: (mode: FilterMode) => void;
  /** Callback to clear all values in this group */
  onClear?: () => void;
  /** Whether to show the mode toggle (default: true) */
  showModeToggle?: boolean;
  /** Whether the group is disabled */
  disabled?: boolean;
  /**
   * Chip variant applied to every option in the group. Defaults to
   * `metadata` so existing call sites (Genres, Tags) keep the square
   * radius / no-leading-slot look. See `TriStateChip` for the per-variant
   * shape language.
   */
  variant?: TriStateChipVariant;
  /**
   * When true, renders a search input above the chip list that filters
   * options by case-insensitive substring match against the label.
   * Useful for long lists (genres, tags) where scrolling becomes painful,
   * especially on mobile.
   */
  searchable?: boolean;
}

/**
 * A filter group component that displays a collection of tri-state chips
 * with an optional mode toggle (All/Any).
 *
 * Usage:
 * - "All selected" (allOf): All included values must match
 * - "Any selected" (anyOf): Any included value can match
 * - Excluded values are always AND-ed (must NOT have any of them)
 */
export function FilterGroup({
  title,
  options,
  state,
  onValueChange,
  onModeChange,
  onClear,
  showModeToggle = true,
  disabled = false,
  variant = "metadata",
  searchable = false,
}: FilterGroupProps) {
  const [query, setQuery] = useState("");

  // Get the current state for a value
  const getValueState = (value: string): TriState => {
    return state.values.get(value) || "neutral";
  };

  // Check if this group has any active filters
  const hasActiveFilters = state.values.size > 0;

  const trimmedQuery = query.trim().toLowerCase();
  const visibleOptions = useMemo(() => {
    if (!searchable || trimmedQuery === "") return options;
    return options.filter((option) =>
      option.label.toLowerCase().includes(trimmedQuery),
    );
  }, [options, searchable, trimmedQuery]);

  return (
    <Stack gap={6} className={classes.container}>
      <Group justify="space-between" align="center" gap="xs">
        <Group gap={6}>
          <Text size="xs" fw={600} c="dimmed">
            {title}
          </Text>
          {hasActiveFilters && onClear && (
            <Tooltip label={`Clear ${title.toLowerCase()}`} position="right">
              <ActionIcon
                size="xs"
                variant="subtle"
                color="gray"
                onClick={onClear}
                disabled={disabled}
                aria-label={`Clear ${title.toLowerCase()} filters`}
              >
                <IconX size={12} />
              </ActionIcon>
            </Tooltip>
          )}
        </Group>
        {showModeToggle && (
          <SegmentedControl
            size="xs"
            value={state.mode}
            onChange={(value) => onModeChange(value as FilterMode)}
            disabled={disabled}
            data={[
              { label: "All", value: "allOf" },
              { label: "Any", value: "anyOf" },
            ]}
            className={classes.modeToggle}
            transitionDuration={200}
            transitionTimingFunction="cubic-bezier(0.32, 0.72, 0, 1)"
          />
        )}
      </Group>

      {searchable && options.length > 0 && (
        <TextInput
          size="xs"
          placeholder={`Search ${title.toLowerCase()}...`}
          value={query}
          onChange={(event) => setQuery(event.currentTarget.value)}
          leftSection={<IconSearch size={12} />}
          rightSection={
            query ? (
              <CloseButton
                size="xs"
                onClick={() => setQuery("")}
                aria-label={`Clear ${title.toLowerCase()} search`}
              />
            ) : null
          }
          disabled={disabled}
          aria-label={`Search ${title.toLowerCase()}`}
        />
      )}

      <Box className={classes.chipsContainer}>
        <Group gap={6} wrap="wrap">
          {visibleOptions.map((option) => (
            <TriStateChip
              key={option.value}
              label={option.label}
              state={getValueState(option.value)}
              onChange={(newState) => onValueChange(option.value, newState)}
              count={option.count}
              disabled={disabled}
              variant={variant}
              decorationKey={option.value}
            />
          ))}
        </Group>
      </Box>

      {options.length === 0 && (
        <Text size="sm" c="dimmed" fs="italic">
          No options available
        </Text>
      )}

      {options.length > 0 && visibleOptions.length === 0 && (
        <Text size="sm" c="dimmed" fs="italic">
          No matches for "{query.trim()}"
        </Text>
      )}
    </Stack>
  );
}
