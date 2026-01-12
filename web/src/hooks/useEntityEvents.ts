import { useQueryClient } from "@tanstack/react-query";
import { useEffect, useState } from "react";
import { eventsApi } from "@/api/events";
import { useAuthStore } from "@/store/authStore";
import type { EntityChangeEvent } from "@/types";

type ConnectionState = "connecting" | "connected" | "disconnected" | "failed";

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
			console.debug("Not authenticated, skipping entity events subscription");
			return;
		}

		const unsubscribe = eventsApi.subscribeToEntityEvents(
			(event: EntityChangeEvent) => {
				handleEntityEvent(event, queryClient);
			},
			(error: Error) => {
				console.error("[EntityEvents] Connection error:", error);
			},
			(state) => {
				console.debug(`Entity events connection state: ${state}`);
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
	console.debug("Received entity event:", event);

	// Handle events using the discriminated union type field
	switch (event.type) {
		case "book_created":
		case "book_updated":
		case "book_deleted": {
			// Invalidate book queries with immediate refetch for active queries
			queryClient.invalidateQueries({
				queryKey: ["books"],
				refetchType: "active",
			});

			// Invalidate specific book if it's an update
			if (event.type === "book_updated") {
				queryClient.invalidateQueries({
					queryKey: ["books", event.book_id],
					refetchType: "active",
				});
			}

			// Invalidate library queries
			if (event.library_id) {
				queryClient.invalidateQueries({
					queryKey: ["libraries", event.library_id],
					refetchType: "active",
				});

				// Invalidate series in this library
				queryClient.invalidateQueries({
					queryKey: ["series"],
					refetchType: "active",
				});
			}
			break;
		}

		case "series_created":
		case "series_updated":
		case "series_deleted":
		case "series_bulk_purged": {
			// Invalidate series queries with immediate refetch for active queries
			queryClient.invalidateQueries({
				queryKey: ["series"],
				refetchType: "active",
			});

			// Invalidate specific series if it's an update
			if (event.type === "series_updated") {
				queryClient.invalidateQueries({
					queryKey: ["series", event.series_id],
					refetchType: "active",
				});
			}

			// Invalidate library queries
			if (event.library_id) {
				queryClient.invalidateQueries({
					queryKey: ["libraries", event.library_id],
					refetchType: "active",
				});
			}
			break;
		}

		case "cover_updated": {
			if (event.entity_type === "book") {
				// Invalidate book queries with immediate refetch
				queryClient.invalidateQueries({
					queryKey: ["books", event.entity_id],
					refetchType: "active",
				});
				queryClient.invalidateQueries({
					queryKey: ["books"],
					refetchType: "active",
				});
			} else if (event.entity_type === "series") {
				// Invalidate series queries with immediate refetch
				queryClient.invalidateQueries({
					queryKey: ["series", event.entity_id],
					refetchType: "active",
				});
				queryClient.invalidateQueries({
					queryKey: ["series"],
					refetchType: "active",
				});
			}
			break;
		}

		case "library_updated":
		case "library_deleted": {
			// Invalidate library queries with immediate refetch
			queryClient.invalidateQueries({
				queryKey: ["libraries"],
				refetchType: "active",
			});
			queryClient.invalidateQueries({
				queryKey: ["libraries", event.library_id],
				refetchType: "active",
			});
			break;
		}

		default:
			console.debug("Unknown event type:", event);
	}
}
