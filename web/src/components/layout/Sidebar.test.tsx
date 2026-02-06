import { screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { librariesApi } from "@/api/libraries";
import { useAuthStore } from "@/store/authStore";
import { renderWithProviders, userEvent } from "@/test/utils";
import type { Library, User } from "@/types";
import { AppLayout } from "./AppLayout";

vi.mock("@/api/libraries");
vi.mock("@/api/tasks", () => ({
  subscribeToTaskProgress: vi.fn(() => vi.fn()),
  fetchPendingTaskCounts: vi.fn(() => Promise.resolve({})),
  fetchTasksByStatus: vi.fn(() => Promise.resolve([])),
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

    // Should clear auth (navigation is handled by React Router now)
    expect(localStorage.getItem("jwt_token")).toBeNull();
  });

  describe("Settings navigation", () => {
    it("should open Settings menu when navigating to a settings page", async () => {
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
      const { rerender } = renderWithProviders(
        <AppLayout currentPath="/">
          <div>Content</div>
        </AppLayout>,
      );

      // Settings submenu items (like Plugins) should not be visible when Settings is collapsed
      // Mantine NavLink keeps children in DOM but hides them visually
      const pluginsLinkBefore = screen.getByText("Plugins");
      expect(pluginsLinkBefore).not.toBeVisible();

      // Now rerender with a settings path to simulate navigation
      rerender(
        <AppLayout currentPath="/settings/plugins">
          <div>Content</div>
        </AppLayout>,
      );

      // After navigation to settings page, the Settings menu should be expanded
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
        <AppLayout currentPath="/settings/plugins">
          <div>Content</div>
        </AppLayout>,
      );

      // Settings submenu items should be visible since we started on a settings page
      expect(screen.getByText("Plugins")).toBeVisible();
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
});
