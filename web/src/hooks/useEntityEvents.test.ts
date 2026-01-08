import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { renderHook, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import React from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import * as eventsApi from "@/api/events";
import { useAuthStore } from "@/store/authStore";
import type { EntityChangeEvent } from "@/types/events";
import { useEntityEvents } from "./useEntityEvents";

// Mock the events API
vi.mock("@/api/events");

// Mock the auth store
vi.mock("@/store/authStore", () => ({
	useAuthStore: vi.fn(() => ({
		isAuthenticated: true,
	})),
}));

describe("useEntityEvents", () => {
	let queryClient: QueryClient;
	let mockUnsubscribe: ReturnType<typeof vi.fn>;

	beforeEach(() => {
		queryClient = new QueryClient({
			defaultOptions: {
				queries: { retry: false },
			},
		});

		mockUnsubscribe = vi.fn();

		Storage.prototype.getItem = vi.fn((key) => {
			if (key === "jwt_token") return "test-token";
			return null;
		});
	});

	afterEach(() => {
		vi.restoreAllMocks();
		queryClient.clear();
	});

	const wrapper = ({ children }: { children: ReactNode }) =>
		React.createElement(QueryClientProvider, { client: queryClient }, children);

	it("should subscribe to entity events on mount", async () => {
		const mockSubscribe = vi
			.spyOn(eventsApi.eventsApi, "subscribeToEntityEvents")
			.mockReturnValue(mockUnsubscribe);

		renderHook(() => useEntityEvents(), { wrapper });

		await waitFor(() => {
			expect(mockSubscribe).toHaveBeenCalled();
		});
	});

	it("should not subscribe if no token is present", () => {
		// Mock auth store to return not authenticated
		vi.mocked(useAuthStore).mockReturnValue({
			isAuthenticated: false,
		} as ReturnType<typeof useAuthStore>);

		const mockSubscribe = vi
			.spyOn(eventsApi.eventsApi, "subscribeToEntityEvents")
			.mockReturnValue(mockUnsubscribe);

		renderHook(() => useEntityEvents(), { wrapper });

		expect(mockSubscribe).not.toHaveBeenCalled();
	});

	it("should unsubscribe on unmount", async () => {
		const mockSubscribe = vi
			.spyOn(eventsApi.eventsApi, "subscribeToEntityEvents")
			.mockReturnValue(mockUnsubscribe);

		const { unmount } = renderHook(() => useEntityEvents(), { wrapper });

		await waitFor(() => {
			expect(mockSubscribe).toHaveBeenCalled();
		});

		unmount();

		await waitFor(() => {
			expect(mockUnsubscribe).toHaveBeenCalled();
		});
	});

	it("should invalidate book queries on CoverUpdated event", async () => {
		let capturedCallback: ((event: EntityChangeEvent) => void) | undefined;

		vi.spyOn(eventsApi.eventsApi, "subscribeToEntityEvents").mockImplementation(
			(onEvent) => {
				capturedCallback = onEvent;
				return mockUnsubscribe;
			},
		);

		const invalidateSpy = vi.spyOn(queryClient, "invalidateQueries");

		renderHook(() => useEntityEvents(), { wrapper });

		await waitFor(() => {
			expect(capturedCallback).toBeDefined();
		});

		// Simulate receiving a CoverUpdated event
		const event: EntityChangeEvent = {
			event: {
				CoverUpdated: {
					entity_type: "series",
					entity_id: "series-123",
				},
			},
			timestamp: "2026-01-07T12:00:00Z",
			user_id: undefined,
		};

		capturedCallback!(event);

		await waitFor(() => {
			expect(invalidateSpy).toHaveBeenCalledWith({
				queryKey: ["series", "series-123"],
			});
		});
	});

	it("should invalidate series queries on SeriesBulkPurged event", async () => {
		let capturedCallback: ((event: EntityChangeEvent) => void) | undefined;

		vi.spyOn(eventsApi.eventsApi, "subscribeToEntityEvents").mockImplementation(
			(onEvent) => {
				capturedCallback = onEvent;
				return mockUnsubscribe;
			},
		);

		const invalidateSpy = vi.spyOn(queryClient, "invalidateQueries");

		renderHook(() => useEntityEvents(), { wrapper });

		await waitFor(() => {
			expect(capturedCallback).toBeDefined();
		});

		// Simulate receiving a SeriesBulkPurged event
		const event: EntityChangeEvent = {
			event: {
				SeriesBulkPurged: {
					series_id: "series-456",
					library_id: "lib-2",
					count: 5,
				},
			},
			timestamp: "2026-01-07T12:00:00Z",
			user_id: "user-1",
		};

		capturedCallback!(event);

		await waitFor(() => {
			expect(invalidateSpy).toHaveBeenCalledWith({
				queryKey: ["series"],
			});
		});
	});

	it("should track connection state", async () => {
		let capturedConnectionChange:
			| ((
					state: "connecting" | "connected" | "disconnected" | "failed",
			  ) => void)
			| undefined;

		vi.spyOn(eventsApi.eventsApi, "subscribeToEntityEvents").mockImplementation(
			(_onEvent, _onError, onConnectionChange) => {
				capturedConnectionChange = onConnectionChange;
				// Simulate the real behavior: call "connecting" immediately
				onConnectionChange?.("connecting");
				return mockUnsubscribe;
			},
		);

		const { result } = renderHook(() => useEntityEvents(), { wrapper });

		await waitFor(() => {
			expect(capturedConnectionChange).toBeDefined();
		});

		// Initially connecting
		expect(result.current.connectionState).toBe("connecting");

		// Simulate connection established
		capturedConnectionChange!("connected");

		await waitFor(() => {
			expect(result.current.connectionState).toBe("connected");
		});

		// Simulate disconnection
		capturedConnectionChange!("disconnected");

		await waitFor(() => {
			expect(result.current.connectionState).toBe("disconnected");
		});
	});

	it("should handle errors gracefully", async () => {
		const consoleError = vi
			.spyOn(console, "error")
			.mockImplementation(() => {});
		let capturedErrorHandler: ((error: Error) => void) | undefined;

		vi.spyOn(eventsApi.eventsApi, "subscribeToEntityEvents").mockImplementation(
			(_onEvent, onError) => {
				capturedErrorHandler = onError;
				return mockUnsubscribe;
			},
		);

		renderHook(() => useEntityEvents(), { wrapper });

		await waitFor(() => {
			expect(capturedErrorHandler).toBeDefined();
		});

		// Simulate an error
		const testError = new Error("Connection failed");
		capturedErrorHandler!(testError);

		await waitFor(() => {
			expect(consoleError).toHaveBeenCalledWith(
				"[EntityEvents] Connection error:",
				testError,
			);
		});

		consoleError.mockRestore();
	});
});
