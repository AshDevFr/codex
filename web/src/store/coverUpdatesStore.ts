import { create } from "zustand";

interface CoverUpdatesState {
	/**
	 * Map of entity ID to the timestamp when their cover was last updated.
	 * Used for cache-busting image URLs when covers are regenerated.
	 */
	updates: Record<string, number>;

	/**
	 * Record a cover update for an entity.
	 * @param entityId The ID of the book or series whose cover was updated
	 */
	recordCoverUpdate: (entityId: string) => void;

	/**
	 * Get the cache-busting timestamp for an entity's cover.
	 * Returns undefined if no update has been recorded.
	 * @param entityId The ID of the book or series
	 */
	getCoverTimestamp: (entityId: string) => number | undefined;
}

export const useCoverUpdatesStore = create<CoverUpdatesState>()((set, get) => ({
	updates: {},

	recordCoverUpdate: (entityId: string) => {
		set((state) => ({
			updates: {
				...state.updates,
				[entityId]: Date.now(),
			},
		}));
	},

	getCoverTimestamp: (entityId: string) => {
		return get().updates[entityId];
	},
}));
