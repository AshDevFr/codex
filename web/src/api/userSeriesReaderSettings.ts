import type { components } from "@/types/api.generated";
import { api } from "./client";

/**
 * A user's content-setting overrides for one series.
 *
 * Only settings that describe the book are here. Reading direction is a fact
 * about how the file was made, so a reader's correction has to follow them
 * between devices. Settings that describe the screen (fit mode, page layout,
 * background) stay in this client's own storage.
 *
 * Sparse: an absent key means the setting is inherited, for reading direction
 * from the series metadata and then the library default.
 */
export type SeriesReaderSettings =
  components["schemas"]["SeriesReaderSettingsResponse"];

/**
 * A partial update. A key set to `null` clears that override so the setting
 * inherits again; a key left out is not touched.
 */
export type PatchSeriesReaderSettings =
  components["schemas"]["PatchSeriesReaderSettingsRequest"];

export const userSeriesReaderSettingsApi = {
  /** The caller's overrides for one series, `{}` when there are none. */
  get: async (seriesId: string): Promise<SeriesReaderSettings> => {
    const response = await api.get<SeriesReaderSettings>(
      `/user/series/${seriesId}/reader-settings`,
    );
    return response.data;
  },

  /** Merge a partial update and return the resulting overrides. */
  patch: async (
    seriesId: string,
    patch: PatchSeriesReaderSettings,
  ): Promise<SeriesReaderSettings> => {
    const response = await api.patch<SeriesReaderSettings>(
      `/user/series/${seriesId}/reader-settings`,
      patch,
    );
    return response.data;
  },

  /** Drop every override for this series at once. */
  remove: async (seriesId: string): Promise<void> => {
    await api.delete(`/user/series/${seriesId}/reader-settings`);
  },
};
