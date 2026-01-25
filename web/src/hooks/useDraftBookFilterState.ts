import { useQueryClient } from "@tanstack/react-query";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useSearchParams } from "react-router-dom";
import {
	BOOK_FILTER_PARAM_KEYS,
	type BookFilterState,
	countActiveFilters,
	createEmptyBookFilterState,
	type FilterGroupState,
	type FilterMode,
	parseBookFilters,
	serializeBookFilters,
	type TriState,
} from "@/types";

interface UseDraftBookFilterStateReturn {
	// Draft filter state (local, not yet applied)
	draftFilters: BookFilterState;

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

	// Bulk actions on draft
	clearAllDraft: () => void;
	clearGroupDraft: (group: keyof Omit<BookFilterState, "hasError">) => void;
	clearAllAndApply: () => void;

	// Commit/discard actions
	applyFilters: () => void;
	discardChanges: () => void;

	// Computed values (based on draft)
	hasActiveFilters: boolean;
	activeFilterCount: number;
	activeFiltersByGroup: {
		genres: number;
		tags: number;
		readStatus: number;
		hasError: number;
	};

	// Track if there are uncommitted changes
	hasChanges: boolean;
}

/**
 * Deep clone a BookFilterState (Maps need special handling)
 */
function cloneBookFilterState(state: BookFilterState): BookFilterState {
	return {
		genres: { mode: state.genres.mode, values: new Map(state.genres.values) },
		tags: { mode: state.tags.mode, values: new Map(state.tags.values) },
		readStatus: {
			mode: state.readStatus.mode,
			values: new Map(state.readStatus.values),
		},
		hasError: state.hasError,
	};
}

/**
 * Compare two book filter states for equality
 */
function bookFilterStatesEqual(
	a: BookFilterState,
	b: BookFilterState,
): boolean {
	const groups: (keyof Omit<BookFilterState, "hasError">)[] = [
		"genres",
		"tags",
		"readStatus",
	];

	for (const group of groups) {
		if (a[group].mode !== b[group].mode) return false;
		if (a[group].values.size !== b[group].values.size) return false;
		for (const [key, value] of a[group].values) {
			if (b[group].values.get(key) !== value) return false;
		}
	}

	if (a.hasError !== b.hasError) return false;

	return true;
}

/**
 * Hook for managing draft book filter state with explicit apply/discard.
 *
 * Changes are kept in local state until explicitly applied to the URL.
 * Discarding reverts to the current URL state.
 */
export function useDraftBookFilterState(): UseDraftBookFilterStateReturn {
	const [searchParams, setSearchParams] = useSearchParams();
	const queryClient = useQueryClient();

	// Parse committed filter state from URL
	const committedFilters = useMemo(
		() => parseBookFilters(searchParams),
		[searchParams],
	);

	// Local draft state - initialized from URL
	const [draftFilters, setDraftFilters] = useState<BookFilterState>(() =>
		cloneBookFilterState(committedFilters),
	);

	// Track the previous committed filters to detect external URL changes
	const prevCommittedFiltersRef = useRef(committedFilters);

	// Sync draft state when URL changes externally (e.g., Clear all from list header)
	useEffect(() => {
		if (
			!bookFilterStatesEqual(committedFilters, prevCommittedFiltersRef.current)
		) {
			setDraftFilters(cloneBookFilterState(committedFilters));
			prevCommittedFiltersRef.current = committedFilters;
		}
	}, [committedFilters]);

	// Check if draft differs from committed
	const hasChanges = useMemo(
		() => !bookFilterStatesEqual(draftFilters, committedFilters),
		[draftFilters, committedFilters],
	);

	// Helper to update draft state
	const updateDraft = useCallback(
		(updater: (current: BookFilterState) => BookFilterState) => {
			setDraftFilters((current) => updater(current));
		},
		[],
	);

	// Helper to update a single group in draft
	const updateGroup = useCallback(
		(
			group: keyof Omit<BookFilterState, "hasError">,
			updater: (current: FilterGroupState) => FilterGroupState,
		) => {
			updateDraft((current) => ({
				...current,
				[group]: updater(current[group]),
			}));
		},
		[updateDraft],
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
			updateDraft((current) => ({ ...current, hasError: state }));
		},
		[updateDraft],
	);

	// Clear all draft filters
	const clearAllDraft = useCallback(() => {
		setDraftFilters(createEmptyBookFilterState());
	}, []);

	// Clear all filters and apply immediately (for "Clear all" button that closes drawer)
	const clearAllAndApply = useCallback(() => {
		const emptyState = createEmptyBookFilterState();
		setDraftFilters(emptyState);

		// Apply empty filter state to URL (same logic as applyFilters but with empty state)
		const filterParams = serializeBookFilters(emptyState);

		// Merge with existing non-filter params (page, sort, etc.)
		const newParams = new URLSearchParams(searchParams);

		// Remove old filter params
		newParams.delete(BOOK_FILTER_PARAM_KEYS.genres);
		newParams.delete(BOOK_FILTER_PARAM_KEYS.tags);
		newParams.delete(BOOK_FILTER_PARAM_KEYS.readStatus);
		newParams.delete(BOOK_FILTER_PARAM_KEYS.hasError);

		// Add new filter params (will be empty for cleared filters)
		for (const [key, value] of filterParams) {
			newParams.set(key, value);
		}

		// Reset to page 1 when filters change
		newParams.set("page", "1");

		setSearchParams(newParams, { replace: true });

		// Mark books queries as stale so they refetch when the component re-renders
		// with the new URL state. Using refetchType: 'none' prevents immediate refetch
		// which would use stale filter state - instead we let React's re-render trigger it.
		queryClient.invalidateQueries({
			queryKey: ["books", "search"],
			refetchType: "none",
		});
	}, [searchParams, setSearchParams, queryClient]);

	// Clear a specific group in draft
	const clearGroupDraft = useCallback(
		(group: keyof Omit<BookFilterState, "hasError">) => {
			setDraftFilters((current) => ({
				...current,
				[group]: { ...current[group], values: new Map() },
			}));
		},
		[],
	);

	// Apply draft to URL
	const applyFilters = useCallback(() => {
		const filterParams = serializeBookFilters(draftFilters);

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
	}, [draftFilters, searchParams, setSearchParams]);

	// Discard draft and revert to URL state
	const discardChanges = useCallback(() => {
		setDraftFilters(cloneBookFilterState(committedFilters));
	}, [committedFilters]);

	// Computed values (based on draft)
	const activeFiltersByGroup = useMemo(
		() => ({
			genres: countActiveFilters(draftFilters.genres),
			tags: countActiveFilters(draftFilters.tags),
			readStatus: countActiveFilters(draftFilters.readStatus),
			hasError: draftFilters.hasError !== "neutral" ? 1 : 0,
		}),
		[draftFilters],
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
		draftFilters,
		setGenreState,
		setGenreMode,
		setTagState,
		setTagMode,
		setReadStatusState,
		setReadStatusMode,
		setHasErrorState,
		clearAllDraft,
		clearGroupDraft,
		clearAllAndApply,
		applyFilters,
		discardChanges,
		hasActiveFilters,
		activeFilterCount,
		activeFiltersByGroup,
		hasChanges,
	};
}
