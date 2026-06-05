import { Anchor } from "@mantine/core";
import { notifications } from "@mantine/notifications";
import { useQueryClient } from "@tanstack/react-query";
import { throttle } from "es-toolkit";
import { useEffect, useState } from "react";
import { eventsApi } from "@/api/events";
import { navigationService } from "@/services/navigation";
import { useAuthStore } from "@/store/authStore";
import { useCoverUpdatesStore } from "@/store/coverUpdatesStore";
import { useReleaseAnnouncementsStore } from "@/store/releaseAnnouncementsStore";
import { useUserPreferencesStore } from "@/store/userPreferencesStore";
import type { EntityChangeEvent } from "@/types";
import { createDevLog } from "@/utils/devLog";

type ConnectionState = "connecting" | "connected" | "disconnected" | "failed";

const log = createDevLog("[SSE]");

/** Second key segment of every series LIST/grid/home-section query (see
 * SeriesSection / SeriesSection home rows). Detail queries are keyed
 * ["series", <id>, ...] instead, so invalidating these prefixes refreshes the
 * lists without touching open detail tabs. Keep in sync with the query keys in
 * the components that own these lists. */
const SERIES_LIST_SECTIONS = [
  "search",
  "alphabetical-groups",
  "recently-added",
  "recently-updated",
] as const;

/** Second key segment of every book LIST/home-section query (see Recommended
 * section rows + the books grid). Detail queries are ["books", <id>, ...]. */
const BOOKS_LIST_SECTIONS = [
  "search",
  "in-progress",
  "on-deck",
  "recently-added",
  "recently-read",
] as const;

/** Per-series state for the aggregated release toast. Lives at module scope
 * so a burst of `release_announced` events for the same series collapses
 * into a single toast that updates in place rather than spawning N toasts.
 *
 * The set of chapters/volumes accumulates while the toast is visible; the
 * `onClose` handler clears the entry, so the next event after dismissal
 * starts a fresh toast. Exported for testability. */
type ReleaseToastState = {
  chapters: Set<number>;
  volumes: Set<number>;
  pluginId: string;
};
export const releaseToastState = new Map<string, ReleaseToastState>();

/** Cap on the rendered chapter/volume label so a series with dozens of
 * announced releases doesn't blow the toast width out. */
const RELEASE_LABEL_MAX_CHARS = 70;

/** Build the message body for the aggregated release toast. Lists volumes
 * first (typically fewer entries), then chapters; truncates with `…` once
 * the joined string exceeds `RELEASE_LABEL_MAX_CHARS`. Pure helper —
 * exported for testing. */
export function formatReleaseLabel(state: ReleaseToastState): string {
  const sortAsc = (s: Set<number>) => [...s].sort((a, b) => a - b);
  const parts: string[] = [];
  if (state.volumes.size > 0) {
    parts.push(`Vol ${sortAsc(state.volumes).join(", ")}`);
  }
  if (state.chapters.size > 0) {
    parts.push(`Ch ${sortAsc(state.chapters).join(", ")}`);
  }
  let label = parts.length > 0 ? parts.join(" / ") : "New release";
  if (label.length > RELEASE_LABEL_MAX_CHARS) {
    label = `${label.slice(0, RELEASE_LABEL_MAX_CHARS - 1).trimEnd()}…`;
  }
  return label;
}

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

    // Throttle the heavy list invalidations: a burst of entity events (e.g. a
    // scan or bulk thumbnail regen) collapses into at most one series/books
    // list refetch per interval instead of one per event.
    //
    // IMPORTANT: invalidate the specific list/grid/home-section query keys, NOT
    // the bare ["series"] / ["books"] roots. React Query matches by prefix, so
    // ["series"] also matches every open detail query — ["series", id, "full"],
    // "tracking", "aliases", "releases", "covers" — and ["books"] matches
    // ["books", id, ...]. With several detail tabs open during a scan, the bare
    // root turned one list refetch into a full+tracking+aliases+releases refetch
    // wave on every tab. Targeting the section prefixes refreshes the lists
    // (counts, membership) while leaving open detail tabs untouched; a detail
    // that genuinely changed is refreshed by the targeted ["series", id] /
    // ["books", id] invalidation in the per-event branches below.
    const invalidateSeriesList = throttle(() => {
      for (const section of SERIES_LIST_SECTIONS) {
        queryClient.invalidateQueries({ queryKey: ["series", section] });
      }
    }, 1500);
    const invalidateBooksList = throttle(() => {
      for (const section of BOOKS_LIST_SECTIONS) {
        queryClient.invalidateQueries({ queryKey: ["books", section] });
      }
    }, 1500);

    // Coalesce a burst of `release_announced` events (a poll or a source reset
    // can announce hundreds of releases in one wave) into a single refetch
    // wave — the release-tracking equivalent of the list throttle above.
    // Affected series IDs accumulate between flushes; the throttled flush
    // refetches the shared inbox/facets once plus each touched series' ledger
    // and tracking row, then clears the set.
    const pendingReleaseSeriesIds = new Set<string>();
    const flushReleaseInvalidations = throttle(() => {
      // Inbox + facets (keyed ["releases", ...]). One refetch per wave instead
      // of one per event.
      queryClient.invalidateQueries({ queryKey: ["releases"] });
      for (const id of pendingReleaseSeriesIds) {
        // Per-series ledger view + the tracking row that drives the Behind-by
        // badge's moving value (latest_known_*). The heavy ["series", id,
        // "full"] query is intentionally NOT invalidated: a release
        // announcement advances tracking.latestKnownChapter, not the series'
        // localMaxChapter (local books) or upstream gap (metadata), so the
        // badge recomputes from the tracking refetch alone.
        queryClient.invalidateQueries({ queryKey: ["series", id, "releases"] });
        queryClient.invalidateQueries({ queryKey: ["series", id, "tracking"] });
      }
      pendingReleaseSeriesIds.clear();
    }, 1500);

    const listInvalidate: ListInvalidators = {
      series: invalidateSeriesList,
      books: invalidateBooksList,
      release: (seriesId: string) => {
        pendingReleaseSeriesIds.add(seriesId);
        flushReleaseInvalidations();
      },
    };

    const unsubscribe = eventsApi.subscribeToEntityEvents(
      (event: EntityChangeEvent) => {
        handleEntityEvent(event, queryClient, listInvalidate);
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
      // Drop any pending trailing refetch so we don't fire after teardown/logout.
      invalidateSeriesList.cancel();
      invalidateBooksList.cancel();
      flushReleaseInvalidations.cancel();
    };
  }, [queryClient, isAuthenticated]);

  return {
    connectionState,
  };
}

/**
 * Handle entity change events and invalidate appropriate query caches
 */
/** Throttled invalidators for the heavy list queries. A scan/analyze can emit
 * hundreds of entity events; invalidating the (large) series/books list on each
 * one would refetch it hundreds of times. Throttling coalesces a burst into at
 * most one refetch per interval (leading + trailing), so a single event still
 * updates promptly while a sustained stream refreshes ~once/interval. */
type ListInvalidators = {
  series: () => void;
  books: () => void;
  /** Queue a throttled refetch wave for release views touched by a
   * `release_announced` burst (shared inbox/facets + this series' ledger and
   * tracking row). Coalesces a poll/reset storm into ~one wave per interval. */
  release: (seriesId: string) => void;
};

function handleEntityEvent(
  event: EntityChangeEvent,
  queryClient: ReturnType<typeof useQueryClient>,
  listInvalidate: ListInvalidators,
) {
  log("Received entity event:", event.type, event);

  // Handle events using the discriminated union type field
  switch (event.type) {
    case "book_created":
    case "book_updated":
    case "book_deleted": {
      // Invalidate the (heavy) book list, throttled so a scan adding many
      // books coalesces into a handful of refetches instead of one per book.
      listInvalidate.books();

      // Invalidate specific book if it's an update (targeted + cheap)
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

        // Series in this library may have changed (book counts, etc.)
        listInvalidate.series();
      }
      break;
    }

    case "series_created":
    case "series_updated":
    case "series_deleted":
    case "series_bulk_purged":
    case "series_metadata_updated": {
      // Invalidate the (heavy) series list, throttled so a bulk operation
      // emitting many series events coalesces into a handful of refetches.
      listInvalidate.series();

      // Invalidate specific series if it's an update.
      if (
        event.type === "series_updated" ||
        event.type === "series_metadata_updated"
      ) {
        // A content/metadata change affects the detail DTO and the metadata
        // view — NOT the independent sub-resources (tracking config, aliases,
        // release ledger), which change only via their own event types
        // (release_announced, etc.). Invalidating the bare ["series", id]
        // prefix would refetch all of them: an analyze run over many series
        // with detail tabs open turned that into a needless
        // tracking+aliases+releases refetch burst per analyzed series.
        const detailKeys = [
          ["series", event.seriesId, "full"],
          ["series", event.seriesId, "metadata"],
        ] as const;
        for (const queryKey of detailKeys) {
          queryClient.invalidateQueries({ queryKey });
          // For metadata updates, refetch the active detail views immediately
          // so the open page reflects the change without waiting for staleness.
          if (event.type === "series_metadata_updated") {
            queryClient.refetchQueries({ queryKey, type: "active" });
          }
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

      // A cover change only affects the entity's IMAGE, not its list data.
      // MediaCard / detail views read the cache-bust timestamp recorded above
      // from the cover store and re-render the image on their own — no list
      // refetch is needed. Re-pulling the whole series/books list on every
      // cover event caused a refetch storm during bulk thumbnail regeneration
      // (one heavy refetch per series, hundreds of them). So invalidate only
      // the entity's DETAIL DTO (which carries the cover-source fields), not
      // the bare ["series"/"books", id] prefix — that prefix also matches the
      // independent sub-resources (tracking/aliases/releases for series,
      // genres/tags/external-* for books) that a cover change never touches,
      // and bulk thumbnail regen turned that into a per-entity refetch burst.
      if (event.entityType === "book") {
        queryClient.invalidateQueries({
          queryKey: ["books", event.entityId, "detail"],
        });
      } else if (event.entityType === "series") {
        queryClient.invalidateQueries({
          queryKey: ["series", event.entityId, "full"],
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
        listInvalidate.books();
        listInvalidate.series();
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

      // Refresh inbox + per-series ledger + the tracking row (drives the
      // Behind-by-N badge's latest_known_* high-water mark) — but throttled,
      // so a poll/reset announcing hundreds of releases coalesces into ~one
      // refetch wave instead of one heavy cascade per event. The series'
      // heavy ["...","full"] query is deliberately not refetched here (see the
      // flush comment): a release advances tracking, not local book counts.
      listInvalidate.release(event.seriesId);

      // Surface a low-priority toast. To avoid spamming the user when a
      // single poll lands a dozen releases for one series, we aggregate by
      // `seriesId`: one toast per series, updated in place as more events
      // arrive. The title is the series name (clickable); the body lists
      // every volume/chapter announced while the toast is visible.
      const seriesTitle =
        event.seriesTitle.length > 0 ? event.seriesTitle : "New release";
      const toastId = `release-series-${event.seriesId}`;
      const seriesPath = `/series/${event.seriesId}#releases`;

      const existing = releaseToastState.get(event.seriesId);
      const state: ReleaseToastState = existing ?? {
        chapters: new Set(),
        volumes: new Set(),
        pluginId: event.pluginId,
      };
      if (event.chapter !== null && event.chapter !== undefined) {
        state.chapters.add(event.chapter);
      }
      if (event.volume !== null && event.volume !== undefined) {
        state.volumes.add(event.volume);
      }
      state.pluginId = event.pluginId;

      const message = `${formatReleaseLabel(state)} from ${event.pluginId}`;
      // Mantine renders the toast inside its own portal, which sits above
      // <BrowserRouter> in the tree, so a `<Link>` here would crash with
      // "Cannot destructure property 'basename' of useContext(...)".
      // Use the global navigationService (already wired to react-router's
      // navigate) so SPA navigation still works from inside the toast.
      const titleNode = (
        <Anchor
          href={seriesPath}
          onClick={(e) => {
            e.preventDefault();
            notifications.hide(toastId);
            navigationService.navigateTo(seriesPath);
          }}
          inherit
        >
          {seriesTitle}
        </Anchor>
      );

      if (existing) {
        notifications.update({
          id: toastId,
          title: titleNode,
          message,
          color: "orange",
        });
      } else {
        releaseToastState.set(event.seriesId, state);
        notifications.show({
          id: toastId,
          title: titleNode,
          message,
          color: "orange",
          onClose: () => releaseToastState.delete(event.seriesId),
        });
      }
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
