import { beforeEach, describe, expect, it, vi } from "vitest";
import { librariesApi } from "@/api/libraries";
import { renderWithProviders, screen, userEvent } from "@/test/utils";
import { SeriesFilterPanel } from "./SeriesFilterPanel";

vi.mock("@/api/libraries");
vi.mock("@/api/sharingTags", () => ({
  sharingTagsApi: { list: vi.fn(() => Promise.resolve([])) },
}));
vi.mock("@/hooks/useReferenceData", () => ({
  useAllGenres: () => ({ data: [{ name: "Action" }], isLoading: false }),
  useAllTags: () => ({ data: [], isLoading: false }),
}));

const LIB_A = "11111111-1111-1111-1111-111111111111";
const LIB_B = "22222222-2222-2222-2222-222222222222";

const LIBRARIES = [
  { id: LIB_A, name: "Manga" },
  { id: LIB_B, name: "Comics" },
];

async function openDrawer() {
  const user = userEvent.setup();
  await user.click(screen.getByRole("button", { name: /filter options/i }));
  return user;
}

describe("SeriesFilterPanel libraries group", () => {
  beforeEach(() => {
    vi.mocked(librariesApi.getAll).mockResolvedValue(
      LIBRARIES as Awaited<ReturnType<typeof librariesApi.getAll>>,
    );
  });

  it("shows the Libraries group in all-libraries scope", async () => {
    renderWithProviders(<SeriesFilterPanel libraryId="all" />);
    await openDrawer();

    expect(await screen.findByText("Libraries")).toBeInTheDocument();
    expect(await screen.findByText("Manga")).toBeInTheDocument();
    expect(await screen.findByText("Comics")).toBeInTheDocument();
  });

  it("treats a missing libraryId as all-libraries scope", async () => {
    renderWithProviders(<SeriesFilterPanel />);
    await openDrawer();

    expect(await screen.findByText("Libraries")).toBeInTheDocument();
  });

  // On a single-library route the request is already scoped to that library and
  // the condition is ANDed with it, so any other choice can only return nothing.
  it("hides the Libraries group when scoped to one library", async () => {
    renderWithProviders(<SeriesFilterPanel libraryId={LIB_A} />);
    await openDrawer();

    // Wait for the drawer body to settle on a control we do expect.
    expect(await screen.findByText("Read Status")).toBeInTheDocument();
    expect(screen.queryByText("Libraries")).not.toBeInTheDocument();
  });

  it("does not fetch libraries when scoped to one library", async () => {
    renderWithProviders(<SeriesFilterPanel libraryId={LIB_A} />);
    await openDrawer();

    expect(await screen.findByText("Read Status")).toBeInTheDocument();
    expect(librariesApi.getAll).not.toHaveBeenCalled();
  });

  it("offers no all/any mode toggle, since a series has exactly one library", async () => {
    renderWithProviders(<SeriesFilterPanel libraryId="all" />);
    await openDrawer();

    await screen.findByText("Libraries");
    // Genres is the only group in this fixture that opts into the toggle, so
    // exactly one All/Any control proves Libraries did not add a second.
    expect(screen.getAllByText("Any")).toHaveLength(1);
    expect(screen.getAllByText("All")).toHaveLength(1);
  });
});
