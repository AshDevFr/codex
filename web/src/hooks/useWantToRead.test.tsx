import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, renderHook, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { wantToReadApi } from "@/api/wantToRead";
import { useAddToWantToRead, useRemoveFromWantToRead } from "./useWantToRead";

vi.mock("@/api/wantToRead", () => ({
  wantToReadApi: {
    addSeries: vi.fn().mockResolvedValue({}),
    addBook: vi.fn().mockResolvedValue({}),
    removeSeries: vi.fn().mockResolvedValue(undefined),
    removeBook: vi.fn().mockResolvedValue(undefined),
  },
}));

function makeWrapper() {
  const queryClient = new QueryClient({
    defaultOptions: {
      queries: { retry: false },
      mutations: { retry: false },
    },
  });
  const invalidateSpy = vi.spyOn(queryClient, "invalidateQueries");
  const wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
  );
  return { wrapper, invalidateSpy };
}

function invalidatedKeys(invalidateSpy: ReturnType<typeof vi.fn>) {
  return invalidateSpy.mock.calls.map((call) => call[0]?.queryKey);
}

describe("useWantToRead invalidation", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("refreshes the series list/home sections after adding a series", async () => {
    const { wrapper, invalidateSpy } = makeWrapper();
    const { result } = renderHook(() => useAddToWantToRead(), { wrapper });

    await act(async () => {
      result.current.mutate({ itemType: "series", id: "series-1" });
    });

    await waitFor(() =>
      expect(wantToReadApi.addSeries).toHaveBeenCalledWith("series-1"),
    );

    const keys = invalidatedKeys(invalidateSpy);
    // The queue itself and the changed series' detail DTO.
    expect(keys).toContainEqual(["want-to-read"]);
    expect(keys).toContainEqual(["series", "series-1"]);
    // The card grids/home sections source `wantToRead` from list queries keyed
    // by a section string in slot 2, which the detail-id prefix never matches.
    expect(keys).toContainEqual(["series", "recently-added"]);
    expect(keys).toContainEqual(["series", "search"]);
  });

  it("refreshes the books list/home sections after removing a book", async () => {
    const { wrapper, invalidateSpy } = makeWrapper();
    const { result } = renderHook(() => useRemoveFromWantToRead(), { wrapper });

    await act(async () => {
      result.current.mutate({ itemType: "book", id: "book-1" });
    });

    await waitFor(() =>
      expect(wantToReadApi.removeBook).toHaveBeenCalledWith("book-1"),
    );

    const keys = invalidatedKeys(invalidateSpy);
    expect(keys).toContainEqual(["want-to-read"]);
    expect(keys).toContainEqual(["books", "book-1"]);
    expect(keys).toContainEqual(["books", "in-progress"]);
    expect(keys).toContainEqual(["books", "on-deck"]);
  });
});
