import { describe, expect, it, vi } from "vitest";
import { renderWithProviders, screen } from "@/test/utils";
import { Collections } from "./Collections";

let collections: Record<string, unknown>[] = [];

vi.mock("@/hooks/useCollections", async (importOriginal) => {
  const actual =
    await importOriginal<typeof import("@/hooks/useCollections")>();
  return {
    ...actual,
    useCollections: () => ({ data: collections, isLoading: false }),
  };
});

vi.mock("@/hooks/usePermissions", () => ({
  usePermissions: () => ({ hasPermission: () => true }),
}));

vi.mock("@/api/libraries", () => ({
  librariesApi: { getAll: vi.fn(() => Promise.resolve([])) },
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

describe("Collections list", () => {
  it("reports a hand-picked collection's series count", () => {
    collections = [base({ name: "Hand Picked", seriesCount: 7 })];
    renderWithProviders(<Collections />);
    expect(screen.getByText("7 series")).toBeInTheDocument();
  });

  // seriesCount is null for automatic collections, so rendering it directly
  // would print "0 series" and read as an empty collection.
  it("never claims an automatic collection has zero series", () => {
    collections = [
      base({
        id: "bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb",
        name: "Isekai",
        automatic: true,
        condition: { tag: { operator: "is", value: "isekai" } },
        seriesCount: null,
      }),
    ];
    renderWithProviders(<Collections />);

    expect(screen.queryByText("0 series")).not.toBeInTheDocument();
    expect(screen.getByText("Automatic · 1 condition")).toBeInTheDocument();
  });

  it("pluralizes the condition count", () => {
    collections = [
      base({
        automatic: true,
        condition: {
          anyOf: [
            { tag: { operator: "is", value: "isekai" } },
            { tag: { operator: "is", value: "reincarnation" } },
          ],
        },
        seriesCount: null,
      }),
    ];
    renderWithProviders(<Collections />);
    expect(screen.getByText("Automatic · 2 conditions")).toBeInTheDocument();
  });

  it("badges automatic collections", () => {
    collections = [
      base({
        automatic: true,
        condition: { tag: { operator: "is", value: "isekai" } },
        seriesCount: null,
      }),
    ];
    renderWithProviders(<Collections />);
    expect(screen.getByText("Automatic")).toBeInTheDocument();
    expect(screen.queryByText("Personal")).not.toBeInTheDocument();
  });

  it("adds a personal badge for a viewer-dependent rule", () => {
    collections = [
      base({
        automatic: true,
        condition: { userRating: { operator: "gte", value: 85 } },
        seriesCount: null,
      }),
    ];
    renderWithProviders(<Collections />);
    expect(screen.getByText("Automatic")).toBeInTheDocument();
    expect(screen.getByText("Personal")).toBeInTheDocument();
  });

  it("does not badge hand-picked collections", () => {
    collections = [base({ name: "Hand Picked", seriesCount: 3 })];
    renderWithProviders(<Collections />);
    expect(screen.queryByText("Automatic")).not.toBeInTheDocument();
    expect(screen.queryByText("Personal")).not.toBeInTheDocument();
  });

  it("renders both kinds side by side", () => {
    collections = [
      base({ name: "Hand Picked", seriesCount: 3 }),
      base({
        id: "bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb",
        name: "Isekai",
        automatic: true,
        condition: { tag: { operator: "is", value: "isekai" } },
        seriesCount: null,
      }),
    ];
    renderWithProviders(<Collections />);
    expect(screen.getByText("Hand Picked")).toBeInTheDocument();
    expect(screen.getByText("Isekai")).toBeInTheDocument();
    expect(screen.getByText("3 series")).toBeInTheDocument();
    expect(screen.getByText("Automatic · 1 condition")).toBeInTheDocument();
  });
});
