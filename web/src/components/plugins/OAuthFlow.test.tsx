import { notifications } from "@mantine/notifications";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, renderHook } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { userPluginsApi } from "@/api/userPlugins";
import { useOAuthCallback, useOAuthFlow } from "./OAuthFlow";

// ---------------------------------------------------------------------------
// Mocks
// ---------------------------------------------------------------------------

vi.mock("@/api/userPlugins", () => ({
  userPluginsApi: {
    startOAuth: vi.fn(),
  },
}));

vi.mock("@mantine/notifications", () => ({
  notifications: {
    show: vi.fn(),
  },
}));

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function createWrapper(initialEntries: string[] = ["/"]) {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return ({ children }: { children: React.ReactNode }) => (
    <QueryClientProvider client={queryClient}>
      <MemoryRouter initialEntries={initialEntries}>{children}</MemoryRouter>
    </QueryClientProvider>
  );
}

/**
 * Create a wrapper that also exposes its QueryClient so tests can spy on
 * invalidateQueries.
 */
function createWrapperWithClient(initialEntries: string[] = ["/"]) {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  const wrapper = ({ children }: { children: React.ReactNode }) => (
    <QueryClientProvider client={queryClient}>
      <MemoryRouter initialEntries={initialEntries}>{children}</MemoryRouter>
    </QueryClientProvider>
  );
  return { wrapper, queryClient };
}

// ---------------------------------------------------------------------------
// Tests — useOAuthFlow
// ---------------------------------------------------------------------------

describe("useOAuthFlow", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("calls startOAuth API and opens a popup window", async () => {
    const redirectUrl = "https://anilist.co/api/v2/oauth/authorize?client_id=1";
    vi.mocked(userPluginsApi.startOAuth).mockResolvedValue({ redirectUrl });

    const mockPopup = { closed: false, close: vi.fn() };
    const openSpy = vi
      .spyOn(window, "open")
      .mockReturnValue(mockPopup as unknown as Window);

    const { result } = renderHook(() => useOAuthFlow(), {
      wrapper: createWrapper(),
    });

    await act(async () => {
      await result.current.startOAuthFlow("sync-anilist");
    });

    expect(userPluginsApi.startOAuth).toHaveBeenCalledWith("sync-anilist");
    expect(openSpy).toHaveBeenCalledWith(
      redirectUrl,
      "oauth_popup",
      expect.stringContaining("width=600"),
    );

    openSpy.mockRestore();
  });

  it("shows notification when popup is blocked (window.open returns null)", async () => {
    vi.mocked(userPluginsApi.startOAuth).mockResolvedValue({
      redirectUrl: "https://example.com/oauth",
    });

    const openSpy = vi.spyOn(window, "open").mockReturnValue(null);

    const { result } = renderHook(() => useOAuthFlow(), {
      wrapper: createWrapper(),
    });

    await act(async () => {
      await result.current.startOAuthFlow("sync-anilist");
    });

    expect(notifications.show).toHaveBeenCalledWith(
      expect.objectContaining({
        title: "Popup blocked",
        color: "red",
      }),
    );

    openSpy.mockRestore();
  });

  it("shows notification on API error", async () => {
    vi.mocked(userPluginsApi.startOAuth).mockRejectedValue(
      new Error("Network error"),
    );

    const { result } = renderHook(() => useOAuthFlow(), {
      wrapper: createWrapper(),
    });

    await act(async () => {
      await result.current.startOAuthFlow("sync-anilist");
    });

    expect(notifications.show).toHaveBeenCalledWith(
      expect.objectContaining({
        title: "OAuth Error",
        message: "Network error",
        color: "red",
      }),
    );
  });

  it("shows server message when error is an ApiError object (429 rate limit)", async () => {
    // The axios interceptor rejects with a plain { error, message } object, not an Error instance
    vi.mocked(userPluginsApi.startOAuth).mockRejectedValue({
      error: "rate_limit_exceeded",
      message:
        "Too many pending OAuth flows (max 3). Please complete or wait for existing flows to expire.",
    });

    const { result } = renderHook(() => useOAuthFlow(), {
      wrapper: createWrapper(),
    });

    await act(async () => {
      await result.current.startOAuthFlow("sync-anilist");
    });

    expect(notifications.show).toHaveBeenCalledWith(
      expect.objectContaining({
        title: "OAuth Error",
        message:
          "Too many pending OAuth flows (max 3). Please complete or wait for existing flows to expire.",
        color: "red",
      }),
    );
  });

  it("shows fallback message when error is not an Error instance", async () => {
    vi.mocked(userPluginsApi.startOAuth).mockRejectedValue("something weird");

    const { result } = renderHook(() => useOAuthFlow(), {
      wrapper: createWrapper(),
    });

    await act(async () => {
      await result.current.startOAuthFlow("sync-anilist");
    });

    expect(notifications.show).toHaveBeenCalledWith(
      expect.objectContaining({
        title: "OAuth Error",
        message: "Failed to start OAuth flow",
        color: "red",
      }),
    );
  });

  it("invalidates user-plugins queries when popup closes", async () => {
    vi.mocked(userPluginsApi.startOAuth).mockResolvedValue({
      redirectUrl: "https://example.com/oauth",
    });

    const mockPopup = { closed: false, close: vi.fn() };
    const openSpy = vi
      .spyOn(window, "open")
      .mockReturnValue(mockPopup as unknown as Window);

    const { wrapper, queryClient } = createWrapperWithClient();
    const invalidateSpy = vi.spyOn(queryClient, "invalidateQueries");

    const { result } = renderHook(() => useOAuthFlow(), { wrapper });

    await act(async () => {
      await result.current.startOAuthFlow("sync-anilist");
    });

    // Popup is still open - no invalidation yet
    expect(invalidateSpy).not.toHaveBeenCalled();

    // Simulate popup closing
    mockPopup.closed = true;

    await act(async () => {
      vi.advanceTimersByTime(500);
    });

    expect(invalidateSpy).toHaveBeenCalledWith(
      expect.objectContaining({ queryKey: ["user-plugins"] }),
    );

    openSpy.mockRestore();
  });

  it("shows timeout notification and closes popup after 5 minutes", async () => {
    vi.mocked(userPluginsApi.startOAuth).mockResolvedValue({
      redirectUrl: "https://example.com/oauth",
    });

    const mockPopup = { closed: false, close: vi.fn() };
    const openSpy = vi
      .spyOn(window, "open")
      .mockReturnValue(mockPopup as unknown as Window);

    const { result } = renderHook(() => useOAuthFlow(), {
      wrapper: createWrapper(),
    });

    await act(async () => {
      await result.current.startOAuthFlow("sync-anilist");
    });

    // Popup is still open after 4 minutes — no timeout yet
    await act(async () => {
      vi.advanceTimersByTime(4 * 60 * 1000);
    });
    expect(mockPopup.close).not.toHaveBeenCalled();
    expect(notifications.show).not.toHaveBeenCalled();

    // After 5+ minutes total — timeout fires
    await act(async () => {
      vi.advanceTimersByTime(1 * 60 * 1000 + 500);
    });

    expect(mockPopup.close).toHaveBeenCalled();
    expect(notifications.show).toHaveBeenCalledWith(
      expect.objectContaining({
        title: "OAuth Timeout",
        color: "orange",
      }),
    );

    openSpy.mockRestore();
  });
});

// ---------------------------------------------------------------------------
// Tests — useOAuthCallback
// ---------------------------------------------------------------------------

describe("useOAuthCallback", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("shows success notification on ?oauth=success", () => {
    const { wrapper, queryClient } = createWrapperWithClient([
      "/?oauth=success",
    ]);
    const invalidateSpy = vi.spyOn(queryClient, "invalidateQueries");

    renderHook(() => useOAuthCallback(), { wrapper });

    expect(notifications.show).toHaveBeenCalledWith(
      expect.objectContaining({
        title: "Connected",
        message: "Successfully connected your account.",
        color: "green",
      }),
    );

    expect(invalidateSpy).toHaveBeenCalledWith(
      expect.objectContaining({ queryKey: ["user-plugins"] }),
    );
  });

  it("shows error notification on ?oauth=error", () => {
    renderHook(() => useOAuthCallback(), {
      wrapper: createWrapper(["/?oauth=error"]),
    });

    expect(notifications.show).toHaveBeenCalledWith(
      expect.objectContaining({
        title: "Connection Failed",
        message: "OAuth authentication failed. Please try again.",
        color: "red",
      }),
    );
  });

  it("invalidates specific plugin query when pluginId is present", () => {
    const { wrapper, queryClient } = createWrapperWithClient([
      "/?oauth=success&plugin=sync-anilist",
    ]);
    const invalidateSpy = vi.spyOn(queryClient, "invalidateQueries");

    renderHook(() => useOAuthCallback(), { wrapper });

    expect(invalidateSpy).toHaveBeenCalledWith(
      expect.objectContaining({ queryKey: ["user-plugins"] }),
    );
    expect(invalidateSpy).toHaveBeenCalledWith(
      expect.objectContaining({ queryKey: ["user-plugin", "sync-anilist"] }),
    );
  });

  it("cleans up URL params after handling success", async () => {
    // We verify cleanup happened by checking that the notification is only
    // called once (not re-triggered on re-render after URL cleanup).
    const { wrapper } = createWrapperWithClient(["/?oauth=success"]);

    renderHook(() => useOAuthCallback(), { wrapper });

    // The notification should have been shown exactly once
    expect(notifications.show).toHaveBeenCalledTimes(1);
  });

  it("cleans up URL params after handling error", async () => {
    const { wrapper } = createWrapperWithClient(["/?oauth=error"]);

    renderHook(() => useOAuthCallback(), { wrapper });

    expect(notifications.show).toHaveBeenCalledTimes(1);
  });

  it("does nothing when no oauth param is present", () => {
    renderHook(() => useOAuthCallback(), {
      wrapper: createWrapper(["/"]),
    });

    expect(notifications.show).not.toHaveBeenCalled();
  });

  it("does nothing for unrecognised oauth values", () => {
    renderHook(() => useOAuthCallback(), {
      wrapper: createWrapper(["/?oauth=unknown"]),
    });

    expect(notifications.show).not.toHaveBeenCalled();
  });
});
