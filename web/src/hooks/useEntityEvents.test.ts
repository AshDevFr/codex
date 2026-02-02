import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { renderHook, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import React from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import * as eventsApi from "@/api/events";
import { useAuthStore } from "@/store/authStore";
import { useCoverUpdatesStore } from "@/store/coverUpdatesStore";
import type { EntityChangeEvent } from "@/types";
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

  it("should invalidate and refetch queries and record cover update on CoverUpdated event", async () => {
    let capturedCallback: ((event: EntityChangeEvent) => void) | undefined;

    vi.spyOn(eventsApi.eventsApi, "subscribeToEntityEvents").mockImplementation(
      (onEvent) => {
        capturedCallback = onEvent;
        return mockUnsubscribe;
      },
    );

    const invalidateSpy = vi.spyOn(queryClient, "invalidateQueries");
    const refetchSpy = vi.spyOn(queryClient, "refetchQueries");

    // Reset cover updates store before test
    useCoverUpdatesStore.setState({ updates: {} });

    renderHook(() => useEntityEvents(), { wrapper });

    await waitFor(() => {
      expect(capturedCallback).toBeDefined();
    });

    // Simulate receiving a CoverUpdated event
    const event: EntityChangeEvent = {
      type: "cover_updated",
      entity_type: "series",
      entity_id: "series-123",
      timestamp: "2026-01-07T12:00:00Z",
      user_id: undefined,
    };

    if (capturedCallback) {
      capturedCallback(event);
    }

    await waitFor(() => {
      // Invalidate the specific series
      expect(invalidateSpy).toHaveBeenCalledWith({
        queryKey: ["series", "series-123"],
      });
      // Invalidate all series list queries
      expect(invalidateSpy).toHaveBeenCalledWith({
        queryKey: ["series"],
      });
      // Refetch all active series queries to trigger component re-render
      expect(refetchSpy).toHaveBeenCalledWith({
        queryKey: ["series"],
        type: "active",
      });
    });

    // Verify cover update was recorded in the store for cache-busting
    const coverTimestamp = useCoverUpdatesStore
      .getState()
      .getCoverTimestamp("series-123");
    expect(coverTimestamp).toBeDefined();
    expect(typeof coverTimestamp).toBe("number");
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
      type: "series_bulk_purged",
      series_id: "series-456",
      library_id: "lib-2",
      count: 5,
      timestamp: "2026-01-07T12:00:00Z",
      user_id: "user-1",
    };

    if (capturedCallback) {
      capturedCallback(event);
    }

    await waitFor(() => {
      expect(invalidateSpy).toHaveBeenCalledWith({
        queryKey: ["series"],
      });
      expect(invalidateSpy).toHaveBeenCalledWith({
        queryKey: ["libraries", "lib-2"],
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
    capturedConnectionChange?.("connected");

    await waitFor(() => {
      expect(result.current.connectionState).toBe("connected");
    });

    // Simulate disconnection
    capturedConnectionChange?.("disconnected");

    await waitFor(() => {
      expect(result.current.connectionState).toBe("disconnected");
    });
  });

  it("should invalidate library queries on library_deleted event", async () => {
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

    // Simulate receiving a library_deleted event
    const event: EntityChangeEvent = {
      type: "library_deleted",
      library_id: "lib-123",
      timestamp: "2026-01-12T12:00:00Z",
      user_id: "user-1",
    };

    if (capturedCallback) {
      capturedCallback(event);
    }

    await waitFor(() => {
      // Should invalidate the libraries list
      expect(invalidateSpy).toHaveBeenCalledWith({
        queryKey: ["libraries"],
      });
      // Should invalidate the specific library with both key patterns
      expect(invalidateSpy).toHaveBeenCalledWith({
        queryKey: ["libraries", "lib-123"],
      });
      expect(invalidateSpy).toHaveBeenCalledWith({
        queryKey: ["library", "lib-123"],
      });
      // Should also invalidate books and series queries
      expect(invalidateSpy).toHaveBeenCalledWith({
        queryKey: ["books"],
      });
      expect(invalidateSpy).toHaveBeenCalledWith({
        queryKey: ["series"],
      });
    });
  });

  it("should invalidate Recommended section queries on cover_updated event", async () => {
    let capturedCallback: ((event: EntityChangeEvent) => void) | undefined;

    vi.spyOn(eventsApi.eventsApi, "subscribeToEntityEvents").mockImplementation(
      (onEvent) => {
        capturedCallback = onEvent;
        return mockUnsubscribe;
      },
    );

    // Set up queries that match the Recommended section's query keys
    queryClient.setQueryData(["series", "recently-added", "lib-1"], []);
    queryClient.setQueryData(["series", "recently-updated", "lib-1"], []);
    queryClient.setQueryData(["books", "recently-added", "lib-1"], []);
    queryClient.setQueryData(["books", "in-progress", "lib-1"], []);

    renderHook(() => useEntityEvents(), { wrapper });

    await waitFor(() => {
      expect(capturedCallback).toBeDefined();
    });

    // Simulate receiving a cover_updated event for a series
    const seriesEvent: EntityChangeEvent = {
      type: "cover_updated",
      entity_type: "series",
      entity_id: "series-123",
      timestamp: "2026-01-12T12:00:00Z",
      user_id: undefined,
    };

    if (capturedCallback) {
      capturedCallback(seriesEvent);
    }

    // All series queries should be invalidated (stale)
    await waitFor(() => {
      const recentlyAddedState = queryClient.getQueryState([
        "series",
        "recently-added",
        "lib-1",
      ]);
      const recentlyUpdatedState = queryClient.getQueryState([
        "series",
        "recently-updated",
        "lib-1",
      ]);

      expect(recentlyAddedState?.isInvalidated).toBe(true);
      expect(recentlyUpdatedState?.isInvalidated).toBe(true);
    });

    // Books queries should NOT be invalidated by series cover update
    const booksRecentlyAddedState = queryClient.getQueryState([
      "books",
      "recently-added",
      "lib-1",
    ]);
    expect(booksRecentlyAddedState?.isInvalidated).toBe(false);

    // Simulate receiving a cover_updated event for a book
    const bookEvent: EntityChangeEvent = {
      type: "cover_updated",
      entity_type: "book",
      entity_id: "book-456",
      timestamp: "2026-01-12T12:00:00Z",
      user_id: undefined,
    };

    if (capturedCallback) {
      capturedCallback(bookEvent);
    }

    // All book queries should now be invalidated (stale)
    await waitFor(() => {
      const booksRecentlyAddedState = queryClient.getQueryState([
        "books",
        "recently-added",
        "lib-1",
      ]);
      const booksInProgressState = queryClient.getQueryState([
        "books",
        "in-progress",
        "lib-1",
      ]);

      expect(booksRecentlyAddedState?.isInvalidated).toBe(true);
      expect(booksInProgressState?.isInvalidated).toBe(true);
    });
  });

  it("should invalidate and refetch plugin queries on plugin events", async () => {
    let capturedCallback: ((event: EntityChangeEvent) => void) | undefined;

    vi.spyOn(eventsApi.eventsApi, "subscribeToEntityEvents").mockImplementation(
      (onEvent) => {
        capturedCallback = onEvent;
        return mockUnsubscribe;
      },
    );

    const invalidateSpy = vi.spyOn(queryClient, "invalidateQueries");
    const refetchSpy = vi.spyOn(queryClient, "refetchQueries");

    renderHook(() => useEntityEvents(), { wrapper });

    await waitFor(() => {
      expect(capturedCallback).toBeDefined();
    });

    // Test plugin_created event
    const createdEvent: EntityChangeEvent = {
      type: "plugin_created",
      plugin_id: "plugin-123",
      timestamp: "2026-01-31T12:00:00Z",
      user_id: "user-1",
    };

    if (capturedCallback) {
      capturedCallback(createdEvent);
    }

    await waitFor(() => {
      // Should invalidate plugins list
      expect(invalidateSpy).toHaveBeenCalledWith({
        queryKey: ["plugins"],
      });
      // Should force refetch of active plugin-actions queries
      expect(refetchSpy).toHaveBeenCalledWith({
        queryKey: ["plugin-actions"],
        type: "active",
      });
    });

    // Reset spies
    invalidateSpy.mockClear();
    refetchSpy.mockClear();

    // Test plugin_enabled event
    const enabledEvent: EntityChangeEvent = {
      type: "plugin_enabled",
      plugin_id: "plugin-456",
      timestamp: "2026-01-31T12:00:00Z",
      user_id: "user-1",
    };

    if (capturedCallback) {
      capturedCallback(enabledEvent);
    }

    await waitFor(() => {
      expect(invalidateSpy).toHaveBeenCalledWith({
        queryKey: ["plugins"],
      });
      expect(refetchSpy).toHaveBeenCalledWith({
        queryKey: ["plugin-actions"],
        type: "active",
      });
    });

    // Reset spies
    invalidateSpy.mockClear();
    refetchSpy.mockClear();

    // Test plugin_disabled event
    const disabledEvent: EntityChangeEvent = {
      type: "plugin_disabled",
      plugin_id: "plugin-789",
      timestamp: "2026-01-31T12:00:00Z",
      user_id: "user-1",
    };

    if (capturedCallback) {
      capturedCallback(disabledEvent);
    }

    await waitFor(() => {
      expect(invalidateSpy).toHaveBeenCalledWith({
        queryKey: ["plugins"],
      });
      expect(refetchSpy).toHaveBeenCalledWith({
        queryKey: ["plugin-actions"],
        type: "active",
      });
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
    capturedErrorHandler?.(testError);

    await waitFor(() => {
      expect(consoleError).toHaveBeenCalledWith(
        "[SSE] Connection error:",
        testError,
      );
    });

    consoleError.mockRestore();
  });
});
