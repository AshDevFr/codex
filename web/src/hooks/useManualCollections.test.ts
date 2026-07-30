import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { renderHook, waitFor } from "@testing-library/react";
import { createElement, type ReactNode } from "react";
import { describe, expect, it, vi } from "vitest";
import { collectionsApi } from "@/api/collections";
import { useManualCollections } from "./useCollections";

vi.mock("@/api/collections", () => ({
  collectionsApi: { list: vi.fn() },
}));

function base(overrides: Record<string, unknown>) {
  return {
    id: "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa",
    name: "Collection",
    summary: null,
    ordered: false,
    condition: null,
    automatic: false,
    seriesCount: 0,
    createdAt: "2026-01-01T00:00:00Z",
    updatedAt: "2026-01-01T00:00:00Z",
    ...overrides,
  };
}

function wrapper({ children }: { children: ReactNode }) {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return createElement(QueryClientProvider, { client }, children);
}

describe("useManualCollections", () => {
  // Automatic collections are a dead end in an "add to collection" picker: the
  // API returns 409 for hand-editing them.
  it("drops automatic collections", async () => {
    vi.mocked(collectionsApi.list).mockResolvedValue([
      base({ name: "Hand Picked" }),
      base({
        id: "bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb",
        name: "Isekai",
        automatic: true,
        condition: { tag: { operator: "is", value: "isekai" } },
        seriesCount: null,
      }),
    ] as never);

    const { result } = renderHook(() => useManualCollections(), { wrapper });

    await waitFor(() => expect(result.current.data).toBeDefined());
    expect(result.current.data?.map((c) => c.name)).toEqual(["Hand Picked"]);
  });

  it("passes hand-picked collections through untouched", async () => {
    vi.mocked(collectionsApi.list).mockResolvedValue([
      base({ name: "One" }),
      base({ id: "bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb", name: "Two" }),
    ] as never);

    const { result } = renderHook(() => useManualCollections(), { wrapper });

    await waitFor(() => expect(result.current.data).toBeDefined());
    expect(result.current.data?.map((c) => c.name)).toEqual(["One", "Two"]);
  });

  it("leaves data undefined while loading", () => {
    vi.mocked(collectionsApi.list).mockReturnValue(new Promise(() => {}));
    const { result } = renderHook(() => useManualCollections(), { wrapper });
    expect(result.current.data).toBeUndefined();
  });
});
