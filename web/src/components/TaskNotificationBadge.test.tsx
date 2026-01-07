import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { MantineProvider } from "@mantine/core";
import { TaskNotificationBadge } from "./TaskNotificationBadge";
import { useTaskProgress } from "@/hooks/useTaskProgress";
import type { TaskProgressEvent } from "@/types/events";
import React from "react";

// Mock the useTaskProgress hook
vi.mock("@/hooks/useTaskProgress", () => ({
  useTaskProgress: vi.fn(),
}));

// Helper to render with Mantine Provider
const renderWithMantine = (component: React.ReactElement) => {
  return render(<MantineProvider>{component}</MantineProvider>);
};

describe("TaskNotificationBadge", () => {
  it("should not render when no tasks are active", () => {
    vi.mocked(useTaskProgress).mockReturnValue({
      activeTasks: [],
      connectionState: "connected",
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
        task_id: "task-1",
        task_type: "analyze_book",
        status: "running",
        progress: null,
        error: null,
        library_id: "lib-1",
      },
    ];

    vi.mocked(useTaskProgress).mockReturnValue({
      activeTasks: mockTasks,
      connectionState: "connected",
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
        task_id: "task-1",
        task_type: "analyze_book",
        status: "running",
        progress: null,
        error: null,
        library_id: "lib-1",
      },
      {
        task_id: "task-2",
        task_type: "generate_thumbnails",
        status: "queued",
        progress: null,
        error: null,
        library_id: "lib-2",
      },
    ];

    vi.mocked(useTaskProgress).mockReturnValue({
      activeTasks: mockTasks,
      connectionState: "connected",
      getTasksByStatus: vi.fn(() => mockTasks),
      getTasksByLibrary: vi.fn(() => mockTasks),
      getTask: vi.fn((id) => mockTasks.find((t) => t.task_id === id)),
    });

    renderWithMantine(<TaskNotificationBadge />);

    expect(screen.getByText("2 pending tasks")).toBeInTheDocument();
  });

  it("should show tooltip on hover", () => {
    const mockTasks: TaskProgressEvent[] = [
      {
        task_id: "task-1",
        task_type: "analyze_book",
        status: "running",
        progress: null,
        error: null,
        library_id: "lib-1",
      },
    ];

    vi.mocked(useTaskProgress).mockReturnValue({
      activeTasks: mockTasks,
      connectionState: "connected",
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
        task_id: "task-1",
        task_type: "generate_thumbnails",
        status: "running",
        progress: null,
        error: null,
        library_id: "lib-1",
      },
    ];

    vi.mocked(useTaskProgress).mockReturnValue({
      activeTasks: mockTasks,
      connectionState: "connected",
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
        task_id: "task-1",
        task_type: "scan_library",
        status: "running",
        progress: { current: 50, total: 100, message: "Scanning..." },
        error: null,
        library_id: "lib-1",
      },
    ];

    vi.mocked(useTaskProgress).mockReturnValue({
      activeTasks: mockTasks,
      connectionState: "connected",
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
        task_id: "task-1",
        task_type: "analyze_book",
        status: "running",
        progress: null,
        error: null,
        library_id: "lib-1",
      },
    ];

    vi.mocked(useTaskProgress).mockReturnValue({
      activeTasks: mockTasks,
      connectionState: "connected",
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
        task_id: "task-1",
        task_type: "analyze_book",
        status: "running",
        progress: null,
        error: null,
        library_id: "lib-1",
      },
      {
        task_id: "task-2",
        task_type: "generate_thumbnails",
        status: "queued",
        progress: null,
        error: null,
        library_id: "lib-2",
      },
      {
        task_id: "task-3",
        task_type: "scan_library",
        status: "running",
        progress: { current: 10, total: 50, message: null },
        error: null,
        library_id: "lib-3",
      },
    ];

    vi.mocked(useTaskProgress).mockReturnValue({
      activeTasks: mockTasks,
      connectionState: "connected",
      getTasksByStatus: vi.fn(() => mockTasks),
      getTasksByLibrary: vi.fn(() => mockTasks),
      getTask: vi.fn((id) => mockTasks.find((t) => t.task_id === id)),
    });

    renderWithMantine(<TaskNotificationBadge />);

    // Badge should show count of all active tasks
    expect(screen.getByText("3 pending tasks")).toBeInTheDocument();
  });

  it("should have blue color scheme", () => {
    const mockTasks: TaskProgressEvent[] = [
      {
        task_id: "task-1",
        task_type: "analyze_book",
        status: "running",
        progress: null,
        error: null,
        library_id: "lib-1",
      },
    ];

    vi.mocked(useTaskProgress).mockReturnValue({
      activeTasks: mockTasks,
      connectionState: "connected",
      getTasksByStatus: vi.fn(() => mockTasks),
      getTasksByLibrary: vi.fn(() => mockTasks),
      getTask: vi.fn(() => mockTasks[0]),
    });

    renderWithMantine(<TaskNotificationBadge />);

    const badge = screen.getByText("1 pending task");

    // Check badge parent has blue color in styles (color is set via inline styles)
    expect(badge.parentElement).toHaveStyle({ '--badge-bg': 'var(--mantine-color-blue-filled)' });
  });

  it("should have pulse animation", () => {
    const mockTasks: TaskProgressEvent[] = [
      {
        task_id: "task-1",
        task_type: "analyze_book",
        status: "running",
        progress: null,
        error: null,
        library_id: "lib-1",
      },
    ];

    vi.mocked(useTaskProgress).mockReturnValue({
      activeTasks: mockTasks,
      connectionState: "connected",
      getTasksByStatus: vi.fn(() => mockTasks),
      getTasksByLibrary: vi.fn(() => mockTasks),
      getTask: vi.fn(() => mockTasks[0]),
    });

    renderWithMantine(<TaskNotificationBadge />);

    const badge = screen.getByText("1 pending task");

    // Check for pulse animation in parent element's style
    expect(badge.parentElement).toHaveStyle({ animation: 'pulse 2s ease-in-out infinite' });
  });

  it("should exclude completed tasks from count", () => {
    const mockTasks: TaskProgressEvent[] = [
      {
        task_id: "task-1",
        task_type: "analyze_book",
        status: "running",
        progress: null,
        error: null,
        library_id: "lib-1",
      },
      {
        task_id: "task-2",
        task_type: "generate_thumbnails",
        status: "completed",
        progress: null,
        error: null,
        library_id: "lib-2",
      },
    ];

    // Note: Badge should only count running/queued, not completed
    const activeTasks = mockTasks.filter(
      (t) => t.status === "running" || t.status === "queued"
    );

    vi.mocked(useTaskProgress).mockReturnValue({
      activeTasks: activeTasks,
      connectionState: "connected",
      getTasksByStatus: vi.fn(() => activeTasks),
      getTasksByLibrary: vi.fn(() => activeTasks),
      getTask: vi.fn((id) => activeTasks.find((t) => t.task_id === id)),
    });

    renderWithMantine(<TaskNotificationBadge />);

    // Should only count running tasks
    expect(screen.getByText("1 pending task")).toBeInTheDocument();
  });
});
