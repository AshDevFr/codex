import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { type NewsEntryDto, type NewsResponse, newsApi } from "@/api/news";
import { useAuthStore } from "@/store/authStore";
import { renderWithProviders, screen, userEvent, waitFor } from "@/test/utils";
import type { User } from "@/types";
import { NewsButton } from "./NewsButton";

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

const readerUser: User = { ...adminUser, id: "u2", role: "reader" };

function entry(overrides: Partial<NewsEntryDto> = {}): NewsEntryDto {
  return {
    id: "codex-ios-1.1.0",
    project: "codex-ios",
    kind: "release",
    title: "Codex Reader (iOS) 1.1.0",
    version: "1.1.0",
    summary: "Collections and read lists from the Actions menu.",
    body: "- Collections and read lists from the Actions menu.",
    url: "https://apps.apple.com/us/app/codex-reader-manga-comics/id6801989441",
    publishedAt: "2026-10-16T16:16:49Z",
    unread: true,
    ...overrides,
  };
}

function news(overrides: Partial<NewsResponse> = {}): NewsResponse {
  return {
    enabled: true,
    lastFetchedAt: "2026-10-18T04:17:00Z",
    unreadCount: 1,
    entries: [entry()],
    ...overrides,
  };
}

function signIn(user: User) {
  useAuthStore.setState({ user, token: "tok", isAuthenticated: true });
}

beforeEach(() => {
  vi.mocked(newsApi.get).mockReset();
  vi.mocked(newsApi.markSeen).mockReset().mockResolvedValue(undefined);
});

afterEach(() => {
  useAuthStore.setState({ user: null, token: null, isAuthenticated: false });
});

describe("NewsButton", () => {
  it("renders nothing for a non-admin and never asks for news", () => {
    signIn(readerUser);
    renderWithProviders(<NewsButton />);

    expect(screen.queryByRole("button", { name: /news/i })).toBeNull();
    expect(newsApi.get).not.toHaveBeenCalled();
  });

  it("renders nothing when the server has news turned off", async () => {
    signIn(adminUser);
    vi.mocked(newsApi.get).mockResolvedValue(
      news({ enabled: false, unreadCount: 0, entries: [] }),
    );
    renderWithProviders(<NewsButton />);

    await waitFor(() => expect(newsApi.get).toHaveBeenCalled());
    expect(screen.queryByRole("button", { name: /news/i })).toBeNull();
  });

  it("shows an unread marker only when something is unread", async () => {
    signIn(adminUser);
    vi.mocked(newsApi.get).mockResolvedValue(news());
    const { unmount } = renderWithProviders(<NewsButton />);

    expect(
      await screen.findByRole("button", { name: "News, 1 unread" }),
    ).toBeInTheDocument();
    unmount();

    vi.mocked(newsApi.get).mockResolvedValue(
      news({ unreadCount: 0, entries: [entry({ unread: false })] }),
    );
    renderWithProviders(<NewsButton />);
    expect(
      await screen.findByRole("button", { name: "News" }),
    ).toBeInTheDocument();
  });

  it("opens the drawer, marks news seen, and keeps the new marker while open", async () => {
    const user = userEvent.setup();
    signIn(adminUser);
    vi.mocked(newsApi.get)
      .mockResolvedValueOnce(news())
      .mockResolvedValue(
        news({ unreadCount: 0, entries: [entry({ unread: false })] }),
      );
    renderWithProviders(<NewsButton />);

    await user.click(
      await screen.findByRole("button", { name: "News, 1 unread" }),
    );

    expect(
      await screen.findByText("Codex Reader (iOS) 1.1.0"),
    ).toBeInTheDocument();
    expect(screen.getByText("Codex Reader")).toBeInTheDocument();
    await waitFor(() => expect(newsApi.markSeen).toHaveBeenCalledTimes(1));
    // The refetch after marking seen reports nothing unread, but what was new
    // when the drawer opened stays marked until it closes.
    await waitFor(() => expect(newsApi.get).toHaveBeenCalledTimes(2));
    expect(screen.getByText("New")).toBeInTheDocument();
  });

  it("expands the full notes and renders raw HTML as text", async () => {
    const user = userEvent.setup();
    signIn(adminUser);
    vi.mocked(newsApi.get).mockResolvedValue(
      news({
        entries: [
          entry({
            body: "Fixed <script>alert(1)</script> and [a link](javascript:alert(1))",
          }),
        ],
      }),
    );
    const { container } = renderWithProviders(<NewsButton />);

    await user.click(await screen.findByRole("button", { name: /news/i }));
    await user.click(await screen.findByRole("button", { name: "Show more" }));

    expect(document.querySelector("script")).toBeNull();
    for (const link of Array.from(document.querySelectorAll("a"))) {
      expect(link.getAttribute("href") ?? "").not.toMatch(/^javascript:/i);
    }
    expect(container).toBeDefined();
  });

  it("announces an available update at the top of the drawer", async () => {
    const user = userEvent.setup();
    signIn(adminUser);
    vi.mocked(newsApi.get).mockResolvedValue(
      news({
        unreadCount: 0,
        entries: [],
        updateAvailable: {
          currentVersion: "2.6.3",
          latestVersion: "2.7.0",
          url: "https://github.com/AshDevFr/codex/releases/tag/v2.7.0",
        },
      }),
    );
    renderWithProviders(<NewsButton />);

    await user.click(
      await screen.findByRole("button", { name: "News, update available" }),
    );

    expect(
      await screen.findByText(/Codex 2\.7\.0 is available/),
    ).toBeInTheDocument();
    expect(screen.getByText(/You are running 2\.6\.3/)).toBeInTheDocument();
  });

  it("shows an empty state when nothing has been fetched", async () => {
    const user = userEvent.setup();
    signIn(adminUser);
    vi.mocked(newsApi.get).mockResolvedValue(
      news({ unreadCount: 0, entries: [], lastFetchedAt: null }),
    );
    renderWithProviders(<NewsButton />);

    await user.click(await screen.findByRole("button", { name: "News" }));
    expect(await screen.findByText("No news yet.")).toBeInTheDocument();
  });
});
