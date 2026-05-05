import { notifications } from "@mantine/notifications";
import { useQueryClient } from "@tanstack/react-query";
import { useEffect, useState } from "react";
import { eventsApi } from "@/api/events";
import { useAuthStore } from "@/store/authStore";
import { useCoverUpdatesStore } from "@/store/coverUpdatesStore";
import { useReleaseAnnouncementsStore } from "@/store/releaseAnnouncementsStore";
import { useUserPreferencesStore } from "@/store/userPreferencesStore";
import type { EntityChangeEvent } from "@/types";
import { createDevLog } from "@/utils/devLog";

type ConnectionState = "connecting" | "connected" | "disconnected" | "failed";

const log = createDevLog("[SSE]");

/** Best-effort decode of a JSON-array string (settings + user_preferences
 * values are stored as JSON-encoded strings). Non-string entries and parse
 * failures collapse to an empty list. */
function parseStringArray(value: string | undefined | null): string[] {
  if (!value) return [];
  try {
    const parsed = JSON.parse(value);
    return Array.isArray(parsed)
      ? parsed.filter((v): v is string => typeof v === "string")
      : [];
  } catch {
    return [];
  }
}

/**
 * Decide whether a `release_announced` event should bump the badge / surface
 * a toast for the current user.
 *
 * Three filters apply (in order):
 *   1. Per-user mute (user_preferences) — drops the event for muted series.
 *   2. Server-wide language allowlist — empty = let everything through.
 *   3. Server-wide plugin allowlist — empty = let everything through.
 *
 * Pure helper, exported only for testing.
 */
export function shouldNotifyRelease(params: {
  seriesId: string;
  pluginId: string;
  language: string;
  notifyLanguagesValue: string | undefined | null;
  notifyPluginsValue: string | undefined | null;
  mutedSeriesIds: readonly string[];
}): boolean {
  if (params.mutedSeriesIds.includes(params.seriesId)) return false;

  const allowedLanguages = parseStringArray(params.notifyLanguagesValue).map(
    (l) => l.toLowerCase(),
  );
  if (
    allowedLanguages.length > 0 &&
    !allowedLanguages.includes(params.language.toLowerCase())
  ) {
    return false;
  }

  const allowedPlugins = parseStringArray(params.notifyPluginsValue);
  if (allowedPlugins.length > 0 && !allowedPlugins.includes(params.pluginId)) {
    return false;
  }

  return true;
}

/**
 * React hook that subscribes to entity change events and automatically
 * invalidates relevant React Query caches when entities are created,
 * updated, or deleted.
 *
 * This provides real-time updates across the application without manual refreshes.
 *
 * @example
 * ```tsx
 * function App() {
 *   useEntityEvents(); // Subscribe to all entity changes
 *   return <RouterProvider router={router} />;
 * }
 * ```
 */
export function useEntityEvents() {
  const queryClient = useQueryClient();
  const { isAuthenticated } = useAuthStore();
  const [connectionState, setConnectionState] =
    useState<ConnectionState>("disconnected");

  useEffect(() => {
    if (!isAuthenticated) {
      log("Not authenticated, skipping subscription");
      return;
    }

    const unsubscribe = eventsApi.subscribeToEntityEvents(
      (event: EntityChangeEvent) => {
        handleEntityEvent(event, queryClient);
      },
      (error: Error) => {
        console.error("[SSE] Connection error:", error);
      },
      (state) => {
        log("Connection state:", state);
        setConnectionState(state as ConnectionState);
      },
    );

    return () => {
      unsubscribe();
    };
  }, [queryClient, isAuthenticated]);

  return {
    connectionState,
  };
}

/**
 * Handle entity change events and invalidate appropriate query caches
 */
function handleEntityEvent(
  event: EntityChangeEvent,
  queryClient: ReturnType<typeof useQueryClient>,
) {
  log("Received entity event:", event.type, event);

  // Handle events using the discriminated union type field
  switch (event.type) {
    case "book_created":
    case "book_updated":
    case "book_deleted": {
      // Invalidate book queries - use "all" to ensure Recommended section updates
      // even when user switches between tabs
      queryClient.invalidateQueries({
        queryKey: ["books"],
      });

      // Invalidate specific book if it's an update
      if (event.type === "book_updated") {
        queryClient.invalidateQueries({
          queryKey: ["books", event.bookId],
        });
      }

      // Invalidate library queries
      if (event.libraryId) {
        queryClient.invalidateQueries({
          queryKey: ["libraries", event.libraryId],
        });

        // Invalidate series in this library
        queryClient.invalidateQueries({
          queryKey: ["series"],
        });
      }
      break;
    }

    case "series_created":
    case "series_updated":
    case "series_deleted":
    case "series_bulk_purged":
    case "series_metadata_updated": {
      // Invalidate series queries - use default to ensure Recommended section updates
      queryClient.invalidateQueries({
        queryKey: ["series"],
      });

      // Invalidate specific series if it's an update
      if (
        event.type === "series_updated" ||
        event.type === "series_metadata_updated"
      ) {
        queryClient.invalidateQueries({
          queryKey: ["series", event.seriesId],
        });
        // For metadata updates, also refetch active queries to immediately update the UI
        if (event.type === "series_metadata_updated") {
          queryClient.refetchQueries({
            queryKey: ["series", event.seriesId],
            type: "active",
          });
        }
      }

      // Invalidate library queries
      if (event.libraryId) {
        queryClient.invalidateQueries({
          queryKey: ["libraries", event.libraryId],
        });
      }
      break;
    }

    case "cover_updated": {
      // Record the cover update for cache-busting image URLs
      // This is needed because query invalidation only refetches JSON data,
      // not images. The timestamp is used as a query param to force image reload.
      useCoverUpdatesStore.getState().recordCoverUpdate(event.entityId);

      const timestamp = useCoverUpdatesStore
        .getState()
        .getCoverTimestamp(event.entityId);
      log(
        `Cover updated for ${event.entityType} ${event.entityId}, cache-bust timestamp: ${timestamp}`,
      );

      if (event.entityType === "book") {
        // Invalidate the specific book query
        queryClient.invalidateQueries({
          queryKey: ["books", event.entityId],
        });
        // Invalidate all book list queries (marks them as stale)
        queryClient.invalidateQueries({
          queryKey: ["books"],
        });
        // Force immediate refetch of active queries to trigger component re-render
        // This ensures MediaCard components pick up the new cache-busting timestamp
        queryClient.refetchQueries({
          queryKey: ["books"],
          type: "active",
        });
      } else if (event.entityType === "series") {
        // Invalidate the specific series query
        queryClient.invalidateQueries({
          queryKey: ["series", event.entityId],
        });
        // Invalidate all series list queries (marks them as stale)
        queryClient.invalidateQueries({
          queryKey: ["series"],
        });
        // Force immediate refetch of active queries to trigger component re-render
        // This ensures MediaCard components pick up the new cache-busting timestamp
        queryClient.refetchQueries({
          queryKey: ["series"],
          type: "active",
        });
      }
      break;
    }

    case "library_updated":
    case "library_deleted": {
      // Invalidate library queries
      queryClient.invalidateQueries({
        queryKey: ["libraries"],
      });
      // Invalidate both query key patterns used in the codebase
      queryClient.invalidateQueries({
        queryKey: ["libraries", event.libraryId],
      });
      queryClient.invalidateQueries({
        queryKey: ["library", event.libraryId],
      });
      // When a library is deleted, also invalidate all books and series queries
      // since they may contain data from the deleted library
      if (event.type === "library_deleted") {
        queryClient.invalidateQueries({
          queryKey: ["books"],
        });
        queryClient.invalidateQueries({
          queryKey: ["series"],
        });
      }
      break;
    }

    case "plugin_created":
    case "plugin_updated":
    case "plugin_enabled":
    case "plugin_disabled":
    case "plugin_deleted": {
      // Invalidate plugin list queries
      queryClient.invalidateQueries({
        queryKey: ["plugins"],
      });
      // Force immediate refetch of active plugin-actions queries
      // This ensures the sidebar and other components see the changes immediately
      queryClient.refetchQueries({
        queryKey: ["plugin-actions"],
        type: "active",
      });
      break;
    }

    case "release_announced": {
      // Snapshot the latest filter state synchronously inside the SSE
      // callback so the predicate sees fresh data on every event.
      //
      // Server-wide allowlists live in React Query cache (loaded by the
      // settings page); per-user mutes live in the userPreferences store
      // (auto-loaded + persisted to localStorage with debounced sync).
      //
      // The query keys here MUST match what the settings page uses — kept
      // in sync explicitly so a typo doesn't silently bypass filtering.
      const notifyLanguagesSetting = queryClient.getQueryData<{
        value?: string;
      }>(["admin-setting", "release_tracking.notify_languages"]);
      const notifyPluginsSetting = queryClient.getQueryData<{
        value?: string;
      }>(["admin-setting", "release_tracking.notify_plugins"]);
      const mutedSeriesIds = useUserPreferencesStore
        .getState()
        .getPreference("release_tracking.muted_series_ids");
      if (
        !shouldNotifyRelease({
          seriesId: event.seriesId,
          pluginId: event.pluginId,
          language: event.language ?? "",
          notifyLanguagesValue: notifyLanguagesSetting?.value,
          notifyPluginsValue: notifyPluginsSetting?.value,
          mutedSeriesIds,
        })
      ) {
        break;
      }
      useReleaseAnnouncementsStore.getState().bump();

      // Refresh inbox + per-series ledger views in case the user is
      // watching them.
      queryClient.invalidateQueries({ queryKey: ["releases"] });
      queryClient.invalidateQueries({
        queryKey: ["series", event.seriesId, "releases"],
      });
      // Refresh the series tracking row so the Behind-by-N badge can
      // pick up the latest_known_* high-water mark advance.
      queryClient.invalidateQueries({
        queryKey: ["series", event.seriesId, "tracking"],
      });
      // Refresh the full series so localMaxChapter / upstream gap props
      // recompute against the latest state.
      queryClient.invalidateQueries({
        queryKey: ["series", event.seriesId, "full"],
      });

      // Surface a low-priority toast. Toast text uses chapter or volume
      // when the source provided one; falls back to a neutral message.
      const label =
        event.chapter !== null && event.chapter !== undefined
          ? `Ch ${event.chapter}`
          : event.volume !== null && event.volume !== undefined
            ? `Vol ${event.volume}`
            : "New release";
      notifications.show({
        id: `release-${event.ledgerId}`,
        title: "New release",
        message: `${label} from ${event.pluginId}`,
        color: "orange",
      });
      break;
    }

    case "release_source_polled": {
      // A release source's poll task finished; refresh the Release tracking
      // settings list so users see updated last_polled_at / last_summary
      // / status without manually reloading. Cheap: one query invalidate.
      queryClient.invalidateQueries({ queryKey: ["release-sources"] });
      break;
    }

    default:
      log("Unknown event type:", event);
  }
}
