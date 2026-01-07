import { useEffect, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { eventsApi } from "@/api/events";
import type { EntityChangeEvent } from "@/types/events";

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
	const [connectionState, setConnectionState] = useState<ConnectionState>("connecting");

	useEffect(() => {
		const token = localStorage.getItem("jwt_token");
		if (!token) {
			console.debug("Not authenticated, skipping entity events subscription");
			return;
		}

		let unsubscribe: (() => void) | null = null;

		const subscribe = async () => {
			try {
				unsubscribe = await eventsApi.subscribeToEntityEvents(
					(event: EntityChangeEvent) => {
						handleEntityEvent(event, queryClient);
					},
					(error: Error) => {
						console.error("[EntityEvents] Connection error:", error);
					},
					(state) => {
						console.debug(`Entity events connection state: ${state}`);
						setConnectionState(state as ConnectionState);
					}
				);
			} catch (error) {
				console.error("Failed to subscribe to entity events:", error);
			}
		};

		subscribe();

		return () => {
			unsubscribe?.();
		};
	}, [queryClient]);

	return {
		connectionState,
	};
}

/**
 * Handle entity change events and invalidate appropriate query caches
 */
function handleEntityEvent(
	event: EntityChangeEvent,
	queryClient: ReturnType<typeof useQueryClient>
) {
	console.debug("Received entity event:", event);

	// Extract event type and data
	const eventType = event.event;

	// Handle book events
	if ("BookCreated" in eventType || "BookUpdated" in eventType || "BookDeleted" in eventType) {
		const data = "BookCreated" in eventType ? eventType.BookCreated
			: "BookUpdated" in eventType ? eventType.BookUpdated
			: eventType.BookDeleted;

		// Invalidate book queries
		queryClient.invalidateQueries({
			queryKey: ["books"],
		});

		// Invalidate specific book if it's an update
		if ("BookUpdated" in eventType) {
			queryClient.invalidateQueries({
				queryKey: ["books", data.book_id],
			});
		}

		// Invalidate library queries
		if (data.library_id) {
			queryClient.invalidateQueries({
				queryKey: ["libraries", data.library_id],
			});

			// Invalidate series in this library
			queryClient.invalidateQueries({
				queryKey: ["series"],
			});
		}

		return;
	}

	// Handle series events
	if ("SeriesCreated" in eventType || "SeriesUpdated" in eventType || "SeriesDeleted" in eventType || "SeriesBulkPurged" in eventType) {
		const data = "SeriesCreated" in eventType ? eventType.SeriesCreated
			: "SeriesUpdated" in eventType ? eventType.SeriesUpdated
			: "SeriesDeleted" in eventType ? eventType.SeriesDeleted
			: eventType.SeriesBulkPurged;

		// Invalidate series queries
		queryClient.invalidateQueries({
			queryKey: ["series"],
		});

		// Invalidate specific series if it's an update
		if ("SeriesUpdated" in eventType) {
			queryClient.invalidateQueries({
				queryKey: ["series", data.series_id],
			});
		}

		// Invalidate library queries
		if (data.library_id) {
			queryClient.invalidateQueries({
				queryKey: ["libraries", data.library_id],
			});
		}

		return;
	}

	// Handle cover update events
	if ("CoverUpdated" in eventType) {
		const data = eventType.CoverUpdated;

		if (data.entity_type === "book") {
			// Invalidate book queries
			queryClient.invalidateQueries({
				queryKey: ["books", data.entity_id],
			});
			queryClient.invalidateQueries({
				queryKey: ["books"],
			});
		} else if (data.entity_type === "series") {
			// Invalidate series queries
			queryClient.invalidateQueries({
				queryKey: ["series", data.entity_id],
			});
			queryClient.invalidateQueries({
				queryKey: ["series"],
			});
		}

		return;
	}

	// Handle library update events
	if ("LibraryUpdated" in eventType) {
		const data = eventType.LibraryUpdated;

		// Invalidate library queries
		queryClient.invalidateQueries({
			queryKey: ["libraries"],
		});
		queryClient.invalidateQueries({
			queryKey: ["libraries", data.library_id],
		});

		return;
	}
}
