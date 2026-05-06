import { beforeEach, describe, expect, it, vi } from "vitest";
import { usePermissions } from "@/hooks/usePermissions";
import { useBulkSelectionStore } from "@/store/bulkSelectionStore";
import { renderWithProviders, screen, userEvent, waitFor } from "@/test/utils";
import { PERMISSIONS, type Permission } from "@/types/permissions";
import { BulkSelectionToolbar } from "./BulkSelectionToolbar";

// Mock the API modules
vi.mock("@/api/books", () => ({
  booksApi: {
    bulkMarkAsRead: vi
      .fn()
      .mockResolvedValue({ count: 2, message: "2 books marked as read" }),
    bulkMarkAsUnread: vi
      .fn()
      .mockResolvedValue({ count: 2, message: "2 books marked as unread" }),
    bulkAnalyze: vi.fn().mockResolvedValue({
      tasksEnqueued: 2,
      message: "Enqueued 2 analysis tasks",
    }),
  },
}));

vi.mock("@/api/series", () => ({
  seriesApi: {
    bulkMarkAsRead: vi.fn().mockResolvedValue({
      count: 5,
      message: "5 books marked as read across 2 series",
    }),
    bulkMarkAsUnread: vi.fn().mockResolvedValue({
      count: 5,
      message: "5 books marked as unread across 2 series",
    }),
    bulkAnalyze: vi.fn().mockResolvedValue({
      tasksEnqueued: 5,
      message: "Enqueued 5 analysis tasks for 2 series",
    }),
    bulkTrackForReleases: vi.fn().mockResolvedValue({
      changed: 2,
      alreadyInState: 0,
      errored: 0,
      results: [],
    }),
    bulkUntrackForReleases: vi.fn().mockResolvedValue({
      changed: 1,
      alreadyInState: 0,
      errored: 0,
      results: [],
    }),
  },
}));

// Mock the usePermissions hook - default to admin (all permissions)
vi.mock("@/hooks/usePermissions", () => ({
  usePermissions: vi.fn(),
}));

// Mock the applicability hook so the Release Tracking menu entries render.
// Tests that need to hide them can override the mock.
vi.mock("@/hooks/useReleaseTrackingApplicability", () => ({
  useReleaseTrackingApplicability: vi.fn(() => ({
    data: { applicable: true, pluginDisplayNames: ["Nyaa Releases"] },
    isLoading: false,
  })),
}));

const mockPermissionsAdmin = () => {
  vi.mocked(usePermissions).mockReturnValue({
    user: { id: "user-1", username: "admin", role: "admin" } as ReturnType<
      typeof usePermissions
    >["user"],
    isAdmin: true,
    isMaintainer: true,
    hasPermission: () => true,
    hasAnyPermission: () => true,
    hasAllPermissions: () => true,
    getEffectivePermissions: () => Object.values(PERMISSIONS),
  });
};

const READER_PERMISSIONS = new Set<Permission>([
  PERMISSIONS.LIBRARIES_READ,
  PERMISSIONS.SERIES_READ,
  PERMISSIONS.BOOKS_READ,
  PERMISSIONS.PAGES_READ,
  PERMISSIONS.API_KEYS_READ,
  PERMISSIONS.API_KEYS_WRITE,
  PERMISSIONS.API_KEYS_DELETE,
  PERMISSIONS.SYSTEM_HEALTH,
]);

const mockPermissionsReader = () => {
  vi.mocked(usePermissions).mockReturnValue({
    user: { id: "user-2", username: "reader", role: "reader" } as ReturnType<
      typeof usePermissions
    >["user"],
    isAdmin: false,
    isMaintainer: false,
    hasPermission: (perm) => READER_PERMISSIONS.has(perm),
    hasAnyPermission: (perms) => perms.some((p) => READER_PERMISSIONS.has(p)),
    hasAllPermissions: (perms) => perms.every((p) => READER_PERMISSIONS.has(p)),
    getEffectivePermissions: () => Array.from(READER_PERMISSIONS),
  });
};

// Mock the plugins API
const mockPluginActions = {
  actions: [
    {
      pluginId: "plugin-mangabaka",
      pluginName: "mangabaka",
      pluginDisplayName: "MangaBaka",
      actionType: "metadata_search",
      label: "Search MangaBaka",
      description: "Fetches manga metadata from MangaUpdates",
      icon: null,
    },
  ],
  scope: "series:bulk",
};

vi.mock("@/api/plugins", () => ({
  pluginsApi: {
    getActions: vi.fn().mockImplementation((scope: string) => {
      if (scope === "series:bulk") {
        return Promise.resolve(mockPluginActions);
      }
      return Promise.resolve({ actions: [], scope });
    }),
  },
  pluginActionsApi: {
    enqueueBulkAutoMatchTasks: vi.fn().mockResolvedValue({
      success: true,
      tasksEnqueued: 2,
      taskIds: ["task-1", "task-2"],
      message: "Enqueued 2 auto-match task(s)",
    }),
  },
}));

describe("BulkSelectionToolbar", () => {
  beforeEach(() => {
    // Reset the store state before each test
    useBulkSelectionStore.getState().clearSelection();
    vi.clearAllMocks();
    mockPermissionsAdmin();
  });

  describe("visibility", () => {
    it("should not render when no items are selected", () => {
      renderWithProviders(<BulkSelectionToolbar />);

      // Toolbar should not be visible
      expect(screen.queryByText(/selected/)).not.toBeInTheDocument();
    });

    it("should render when books are selected", () => {
      // Select a book
      useBulkSelectionStore.getState().toggleSelection("book-1", "book");

      renderWithProviders(<BulkSelectionToolbar />);

      expect(screen.getByText("1 book selected")).toBeInTheDocument();
    });

    it("should render when series are selected", () => {
      // Select a series
      useBulkSelectionStore.getState().toggleSelection("series-1", "series");

      renderWithProviders(<BulkSelectionToolbar />);

      expect(screen.getByText("1 series selected")).toBeInTheDocument();
    });

    it("should show correct count for multiple books", () => {
      useBulkSelectionStore.getState().toggleSelection("book-1", "book");
      useBulkSelectionStore.getState().toggleSelection("book-2", "book");
      useBulkSelectionStore.getState().toggleSelection("book-3", "book");

      renderWithProviders(<BulkSelectionToolbar />);

      expect(screen.getByText("3 books selected")).toBeInTheDocument();
    });

    it("should show correct count for multiple series", () => {
      useBulkSelectionStore.getState().toggleSelection("series-1", "series");
      useBulkSelectionStore.getState().toggleSelection("series-2", "series");

      renderWithProviders(<BulkSelectionToolbar />);

      expect(screen.getByText("2 series selected")).toBeInTheDocument();
    });
  });

  describe("action buttons", () => {
    it("should display Mark Read, Mark Unread, and More buttons", () => {
      useBulkSelectionStore.getState().toggleSelection("book-1", "book");

      renderWithProviders(<BulkSelectionToolbar />);

      expect(
        screen.getByRole("button", { name: /mark read/i }),
      ).toBeInTheDocument();
      expect(
        screen.getByRole("button", { name: /mark unread/i }),
      ).toBeInTheDocument();
      // Analyze is now in the More dropdown menu
      expect(
        screen.getByRole("button", { name: /more actions/i }),
      ).toBeInTheDocument();
    });

    it("should display close button", () => {
      useBulkSelectionStore.getState().toggleSelection("book-1", "book");

      renderWithProviders(<BulkSelectionToolbar />);

      expect(
        screen.getByRole("button", { name: /clear selection/i }),
      ).toBeInTheDocument();
    });
  });

  describe("clear selection", () => {
    it("should clear selection when close button is clicked", async () => {
      const user = userEvent.setup();
      useBulkSelectionStore.getState().toggleSelection("book-1", "book");
      useBulkSelectionStore.getState().toggleSelection("book-2", "book");

      const { rerender } = renderWithProviders(<BulkSelectionToolbar />);

      expect(screen.getByText("2 books selected")).toBeInTheDocument();

      await user.click(
        screen.getByRole("button", { name: /clear selection/i }),
      );

      rerender(<BulkSelectionToolbar />);

      // After clearing, toolbar should not render
      expect(screen.queryByText(/selected/)).not.toBeInTheDocument();
      expect(useBulkSelectionStore.getState().selectedIds.size).toBe(0);
    });

    it("should clear selection when Escape key is pressed", async () => {
      const user = userEvent.setup();
      useBulkSelectionStore.getState().toggleSelection("book-1", "book");

      const { rerender } = renderWithProviders(<BulkSelectionToolbar />);

      expect(screen.getByText("1 book selected")).toBeInTheDocument();

      await user.keyboard("{Escape}");

      rerender(<BulkSelectionToolbar />);

      expect(screen.queryByText(/selected/)).not.toBeInTheDocument();
      expect(useBulkSelectionStore.getState().selectedIds.size).toBe(0);
    });
  });

  describe("book bulk actions", () => {
    it("should call bulkMarkAsRead when Mark Read is clicked for books", async () => {
      const { booksApi } = await import("@/api/books");
      const user = userEvent.setup();

      useBulkSelectionStore.getState().toggleSelection("book-1", "book");
      useBulkSelectionStore.getState().toggleSelection("book-2", "book");

      renderWithProviders(<BulkSelectionToolbar />);

      await user.click(screen.getByRole("button", { name: /mark read/i }));

      await waitFor(() => {
        expect(booksApi.bulkMarkAsRead).toHaveBeenCalledWith([
          "book-1",
          "book-2",
        ]);
      });
    });

    it("should call bulkMarkAsUnread when Mark Unread is clicked for books", async () => {
      const { booksApi } = await import("@/api/books");
      const user = userEvent.setup();

      useBulkSelectionStore.getState().toggleSelection("book-1", "book");

      renderWithProviders(<BulkSelectionToolbar />);

      await user.click(screen.getByRole("button", { name: /mark unread/i }));

      await waitFor(() => {
        expect(booksApi.bulkMarkAsUnread).toHaveBeenCalledWith(["book-1"]);
      });
    });

    it("should call bulkAnalyze when Analyze is clicked for books", async () => {
      const { booksApi } = await import("@/api/books");
      const user = userEvent.setup();

      useBulkSelectionStore.getState().toggleSelection("book-1", "book");
      useBulkSelectionStore.getState().toggleSelection("book-2", "book");

      renderWithProviders(<BulkSelectionToolbar />);

      // Open the More menu first, then click Analyze
      await user.click(screen.getByRole("button", { name: /more actions/i }));
      // Wait for the menu dropdown to appear in the portal
      await waitFor(() => {
        expect(screen.getByText("Analyze")).toBeInTheDocument();
      });
      await user.click(screen.getByText("Analyze"));

      await waitFor(() => {
        expect(booksApi.bulkAnalyze).toHaveBeenCalledWith(["book-1", "book-2"]);
      });
    });
  });

  describe("series bulk actions", () => {
    it("should call bulkMarkAsRead when Mark Read is clicked for series", async () => {
      const { seriesApi } = await import("@/api/series");
      const user = userEvent.setup();

      useBulkSelectionStore.getState().toggleSelection("series-1", "series");
      useBulkSelectionStore.getState().toggleSelection("series-2", "series");

      renderWithProviders(<BulkSelectionToolbar />);

      await user.click(screen.getByRole("button", { name: /mark read/i }));

      await waitFor(() => {
        expect(seriesApi.bulkMarkAsRead).toHaveBeenCalledWith([
          "series-1",
          "series-2",
        ]);
      });
    });

    it("should call bulkMarkAsUnread when Mark Unread is clicked for series", async () => {
      const { seriesApi } = await import("@/api/series");
      const user = userEvent.setup();

      useBulkSelectionStore.getState().toggleSelection("series-1", "series");

      renderWithProviders(<BulkSelectionToolbar />);

      await user.click(screen.getByRole("button", { name: /mark unread/i }));

      await waitFor(() => {
        expect(seriesApi.bulkMarkAsUnread).toHaveBeenCalledWith(["series-1"]);
      });
    });

    it("should call bulkAnalyze when Analyze is clicked for series", async () => {
      const { seriesApi } = await import("@/api/series");
      const user = userEvent.setup();

      useBulkSelectionStore.getState().toggleSelection("series-1", "series");

      renderWithProviders(<BulkSelectionToolbar />);

      // Open the More menu first, then click Analyze
      await user.click(screen.getByRole("button", { name: /more actions/i }));
      // Wait for the menu dropdown to appear in the portal
      await waitFor(() => {
        expect(screen.getByText("Analyze")).toBeInTheDocument();
      });
      await user.click(screen.getByText("Analyze"));

      await waitFor(() => {
        expect(seriesApi.bulkAnalyze).toHaveBeenCalledWith(["series-1"]);
      });
    });

    it("calls bulkTrackForReleases with all selected series when Track for releases is clicked", async () => {
      const { seriesApi } = await import("@/api/series");
      const user = userEvent.setup();

      useBulkSelectionStore.getState().toggleSelection("series-1", "series");
      useBulkSelectionStore.getState().toggleSelection("series-2", "series");

      renderWithProviders(<BulkSelectionToolbar />);

      await user.click(screen.getByRole("button", { name: /more actions/i }));
      await waitFor(() => {
        expect(screen.getByText("Track for releases")).toBeInTheDocument();
      });
      await user.click(screen.getByText("Track for releases"));

      await waitFor(() => {
        expect(seriesApi.bulkTrackForReleases).toHaveBeenCalledTimes(1);
      });
      // The toolbar passes the full selected-id list as a single argument.
      const calls = (seriesApi.bulkTrackForReleases as ReturnType<typeof vi.fn>)
        .mock.calls;
      expect(calls[0][0]).toEqual(
        expect.arrayContaining(["series-1", "series-2"]),
      );
    });

    it("calls bulkUntrackForReleases when Don't track for releases is clicked", async () => {
      const { seriesApi } = await import("@/api/series");
      const user = userEvent.setup();

      useBulkSelectionStore.getState().toggleSelection("series-1", "series");

      renderWithProviders(<BulkSelectionToolbar />);

      await user.click(screen.getByRole("button", { name: /more actions/i }));
      await waitFor(() => {
        expect(
          screen.getByText("Don't track for releases"),
        ).toBeInTheDocument();
      });
      await user.click(screen.getByText("Don't track for releases"));

      await waitFor(() => {
        expect(seriesApi.bulkUntrackForReleases).toHaveBeenCalledWith([
          "series-1",
        ]);
      });
    });
  });

  describe("selection clearing after action", () => {
    it("should clear selection after successful Mark Read action", async () => {
      const user = userEvent.setup();
      useBulkSelectionStore.getState().toggleSelection("book-1", "book");

      const { rerender } = renderWithProviders(<BulkSelectionToolbar />);

      await user.click(screen.getByRole("button", { name: /mark read/i }));

      await waitFor(() => {
        expect(useBulkSelectionStore.getState().selectedIds.size).toBe(0);
      });

      rerender(<BulkSelectionToolbar />);
      expect(screen.queryByText(/selected/)).not.toBeInTheDocument();
    });

    it("should clear selection after successful Mark Unread action", async () => {
      const user = userEvent.setup();
      useBulkSelectionStore.getState().toggleSelection("book-1", "book");

      renderWithProviders(<BulkSelectionToolbar />);

      await user.click(screen.getByRole("button", { name: /mark unread/i }));

      await waitFor(() => {
        expect(useBulkSelectionStore.getState().selectedIds.size).toBe(0);
      });
    });

    it("should clear selection after successful Analyze action", async () => {
      const user = userEvent.setup();
      useBulkSelectionStore.getState().toggleSelection("book-1", "book");

      renderWithProviders(<BulkSelectionToolbar />);

      // Open the More menu first, then click Analyze
      await user.click(screen.getByRole("button", { name: /more actions/i }));
      // Wait for the menu dropdown to appear in the portal
      await waitFor(() => {
        expect(screen.getByText("Analyze")).toBeInTheDocument();
      });
      await user.click(screen.getByText("Analyze"));

      await waitFor(() => {
        expect(useBulkSelectionStore.getState().selectedIds.size).toBe(0);
      });
    });
  });

  describe("plugin actions", () => {
    it("should display Plugins button when series are selected and plugins are available", async () => {
      useBulkSelectionStore.getState().toggleSelection("series-1", "series");
      useBulkSelectionStore.getState().toggleSelection("series-2", "series");

      renderWithProviders(<BulkSelectionToolbar />);

      // Wait for the plugin actions query to complete
      await waitFor(() => {
        expect(
          screen.getByRole("button", { name: /plugin actions/i }),
        ).toBeInTheDocument();
      });
    });

    it("should not display Plugins button when books are selected", async () => {
      useBulkSelectionStore.getState().toggleSelection("book-1", "book");

      renderWithProviders(<BulkSelectionToolbar />);

      // Wait a bit to ensure query would have completed
      await waitFor(() => {
        expect(screen.getByText("1 book selected")).toBeInTheDocument();
      });

      // Plugins button should not be present for books (no book:bulk plugins available)
      expect(
        screen.queryByRole("button", { name: /plugin actions/i }),
      ).not.toBeInTheDocument();
    });

    it("should show plugin menu items when Plugins button is clicked", async () => {
      const user = userEvent.setup();
      useBulkSelectionStore.getState().toggleSelection("series-1", "series");

      renderWithProviders(<BulkSelectionToolbar />);

      // Wait for the plugin actions query to complete
      await waitFor(() => {
        expect(
          screen.getByRole("button", { name: /plugin actions/i }),
        ).toBeInTheDocument();
      });

      // Click the Plugins button to open the menu
      await user.click(screen.getByRole("button", { name: /plugin actions/i }));

      // Check that the menu item is visible
      await waitFor(() => {
        expect(screen.getByText("MangaBaka")).toBeInTheDocument();
      });
    });

    it("should call enqueueBulkAutoMatchTasks when a plugin action is clicked", async () => {
      const { pluginActionsApi } = await import("@/api/plugins");
      const user = userEvent.setup();

      useBulkSelectionStore.getState().toggleSelection("series-1", "series");
      useBulkSelectionStore.getState().toggleSelection("series-2", "series");

      renderWithProviders(<BulkSelectionToolbar />);

      // Wait for the plugin actions query to complete
      await waitFor(() => {
        expect(
          screen.getByRole("button", { name: /plugin actions/i }),
        ).toBeInTheDocument();
      });

      // Click the Plugins button to open the menu
      await user.click(screen.getByRole("button", { name: /plugin actions/i }));

      // Wait for menu to appear and click the plugin action
      await waitFor(() => {
        expect(screen.getByText("MangaBaka")).toBeInTheDocument();
      });

      await user.click(screen.getByText("MangaBaka"));

      await waitFor(() => {
        expect(pluginActionsApi.enqueueBulkAutoMatchTasks).toHaveBeenCalledWith(
          "plugin-mangabaka",
          ["series-1", "series-2"],
        );
      });
    });

    it("should clear selection after successful plugin action", async () => {
      const user = userEvent.setup();
      useBulkSelectionStore.getState().toggleSelection("series-1", "series");

      const { rerender } = renderWithProviders(<BulkSelectionToolbar />);

      // Wait for the plugin actions query to complete
      await waitFor(() => {
        expect(
          screen.getByRole("button", { name: /plugin actions/i }),
        ).toBeInTheDocument();
      });

      // Click the Plugins button and select an action
      await user.click(screen.getByRole("button", { name: /plugin actions/i }));
      await waitFor(() => {
        expect(screen.getByText("MangaBaka")).toBeInTheDocument();
      });
      await user.click(screen.getByText("MangaBaka"));

      // Wait for the selection to be cleared
      await waitFor(() => {
        expect(useBulkSelectionStore.getState().selectedIds.size).toBe(0);
      });

      rerender(<BulkSelectionToolbar />);
      expect(screen.queryByText(/selected/)).not.toBeInTheDocument();
    });
  });

  describe("reader permissions", () => {
    beforeEach(() => {
      mockPermissionsReader();
    });

    it("should not show More button for reader users selecting books", () => {
      useBulkSelectionStore.getState().toggleSelection("book-1", "book");

      renderWithProviders(<BulkSelectionToolbar />);

      expect(screen.getByText("1 book selected")).toBeInTheDocument();
      expect(
        screen.getByRole("button", { name: /mark read/i }),
      ).toBeInTheDocument();
      expect(
        screen.queryByRole("button", { name: /more actions/i }),
      ).not.toBeInTheDocument();
    });

    it("should not show More button for reader users selecting series", () => {
      useBulkSelectionStore.getState().toggleSelection("series-1", "series");

      renderWithProviders(<BulkSelectionToolbar />);

      expect(screen.getByText("1 series selected")).toBeInTheDocument();
      expect(
        screen.getByRole("button", { name: /mark read/i }),
      ).toBeInTheDocument();
      expect(
        screen.queryByRole("button", { name: /more actions/i }),
      ).not.toBeInTheDocument();
    });

    it("should not show Plugins button for reader users", async () => {
      useBulkSelectionStore.getState().toggleSelection("series-1", "series");

      renderWithProviders(<BulkSelectionToolbar />);

      await waitFor(() => {
        expect(screen.getByText("1 series selected")).toBeInTheDocument();
      });

      expect(
        screen.queryByRole("button", { name: /plugin actions/i }),
      ).not.toBeInTheDocument();
    });
  });
});
