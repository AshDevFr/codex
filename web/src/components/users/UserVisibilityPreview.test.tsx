import { beforeEach, describe, expect, it, vi } from "vitest";
import type { VisibilityPreviewDto } from "@/api/accessGroups";
import { accessGroupsApi } from "@/api/accessGroups";
import { seriesApi } from "@/api/series";
import { renderWithProviders, screen, userEvent, waitFor } from "@/test/utils";
import { UserVisibilityPreview } from "./UserVisibilityPreview";

vi.mock("@/api/accessGroups", () => ({
  accessGroupsApi: { getVisibilityPreview: vi.fn() },
}));
vi.mock("@/api/series", () => ({ seriesApi: { search: vi.fn() } }));

const summary = (
  overrides: Partial<VisibilityPreviewDto> = {},
): VisibilityPreviewDto => ({
  userId: "kid",
  restricted: true,
  roleBypass: false,
  visibleSeriesCount: 12,
  hiddenSeriesCount: 30,
  libraries: [
    { id: "l1", name: "Kids", visible: true },
    { id: "l2", name: "Adult", visible: false },
  ],
  series: null,
  ...overrides,
});

describe("UserVisibilityPreview", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("summarises what the user can see", async () => {
    vi.mocked(accessGroupsApi.getVisibilityPreview).mockResolvedValue(
      summary(),
    );
    renderWithProviders(<UserVisibilityPreview userId="kid" />);

    expect(await screen.findByText(/12 series visible/i)).toBeInTheDocument();
    expect(screen.getByText(/30 hidden/i)).toBeInTheDocument();
    expect(screen.getByLabelText("Kids: visible")).toBeInTheDocument();
    expect(screen.getByLabelText("Adult: hidden")).toBeInTheDocument();
  });

  it("says when the role exempts the user", async () => {
    vi.mocked(accessGroupsApi.getVisibilityPreview).mockResolvedValue(
      summary({ restricted: false, roleBypass: true, hiddenSeriesCount: 0 }),
    );
    renderWithProviders(<UserVisibilityPreview userId="admin" />);

    expect(
      await screen.findByText(/role is never restricted/i),
    ).toBeInTheDocument();
  });

  it("explains why a picked series is hidden", async () => {
    const user = userEvent.setup();
    vi.mocked(accessGroupsApi.getVisibilityPreview).mockImplementation(
      async (_user, seriesId) =>
        seriesId
          ? summary({
              series: {
                seriesId,
                title: "Adult manga",
                visible: false,
                decision: "denied",
                deniedBy: [
                  {
                    ruleType: "genre",
                    value: "Adult",
                    source: { kind: "group", groupId: "g", groupName: "Kids" },
                  },
                ],
                allowedBy: [],
              },
            })
          : summary(),
    );
    vi.mocked(seriesApi.search).mockResolvedValue({
      data: [{ id: "s1", title: "Adult manga" }],
    } as never);
    renderWithProviders(<UserVisibilityPreview userId="kid" />);

    await user.type(
      await screen.findByRole("textbox", { name: /check a series/i }),
      "Adult",
    );
    await user.click(
      await screen.findByRole("option", { name: "Adult manga", hidden: true }),
    );

    await waitFor(() =>
      expect(accessGroupsApi.getVisibilityPreview).toHaveBeenCalledWith(
        "kid",
        "s1",
      ),
    );
    expect(await screen.findByText(/hidden by/i)).toBeInTheDocument();
    expect(screen.getByText(/genre "Adult"/i)).toBeInTheDocument();
    expect(screen.getByText(/group Kids/i)).toBeInTheDocument();
  });
});
