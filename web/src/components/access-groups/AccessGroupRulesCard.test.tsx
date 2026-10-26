import { beforeEach, describe, expect, it, vi } from "vitest";
import type { AccessGroupRulesDto } from "@/api/accessGroups";
import { accessGroupsApi } from "@/api/accessGroups";
import { genresApi } from "@/api/genres";
import { librariesApi } from "@/api/libraries";
import { sharingTagsApi } from "@/api/sharingTags";
import { tagsApi } from "@/api/tags";
import { renderWithProviders, screen, userEvent, waitFor } from "@/test/utils";
import { AccessGroupRulesCard } from "./AccessGroupRulesCard";

vi.mock("@mantine/notifications", () => ({
  notifications: { show: vi.fn() },
}));

vi.mock("@/api/accessGroups", () => ({
  accessGroupsApi: {
    getRules: vi.fn(),
    replaceRules: vi.fn(),
  },
}));
vi.mock("@/api/genres", () => ({ genresApi: { getAll: vi.fn() } }));
vi.mock("@/api/tags", () => ({ tagsApi: { getAll: vi.fn() } }));
vi.mock("@/api/libraries", () => ({ librariesApi: { getAll: vi.fn() } }));
vi.mock("@/api/sharingTags", () => ({ sharingTagsApi: { list: vi.fn() } }));

const rules = (
  overrides: Partial<AccessGroupRulesDto> = {},
): AccessGroupRulesDto => ({
  sharingTagMatch: "any",
  tagMatch: "any",
  genreMatch: "any",
  sharingTags: [],
  tags: [],
  genres: [],
  libraries: [],
  ...overrides,
});

describe("AccessGroupRulesCard", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(genresApi.getAll).mockResolvedValue([
      {
        id: "g1",
        name: "Adult",
        seriesCount: 3,
        createdAt: "2026-01-01T00:00:00Z",
      },
    ] as never);
    vi.mocked(tagsApi.getAll).mockResolvedValue([]);
    vi.mocked(librariesApi.getAll).mockResolvedValue([
      { id: "lib-kids", name: "Kids" },
      { id: "lib-manga", name: "Manga" },
    ] as never);
    vi.mocked(sharingTagsApi.list).mockResolvedValue([]);
    vi.mocked(accessGroupsApi.replaceRules).mockImplementation(
      async (_id, request) =>
        rules({
          ...request,
          sharingTags: [],
          libraries: (request.libraries ?? []).map((l) => ({
            ...l,
            name: l.libraryId,
          })),
        } as never),
    );
  });

  it("shows the existing rules and that admins are not restricted", async () => {
    vi.mocked(accessGroupsApi.getRules).mockResolvedValue(
      rules({
        genres: [{ name: "Adult", accessMode: "deny" }],
        libraries: [
          { libraryId: "lib-kids", name: "Kids", accessMode: "allow" },
        ],
      }),
    );
    renderWithProviders(<AccessGroupRulesCard groupId="group-1" />);

    expect(await screen.findByText("Adult")).toBeInTheDocument();
    expect(screen.getByText("Kids")).toBeInTheDocument();
    expect(
      screen.getByText(/admins and maintainers are never restricted/i),
    ).toBeInTheDocument();
  });

  it("adds a genre deny on top of the existing rules", async () => {
    const user = userEvent.setup();
    vi.mocked(accessGroupsApi.getRules).mockResolvedValue(
      rules({
        libraries: [
          { libraryId: "lib-kids", name: "Kids", accessMode: "allow" },
        ],
      }),
    );
    renderWithProviders(<AccessGroupRulesCard groupId="group-1" />);

    const input = await screen.findByRole("textbox", { name: /add genre/i });
    await user.type(input, "Adult");
    await user.click(screen.getByRole("button", { name: /deny genre/i }));

    await waitFor(() =>
      expect(accessGroupsApi.replaceRules).toHaveBeenCalledWith(
        "group-1",
        expect.objectContaining({
          genres: [{ name: "Adult", accessMode: "deny" }],
          libraries: [{ libraryId: "lib-kids", accessMode: "allow" }],
        }),
      ),
    );
  });

  it("removes a rule", async () => {
    const user = userEvent.setup();
    vi.mocked(accessGroupsApi.getRules).mockResolvedValue(
      rules({
        genres: [
          { name: "Adult", accessMode: "deny" },
          { name: "Horror", accessMode: "deny" },
        ],
      }),
    );
    renderWithProviders(<AccessGroupRulesCard groupId="group-1" />);

    await user.click(
      await screen.findByRole("button", { name: "Remove genre Adult" }),
    );

    await waitFor(() =>
      expect(accessGroupsApi.replaceRules).toHaveBeenCalledWith(
        "group-1",
        expect.objectContaining({
          genres: [{ name: "Horror", accessMode: "deny" }],
        }),
      ),
    );
  });

  it("switches how allowed genres combine", async () => {
    const user = userEvent.setup();
    vi.mocked(accessGroupsApi.getRules).mockResolvedValue(
      rules({
        genres: [
          { name: "Kids", accessMode: "allow" },
          { name: "Comedy", accessMode: "allow" },
        ],
      }),
    );
    renderWithProviders(<AccessGroupRulesCard groupId="group-1" />);

    await user.click(
      await screen.findByRole("radio", { name: /all allowed genres/i }),
    );

    await waitFor(() =>
      expect(accessGroupsApi.replaceRules).toHaveBeenCalledWith(
        "group-1",
        expect.objectContaining({ genreMatch: "all" }),
      ),
    );
  });

  it("offers no match switch for libraries", async () => {
    vi.mocked(accessGroupsApi.getRules).mockResolvedValue(
      rules({
        libraries: [
          { libraryId: "lib-kids", name: "Kids", accessMode: "allow" },
          { libraryId: "lib-manga", name: "Manga", accessMode: "allow" },
        ],
      }),
    );
    renderWithProviders(<AccessGroupRulesCard groupId="group-1" />);

    await screen.findByText("Manga");
    expect(
      screen.queryByRole("radio", { name: /all allowed libraries/i }),
    ).not.toBeInTheDocument();
  });
});
