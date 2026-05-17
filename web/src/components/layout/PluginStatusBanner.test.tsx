import { MantineProvider } from "@mantine/core";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { type PluginDto, pluginsApi } from "@/api/plugins";
import { useAuthStore } from "@/store/authStore";
import { userEvent } from "@/test/utils";
import { theme } from "@/theme";
import type { User } from "@/types";
import { PluginStatusBanner } from "./PluginStatusBanner";

vi.mock("@/api/plugins", async () => {
  const actual =
    await vi.importActual<typeof import("@/api/plugins")>("@/api/plugins");
  return {
    ...actual,
    pluginsApi: {
      ...actual.pluginsApi,
      getAll: vi.fn(),
    },
  };
});

function basePlugin(overrides: Partial<PluginDto>): PluginDto {
  return {
    args: [],
    command: "node",
    config: {},
    createdAt: "2026-01-01T00:00:00Z",
    credentialDelivery: "env",
    displayName: "Test Plugin",
    enabled: false,
    env: {},
    failureCount: 1,
    hasCredentials: false,
    healthStatus: "unhealthy",
    id: "plugin-1",
    name: "test-plugin",
    updatedAt: "2026-01-01T00:00:00Z",
    version: "1.0.0",
    capabilities: {},
    disabledReason: "max-failures",
    // PluginDto has additional fields we don't care about for this test; use
    // an unknown spread to satisfy the structural type.
    ...overrides,
  } as PluginDto;
}

function renderBanner() {
  const queryClient = new QueryClient({
    defaultOptions: {
      queries: { retry: false, gcTime: 0 },
      mutations: { retry: false },
    },
  });
  return render(
    <MantineProvider theme={theme} defaultColorScheme="dark">
      <QueryClientProvider client={queryClient}>
        <MemoryRouter>
          <PluginStatusBanner />
        </MemoryRouter>
      </QueryClientProvider>
    </MantineProvider>,
  );
}

describe("PluginStatusBanner (U5)", () => {
  beforeEach(() => {
    localStorage.clear();
    vi.clearAllMocks();
    const adminUser: User = {
      id: "u1",
      username: "admin",
      email: "admin@test.com",
      role: "admin",
      emailVerified: true,
      permissions: [],
    };
    useAuthStore.setState({
      user: adminUser,
      token: "tok",
      isAuthenticated: true,
    });
  });

  it("shows the banner for failed plugins", async () => {
    vi.mocked(pluginsApi.getAll).mockResolvedValue({
      plugins: [
        basePlugin({ id: "p1", displayName: "Buggy", failureCount: 3 }),
      ],
    });

    renderBanner();

    expect(
      await screen.findByText(/Plugin "Buggy" is disabled/i),
    ).toBeInTheDocument();
  });

  it("persists dismissal across remounts (localStorage, not sessionStorage)", async () => {
    const user = userEvent.setup();
    vi.mocked(pluginsApi.getAll).mockResolvedValue({
      plugins: [
        basePlugin({ id: "p1", displayName: "Buggy", failureCount: 3 }),
      ],
    });

    const { unmount } = renderBanner();

    const dismissBtn = await screen.findByLabelText("Dismiss all");
    await user.click(dismissBtn);

    await waitFor(() => {
      expect(
        screen.queryByText(/Plugin "Buggy" is disabled/i),
      ).not.toBeInTheDocument();
    });

    unmount();

    // Re-render with the same failureCount; dismissal should persist.
    renderBanner();
    await waitFor(() =>
      expect(vi.mocked(pluginsApi.getAll)).toHaveBeenCalledTimes(2),
    );
    expect(
      screen.queryByText(/Plugin "Buggy" is disabled/i),
    ).not.toBeInTheDocument();
  });

  it("reappears when failureCount increases beyond the dismissed value", async () => {
    const user = userEvent.setup();
    vi.mocked(pluginsApi.getAll).mockResolvedValueOnce({
      plugins: [
        basePlugin({ id: "p1", displayName: "Buggy", failureCount: 3 }),
      ],
    });

    const { unmount } = renderBanner();
    const dismissBtn = await screen.findByLabelText("Dismiss all");
    await user.click(dismissBtn);
    unmount();

    // A new failure has incremented failureCount; the banner should return.
    vi.mocked(pluginsApi.getAll).mockResolvedValueOnce({
      plugins: [
        basePlugin({ id: "p1", displayName: "Buggy", failureCount: 4 }),
      ],
    });

    renderBanner();
    expect(
      await screen.findByText(/Plugin "Buggy" is disabled/i),
    ).toBeInTheDocument();
  });
});
