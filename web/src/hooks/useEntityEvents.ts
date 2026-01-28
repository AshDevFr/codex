import { useQueryClient } from "@tanstack/react-query";
import { useEffect, useState } from "react";
import { eventsApi } from "@/api/events";
import { useAuthStore } from "@/store/authStore";
import { useCoverUpdatesStore } from "@/store/coverUpdatesStore";
import type { EntityChangeEvent } from "@/types";
import { createDevLog } from "@/utils/devLog";

type ConnectionState = "connecting" | "connected" | "disconnected" | "failed";

const log = createDevLog("[SSE]");

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
					queryKey: ["books", event.book_id],
				});
			}

			// Invalidate library queries
			if (event.library_id) {
				queryClient.invalidateQueries({
					queryKey: ["libraries", event.library_id],
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
					queryKey: ["series", event.series_id],
				});
				// For metadata updates, also refetch active queries to immediately update the UI
				if (event.type === "series_metadata_updated") {
					queryClient.refetchQueries({
						queryKey: ["series", event.series_id],
						type: "active",
					});
				}
			}

			// Invalidate library queries
			if (event.library_id) {
				queryClient.invalidateQueries({
					queryKey: ["libraries", event.library_id],
				});
			}
			break;
		}

		case "cover_updated": {
			// Record the cover update for cache-busting image URLs
			// This is needed because query invalidation only refetches JSON data,
			// not images. The timestamp is used as a query param to force image reload.
			useCoverUpdatesStore.getState().recordCoverUpdate(event.entity_id);

			const timestamp = useCoverUpdatesStore
				.getState()
				.getCoverTimestamp(event.entity_id);
			log(
				`Cover updated for ${event.entity_type} ${event.entity_id}, cache-bust timestamp: ${timestamp}`,
			);

			if (event.entity_type === "book") {
				// Invalidate the specific book query
				queryClient.invalidateQueries({
					queryKey: ["books", event.entity_id],
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
			} else if (event.entity_type === "series") {
				// Invalidate the specific series query
				queryClient.invalidateQueries({
					queryKey: ["series", event.entity_id],
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
				queryKey: ["libraries", event.library_id],
			});
			queryClient.invalidateQueries({
				queryKey: ["library", event.library_id],
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

		default:
			log("Unknown event type:", event);
	}
}
