import { create } from "zustand";

interface ReleaseAnnouncementsState {
  /** Number of unseen `release_announced` events since the user last visited /releases. */
  unseenCount: number;
  /** Per-series mute list (series IDs whose announcements should be ignored). */
  mutedSeriesIds: Set<string>;
  /** Allowed languages; empty set means "all". Stored lower-case. */
  allowedLanguages: Set<string>;
  /** Allowed plugin IDs; empty set means "all". */
  allowedPlugins: Set<string>;

  /** Increment the badge counter (called by the SSE handler). */
  bump: () => void;
  /** Reset the badge counter (called when the user visits /releases). */
  reset: () => void;
  /** Toggle a per-series mute. */
  toggleMute: (seriesId: string) => void;
  /** Replace the language allowlist. */
  setAllowedLanguages: (languages: string[]) => void;
  /** Replace the plugin allowlist. */
  setAllowedPlugins: (plugins: string[]) => void;
  /**
   * Decide whether an incoming event should bump the badge or surface as a
   * toast. Pure function, exposed so the SSE handler and tests can share it.
   */
  shouldNotify: (params: {
    seriesId: string;
    pluginId: string;
    language: string;
  }) => boolean;
}

export const useReleaseAnnouncementsStore = create<ReleaseAnnouncementsState>()(
  (set, get) => ({
    unseenCount: 0,
    mutedSeriesIds: new Set<string>(),
    allowedLanguages: new Set<string>(),
    allowedPlugins: new Set<string>(),

    bump: () => set((state) => ({ unseenCount: state.unseenCount + 1 })),
    reset: () => set({ unseenCount: 0 }),

    toggleMute: (seriesId) =>
      set((state) => {
        const next = new Set(state.mutedSeriesIds);
        if (next.has(seriesId)) {
          next.delete(seriesId);
        } else {
          next.add(seriesId);
        }
        return { mutedSeriesIds: next };
      }),

    setAllowedLanguages: (languages) =>
      set({
        allowedLanguages: new Set(languages.map((l) => l.toLowerCase())),
      }),

    setAllowedPlugins: (plugins) => set({ allowedPlugins: new Set(plugins) }),

    shouldNotify: ({ seriesId, pluginId, language }) => {
      const { mutedSeriesIds, allowedLanguages, allowedPlugins } = get();
      if (mutedSeriesIds.has(seriesId)) return false;
      if (
        allowedLanguages.size > 0 &&
        !allowedLanguages.has(language.toLowerCase())
      ) {
        return false;
      }
      if (allowedPlugins.size > 0 && !allowedPlugins.has(pluginId)) {
        return false;
      }
      return true;
    },
  }),
);
