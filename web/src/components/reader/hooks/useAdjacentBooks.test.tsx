import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { renderHook, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { booksApi } from "@/api/books";
import { useReaderStore } from "@/store/readerStore";
import { useAdjacentBooks } from "./useAdjacentBooks";

// Mock the books API
vi.mock("@/api/books", () => ({
  booksApi: {
    getAdjacent: vi.fn(),
  },
}));

const mockGetAdjacent = vi.mocked(booksApi.getAdjacent);

function createWrapper() {
  const queryClient = new QueryClient({
    defaultOptions: {
      queries: {
        retry: false,
      },
    },
  });
  return ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
  );
}

describe("useAdjacentBooks", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useReaderStore.getState().resetSession();
  });

  it("should return null books while loading", () => {
    mockGetAdjacent.mockReturnValue(new Promise(() => {})); // Never resolves

    const { result } = renderHook(
      () => useAdjacentBooks({ bookId: "book-1" }),
      { wrapper: createWrapper() },
    );

    expect(result.current.isLoading).toBe(true);
    expect(result.current.prevBook).toBeNull();
    expect(result.current.nextBook).toBeNull();
  });

  it("should return adjacent books when loaded", async () => {
    mockGetAdjacent.mockResolvedValue({
      prev: { id: "book-0", title: "Previous Book", pageCount: 50 } as never,
      next: { id: "book-2", title: "Next Book", pageCount: 100 } as never,
    });

    const { result } = renderHook(
      () => useAdjacentBooks({ bookId: "book-1" }),
      { wrapper: createWrapper() },
    );

    await waitFor(() => expect(result.current.isLoading).toBe(false));

    expect(result.current.prevBook).toEqual({
      id: "book-0",
      title: "Previous Book",
      pageCount: 50,
    });
    expect(result.current.nextBook).toEqual({
      id: "book-2",
      title: "Next Book",
      pageCount: 100,
    });
  });

  it("should sync adjacent books to the store", async () => {
    mockGetAdjacent.mockResolvedValue({
      prev: { id: "book-0", title: "Previous Book", pageCount: 50 } as never,
      next: { id: "book-2", title: "Next Book", pageCount: 100 } as never,
    });

    const { result } = renderHook(
      () => useAdjacentBooks({ bookId: "book-1" }),
      { wrapper: createWrapper() },
    );

    await waitFor(() => expect(result.current.isLoading).toBe(false));

    const storeState = useReaderStore.getState();
    expect(storeState.adjacentBooks).toEqual({
      prev: { id: "book-0", title: "Previous Book", pageCount: 50 },
      next: { id: "book-2", title: "Next Book", pageCount: 100 },
    });
  });

  it("should handle null prev book", async () => {
    mockGetAdjacent.mockResolvedValue({
      prev: null,
      next: { id: "book-2", title: "Next Book", pageCount: 100 } as never,
    });

    const { result } = renderHook(
      () => useAdjacentBooks({ bookId: "book-1" }),
      { wrapper: createWrapper() },
    );

    await waitFor(() => expect(result.current.isLoading).toBe(false));

    expect(result.current.prevBook).toBeNull();
    expect(result.current.nextBook).toEqual({
      id: "book-2",
      title: "Next Book",
      pageCount: 100,
    });
  });

  it("should handle null next book", async () => {
    mockGetAdjacent.mockResolvedValue({
      prev: { id: "book-0", title: "Previous Book", pageCount: 50 } as never,
      next: null,
    });

    const { result } = renderHook(
      () => useAdjacentBooks({ bookId: "book-1" }),
      { wrapper: createWrapper() },
    );

    await waitFor(() => expect(result.current.isLoading).toBe(false));

    expect(result.current.prevBook).toEqual({
      id: "book-0",
      title: "Previous Book",
      pageCount: 50,
    });
    expect(result.current.nextBook).toBeNull();
  });

  it("should handle both books being null", async () => {
    mockGetAdjacent.mockResolvedValue({
      prev: null,
      next: null,
    });

    const { result } = renderHook(
      () => useAdjacentBooks({ bookId: "book-1" }),
      { wrapper: createWrapper() },
    );

    await waitFor(() => expect(result.current.isLoading).toBe(false));

    expect(result.current.prevBook).toBeNull();
    expect(result.current.nextBook).toBeNull();
  });

  it("should not fetch when disabled", () => {
    const { result } = renderHook(
      () => useAdjacentBooks({ bookId: "book-1", enabled: false }),
      { wrapper: createWrapper() },
    );

    expect(mockGetAdjacent).not.toHaveBeenCalled();
    expect(result.current.isLoading).toBe(false);
  });

  it("should handle API errors", async () => {
    mockGetAdjacent.mockRejectedValue(new Error("API Error"));

    const { result } = renderHook(
      () => useAdjacentBooks({ bookId: "book-1" }),
      { wrapper: createWrapper() },
    );

    await waitFor(() => expect(result.current.isError).toBe(true));

    expect(result.current.prevBook).toBeNull();
    expect(result.current.nextBook).toBeNull();
  });
});
