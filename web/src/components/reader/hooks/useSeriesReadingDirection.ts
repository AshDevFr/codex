import { notifications } from "@mantine/notifications";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useCallback } from "react";
import { seriesMetadataApi } from "@/api/seriesMetadata";
import {
  type InheritedFrom,
  userSeriesReaderSettingsApi,
} from "@/api/userSeriesReaderSettings";
import { type ReadingDirection, useReaderStore } from "@/store/readerStore";

export const SERIES_READER_SETTINGS_KEY = "seriesReaderSettings";

/** Query key for one series' content-setting overrides. */
export function seriesReaderSettingsKey(seriesId: string) {
  return [SERIES_READER_SETTINGS_KEY, seriesId] as const;
}

export interface UseSeriesReadingDirectionReturn {
  /** This user's direction for the series, or null when inheriting. */
  userDirection: ReadingDirection | null;

  /**
   * The direction this user would get with no override, or null when no layer
   * holds one. Resolved server-side, because a book response carries the
   * direction already resolved and so hides the layers beneath it.
   */
  inheritedDirection: ReadingDirection | null;

  /** Which layer `inheritedDirection` came from, for naming it in the UI. */
  inheritedSource: InheritedFrom | null;

  /** Save a direction for this series, for this user only. */
  setUserDirection: (direction: ReadingDirection) => void;

  /** Stop overriding, so the series metadata or library default applies. */
  clearUserDirection: () => void;

  /**
   * Write a direction into the series metadata, where every user sees it, and
   * lock the field. Requires `series:write`; callers gate the affordance.
   */
  promoteToSeries: (direction: ReadingDirection) => void;

  /** Whether the promote request is in flight. */
  isPromoting: boolean;
}

/**
 * A user's reading direction for one series.
 *
 * Direction describes how a book was made rather than how one screen is used,
 * so it is stored server-side and follows the reader between devices. It also
 * resolves server-side, which is why the value on a book response is already
 * this user's: the client does not merge the layers itself.
 *
 * Changing it here never touches the series metadata. That is a separate,
 * permissioned act, because it changes what every user of the server sees.
 */
export function useSeriesReadingDirection(
  seriesId: string | null | undefined,
): UseSeriesReadingDirectionReturn {
  const queryClient = useQueryClient();
  const setReadingDirectionOverride = useReaderStore(
    (state) => state.setReadingDirectionOverride,
  );

  const { data } = useQuery({
    queryKey: seriesReaderSettingsKey(seriesId ?? ""),
    queryFn: () => userSeriesReaderSettingsApi.get(seriesId as string),
    enabled: Boolean(seriesId),
  });

  const invalidateBooks = useCallback(() => {
    // Direction is resolved server-side, so changing it makes every cached
    // book response for this reader stale.
    queryClient.invalidateQueries({ queryKey: ["books"] });
  }, [queryClient]);

  const saveMutation = useMutation({
    mutationFn: (direction: ReadingDirection | null) =>
      userSeriesReaderSettingsApi.patch(seriesId as string, {
        readingDirection: direction,
      }),
    onSuccess: (updated) => {
      if (seriesId) {
        queryClient.setQueryData(seriesReaderSettingsKey(seriesId), updated);
      }
      invalidateBooks();
    },
    onError: (error: Error) =>
      notifications.show({
        title: "Could not save the reading direction",
        message: error.message || "Unknown error",
        color: "red",
      }),
  });

  const promoteMutation = useMutation({
    mutationFn: async (direction: ReadingDirection) => {
      if (!seriesId) return;
      await seriesMetadataApi.patchMetadata(seriesId, {
        readingDirection: direction,
      });
      // Locking is right here precisely because the change was deliberate: it
      // protects a considered correction from the next metadata apply. It used
      // to fire on any toggle of the reader control, which locked the field for
      // everyone by accident.
      await seriesMetadataApi.updateLocks(seriesId, { readingDirection: true });
    },
    onSuccess: () => {
      if (!seriesId) return;
      queryClient.invalidateQueries({
        queryKey: ["seriesMetadata", seriesId],
      });
      // The personal override would now shadow the value just made canonical.
      saveMutation.mutate(null);
      notifications.show({
        title: "Saved as the series default",
        message: "Everyone reading this series now gets this direction.",
        color: "green",
      });
    },
    onError: (error: Error) =>
      notifications.show({
        title: "Could not save the series default",
        message: error.message || "Unknown error",
        color: "red",
      }),
  });

  const setUserDirection = useCallback(
    (direction: ReadingDirection) => {
      if (!seriesId) return;
      saveMutation.mutate(direction);
    },
    [seriesId, saveMutation],
  );

  const inheritedDirection =
    (data?.inheritedReadingDirection as ReadingDirection) ?? null;

  const clearUserDirection = useCallback(() => {
    if (!seriesId) return;

    // Apply the inherited value to the open book immediately, rather than only
    // clearing the store. Two reasons it cannot be left to a refetch: the store
    // falls back to this reader's *global* preference, which is a different
    // setting entirely, and ComicReader seeds the direction once per book and
    // will not re-seed the one already on screen. Without this the page would
    // keep rendering the direction that was just dropped.
    setReadingDirectionOverride(inheritedDirection);
    saveMutation.mutate(null);
  }, [seriesId, saveMutation, setReadingDirectionOverride, inheritedDirection]);

  const promoteToSeries = useCallback(
    (direction: ReadingDirection) => {
      if (!seriesId) return;
      promoteMutation.mutate(direction);
    },
    [seriesId, promoteMutation],
  );

  return {
    userDirection: (data?.readingDirection as ReadingDirection) ?? null,
    inheritedDirection,
    inheritedSource: data?.inheritedReadingDirectionSource ?? null,
    setUserDirection,
    clearUserDirection,
    promoteToSeries,
    isPromoting: promoteMutation.isPending,
  };
}
