import { screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import * as apiClientModule from "@/api/client";
import * as duplicatesApiModule from "@/api/duplicates";
import { renderWithProviders } from "@/test/utils";
import { DuplicatesSettings } from "./DuplicatesSettings";

vi.mock("@/api/duplicates", () => ({
  duplicatesApi: {
    list: vi.fn(),
    scan: vi.fn(),
    delete: vi.fn(),
  },
  seriesDuplicatesApi: {
    list: vi.fn(),
    delete: vi.fn(),
  },
}));

vi.mock("@/api/client", () => ({
  api: {
    get: vi.fn(),
    post: vi.fn(),
    delete: vi.fn(),
  },
}));

// Stub task-progress so the component does not try to open an SSE connection.
vi.mock("@/hooks/useTaskProgress", () => ({
  useTaskProgress: () => ({
    activeTasks: [],
    connectionState: "connected" as const,
    pendingCounts: {},
    getTasksByStatus: () => [],
    getTasksByLibrary: () => [],
    getTask: () => undefined,
  }),
}));

const { duplicatesApi, seriesDuplicatesApi } = duplicatesApiModule;
const { api } = apiClientModule;

const LIBRARY_ID = "11111111-1111-1111-1111-111111111111";
const SERIES_ID_A = "22222222-2222-2222-2222-22222222aaaa";
const SERIES_ID_B = "22222222-2222-2222-2222-22222222bbbb";
const BOOK_ID_A = "33333333-3333-3333-3333-33333333aaaa";
const BOOK_ID_B = "33333333-3333-3333-3333-33333333bbbb";

const bookGroup: duplicatesApiModule.DuplicateGroup = {
  id: "book-group-1",
  fileHash: "deadbeefcafefacedeadbeefcafeface00000000000000000000000000000000",
  bookIds: [BOOK_ID_A, BOOK_ID_B],
  duplicateCount: 2,
  createdAt: "2026-05-19T10:00:00Z",
  updatedAt: "2026-05-19T11:00:00Z",
};

const memberA: duplicatesApiModule.SeriesDuplicateMember = {
  id: SERIES_ID_A,
  libraryId: LIBRARY_ID,
  libraryName: "Manga",
  title: "Naruto",
  bookCount: 72,
  updatedAt: "2026-05-01T00:00:00Z",
};
const memberB: duplicatesApiModule.SeriesDuplicateMember = {
  ...memberA,
  id: SERIES_ID_B,
  title: "ナルト",
  bookCount: 1,
};

const externalIdGroup: duplicatesApiModule.SeriesDuplicateGroup = {
  id: "series-group-external",
  matchType: "external_id",
  matchKey: "plugin:mangabaka:12345",
  libraryId: null,
  members: [memberA, memberB],
  duplicateCount: 2,
  createdAt: "2026-05-19T08:00:00Z",
  updatedAt: "2026-05-19T09:00:00Z",
};

const titleGroup: duplicatesApiModule.SeriesDuplicateGroup = {
  id: "series-group-title",
  matchType: "title",
  matchKey: "naruto",
  libraryId: LIBRARY_ID,
  members: [memberA, memberB],
  duplicateCount: 2,
  createdAt: "2026-05-19T08:30:00Z",
  updatedAt: "2026-05-19T09:30:00Z",
};

const bookA = {
  id: BOOK_ID_A,
  libraryId: LIBRARY_ID,
  libraryName: "Manga",
  seriesId: SERIES_ID_A,
  seriesName: "Naruto",
  title: "Naruto Vol. 1",
  path: "/library/naruto/vol1.cbz",
  fileSize: 12_000_000,
};
const bookB = {
  ...bookA,
  id: BOOK_ID_B,
  title: "Naruto Vol. 1 (copy)",
  path: "/library/naruto-copy/vol1.cbz",
};

function setupHappyPath(
  options: {
    bookGroups?: duplicatesApiModule.DuplicateGroup[];
    seriesGroups?: duplicatesApiModule.SeriesDuplicateGroup[];
  } = {},
) {
  const bookGroups = options.bookGroups ?? [bookGroup];
  const seriesGroups = options.seriesGroups ?? [externalIdGroup, titleGroup];

  vi.mocked(duplicatesApi.list).mockResolvedValue(bookGroups);
  vi.mocked(seriesDuplicatesApi.list).mockResolvedValue({
    duplicates: seriesGroups,
    totalGroups: seriesGroups.length,
    totalDuplicateSeries: seriesGroups.reduce(
      (sum, g) => sum + g.duplicateCount,
      0,
    ),
    externalIdGroups: seriesGroups.filter((g) => g.matchType === "external_id")
      .length,
    titleGroups: seriesGroups.filter((g) => g.matchType === "title").length,
  });

  // Detail fetches for the expanded rows. Series rows are hydrated via the
  // group `members` field, so only book detail and the trusted-sources setting
  // are fetched here.
  vi.mocked(api.get).mockImplementation((async (url: string) => {
    if (url === `/books/${BOOK_ID_A}`) {
      return { data: { book: bookA } };
    }
    if (url === `/books/${BOOK_ID_B}`) {
      return { data: { book: bookB } };
    }
    if (url.startsWith("/admin/settings/")) {
      return {
        data: {
          key: "duplicate_detection.trusted_external_id_sources",
          value: "[]",
          updatedAt: "2026-05-01T00:00:00Z",
        },
      };
    }
    throw new Error(`Unexpected GET ${url}`);
  }) as typeof api.get);
}

describe("DuplicatesSettings", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("renders the top-level heading and both tabs", async () => {
    setupHappyPath();
    renderWithProviders(<DuplicatesSettings />);

    expect(
      screen.getByRole("heading", { level: 1, name: /duplicate detection/i }),
    ).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: /books/i })).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: /series/i })).toBeInTheDocument();
  });

  it("renders book duplicate groups on the Books tab", async () => {
    setupHappyPath();
    renderWithProviders(<DuplicatesSettings />);

    await waitFor(() => {
      expect(screen.getByText("Naruto Vol. 1")).toBeInTheDocument();
    });
    // Hash preview is shown.
    expect(screen.getByText(/deadbeefcafeface/)).toBeInTheDocument();
  });

  it("switches to the Series tab and renders confidence badges", async () => {
    const user = userEvent.setup();
    setupHappyPath();
    renderWithProviders(<DuplicatesSettings />);

    await user.click(screen.getByRole("tab", { name: /series/i }));

    await waitFor(() => {
      expect(screen.getByText(/plugin:mangabaka:12345/)).toBeInTheDocument();
    });
    // The phrases appear in both the explanatory alert and the badge — assert
    // each shows up at least once.
    expect(screen.getAllByText(/High confidence/i).length).toBeGreaterThan(0);
    expect(screen.getAllByText(/Possible match/i).length).toBeGreaterThan(0);
    // Match-key explanation summary stats.
    expect(screen.getByText(/Total Series/i)).toBeInTheDocument();
  });

  it("calls seriesDuplicatesApi.delete when the trash button is clicked", async () => {
    const user = userEvent.setup();
    setupHappyPath({ seriesGroups: [externalIdGroup] });
    vi.mocked(seriesDuplicatesApi.delete).mockResolvedValue(undefined);

    renderWithProviders(<DuplicatesSettings />);
    await user.click(screen.getByRole("tab", { name: /series/i }));

    await waitFor(() => {
      expect(screen.getByText(/plugin:mangabaka:12345/)).toBeInTheDocument();
    });

    const deleteButton = screen.getByRole("button", {
      name: /delete series duplicate group/i,
    });
    await user.click(deleteButton);

    await waitFor(() => {
      expect(seriesDuplicatesApi.delete).toHaveBeenCalledWith(
        "series-group-external",
      );
    });
  });

  it("shows the series empty state with a scan button when no series groups exist", async () => {
    const user = userEvent.setup();
    setupHappyPath({ seriesGroups: [] });
    vi.mocked(duplicatesApi.scan).mockResolvedValue({
      taskId: "task-1",
      message: "Duplicate scan queued",
    });

    renderWithProviders(<DuplicatesSettings />);
    await user.click(screen.getByRole("tab", { name: /series/i }));

    await waitFor(() => {
      expect(
        screen.getByText(/no duplicate series detected/i),
      ).toBeInTheDocument();
    });

    // The "Scan Now" button inside the empty card calls the shared scan handler.
    const scanNowButton = screen.getByRole("button", { name: /scan now/i });
    await user.click(scanNowButton);

    await waitFor(() => {
      expect(duplicatesApi.scan).toHaveBeenCalledTimes(1);
    });
  });

  it("expands a series group and surfaces hydrated member rows", async () => {
    const user = userEvent.setup();
    setupHappyPath({ seriesGroups: [externalIdGroup] });
    renderWithProviders(<DuplicatesSettings />);

    await user.click(screen.getByRole("tab", { name: /series/i }));
    await waitFor(() => {
      expect(screen.getByText(/plugin:mangabaka:12345/)).toBeInTheDocument();
    });

    const showDetailsButton = screen.getByRole("button", {
      name: /show details/i,
    });
    await user.click(showDetailsButton);

    await waitFor(() => {
      const table = screen.getByRole("table");
      expect(within(table).getByText("Naruto")).toBeInTheDocument();
      expect(within(table).getByText("ナルト")).toBeInTheDocument();
    });
  });
});
