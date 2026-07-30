import { beforeEach, describe, expect, it, vi } from "vitest";
import { renderWithProviders, screen, waitFor } from "@/test/utils";
import { CollectionDetail } from "./CollectionDetail";

const COLLECTION_ID = "cccccccc-cccc-cccc-cccc-cccccccccccc";

let collectionData: Record<string, unknown> | undefined;
let seriesData: Record<string, unknown>[] = [];

vi.mock("react-router-dom", async () => {
  const actual =
    await vi.importActual<typeof import("react-router-dom")>(
      "react-router-dom",
    );
  return {
    ...actual,
    useParams: () => ({ collectionId: COLLECTION_ID }),
    useNavigate: () => vi.fn(),
  };
});

// Partial mock: the page renders CollectionFormModal, which pulls in the
// create/update hooks, so the untouched exports have to stay real.
vi.mock("@/hooks/useCollections", async (importOriginal) => {
  const actual =
    await importOriginal<typeof import("@/hooks/useCollections")>();
  return {
    ...actual,
    useCollection: () => ({ data: collectionData, isLoading: false }),
    useCollectionSeries: () => ({ data: seriesData }),
    useDeleteCollection: () => ({ mutate: vi.fn(), isPending: false }),
    useRemoveSeriesFromCollection: () => ({
      mutate: vi.fn(),
      isPending: false,
      variables: undefined,
    }),
    useReorderCollection: () => ({ mutate: vi.fn(), isPending: false }),
  };
});

vi.mock("@/hooks/usePermissions", () => ({
  usePermissions: () => ({ hasPermission: () => true }),
}));

vi.mock("@/api/libraries", () => ({
  librariesApi: { getAll: vi.fn(() => Promise.resolve([])) },
}));

function makeSeries(id: string, title: string) {
  return {
    id,
    title,
    libraryId: "lib",
    bookCount: 1,
    createdAt: "2026-01-01T00:00:00Z",
    updatedAt: "2026-01-01T00:00:00Z",
  };
}

function manual(overrides: Record<string, unknown> = {}) {
  return {
    id: COLLECTION_ID,
    name: "Hand Picked",
    summary: null,
    ordered: false,
    condition: null,
    automatic: false,
    seriesCount: 2,
    createdAt: "2026-01-01T00:00:00Z",
    updatedAt: "2026-01-01T00:00:00Z",
    ...overrides,
  };
}

function automatic(
  condition: unknown,
  overrides: Record<string, unknown> = {},
) {
  return manual({
    name: "Isekai",
    automatic: true,
    condition,
    // The API sends null for automatic collections.
    seriesCount: null,
    ...overrides,
  });
}

describe("CollectionDetail for automatic collections", () => {
  beforeEach(() => {
    seriesData = [makeSeries("s1", "Alpha"), makeSeries("s2", "Beta")] as never;
  });

  it("badges an automatic collection and shows its rule", async () => {
    collectionData = automatic({
      tag: { operator: "is", value: "isekai" },
    }) as never;
    renderWithProviders(<CollectionDetail />);

    expect(screen.getByText("Automatic")).toBeInTheDocument();
    expect(screen.getByText("Membership rule")).toBeInTheDocument();
    await waitFor(() => {
      expect(screen.getByText("+ isekai")).toBeInTheDocument();
    });
    expect(
      screen.getByText(/to change what is here, edit the rule/i),
    ).toBeVisible();
  });

  // seriesCount is null for an automatic collection, so the count comes from the
  // already-fetched member list rather than rendering "null series" or "0".
  it("derives the member count from the loaded members", () => {
    collectionData = automatic({
      tag: { operator: "is", value: "isekai" },
    }) as never;
    renderWithProviders(<CollectionDetail />);
    expect(screen.getByText("2 series")).toBeInTheDocument();
  });

  it("labels a rule that reads the viewer's own data", () => {
    collectionData = automatic({
      userRating: { operator: "gte", value: 85 },
    }) as never;
    renderWithProviders(<CollectionDetail />);

    expect(screen.getByText("Personal")).toBeInTheDocument();
    expect(
      screen.getByText(/other people see a different set of series here/i),
    ).toBeVisible();
  });

  it("does not label a rule over library metadata as personal", () => {
    collectionData = automatic({
      tag: { operator: "is", value: "isekai" },
    }) as never;
    renderWithProviders(<CollectionDetail />);
    expect(screen.queryByText("Personal")).not.toBeInTheDocument();
  });

  // Hand-editing is refused by the API (409), so the affordances are absent.
  it("offers no manual sort option", () => {
    collectionData = automatic({
      tag: { operator: "is", value: "isekai" },
    }) as never;
    renderWithProviders(<CollectionDetail />);

    expect(screen.getByText("Title")).toBeInTheDocument();
    expect(screen.queryByText("Manual")).not.toBeInTheDocument();
  });

  it("offers no reorder lock", () => {
    collectionData = automatic({
      tag: { operator: "is", value: "isekai" },
    }) as never;
    renderWithProviders(<CollectionDetail />);
    expect(
      screen.queryByRole("button", { name: /unlock reordering/i }),
    ).not.toBeInTheDocument();
  });

  it("offers no remove action on members", () => {
    collectionData = automatic({
      tag: { operator: "is", value: "isekai" },
    }) as never;
    renderWithProviders(<CollectionDetail />);
    expect(
      screen.queryByRole("button", { name: /remove from collection/i }),
    ).not.toBeInTheDocument();
  });

  it("still offers edit and delete, which act on the rule itself", () => {
    collectionData = automatic({
      tag: { operator: "is", value: "isekai" },
    }) as never;
    renderWithProviders(<CollectionDetail />);
    expect(screen.getByRole("button", { name: "Edit" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Delete" })).toBeInTheDocument();
  });

  it("says no series match, rather than that the collection is empty", () => {
    seriesData = [];
    collectionData = automatic({
      tag: { operator: "is", value: "nothing" },
    }) as never;
    renderWithProviders(<CollectionDetail />);

    expect(screen.getByText(/no series match this rule yet/i)).toBeVisible();
    expect(screen.getByText(/edit the rule to widen it/i)).toBeVisible();
  });
});

describe("CollectionDetail for hand-picked collections", () => {
  beforeEach(() => {
    seriesData = [makeSeries("s1", "Alpha"), makeSeries("s2", "Beta")] as never;
    collectionData = manual() as never;
  });

  it("shows no automatic badge and no rule panel", () => {
    renderWithProviders(<CollectionDetail />);
    expect(screen.queryByText("Automatic")).not.toBeInTheDocument();
    expect(screen.queryByText("Membership rule")).not.toBeInTheDocument();
  });

  it("keeps the manual sort option", () => {
    renderWithProviders(<CollectionDetail />);
    expect(screen.getByText("Manual")).toBeInTheDocument();
  });

  it("reports the server's count", () => {
    renderWithProviders(<CollectionDetail />);
    expect(screen.getByText("2 series")).toBeInTheDocument();
  });

  it("keeps the original empty-state wording", () => {
    seriesData = [];
    collectionData = manual({ seriesCount: 0 }) as never;
    renderWithProviders(<CollectionDetail />);
    expect(
      screen.getByText(/this collection has no series yet/i),
    ).toBeVisible();
  });
});
