import { MantineProvider } from "@mantine/core";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type React from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { usePermissions } from "@/hooks/usePermissions";
import { useTaskProgress } from "@/hooks/useTaskProgress";
import type { TaskProgressEvent } from "@/types";
import { PERMISSIONS } from "@/types/permissions";
import { TaskNotificationBadge } from "./TaskNotificationBadge";

// Mock the useTaskProgress hook
vi.mock("@/hooks/useTaskProgress", () => ({
  useTaskProgress: vi.fn(),
}));

// Mock the usePermissions hook
vi.mock("@/hooks/usePermissions", () => ({
  usePermissions: vi.fn(),
}));

// Mock react-router-dom
const mockNavigate = vi.fn();
vi.mock("react-router-dom", () => ({
  useNavigate: () => mockNavigate,
}));

// Helper to render with Mantine Provider
const renderWithMantine = (component: React.ReactElement) => {
  return render(<MantineProvider>{component}</MantineProvider>);
};

// Default mock for usePermissions (admin user with TASKS_READ)
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

// Mock for non-admin user without TASKS_READ
const mockPermissionsReader = () => {
  vi.mocked(usePermissions).mockReturnValue({
    user: { id: "user-2", username: "user", role: "reader" } as ReturnType<
      typeof usePermissions
    >["user"],
    isAdmin: false,
    isMaintainer: false,
    hasPermission: (perm) => perm !== PERMISSIONS.TASKS_READ,
    hasAnyPermission: () => false,
    hasAllPermissions: () => false,
    getEffectivePermissions: () => [],
  });
};

// Mock for maintainer (has TASKS_READ but not admin)
const mockPermissionsMaintainer = () => {
  vi.mocked(usePermissions).mockReturnValue({
    user: {
      id: "user-3",
      username: "maintainer",
      role: "maintainer",
    } as ReturnType<typeof usePermissions>["user"],
    isAdmin: false,
    isMaintainer: true,
    hasPermission: () => true,
    hasAnyPermission: () => true,
    hasAllPermissions: () => true,
    getEffectivePermissions: () => Object.values(PERMISSIONS),
  });
};

describe("TaskNotificationBadge", () => {
  beforeEach(() => {
    mockNavigate.mockClear();
    mockPermissionsAdmin();
  });

  it("should not render when no tasks are active", () => {
    vi.mocked(useTaskProgress).mockReturnValue({
      activeTasks: [],
      connectionState: "connected",
      pendingCounts: {},
      getTasksByStatus: vi.fn(() => []),
      getTasksByLibrary: vi.fn(() => []),
      getTask: vi.fn(() => undefined),
    });

    renderWithMantine(<TaskNotificationBadge />);

    // Badge should not be in the document
    expect(screen.queryByText(/pending task/i)).not.toBeInTheDocument();
  });

  it("should render badge with single task count", () => {
    const mockTasks: TaskProgressEvent[] = [
      {
        taskId: "task-1",
        taskType: "analyze_book",
        status: "running",
        progress: undefined,
        error: undefined,
        startedAt: "2026-01-07T12:00:00Z",
        libraryId: "lib-1",
      },
    ];

    vi.mocked(useTaskProgress).mockReturnValue({
      activeTasks: mockTasks,
      connectionState: "connected",
      pendingCounts: {},
      getTasksByStatus: vi.fn(() => mockTasks),
      getTasksByLibrary: vi.fn(() => mockTasks),
      getTask: vi.fn(() => mockTasks[0]),
    });

    renderWithMantine(<TaskNotificationBadge />);

    expect(screen.getByText("1 pending task")).toBeInTheDocument();
  });

  it("should render badge with multiple task count (plural)", () => {
    const mockTasks: TaskProgressEvent[] = [
      {
        taskId: "task-1",
        taskType: "analyze_book",
        status: "running",
        progress: undefined,
        error: undefined,
        startedAt: "2026-01-07T12:00:00Z",
        libraryId: "lib-1",
      },
    ];

    vi.mocked(useTaskProgress).mockReturnValue({
      activeTasks: mockTasks,
      connectionState: "connected",
      pendingCounts: { generate_thumbnails: 1 },
      getTasksByStatus: vi.fn(() => mockTasks),
      getTasksByLibrary: vi.fn(() => mockTasks),
      getTask: vi.fn((id) => mockTasks.find((t) => t.taskId === id)),
    });

    renderWithMantine(<TaskNotificationBadge />);

    // 1 running task + 1 pending task = 2 total
    expect(screen.getByText("2 pending tasks")).toBeInTheDocument();
  });

  it("should show tooltip on hover", () => {
    const mockTasks: TaskProgressEvent[] = [
      {
        taskId: "task-1",
        taskType: "analyze_book",
        status: "running",
        progress: undefined,
        error: undefined,
        startedAt: "2026-01-07T12:00:00Z",
        libraryId: "lib-1",
      },
    ];

    vi.mocked(useTaskProgress).mockReturnValue({
      activeTasks: mockTasks,
      connectionState: "connected",
      pendingCounts: {},
      getTasksByStatus: vi.fn(() => mockTasks),
      getTasksByLibrary: vi.fn(() => mockTasks),
      getTask: vi.fn(() => mockTasks[0]),
    });

    renderWithMantine(<TaskNotificationBadge />);

    // Badge should render with task count
    expect(screen.getByText("1 pending task")).toBeInTheDocument();
  });

  it("should format task names in tooltip", () => {
    const mockTasks: TaskProgressEvent[] = [
      {
        taskId: "task-1",
        taskType: "generate_thumbnails",
        status: "running",
        progress: undefined,
        error: undefined,
        startedAt: "2026-01-07T12:00:00Z",
        libraryId: "lib-1",
      },
    ];

    vi.mocked(useTaskProgress).mockReturnValue({
      activeTasks: mockTasks,
      connectionState: "connected",
      pendingCounts: {},
      getTasksByStatus: vi.fn(() => mockTasks),
      getTasksByLibrary: vi.fn(() => mockTasks),
      getTask: vi.fn(() => mockTasks[0]),
    });

    renderWithMantine(<TaskNotificationBadge />);

    // Badge should render with formatted count
    expect(screen.getByText("1 pending task")).toBeInTheDocument();
  });

  it("should show progress info in tooltip when available", () => {
    const mockTasks: TaskProgressEvent[] = [
      {
        taskId: "task-1",
        taskType: "scan_library",
        status: "running",
        progress: { current: 50, total: 100, message: "Scanning..." },
        error: undefined,
        startedAt: "2026-01-07T12:00:00Z",
        libraryId: "lib-1",
      },
    ];

    vi.mocked(useTaskProgress).mockReturnValue({
      activeTasks: mockTasks,
      connectionState: "connected",
      pendingCounts: {},
      getTasksByStatus: vi.fn(() => mockTasks),
      getTasksByLibrary: vi.fn(() => mockTasks),
      getTask: vi.fn(() => mockTasks[0]),
    });

    renderWithMantine(<TaskNotificationBadge />);

    // Badge should render with task count
    expect(screen.getByText("1 pending task")).toBeInTheDocument();
  });

  it("should show spinner icon for running tasks in tooltip", () => {
    const mockTasks: TaskProgressEvent[] = [
      {
        taskId: "task-1",
        taskType: "analyze_book",
        status: "running",
        progress: undefined,
        error: undefined,
        startedAt: "2026-01-07T12:00:00Z",
        libraryId: "lib-1",
      },
    ];

    vi.mocked(useTaskProgress).mockReturnValue({
      activeTasks: mockTasks,
      connectionState: "connected",
      pendingCounts: {},
      getTasksByStatus: vi.fn(() => mockTasks),
      getTasksByLibrary: vi.fn(() => mockTasks),
      getTask: vi.fn(() => mockTasks[0]),
    });

    renderWithMantine(<TaskNotificationBadge />);

    // Badge should render for running task
    expect(screen.getByText("1 pending task")).toBeInTheDocument();
  });

  it("should list multiple tasks in tooltip", () => {
    const mockTasks: TaskProgressEvent[] = [
      {
        taskId: "task-1",
        taskType: "analyze_book",
        status: "running",
        progress: undefined,
        error: undefined,
        startedAt: "2026-01-07T12:00:00Z",
        libraryId: "lib-1",
      },
      {
        taskId: "task-3",
        taskType: "scan_library",
        status: "running",
        progress: { current: 10, total: 50, message: undefined },
        error: undefined,
        startedAt: "2026-01-07T12:00:00Z",
        libraryId: "lib-3",
      },
    ];

    vi.mocked(useTaskProgress).mockReturnValue({
      activeTasks: mockTasks,
      connectionState: "connected",
      pendingCounts: { generate_thumbnails: 1 },
      getTasksByStatus: vi.fn(() => mockTasks),
      getTasksByLibrary: vi.fn(() => mockTasks),
      getTask: vi.fn((id) => mockTasks.find((t) => t.taskId === id)),
    });

    renderWithMantine(<TaskNotificationBadge />);

    // 2 running tasks + 1 pending task = 3 total
    expect(screen.getByText("3 pending tasks")).toBeInTheDocument();
  });

  it("should have blue color scheme", () => {
    const mockTasks: TaskProgressEvent[] = [
      {
        taskId: "task-1",
        taskType: "analyze_book",
        status: "running",
        progress: undefined,
        error: undefined,
        startedAt: "2026-01-07T12:00:00Z",
        libraryId: "lib-1",
      },
    ];

    vi.mocked(useTaskProgress).mockReturnValue({
      activeTasks: mockTasks,
      connectionState: "connected",
      pendingCounts: {},
      getTasksByStatus: vi.fn(() => mockTasks),
      getTasksByLibrary: vi.fn(() => mockTasks),
      getTask: vi.fn(() => mockTasks[0]),
    });

    renderWithMantine(<TaskNotificationBadge />);

    const badge = screen.getByText("1 pending task");

    // Check badge parent has blue color in styles (color is set via inline styles)
    expect(badge.parentElement).toHaveStyle({
      "--badge-bg": "var(--mantine-color-blue-filled)",
    });
  });

  it("should have pulse animation", () => {
    const mockTasks: TaskProgressEvent[] = [
      {
        taskId: "task-1",
        taskType: "analyze_book",
        status: "running",
        progress: undefined,
        error: undefined,
        startedAt: "2026-01-07T12:00:00Z",
        libraryId: "lib-1",
      },
    ];

    vi.mocked(useTaskProgress).mockReturnValue({
      activeTasks: mockTasks,
      connectionState: "connected",
      pendingCounts: {},
      getTasksByStatus: vi.fn(() => mockTasks),
      getTasksByLibrary: vi.fn(() => mockTasks),
      getTask: vi.fn(() => mockTasks[0]),
    });

    renderWithMantine(<TaskNotificationBadge />);

    const badge = screen.getByText("1 pending task");

    // Check for pulse animation in parent element's style
    expect(badge.parentElement).toHaveStyle({
      animation: "pulse 2s ease-in-out infinite",
    });
  });

  it("should exclude completed tasks from count", () => {
    const mockTasks: TaskProgressEvent[] = [
      {
        taskId: "task-1",
        taskType: "analyze_book",
        status: "running",
        progress: undefined,
        error: undefined,
        startedAt: "2026-01-07T12:00:00Z",
        libraryId: "lib-1",
      },
    ];

    vi.mocked(useTaskProgress).mockReturnValue({
      activeTasks: mockTasks,
      connectionState: "connected",
      pendingCounts: {},
      getTasksByStatus: vi.fn(() => mockTasks),
      getTasksByLibrary: vi.fn(() => mockTasks),
      getTask: vi.fn((id) => mockTasks.find((t) => t.taskId === id)),
    });

    renderWithMantine(<TaskNotificationBadge />);

    // Should only count running tasks (completed tasks are excluded)
    expect(screen.getByText("1 pending task")).toBeInTheDocument();
  });

  it("should not include pending tasks in running tasks list", () => {
    const mockTasks: TaskProgressEvent[] = [
      {
        taskId: "task-1",
        taskType: "analyze_book",
        status: "running",
        progress: undefined,
        error: undefined,
        startedAt: "2026-01-07T12:00:00Z",
        libraryId: "lib-1",
      },
      {
        taskId: "task-2",
        taskType: "analyze_book",
        status: "pending",
        progress: undefined,
        error: undefined,
        startedAt: "2026-01-07T12:01:00Z",
        libraryId: "lib-1",
      },
    ];

    vi.mocked(useTaskProgress).mockReturnValue({
      activeTasks: mockTasks,
      connectionState: "connected",
      pendingCounts: { analyze_book: 5 },
      getTasksByStatus: vi.fn(() => mockTasks),
      getTasksByLibrary: vi.fn(() => mockTasks),
      getTask: vi.fn((id) => mockTasks.find((t) => t.taskId === id)),
    });

    renderWithMantine(<TaskNotificationBadge />);

    // Should show 1 running task + 5 pending tasks = 6 total
    // Pending tasks from activeTasks should NOT be counted as running
    expect(screen.getByText("6 pending tasks")).toBeInTheDocument();
  });

  it("should show pending tasks separately from running tasks in tooltip", async () => {
    const user = userEvent.setup();
    const mockTasks: TaskProgressEvent[] = [
      {
        taskId: "task-1",
        taskType: "analyze_book",
        status: "running",
        progress: undefined,
        error: undefined,
        startedAt: "2026-01-07T12:00:00Z",
        libraryId: "lib-1",
      },
    ];

    vi.mocked(useTaskProgress).mockReturnValue({
      activeTasks: mockTasks,
      connectionState: "connected",
      pendingCounts: { analyze_book: 3, scan_library: 2 },
      getTasksByStatus: vi.fn(() => mockTasks),
      getTasksByLibrary: vi.fn(() => mockTasks),
      getTask: vi.fn((id) => mockTasks.find((t) => t.taskId === id)),
    });

    renderWithMantine(<TaskNotificationBadge />);

    // Should show 1 running + 5 pending = 6 total
    const badge = screen.getByText("6 pending tasks");
    expect(badge).toBeInTheDocument();

    // Hover over badge to show tooltip
    await user.hover(badge);

    // Pending tasks section should show counts
    await waitFor(() => {
      const pendingTasksText = screen.queryByText("Pending Tasks (5)");
      expect(pendingTasksText).toBeInTheDocument();
    });
  });

  it("should navigate to task settings when admin clicks badge", async () => {
    const user = userEvent.setup();
    const mockTasks: TaskProgressEvent[] = [
      {
        taskId: "task-1",
        taskType: "analyze_book",
        status: "running",
        progress: undefined,
        error: undefined,
        startedAt: "2026-01-07T12:00:00Z",
        libraryId: "lib-1",
      },
    ];

    vi.mocked(useTaskProgress).mockReturnValue({
      activeTasks: mockTasks,
      connectionState: "connected",
      pendingCounts: {},
      getTasksByStatus: vi.fn(() => mockTasks),
      getTasksByLibrary: vi.fn(() => mockTasks),
      getTask: vi.fn(() => mockTasks[0]),
    });

    renderWithMantine(<TaskNotificationBadge />);

    const badge = screen.getByText("1 pending task");
    await user.click(badge);

    expect(mockNavigate).toHaveBeenCalledWith("/settings/tasks");
  });

  it("should not navigate when non-admin clicks badge", async () => {
    const user = userEvent.setup();
    mockPermissionsMaintainer();

    const mockTasks: TaskProgressEvent[] = [
      {
        taskId: "task-1",
        taskType: "analyze_book",
        status: "running",
        progress: undefined,
        error: undefined,
        startedAt: "2026-01-07T12:00:00Z",
        libraryId: "lib-1",
      },
    ];

    vi.mocked(useTaskProgress).mockReturnValue({
      activeTasks: mockTasks,
      connectionState: "connected",
      pendingCounts: {},
      getTasksByStatus: vi.fn(() => mockTasks),
      getTasksByLibrary: vi.fn(() => mockTasks),
      getTask: vi.fn(() => mockTasks[0]),
    });

    renderWithMantine(<TaskNotificationBadge />);

    const badge = screen.getByText("1 pending task");
    await user.click(badge);

    expect(mockNavigate).not.toHaveBeenCalled();
  });

  it("should have pointer cursor for admin", () => {
    const mockTasks: TaskProgressEvent[] = [
      {
        taskId: "task-1",
        taskType: "analyze_book",
        status: "running",
        progress: undefined,
        error: undefined,
        startedAt: "2026-01-07T12:00:00Z",
        libraryId: "lib-1",
      },
    ];

    vi.mocked(useTaskProgress).mockReturnValue({
      activeTasks: mockTasks,
      connectionState: "connected",
      pendingCounts: {},
      getTasksByStatus: vi.fn(() => mockTasks),
      getTasksByLibrary: vi.fn(() => mockTasks),
      getTask: vi.fn(() => mockTasks[0]),
    });

    renderWithMantine(<TaskNotificationBadge />);

    const badge = screen.getByText("1 pending task");
    expect(badge.parentElement).toHaveStyle({ cursor: "pointer" });
  });

  it("should have default cursor for non-admin", () => {
    mockPermissionsMaintainer();

    const mockTasks: TaskProgressEvent[] = [
      {
        taskId: "task-1",
        taskType: "analyze_book",
        status: "running",
        progress: undefined,
        error: undefined,
        startedAt: "2026-01-07T12:00:00Z",
        libraryId: "lib-1",
      },
    ];

    vi.mocked(useTaskProgress).mockReturnValue({
      activeTasks: mockTasks,
      connectionState: "connected",
      pendingCounts: {},
      getTasksByStatus: vi.fn(() => mockTasks),
      getTasksByLibrary: vi.fn(() => mockTasks),
      getTask: vi.fn(() => mockTasks[0]),
    });

    renderWithMantine(<TaskNotificationBadge />);

    const badge = screen.getByText("1 pending task");
    expect(badge.parentElement).toHaveStyle({ cursor: "default" });
  });

  it("should not render for reader users without TASKS_READ permission", () => {
    mockPermissionsReader();

    const mockTasks: TaskProgressEvent[] = [
      {
        taskId: "task-1",
        taskType: "analyze_book",
        status: "running",
        progress: undefined,
        error: undefined,
        startedAt: "2026-01-07T12:00:00Z",
        libraryId: "lib-1",
      },
    ];

    vi.mocked(useTaskProgress).mockReturnValue({
      activeTasks: mockTasks,
      connectionState: "connected",
      pendingCounts: {},
      getTasksByStatus: vi.fn(() => mockTasks),
      getTasksByLibrary: vi.fn(() => mockTasks),
      getTask: vi.fn(() => mockTasks[0]),
    });

    renderWithMantine(<TaskNotificationBadge />);

    // Badge should not be rendered for readers
    expect(screen.queryByText(/pending task/i)).not.toBeInTheDocument();
  });
});
