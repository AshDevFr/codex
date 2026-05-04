import { create } from "zustand";

/**
 * Releases nav-badge counter.
 *
 * Notification *filters* (server-wide language + plugin allowlists, per-user
 * mute list) used to live here too, but they belong on durable storage
 * (settings + user_preferences) so they survive page reloads. This store now
 * just tracks the in-session "unseen" badge count.
 *
 * The `shouldNotify` decision is made inside the SSE handler in
 * `useEntityEvents` by snapshotting the latest filter values from the query
 * cache; see that file for the predicate.
 */
interface ReleaseAnnouncementsState {
  /** Number of unseen `release_announced` events since the user last visited /releases. */
  unseenCount: number;

  /** Increment the badge counter (called by the SSE handler when shouldNotify passes). */
  bump: () => void;
  /** Reset the badge counter (called when the user visits /releases). */
  reset: () => void;
}

export const useReleaseAnnouncementsStore = create<ReleaseAnnouncementsState>()(
  (set) => ({
    unseenCount: 0,
    bump: () => set((state) => ({ unseenCount: state.unseenCount + 1 })),
    reset: () => set({ unseenCount: 0 }),
  }),
);
