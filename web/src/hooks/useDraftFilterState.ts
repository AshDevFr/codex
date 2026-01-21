import { useCallback, useMemo, useState } from "react";
import { useSearchParams } from "react-router-dom";
import {
	countActiveFilters,
	createEmptySeriesFilterState,
	type FilterGroupState,
	type FilterMode,
	parseSeriesFilters,
	type SeriesFilterState,
	serializeSeriesFilters,
	type TriState,
} from "@/types";

interface UseDraftFilterStateReturn {
	// Draft filter state (local, not yet applied)
	draftFilters: SeriesFilterState;

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

	// Bulk actions on draft
	clearAllDraft: () => void;
	clearGroupDraft: (group: keyof SeriesFilterState) => void;

	// Commit/discard actions
	applyFilters: () => void;
	discardChanges: () => void;

	// Computed values (based on draft)
	hasActiveFilters: boolean;
	activeFilterCount: number;
	activeFiltersByGroup: Record<keyof SeriesFilterState, number>;

	// Track if there are uncommitted changes
	hasChanges: boolean;
}

/**
 * Deep clone a SeriesFilterState (Maps need special handling)
 */
function cloneFilterState(state: SeriesFilterState): SeriesFilterState {
	return {
		genres: { mode: state.genres.mode, values: new Map(state.genres.values) },
		tags: { mode: state.tags.mode, values: new Map(state.tags.values) },
		status: { mode: state.status.mode, values: new Map(state.status.values) },
		readStatus: {
			mode: state.readStatus.mode,
			values: new Map(state.readStatus.values),
		},
		publisher: {
			mode: state.publisher.mode,
			values: new Map(state.publisher.values),
		},
		language: {
			mode: state.language.mode,
			values: new Map(state.language.values),
		},
		sharingTags: {
			mode: state.sharingTags.mode,
			values: new Map(state.sharingTags.values),
		},
	};
}

/**
 * Compare two filter states for equality
 */
function filterStatesEqual(
	a: SeriesFilterState,
	b: SeriesFilterState,
): boolean {
	const groups: (keyof SeriesFilterState)[] = [
		"genres",
		"tags",
		"status",
		"readStatus",
		"publisher",
		"language",
		"sharingTags",
	];

	for (const group of groups) {
		if (a[group].mode !== b[group].mode) return false;
		if (a[group].values.size !== b[group].values.size) return false;
		for (const [key, value] of a[group].values) {
			if (b[group].values.get(key) !== value) return false;
		}
	}
	return true;
}

/**
 * Hook for managing draft filter state with explicit apply/discard.
 *
 * Changes are kept in local state until explicitly applied to the URL.
 * Discarding reverts to the current URL state.
 */
export function useDraftFilterState(): UseDraftFilterStateReturn {
	const [searchParams, setSearchParams] = useSearchParams();

	// Parse committed filter state from URL
	const committedFilters = useMemo(
		() => parseSeriesFilters(searchParams),
		[searchParams],
	);

	// Local draft state - initialized from URL
	const [draftFilters, setDraftFilters] = useState<SeriesFilterState>(() =>
		cloneFilterState(committedFilters),
	);

	// Check if draft differs from committed
	const hasChanges = useMemo(
		() => !filterStatesEqual(draftFilters, committedFilters),
		[draftFilters, committedFilters],
	);

	// Helper to update draft state
	const updateDraft = useCallback(
		(updater: (current: SeriesFilterState) => SeriesFilterState) => {
			setDraftFilters((current) => updater(current));
		},
		[],
	);

	// Helper to update a single group in draft
	const updateGroup = useCallback(
		(
			group: keyof SeriesFilterState,
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

	// Clear all draft filters
	const clearAllDraft = useCallback(() => {
		setDraftFilters(createEmptySeriesFilterState());
	}, []);

	// Clear a specific group in draft
	const clearGroupDraft = useCallback((group: keyof SeriesFilterState) => {
		setDraftFilters((current) => ({
			...current,
			[group]: { ...current[group], values: new Map() },
		}));
	}, []);

	// Apply draft to URL
	const applyFilters = useCallback(() => {
		const filterParams = serializeSeriesFilters(draftFilters);

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
	}, [draftFilters, searchParams, setSearchParams]);

	// Discard draft and revert to URL state
	const discardChanges = useCallback(() => {
		setDraftFilters(cloneFilterState(committedFilters));
	}, [committedFilters]);

	// Computed values (based on draft)
	const activeFiltersByGroup = useMemo(
		() => ({
			genres: countActiveFilters(draftFilters.genres),
			tags: countActiveFilters(draftFilters.tags),
			status: countActiveFilters(draftFilters.status),
			readStatus: countActiveFilters(draftFilters.readStatus),
			publisher: countActiveFilters(draftFilters.publisher),
			language: countActiveFilters(draftFilters.language),
			sharingTags: countActiveFilters(draftFilters.sharingTags),
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
		clearAllDraft,
		clearGroupDraft,
		applyFilters,
		discardChanges,
		hasActiveFilters,
		activeFilterCount,
		activeFiltersByGroup,
		hasChanges,
	};
}
