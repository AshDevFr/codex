import { notifications } from "@mantine/notifications";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { AccessGroupDetailDto } from "@/api/accessGroups";
import { accessGroupsApi } from "@/api/accessGroups";
import { renderWithProviders, screen, userEvent, waitFor } from "@/test/utils";
import { DefaultGroupCard } from "./DefaultGroupCard";

vi.mock("@mantine/notifications", () => ({
  notifications: { show: vi.fn() },
}));

vi.mock("@/api/accessGroups", () => ({
  accessGroupsApi: {
    update: vi.fn(),
    addAllUsers: vi.fn(),
  },
}));

const makeGroup = (
  overrides: Partial<AccessGroupDetailDto> = {},
): AccessGroupDetailDto => ({
  id: "group-1",
  name: "Kids",
  description: null,
  isDefault: false,
  members: [],
  grants: [],
  oidcMappings: [],
  createdAt: "2026-01-01T00:00:00Z",
  updatedAt: "2026-01-01T00:00:00Z",
  ...overrides,
});

describe("DefaultGroupCard", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("reflects whether the group is the default", () => {
    renderWithProviders(
      <DefaultGroupCard group={makeGroup({ isDefault: true })} />,
    );

    expect(
      screen.getByRole("switch", { name: /default group for new users/i }),
    ).toBeChecked();
  });

  it("makes the group the default when switched on", async () => {
    const user = userEvent.setup();
    vi.mocked(accessGroupsApi.update).mockResolvedValue({
      ...makeGroup({ isDefault: true }),
      memberCount: 0,
      grantCount: 0,
    });
    renderWithProviders(<DefaultGroupCard group={makeGroup()} />);

    await user.click(
      screen.getByRole("switch", { name: /default group for new users/i }),
    );

    await waitFor(() =>
      expect(accessGroupsApi.update).toHaveBeenCalledWith("group-1", {
        isDefault: true,
      }),
    );
  });

  it("adds all existing users only after confirmation", async () => {
    const user = userEvent.setup();
    vi.mocked(accessGroupsApi.addAllUsers).mockResolvedValue({ added: 4 });
    renderWithProviders(<DefaultGroupCard group={makeGroup()} />);

    await user.click(
      screen.getByRole("button", { name: /add all existing users/i }),
    );
    expect(accessGroupsApi.addAllUsers).not.toHaveBeenCalled();

    await user.click(
      await screen.findByRole("button", { name: /^add all users$/i }),
    );

    await waitFor(() =>
      expect(accessGroupsApi.addAllUsers).toHaveBeenCalledWith("group-1"),
    );
    await waitFor(() =>
      expect(notifications.show).toHaveBeenCalledWith(
        expect.objectContaining({ message: "Added 4 users" }),
      ),
    );
  });
});
