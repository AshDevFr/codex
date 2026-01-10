import { useQueryClient } from "@tanstack/react-query";
import { useEffect, useState, useRef } from "react";
import { notifications } from "@mantine/notifications";
import { eventsApi } from "@/api/events";
import { useAuthStore } from "@/store/authStore";
import type { EntityChangeEvent } from "@/types/events";

type ConnectionState = "connecting" | "connected" | "disconnected" | "failed";

// Debounce configuration for batched notifications
const DEBOUNCE_DELAY = 2000; // 2 seconds

interface EventBatch {
	booksCreated: number;
	booksUpdated: number;
	booksDeleted: number;
	seriesCreated: number;
	seriesUpdated: number;
	seriesDeleted: number;
	coversUpdated: number;
	librariesUpdated: number;
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

	// Use refs to track event batching and debounce timer
	const eventBatchRef = useRef<EventBatch>({
		booksCreated: 0,
		booksUpdated: 0,
		booksDeleted: 0,
		seriesCreated: 0,
		seriesUpdated: 0,
		seriesDeleted: 0,
		coversUpdated: 0,
		librariesUpdated: 0,
	});
	const debounceTimerRef = useRef<NodeJS.Timeout | null>(null);

	useEffect(() => {
		if (!isAuthenticated) {
			console.debug("Not authenticated, skipping entity events subscription");
			return;
		}

		// Function to show batched notifications
		const showBatchedNotifications = () => {
			const batch = eventBatchRef.current;
			const messages: string[] = [];

			// Build notification message based on what happened
			if (batch.booksCreated > 0) {
				messages.push(
					`${batch.booksCreated} book${batch.booksCreated > 1 ? "s" : ""} added`,
				);
			}
			if (batch.seriesCreated > 0) {
				messages.push(
					`${batch.seriesCreated} series ${batch.seriesCreated > 1 ? "" : ""}created`,
				);
			}
			if (batch.coversUpdated > 0) {
				messages.push(
					`${batch.coversUpdated} cover${batch.coversUpdated > 1 ? "s" : ""} updated`,
				);
			}
			if (batch.booksUpdated > 0) {
				messages.push(
					`${batch.booksUpdated} book${batch.booksUpdated > 1 ? "s" : ""} updated`,
				);
			}
			if (batch.seriesUpdated > 0) {
				messages.push(
					`${batch.seriesUpdated} series updated`,
				);
			}
			if (batch.booksDeleted > 0) {
				messages.push(
					`${batch.booksDeleted} book${batch.booksDeleted > 1 ? "s" : ""} deleted`,
				);
			}
			if (batch.seriesDeleted > 0) {
				messages.push(
					`${batch.seriesDeleted} series deleted`,
				);
			}
			if (batch.librariesUpdated > 0) {
				messages.push(
					`${batch.librariesUpdated} librar${batch.librariesUpdated > 1 ? "ies" : "y"} updated`,
				);
			}

			// Show notification if there are any changes
			if (messages.length > 0) {
				notifications.show({
					title: "Library updated",
					message: messages.join(", "),
					color: "blue",
					autoClose: 3000,
					withCloseButton: true,
				});
			}

			// Reset batch counters
			eventBatchRef.current = {
				booksCreated: 0,
				booksUpdated: 0,
				booksDeleted: 0,
				seriesCreated: 0,
				seriesUpdated: 0,
				seriesDeleted: 0,
				coversUpdated: 0,
				librariesUpdated: 0,
			};
		};

		const unsubscribe = eventsApi.subscribeToEntityEvents(
			(event: EntityChangeEvent) => {
				handleEntityEvent(event, queryClient, eventBatchRef, debounceTimerRef, showBatchedNotifications);
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
			// Clear any pending debounce timer
			if (debounceTimerRef.current) {
				clearTimeout(debounceTimerRef.current);
				debounceTimerRef.current = null;
			}
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
	eventBatchRef: { current: EventBatch },
	debounceTimerRef: { current: NodeJS.Timeout | null },
	showBatchedNotifications: () => void,
) {
	console.debug("Received entity event:", event);

	// Extract event type and data
	const eventType = event.event;

	// Debounce notification display
	const scheduleNotification = () => {
		// Clear existing timer
		if (debounceTimerRef.current) {
			clearTimeout(debounceTimerRef.current);
		}

		// Schedule new notification
		debounceTimerRef.current = setTimeout(() => {
			showBatchedNotifications();
			debounceTimerRef.current = null;
		}, DEBOUNCE_DELAY);
	};

	// Handle book events
	if (
		"BookCreated" in eventType ||
		"BookUpdated" in eventType ||
		"BookDeleted" in eventType
	) {
		const data =
			"BookCreated" in eventType
				? eventType.BookCreated
				: "BookUpdated" in eventType
					? eventType.BookUpdated
					: eventType.BookDeleted;

		// Track event in batch
		if ("BookCreated" in eventType) {
			eventBatchRef.current.booksCreated++;
		} else if ("BookUpdated" in eventType) {
			eventBatchRef.current.booksUpdated++;
		} else if ("BookDeleted" in eventType) {
			eventBatchRef.current.booksDeleted++;
		}

		// Schedule batched notification
		scheduleNotification();

		// Invalidate book queries with immediate refetch for active queries
		queryClient.invalidateQueries({
			queryKey: ["books"],
			refetchType: "active",
		});

		// Invalidate specific book if it's an update
		if ("BookUpdated" in eventType) {
			queryClient.invalidateQueries({
				queryKey: ["books", data.book_id],
				refetchType: "active",
			});
		}

		// Invalidate library queries
		if (data.library_id) {
			queryClient.invalidateQueries({
				queryKey: ["libraries", data.library_id],
				refetchType: "active",
			});

			// Invalidate series in this library
			queryClient.invalidateQueries({
				queryKey: ["series"],
				refetchType: "active",
			});
		}

		return;
	}

	// Handle series events
	if (
		"SeriesCreated" in eventType ||
		"SeriesUpdated" in eventType ||
		"SeriesDeleted" in eventType ||
		"SeriesBulkPurged" in eventType
	) {
		const data =
			"SeriesCreated" in eventType
				? eventType.SeriesCreated
				: "SeriesUpdated" in eventType
					? eventType.SeriesUpdated
					: "SeriesDeleted" in eventType
						? eventType.SeriesDeleted
						: eventType.SeriesBulkPurged;

		// Track event in batch
		if ("SeriesCreated" in eventType) {
			eventBatchRef.current.seriesCreated++;
		} else if ("SeriesUpdated" in eventType) {
			eventBatchRef.current.seriesUpdated++;
		} else if ("SeriesDeleted" in eventType) {
			eventBatchRef.current.seriesDeleted++;
		}

		// Schedule batched notification
		scheduleNotification();

		// Invalidate series queries with immediate refetch for active queries
		queryClient.invalidateQueries({
			queryKey: ["series"],
			refetchType: "active",
		});

		// Invalidate specific series if it's an update
		if ("SeriesUpdated" in eventType) {
			queryClient.invalidateQueries({
				queryKey: ["series", data.series_id],
				refetchType: "active",
			});
		}

		// Invalidate library queries
		if (data.library_id) {
			queryClient.invalidateQueries({
				queryKey: ["libraries", data.library_id],
				refetchType: "active",
			});
		}

		return;
	}

	// Handle cover update events
	if ("CoverUpdated" in eventType) {
		const data = eventType.CoverUpdated;

		// Track event in batch
		eventBatchRef.current.coversUpdated++;

		// Schedule batched notification
		scheduleNotification();

		if (data.entity_type === "book") {
			// Invalidate book queries with immediate refetch
			queryClient.invalidateQueries({
				queryKey: ["books", data.entity_id],
				refetchType: "active",
			});
			queryClient.invalidateQueries({
				queryKey: ["books"],
				refetchType: "active",
			});
		} else if (data.entity_type === "series") {
			// Invalidate series queries with immediate refetch
			queryClient.invalidateQueries({
				queryKey: ["series", data.entity_id],
				refetchType: "active",
			});
			queryClient.invalidateQueries({
				queryKey: ["series"],
				refetchType: "active",
			});
		}

		return;
	}

	// Handle library update events
	if ("LibraryUpdated" in eventType) {
		const data = eventType.LibraryUpdated;

		// Track event in batch
		eventBatchRef.current.librariesUpdated++;

		// Schedule batched notification
		scheduleNotification();

		// Invalidate library queries with immediate refetch
		queryClient.invalidateQueries({
			queryKey: ["libraries"],
			refetchType: "active",
		});
		queryClient.invalidateQueries({
			queryKey: ["libraries", data.library_id],
			refetchType: "active",
		});

		return;
	}
}
