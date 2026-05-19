import { AppShell, MantineProvider } from "@mantine/core";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { librariesApi } from "@/api/libraries";
import { useAuthStore } from "@/store/authStore";
import { renderWithProviders, userEvent } from "@/test/utils";
import { theme } from "@/theme";
import type { Library, User } from "@/types";
import { AppLayout } from "./AppLayout";
import { Sidebar } from "./Sidebar";

vi.mock("@/api/libraries");
vi.mock("@/api/tasks", () => ({
  subscribeToTaskProgress: vi.fn(() => vi.fn()),
  fetchPendingTaskCounts: vi.fn(() => Promise.resolve({})),
  fetchTasksByStatus: vi.fn(() => Promise.resolve([])),
}));
// Mocked at module scope so individual Phase 8 tests can override the return
// shape (running tasks + pending counts) to drive the compact Tasks badge.
vi.mock("@/hooks/useTaskProgress", () => ({
  useTaskProgress: vi.fn(() => ({
    activeTasks: [],
    connectionState: "connected",
    pendingCounts: {},
    getTasksByStatus: vi.fn(() => []),
    getTasksByLibrary: vi.fn(() => []),
    getTask: vi.fn(() => undefined),
  })),
}));
vi.mock("@/api/plugins", () => ({
  pluginsApi: {
    getAll: vi.fn(() => Promise.resolve({ plugins: [] })),
    getActions: vi.fn(() => Promise.resolve({ actions: [] })),
  },
  pluginActionsApi: {
    enqueueLibraryAutoMatchTasks: vi.fn(),
  },
}));

describe("Sidebar Component (via AppLayout)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    localStorage.clear();

    // Mock window.location
    Object.defineProperty(window, "location", {
      value: { href: "" },
      writable: true,
    });

    // Mock libraries API - set default return value for getAll
    vi.mocked(librariesApi.getAll).mockResolvedValue([]);
  });

  it("should render navigation links", () => {
    const mockUser: User = {
      id: "1",
      username: "testuser",
      email: "test@example.com",
      role: "reader",
      emailVerified: true,
      permissions: [],
    };

    useAuthStore.setState({
      user: mockUser,
      token: "token",
      isAuthenticated: true,
    });

    renderWithProviders(
      <AppLayout>
        <div>Content</div>
      </AppLayout>,
    );

    expect(screen.getByText("Home")).toBeInTheDocument();
    expect(screen.getByText("Libraries")).toBeInTheDocument();
    expect(screen.getByText("Settings")).toBeInTheDocument();
    expect(screen.getByText("Logout")).toBeInTheDocument();
  });

  it("should show Users link for admin users", () => {
    const mockAdmin: User = {
      id: "1",
      username: "admin",
      email: "admin@example.com",
      role: "admin",
      emailVerified: true,
      permissions: [],
    };

    useAuthStore.setState({
      user: mockAdmin,
      token: "token",
      isAuthenticated: true,
    });

    renderWithProviders(
      <AppLayout>
        <div>Content</div>
      </AppLayout>,
    );

    expect(screen.getByText("Users")).toBeInTheDocument();
  });

  it("should not show Users link for regular users in sidebar root", () => {
    const mockUser: User = {
      id: "1",
      username: "testuser",
      email: "test@example.com",
      role: "reader",
      emailVerified: true,
      permissions: [],
    };

    useAuthStore.setState({
      user: mockUser,
      token: "token",
      isAuthenticated: true,
    });

    renderWithProviders(
      <AppLayout>
        <div>Content</div>
      </AppLayout>,
    );

    // Users should now be inside Settings menu, not in root
    // Check that Settings exists
    expect(screen.getByText("Settings")).toBeInTheDocument();
  });

  it("should show Profile link inside Settings for all users", () => {
    const mockUser: User = {
      id: "1",
      username: "testuser",
      email: "test@example.com",
      role: "reader",
      emailVerified: true,
      permissions: [],
    };

    useAuthStore.setState({
      user: mockUser,
      token: "token",
      isAuthenticated: true,
    });

    renderWithProviders(
      <AppLayout>
        <div>Content</div>
      </AppLayout>,
    );

    expect(screen.getByText("Profile")).toBeInTheDocument();
  });

  it("should show admin settings options for admin users", () => {
    const mockAdmin: User = {
      id: "1",
      username: "admin",
      email: "admin@example.com",
      role: "admin",
      emailVerified: true,
      permissions: [],
    };

    useAuthStore.setState({
      user: mockAdmin,
      token: "token",
      isAuthenticated: true,
    });

    renderWithProviders(
      <AppLayout>
        <div>Content</div>
      </AppLayout>,
    );

    // Admin settings options should be visible inside Settings
    expect(screen.getByText("Server")).toBeInTheDocument();
    expect(screen.getByText("Users")).toBeInTheDocument();
    expect(screen.getByText("Tasks")).toBeInTheDocument();
    expect(screen.getByText("Duplicates")).toBeInTheDocument();
    expect(screen.getByText("Metrics")).toBeInTheDocument();
  });

  it("should not show admin settings options for regular users", () => {
    const mockUser: User = {
      id: "1",
      username: "testuser",
      email: "test@example.com",
      role: "reader",
      emailVerified: true,
      permissions: [],
    };

    useAuthStore.setState({
      user: mockUser,
      token: "token",
      isAuthenticated: true,
    });

    renderWithProviders(
      <AppLayout>
        <div>Content</div>
      </AppLayout>,
    );

    // Admin options should not be visible
    expect(screen.queryByText("Server")).not.toBeInTheDocument();
    expect(screen.queryByText("Tasks")).not.toBeInTheDocument();
    expect(screen.queryByText("Duplicates")).not.toBeInTheDocument();
    expect(screen.queryByText("Metrics")).not.toBeInTheDocument();
    // Profile should still be visible
    expect(screen.getByText("Profile")).toBeInTheDocument();
  });

  it("should handle logout", async () => {
    const user = userEvent.setup();
    const mockUser: User = {
      id: "1",
      username: "testuser",
      email: "test@example.com",
      role: "reader",
      emailVerified: true,
      permissions: [],
    };

    useAuthStore.setState({
      user: mockUser,
      token: "token",
      isAuthenticated: true,
    });
    localStorage.setItem("jwt_token", "token");

    renderWithProviders(
      <AppLayout>
        <div>Content</div>
      </AppLayout>,
    );

    const logoutButton = screen.getByText("Logout");
    await user.click(logoutButton);

    // Should clear auth (navigation is handled by React Router now).
    // Logout is async (it calls POST /auth/logout to revoke the refresh
    // token server-side before clearing local state), so wait for the
    // store-side cleanup to land.
    await waitFor(() => {
      expect(localStorage.getItem("jwt_token")).toBeNull();
    });
  });

  describe("Settings navigation", () => {
    it("should open Settings menu when clicking Settings toggle", async () => {
      const user = userEvent.setup();
      const mockAdmin: User = {
        id: "1",
        username: "admin",
        email: "admin@example.com",
        role: "admin",
        emailVerified: true,
        permissions: [],
      };

      useAuthStore.setState({
        user: mockAdmin,
        token: "token",
        isAuthenticated: true,
      });

      // Render starting from home page
      renderWithProviders(
        <AppLayout>
          <div>Content</div>
        </AppLayout>,
        { initialEntries: ["/"] },
      );

      // Settings submenu items (like Plugins) should not be visible when Settings is collapsed
      // Mantine NavLink keeps children in DOM but hides them visually
      const pluginsLinkBefore = screen.getByText("Plugins");
      expect(pluginsLinkBefore).not.toBeVisible();

      // Click Settings to expand the submenu
      await user.click(screen.getByText("Settings"));

      // After clicking Settings, the submenu should be expanded
      // and Plugins submenu item should be visible
      await waitFor(() => {
        expect(screen.getByText("Plugins")).toBeVisible();
      });
    });

    it("should have Settings menu open when starting on a settings page", () => {
      const mockAdmin: User = {
        id: "1",
        username: "admin",
        email: "admin@example.com",
        role: "admin",
        emailVerified: true,
        permissions: [],
      };

      useAuthStore.setState({
        user: mockAdmin,
        token: "token",
        isAuthenticated: true,
      });

      // Render directly on a settings page
      renderWithProviders(
        <AppLayout>
          <div>Content</div>
        </AppLayout>,
        { initialEntries: ["/settings/plugins"] },
      );

      // Settings submenu items should be visible since we started on a settings page
      expect(screen.getByText("Plugins")).toBeVisible();
    });
  });

  describe("Sidebar active state highlighting", () => {
    const mockLibrary: Library = {
      id: "lib-123",
      name: "Test Library",
      path: "/test/path",
      isActive: true,
      createdAt: "2024-01-01",
      updatedAt: "2024-01-01",
      bookStrategy: "filename",
      defaultReadingDirection: "ltr",
      numberStrategy: "filename",
      seriesStrategy: "flat",
    };

    it("should highlight the library nav item when on that library's page", async () => {
      const mockUser: User = {
        id: "1",
        username: "testuser",
        email: "test@example.com",
        role: "reader",
        emailVerified: true,
        permissions: [],
      };

      useAuthStore.setState({
        user: mockUser,
        token: "token",
        isAuthenticated: true,
      });

      vi.mocked(librariesApi.getAll).mockResolvedValue([mockLibrary]);

      renderWithProviders(
        <AppLayout>
          <div>Content</div>
        </AppLayout>,
        { initialEntries: ["/libraries/lib-123/series"] },
      );

      // Wait for the library to appear
      await waitFor(() => {
        expect(screen.getByText("Test Library")).toBeInTheDocument();
      });

      // The library nav link should have the active data attribute
      const libraryLink = screen.getByText("Test Library").closest("a");
      expect(libraryLink).toHaveAttribute("data-active", "true");
    });

    it("should highlight Home when on the root path", () => {
      const mockUser: User = {
        id: "1",
        username: "testuser",
        email: "test@example.com",
        role: "reader",
        emailVerified: true,
        permissions: [],
      };

      useAuthStore.setState({
        user: mockUser,
        token: "token",
        isAuthenticated: true,
      });

      renderWithProviders(
        <AppLayout>
          <div>Content</div>
        </AppLayout>,
        { initialEntries: ["/"] },
      );

      const homeLink = screen.getByText("Home").closest("a");
      expect(homeLink).toHaveAttribute("data-active", "true");
    });

    it("should highlight Libraries when on the all-libraries page", () => {
      const mockUser: User = {
        id: "1",
        username: "testuser",
        email: "test@example.com",
        role: "reader",
        emailVerified: true,
        permissions: [],
      };

      useAuthStore.setState({
        user: mockUser,
        token: "token",
        isAuthenticated: true,
      });

      renderWithProviders(
        <AppLayout>
          <div>Content</div>
        </AppLayout>,
        { initialEntries: ["/libraries/all/series"] },
      );

      const librariesLink = screen.getByText("Libraries").closest("a");
      expect(librariesLink).toHaveAttribute("data-active", "true");
    });
  });

  describe("Library dropdown menu", () => {
    const mockLibrary: Library = {
      id: "lib-123",
      name: "Test Library",
      path: "/test/path",
      isActive: true,
      createdAt: "2024-01-01",
      updatedAt: "2024-01-01",
      bookStrategy: "filename",
      defaultReadingDirection: "ltr",
      numberStrategy: "filename",
      seriesStrategy: "flat",
    };

    it("should show thumbnail options for users with tasks:write permission", async () => {
      const user = userEvent.setup();
      const mockMaintainer: User = {
        id: "1",
        username: "maintainer",
        email: "maintainer@example.com",
        role: "maintainer",
        emailVerified: true,
        permissions: ["libraries-write", "tasks-write"],
      };

      useAuthStore.setState({
        user: mockMaintainer,
        token: "token",
        isAuthenticated: true,
      });

      vi.mocked(librariesApi.getAll).mockResolvedValue([mockLibrary]);

      renderWithProviders(
        <AppLayout>
          <div>Content</div>
        </AppLayout>,
      );

      // Wait for the library to appear
      await waitFor(() => {
        expect(screen.getByText("Test Library")).toBeInTheDocument();
      });

      // Find and click the library options button (the dots icon)
      const libraryItem = screen.getByText("Test Library").closest("a");
      const optionsButton = libraryItem?.querySelector(
        'button[title="Library options"]',
      );
      expect(optionsButton).toBeInTheDocument();

      if (optionsButton) {
        await user.click(optionsButton);
      }

      // Should show thumbnail sections with options
      await waitFor(() => {
        expect(screen.getByText("Book Thumbnails")).toBeInTheDocument();
      });
      expect(screen.getByText("Series Thumbnails")).toBeInTheDocument();
      // There should be two "Generate Missing" and two "Regenerate All" options
      expect(screen.getAllByText("Generate Missing")).toHaveLength(2);
      expect(screen.getAllByText("Regenerate All")).toHaveLength(2);
    });

    it("should NOT show thumbnail options for users without tasks:write permission", async () => {
      const user = userEvent.setup();
      // Use a reader role with only libraries-write custom permission (no tasks-write)
      const mockEditor: User = {
        id: "1",
        username: "editor",
        email: "editor@example.com",
        role: "reader",
        emailVerified: true,
        permissions: ["libraries-write"], // Can edit libraries but not write tasks
      };

      useAuthStore.setState({
        user: mockEditor,
        token: "token",
        isAuthenticated: true,
      });

      vi.mocked(librariesApi.getAll).mockResolvedValue([mockLibrary]);

      renderWithProviders(
        <AppLayout>
          <div>Content</div>
        </AppLayout>,
      );

      // Wait for the library to appear
      await waitFor(() => {
        expect(screen.getByText("Test Library")).toBeInTheDocument();
      });

      // Find and click the library options button
      const libraryItem = screen.getByText("Test Library").closest("a");
      const optionsButton = libraryItem?.querySelector(
        'button[title="Library options"]',
      );
      expect(optionsButton).toBeInTheDocument();

      if (optionsButton) {
        await user.click(optionsButton);
      }

      // Wait for menu to open and check that thumbnail options are NOT shown
      await waitFor(() => {
        expect(screen.getByText("Scan Library")).toBeInTheDocument();
      });

      expect(screen.queryByText("Book Thumbnails")).not.toBeInTheDocument();
      expect(screen.queryByText("Series Thumbnails")).not.toBeInTheDocument();
    });

    it("should call generateMissingThumbnails API when clicking the menu item", async () => {
      const user = userEvent.setup();
      const mockAdmin: User = {
        id: "1",
        username: "admin",
        email: "admin@example.com",
        role: "admin",
        emailVerified: true,
        permissions: [],
      };

      useAuthStore.setState({
        user: mockAdmin,
        token: "token",
        isAuthenticated: true,
      });

      vi.mocked(librariesApi.getAll).mockResolvedValue([mockLibrary]);
      vi.mocked(librariesApi.generateMissingThumbnails).mockResolvedValue({
        task_id: "task-123",
      });

      renderWithProviders(
        <AppLayout>
          <div>Content</div>
        </AppLayout>,
      );

      // Wait for the library to appear
      await waitFor(() => {
        expect(screen.getByText("Test Library")).toBeInTheDocument();
      });

      // Find and click the library options button
      const libraryItem = screen.getByText("Test Library").closest("a");
      const optionsButton = libraryItem?.querySelector(
        'button[title="Library options"]',
      );
      expect(optionsButton).toBeInTheDocument();

      if (optionsButton) {
        await user.click(optionsButton);
      }

      // Wait for menu to open
      await waitFor(() => {
        expect(screen.getByText("Book Thumbnails")).toBeInTheDocument();
      });

      // Click the Generate Missing option under Book Thumbnails
      const generateMissingButtons = screen.getAllByText("Generate Missing");
      await user.click(generateMissingButtons[0]);

      // Verify the API was called with the correct library ID
      await waitFor(() => {
        expect(librariesApi.generateMissingThumbnails).toHaveBeenCalledWith(
          "lib-123",
        );
      });
    });
  });

  describe("Mobile drawer auto-close (onNavigate)", () => {
    function renderSidebar(onNavigate: () => void) {
      const queryClient = new QueryClient({
        defaultOptions: {
          queries: { retry: false },
          mutations: { retry: false },
        },
      });
      return render(
        <MantineProvider theme={theme} defaultColorScheme="dark">
          <QueryClientProvider client={queryClient}>
            <MemoryRouter>
              <AppShell navbar={{ width: 280, breakpoint: "sm" }}>
                <Sidebar onNavigate={onNavigate} />
              </AppShell>
            </MemoryRouter>
          </QueryClientProvider>
        </MantineProvider>,
      );
    }

    it("calls onNavigate when the Home link is clicked", async () => {
      const user = userEvent.setup();
      const onNavigate = vi.fn();
      const mockUser: User = {
        id: "1",
        username: "testuser",
        email: "test@example.com",
        role: "reader",
        emailVerified: true,
        permissions: [],
      };

      useAuthStore.setState({
        user: mockUser,
        token: "token",
        isAuthenticated: true,
      });

      renderSidebar(onNavigate);

      await user.click(screen.getByText("Home"));
      expect(onNavigate).toHaveBeenCalled();
    });

    it("calls onNavigate when a settings submenu link is clicked", async () => {
      const user = userEvent.setup();
      const onNavigate = vi.fn();
      const mockUser: User = {
        id: "1",
        username: "testuser",
        email: "test@example.com",
        role: "reader",
        emailVerified: true,
        permissions: [],
      };

      useAuthStore.setState({
        user: mockUser,
        token: "token",
        isAuthenticated: true,
      });

      renderSidebar(onNavigate);

      // Profile is shown to all users inside Settings
      await user.click(screen.getByText("Profile"));
      expect(onNavigate).toHaveBeenCalled();
    });

    it("does NOT call onNavigate when only expanding the Settings submenu", async () => {
      const user = userEvent.setup();
      const onNavigate = vi.fn();
      const mockUser: User = {
        id: "1",
        username: "testuser",
        email: "test@example.com",
        role: "reader",
        emailVerified: true,
        permissions: [],
      };

      useAuthStore.setState({
        user: mockUser,
        token: "token",
        isAuthenticated: true,
      });

      renderSidebar(onNavigate);

      // Clicking the "Settings" parent toggle expands the submenu; it is not a
      // navigation event and must not collapse the drawer.
      await user.click(screen.getByText("Settings"));
      expect(onNavigate).not.toHaveBeenCalled();
    });
  });

  describe("Mobile scroll cue (U4)", () => {
    it("does not render the scroll cue on desktop viewports", () => {
      // Default matchMedia mock returns matches:false for everything.
      const mockUser: User = {
        id: "1",
        username: "testuser",
        email: "test@example.com",
        role: "reader",
        emailVerified: true,
        permissions: [],
      };
      useAuthStore.setState({
        user: mockUser,
        token: "token",
        isAuthenticated: true,
      });

      renderWithProviders(
        <AppLayout>
          <div>Content</div>
        </AppLayout>,
      );

      expect(
        screen.queryByTestId("sidebar-scroll-cue"),
      ).not.toBeInTheDocument();
    });

    it("renders the scroll cue when the mobile navbar overflows", async () => {
      // Force-mobile matchMedia + stub navbar metrics to simulate overflow.
      const originalMatchMedia = window.matchMedia;
      window.matchMedia = vi.fn().mockImplementation((query: string) => ({
        matches: query.includes("max-width"),
        media: query,
        onchange: null,
        addListener: vi.fn(),
        removeListener: vi.fn(),
        addEventListener: vi.fn(),
        removeEventListener: vi.fn(),
        dispatchEvent: vi.fn(),
      }));

      // ResizeObserver is invoked synchronously when we observe(), so we can
      // assert via the initial update() call. Stub it as a noop instance so
      // it doesn't run our update on resize (which we don't simulate).
      class StubResizeObserver {
        observe() {}
        unobserve() {}
        disconnect() {}
      }
      const originalRO = window.ResizeObserver;
      // @ts-expect-error - test stub
      window.ResizeObserver = StubResizeObserver;

      // Stub scrollHeight/clientHeight on the navbar element so update()
      // detects overflow on mount.
      const originalScrollHeight = Object.getOwnPropertyDescriptor(
        Element.prototype,
        "scrollHeight",
      );
      const originalClientHeight = Object.getOwnPropertyDescriptor(
        Element.prototype,
        "clientHeight",
      );
      Object.defineProperty(Element.prototype, "scrollHeight", {
        configurable: true,
        get() {
          return this.classList?.contains("mantine-AppShell-navbar") ? 2000 : 0;
        },
      });
      Object.defineProperty(Element.prototype, "clientHeight", {
        configurable: true,
        get() {
          return this.classList?.contains("mantine-AppShell-navbar") ? 600 : 0;
        },
      });

      const mockUser: User = {
        id: "1",
        username: "testuser",
        email: "test@example.com",
        role: "reader",
        emailVerified: true,
        permissions: [],
      };
      useAuthStore.setState({
        user: mockUser,
        token: "token",
        isAuthenticated: true,
      });

      try {
        renderWithProviders(
          <AppLayout>
            <div>Content</div>
          </AppLayout>,
        );

        await waitFor(() => {
          expect(screen.getByTestId("sidebar-scroll-cue")).toBeInTheDocument();
        });
      } finally {
        window.matchMedia = originalMatchMedia;
        window.ResizeObserver = originalRO;
        if (originalScrollHeight) {
          Object.defineProperty(
            Element.prototype,
            "scrollHeight",
            originalScrollHeight,
          );
        }
        if (originalClientHeight) {
          Object.defineProperty(
            Element.prototype,
            "clientHeight",
            originalClientHeight,
          );
        }
      }
    });
  });

  describe("Phase 8 info-design", () => {
    const mockAdmin: User = {
      id: "1",
      username: "admin",
      email: "admin@example.com",
      role: "admin",
      emailVerified: true,
      permissions: [],
    };

    function renderAsAdmin(initialEntries: string[] = ["/"]) {
      useAuthStore.setState({
        user: mockAdmin,
        token: "token",
        isAuthenticated: true,
      });
      return renderWithProviders(
        <AppLayout>
          <div>Content</div>
        </AppLayout>,
        { initialEntries },
      );
    }

    it("groups primary nav, Libraries and Settings via section-break markers (no elevated panel)", () => {
      renderAsAdmin();

      // The dropped panel must not exist: lifting one section above an
      // otherwise flat list created a visual island in earlier iterations.
      expect(
        screen.queryByTestId("sidebar-primary-group"),
      ).not.toBeInTheDocument();

      // Libraries opens the second section.
      const librariesLink = screen.getByText("Libraries").closest("a");
      expect(librariesLink).toHaveAttribute("data-section-break", "true");

      // Settings opens the third section.
      const settingsToggle = screen.getByText("Settings").closest("a, button");
      expect(settingsToggle).toHaveAttribute("data-section-break", "true");

      // The primary rows (Home and friends) deliberately have NO break;
      // they are the first group, no top-margin needed.
      const homeLink = screen.getByText("Home").closest("a");
      expect(homeLink).not.toHaveAttribute("data-section-break");
    });

    it("renders top-level icons at stroke 2 and Settings sub-icons at stroke 1.5", () => {
      renderAsAdmin(["/settings/server"]);

      const findIconSvg = (label: string): SVGElement | null => {
        const link = screen.getByText(label).closest("a, button");
        return (link?.querySelector("svg") as SVGElement) ?? null;
      };

      // Top-level rows carry a heavier 2.0 stroke. Stroke 1.5 on sub-items
      // is a 25% lighter line — the contrast actually reads at the row
      // size; the previous 1.75 vs 1.5 was sub-pixel and effectively
      // invisible per design review.
      expect(findIconSvg("Home")).toHaveAttribute("stroke-width", "2");
      expect(findIconSvg("Libraries")).toHaveAttribute("stroke-width", "2");
      expect(findIconSvg("Settings")).toHaveAttribute("stroke-width", "2");
      expect(findIconSvg("Logout")).toHaveAttribute("stroke-width", "2");

      expect(findIconSvg("Server")).toHaveAttribute("stroke-width", "1.5");
      expect(findIconSvg("Tasks")).toHaveAttribute("stroke-width", "1.5");
      expect(findIconSvg("Profile")).toHaveAttribute("stroke-width", "1.5");
    });

    it("pins Logout and version inside a semantic footer element", () => {
      renderAsAdmin();

      const footer = screen.getByTestId("sidebar-footer");
      expect(footer.tagName.toLowerCase()).toBe("footer");

      // Logout still lives in the footer; the old loud TaskNotificationBadge
      // pill does NOT.
      const logoutLink = screen.getByText("Logout").closest("a, button");
      expect(footer.contains(logoutLink as Node)).toBe(true);
      expect(
        footer.querySelector("[data-mantine-component='Badge']"),
      ).toBeNull();
      // Sanity: the fullwidth "N pending task(s)" pill is not in the footer.
      expect(screen.queryByText(/pending task/i)).not.toBeInTheDocument();
    });

    it("renders a compact pending-tasks badge on the Tasks NavLink when work is queued", async () => {
      const { useTaskProgress } = await import("@/hooks/useTaskProgress");
      // Set a persistent return value (not Once) so every re-render of the
      // badge sees the same task data, not just the first invocation.
      vi.mocked(useTaskProgress).mockReturnValue({
        activeTasks: [
          {
            taskId: "task-1",
            taskType: "analyze_book",
            status: "running",
            progress: undefined,
            error: undefined,
            startedAt: "2026-01-07T12:00:00Z",
            libraryId: "lib-1",
          },
        ],
        connectionState: "connected",
        pendingCounts: { analyze_book: 4 },
        getTasksByStatus: vi.fn(() => []),
        getTasksByLibrary: vi.fn(() => []),
        getTask: vi.fn(() => undefined),
      });

      try {
        renderAsAdmin(["/settings/tasks"]);

        // The accessible label reflects the precise total for SR users; we
        // assert against this rather than .textContent because Mantine renders
        // the rightSection inside its own portal-friendly tooltip wrapper.
        expect(screen.getByLabelText("5 pending tasks")).toBeInTheDocument();
      } finally {
        // Reset back to the default empty state so other tests aren't polluted.
        vi.mocked(useTaskProgress).mockReturnValue({
          activeTasks: [],
          connectionState: "connected",
          pendingCounts: {},
          getTasksByStatus: vi.fn(() => []),
          getTasksByLibrary: vi.fn(() => []),
          getTask: vi.fn(() => undefined),
        });
      }
    });

    it("hides the compact badge entirely when there are zero pending tasks", () => {
      renderAsAdmin(["/settings/tasks"]);

      // Default useTaskProgress mock returns empty arrays -> badge unmounts.
      expect(screen.queryByLabelText(/pending task/i)).not.toBeInTheDocument();
      expect(screen.queryByText(/pending task/i)).not.toBeInTheDocument();
    });
  });
});
