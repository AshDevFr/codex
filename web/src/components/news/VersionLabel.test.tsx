import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { type NewsEntryDto, type NewsResponse, newsApi } from "@/api/news";
import { useAuthStore } from "@/store/authStore";
import { renderWithProviders, screen, userEvent, waitFor } from "@/test/utils";
import type { User } from "@/types";
import { VersionLabel } from "./VersionLabel";

vi.mock("@/api/news", () => ({
  newsApi: {
    get: vi.fn(),
    markSeen: vi.fn().mockResolvedValue(undefined),
  },
}));

const adminUser: User = {
  id: "u1",
  username: "admin",
  email: "admin@test.com",
  role: "admin",
  emailVerified: true,
  permissions: [],
};

function release(version: string, body: string): NewsEntryDto {
  return {
    id: `codex-v${version}`,
    project: "codex",
    kind: "release",
    title: `Codex ${version}`,
    version,
    summary: `Summary of ${version}`,
    body,
    url: `https://github.com/AshDevFr/codex/releases/tag/v${version}`,
    publishedAt: "2026-10-20T12:00:00Z",
    unread: true,
  };
}

function news(overrides: Partial<NewsResponse> = {}): NewsResponse {
  return {
    enabled: true,
    lastFetchedAt: "2026-10-20T13:00:00Z",
    unreadCount: 0,
    entries: [],
    ...overrides,
  };
}

const withUpdate = news({
  updateAvailable: {
    currentVersion: "2.6.3",
    latestVersion: "2.7.0",
    url: "https://github.com/AshDevFr/codex/releases/tag/v2.7.0",
    releases: [
      release("2.7.0", "### Features\n\n- **Collections** from the reader"),
      release("2.6.4", "### Bug Fixes\n\n- Keep progress on resume"),
    ],
  },
});

beforeEach(() => {
  vi.mocked(newsApi.get).mockReset();
});

afterEach(() => {
  useAuthStore.setState({ user: null, token: null, isAuthenticated: false });
});

describe("VersionLabel", () => {
  it("is plain text for a non-admin and asks for no news", () => {
    useAuthStore.setState({
      user: { ...adminUser, role: "reader" },
      token: "tok",
      isAuthenticated: true,
    });
    renderWithProviders(<VersionLabel version="2.6.3" />);

    expect(screen.getByText("v2.6.3")).toBeInTheDocument();
    expect(screen.queryByRole("button")).toBeNull();
    expect(newsApi.get).not.toHaveBeenCalled();
  });

  it("is plain text for an admin who is up to date", async () => {
    useAuthStore.setState({
      user: adminUser,
      token: "tok",
      isAuthenticated: true,
    });
    vi.mocked(newsApi.get).mockResolvedValue(news());
    renderWithProviders(<VersionLabel version="2.6.3" />);

    await waitFor(() => expect(newsApi.get).toHaveBeenCalled());
    expect(screen.getByText("v2.6.3")).toBeInTheDocument();
    expect(screen.queryByRole("button")).toBeNull();
  });

  it("shows the target version and opens everything new since the running one", async () => {
    const user = userEvent.setup();
    useAuthStore.setState({
      user: adminUser,
      token: "tok",
      isAuthenticated: true,
    });
    vi.mocked(newsApi.get).mockResolvedValue(withUpdate);
    renderWithProviders(<VersionLabel version="2.6.3" />);

    const button = await screen.findByRole("button", {
      name: "Codex 2.7.0 is available, see what's new since 2.6.3",
    });
    expect(button).toHaveTextContent("v2.6.3 → 2.7.0");

    await user.click(button);

    expect(
      await screen.findByText("What's new since 2.6.3"),
    ).toBeInTheDocument();
    const titles = screen
      .getAllByRole("heading", { level: 3 })
      .map((h) => h.textContent);
    expect(titles).toEqual(["Codex 2.7.0", "Codex 2.6.4"]);
    // Notes are shown in full, not behind a "Show more".
    expect(screen.getByText("Collections")).toBeInTheDocument();
    expect(screen.getByText("Keep progress on resume")).toBeInTheDocument();
  });
});
