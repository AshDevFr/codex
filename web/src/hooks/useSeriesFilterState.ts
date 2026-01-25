import { useCallback, useMemo } from "react";
import { useSearchParams } from "react-router-dom";
import {
	countActiveFilters,
	createEmptySeriesFilterState,
	type FilterGroupState,
	type FilterMode,
	parseSeriesFilters,
	type SeriesCondition,
	type SeriesFilterState,
	serializeSeriesFilters,
	seriesFilterStateToCondition,
	type TriState,
} from "@/types";

interface UseSeriesFilterStateReturn {
	// Current filter state (parsed from URL)
	filters: SeriesFilterState;

	// Actions for genre filters
	setGenreState: (value: string, state: TriState) => void;
	setGenreMode: (mode: FilterMode) => void;

	// Actions for tag filters
	setTagState: (value: string, state: TriState) => void;
	setTagMode: (mode: FilterMode) => void;

	// Actions for status filters
	setStatusState: (value: string, state: TriState) => void;
	setStatusMode: (mode: FilterMode) => void;

	// Actions for read status filters
	setReadStatusState: (value: string, state: TriState) => void;
	setReadStatusMode: (mode: FilterMode) => void;

	// Actions for publisher filters
	setPublisherState: (value: string, state: TriState) => void;
	setPublisherMode: (mode: FilterMode) => void;

	// Actions for language filters
	setLanguageState: (value: string, state: TriState) => void;
	setLanguageMode: (mode: FilterMode) => void;

	// Actions for sharing tag filters
	setSharingTagState: (value: string, state: TriState) => void;
	setSharingTagMode: (mode: FilterMode) => void;

	// Bulk actions
	clearAll: () => void;
	clearGroup: (group: keyof SeriesFilterState) => void;

	// Computed values
	hasActiveFilters: boolean;
	activeFilterCount: number;
	activeFiltersByGroup: Record<keyof SeriesFilterState, number>;

	// API-ready condition
	condition: SeriesCondition | undefined;
}

/**
 * Hook for managing filter state with URL synchronization.
 *
 * Filter state is stored in URL search params for shareability and bookmarking.
 * Changes to filters update the URL, which triggers a re-render with new state.
 */
export function useSeriesFilterState(): UseSeriesFilterStateReturn {
	const [searchParams, setSearchParams] = useSearchParams();

	// Parse current filter state from URL
	const filters = useMemo(
		() => parseSeriesFilters(searchParams),
		[searchParams],
	);

	// Convert to API condition
	const condition = useMemo(
		() => seriesFilterStateToCondition(filters),
		[filters],
	);

	// Helper to update URL with new filter state
	const updateFilters = useCallback(
		(newFilters: SeriesFilterState) => {
			const filterParams = serializeSeriesFilters(newFilters);

			// Merge with existing non-filter params (page, sort, etc.)
			const newParams = new URLSearchParams(searchParams);

			// Remove old filter params
			newParams.delete("gf");
			newParams.delete("tf");
			newParams.delete("sf");
			newParams.delete("rf");
			newParams.delete("pf");
			newParams.delete("lf");
			newParams.delete("stf");

			// Add new filter params
			for (const [key, value] of filterParams) {
				newParams.set(key, value);
			}

			// Reset to page 1 when filters change
			newParams.set("page", "1");

			setSearchParams(newParams, { replace: true });
		},
		[searchParams, setSearchParams],
	);

	// Helper to update a single group
	const updateGroup = useCallback(
		(
			group: keyof SeriesFilterState,
			updater: (current: FilterGroupState) => FilterGroupState,
		) => {
			const newFilters = { ...filters };
			newFilters[group] = updater(filters[group]);
			updateFilters(newFilters);
		},
		[filters, updateFilters],
	);

	// Genre actions
	const setGenreState = useCallback(
		(value: string, state: TriState) => {
			updateGroup("genres", (current) => {
				const newValues = new Map(current.values);
				if (state === "neutral") {
					newValues.delete(value);
				} else {
					newValues.set(value, state);
				}
				return { ...current, values: newValues };
			});
		},
		[updateGroup],
	);

	const setGenreMode = useCallback(
		(mode: FilterMode) => {
			updateGroup("genres", (current) => ({ ...current, mode }));
		},
		[updateGroup],
	);

	// Tag actions
	const setTagState = useCallback(
		(value: string, state: TriState) => {
			updateGroup("tags", (current) => {
				const newValues = new Map(current.values);
				if (state === "neutral") {
					newValues.delete(value);
				} else {
					newValues.set(value, state);
				}
				return { ...current, values: newValues };
			});
		},
		[updateGroup],
	);

	const setTagMode = useCallback(
		(mode: FilterMode) => {
			updateGroup("tags", (current) => ({ ...current, mode }));
		},
		[updateGroup],
	);

	// Status actions
	const setStatusState = useCallback(
		(value: string, state: TriState) => {
			updateGroup("status", (current) => {
				const newValues = new Map(current.values);
				if (state === "neutral") {
					newValues.delete(value);
				} else {
					newValues.set(value, state);
				}
				return { ...current, values: newValues };
			});
		},
		[updateGroup],
	);

	const setStatusMode = useCallback(
		(mode: FilterMode) => {
			updateGroup("status", (current) => ({ ...current, mode }));
		},
		[updateGroup],
	);

	// Read status actions
	const setReadStatusState = useCallback(
		(value: string, state: TriState) => {
			updateGroup("readStatus", (current) => {
				const newValues = new Map(current.values);
				if (state === "neutral") {
					newValues.delete(value);
				} else {
					newValues.set(value, state);
				}
				return { ...current, values: newValues };
			});
		},
		[updateGroup],
	);

	const setReadStatusMode = useCallback(
		(mode: FilterMode) => {
			updateGroup("readStatus", (current) => ({ ...current, mode }));
		},
		[updateGroup],
	);

	// Publisher actions
	const setPublisherState = useCallback(
		(value: string, state: TriState) => {
			updateGroup("publisher", (current) => {
				const newValues = new Map(current.values);
				if (state === "neutral") {
					newValues.delete(value);
				} else {
					newValues.set(value, state);
				}
				return { ...current, values: newValues };
			});
		},
		[updateGroup],
	);

	const setPublisherMode = useCallback(
		(mode: FilterMode) => {
			updateGroup("publisher", (current) => ({ ...current, mode }));
		},
		[updateGroup],
	);

	// Language actions
	const setLanguageState = useCallback(
		(value: string, state: TriState) => {
			updateGroup("language", (current) => {
				const newValues = new Map(current.values);
				if (state === "neutral") {
					newValues.delete(value);
				} else {
					newValues.set(value, state);
				}
				return { ...current, values: newValues };
			});
		},
		[updateGroup],
	);

	const setLanguageMode = useCallback(
		(mode: FilterMode) => {
			updateGroup("language", (current) => ({ ...current, mode }));
		},
		[updateGroup],
	);

	// Sharing tag actions
	const setSharingTagState = useCallback(
		(value: string, state: TriState) => {
			updateGroup("sharingTags", (current) => {
				const newValues = new Map(current.values);
				if (state === "neutral") {
					newValues.delete(value);
				} else {
					newValues.set(value, state);
				}
				return { ...current, values: newValues };
			});
		},
		[updateGroup],
	);

	const setSharingTagMode = useCallback(
		(mode: FilterMode) => {
			updateGroup("sharingTags", (current) => ({ ...current, mode }));
		},
		[updateGroup],
	);

	// Clear all filters
	const clearAll = useCallback(() => {
		updateFilters(createEmptySeriesFilterState());
	}, [updateFilters]);

	// Clear a specific group
	const clearGroup = useCallback(
		(group: keyof SeriesFilterState) => {
			updateGroup(group, (current) => ({
				...current,
				values: new Map(),
			}));
		},
		[updateGroup],
	);

	// Computed values
	const activeFiltersByGroup = useMemo(
		() => ({
			genres: countActiveFilters(filters.genres),
			tags: countActiveFilters(filters.tags),
			status: countActiveFilters(filters.status),
			readStatus: countActiveFilters(filters.readStatus),
			publisher: countActiveFilters(filters.publisher),
			language: countActiveFilters(filters.language),
			sharingTags: countActiveFilters(filters.sharingTags),
		}),
		[filters],
	);

	const activeFilterCount = useMemo(
		() =>
			Object.values(activeFiltersByGroup).reduce(
				(sum, count) => sum + count,
				0,
			),
		[activeFiltersByGroup],
	);

	const hasActiveFilters = activeFilterCount > 0;

	return {
		filters,
		setGenreState,
		setGenreMode,
		setTagState,
		setTagMode,
		setStatusState,
		setStatusMode,
		setReadStatusState,
		setReadStatusMode,
		setPublisherState,
		setPublisherMode,
		setLanguageState,
		setLanguageMode,
		setSharingTagState,
		setSharingTagMode,
		clearAll,
		clearGroup,
		hasActiveFilters,
		activeFilterCount,
		activeFiltersByGroup,
		condition,
	};
}
