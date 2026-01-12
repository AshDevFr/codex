import { useCallback, useMemo } from "react";
import { useSearchParams } from "react-router-dom";
import {
	type BookCondition,
	type BookFilterState,
	type FilterGroupState,
	type FilterMode,
	type TriState,
	BOOK_FILTER_PARAM_KEYS,
	bookFilterStateToCondition,
	countActiveFilters,
	countBookActiveFilters,
	createEmptyBookFilterState,
	parseBookFilters,
	serializeBookFilters,
} from "@/types";

interface UseBookFilterStateReturn {
	// Current filter state (parsed from URL)
	filters: BookFilterState;

	// Actions for genre filters
	setGenreState: (value: string, state: TriState) => void;
	setGenreMode: (mode: FilterMode) => void;

	// Actions for tag filters
	setTagState: (value: string, state: TriState) => void;
	setTagMode: (mode: FilterMode) => void;

	// Actions for read status filters
	setReadStatusState: (value: string, state: TriState) => void;
	setReadStatusMode: (mode: FilterMode) => void;

	// Actions for hasError filter
	setHasErrorState: (state: TriState) => void;

	// Bulk actions
	clearAll: () => void;
	clearGroup: (group: keyof Omit<BookFilterState, "hasError">) => void;

	// Computed values
	hasActiveFilters: boolean;
	activeFilterCount: number;
	activeFiltersByGroup: {
		genres: number;
		tags: number;
		readStatus: number;
		hasError: number;
	};

	// API-ready condition
	condition: BookCondition | undefined;
}

/**
 * Hook for managing book filter state with URL synchronization.
 *
 * Filter state is stored in URL search params for shareability and bookmarking.
 * Changes to filters update the URL, which triggers a re-render with new state.
 */
export function useBookFilterState(): UseBookFilterStateReturn {
	const [searchParams, setSearchParams] = useSearchParams();

	// Parse current filter state from URL
	const filters = useMemo(() => parseBookFilters(searchParams), [searchParams]);

	// Convert to API condition
	const condition = useMemo(() => bookFilterStateToCondition(filters), [filters]);

	// Helper to update URL with new filter state
	const updateFilters = useCallback(
		(newFilters: BookFilterState) => {
			const filterParams = serializeBookFilters(newFilters);

			// Merge with existing non-filter params (page, sort, etc.)
			const newParams = new URLSearchParams(searchParams);

			// Remove old filter params
			newParams.delete(BOOK_FILTER_PARAM_KEYS.genres);
			newParams.delete(BOOK_FILTER_PARAM_KEYS.tags);
			newParams.delete(BOOK_FILTER_PARAM_KEYS.readStatus);
			newParams.delete(BOOK_FILTER_PARAM_KEYS.hasError);

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
		(group: keyof Omit<BookFilterState, "hasError">, updater: (current: FilterGroupState) => FilterGroupState) => {
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

	// HasError action
	const setHasErrorState = useCallback(
		(state: TriState) => {
			updateFilters({ ...filters, hasError: state });
		},
		[filters, updateFilters],
	);

	// Clear all filters
	const clearAll = useCallback(() => {
		updateFilters(createEmptyBookFilterState());
	}, [updateFilters]);

	// Clear a specific group
	const clearGroup = useCallback(
		(group: keyof Omit<BookFilterState, "hasError">) => {
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
			readStatus: countActiveFilters(filters.readStatus),
			hasError: filters.hasError !== "neutral" ? 1 : 0,
		}),
		[filters],
	);

	const activeFilterCount = useMemo(() => countBookActiveFilters(filters), [filters]);

	const hasActiveFilters = activeFilterCount > 0;

	return {
		filters,
		setGenreState,
		setGenreMode,
		setTagState,
		setTagMode,
		setReadStatusState,
		setReadStatusMode,
		setHasErrorState,
		clearAll,
		clearGroup,
		hasActiveFilters,
		activeFilterCount,
		activeFiltersByGroup,
		condition,
	};
}
