import { describe, expect, it, vi } from "vitest";
import { useTaskProgress } from "@/hooks/useTaskProgress";
import { renderWithProviders, screen } from "@/test/utils";
import type { ActiveTask } from "@/types";
import { TasksSettings } from "./TasksSettings";

vi.mock("@/hooks/useTaskProgress", () => ({
  useTaskProgress: vi.fn(),
}));

vi.mock("@/api/tasks", () => ({
  fetchTaskStats: vi.fn(async () => ({
    pending: 0,
    processing: 1,
    completed: 0,
    failed: 0,
    total: 1,
  })),
  fetchTasksByStatus: vi.fn(async () => []),
}));

vi.mock("@/api/client", () => ({
  api: { post: vi.fn(), delete: vi.fn() },
}));

const mockActiveTasks = (tasks: ActiveTask[]) => {
  vi.mocked(useTaskProgress).mockReturnValue({
    activeTasks: tasks,
    connectionState: "connected",
    pendingCounts: {},
    getTasksByStatus: vi.fn((status) =>
      tasks.filter((t) => t.status === status),
    ),
    getTasksByLibrary: vi.fn(() => tasks),
    getTask: vi.fn((id) => tasks.find((t) => t.taskId === id)),
  });
};

const runningTask: ActiveTask = {
  taskId: "task-1",
  taskType: "analyze_book",
  status: "running",
  startedAt: "2026-01-07T12:00:00Z",
  libraryId: "lib-1",
};

describe("TasksSettings active tasks panel", () => {
  it("labels a task with its resolved target when no message has arrived", async () => {
    // The poll snapshot resolves the title; SSE is the only source of messages
    // and analyze tasks emit theirs once at start, so a page opened mid-task
    // has a title but no message.
    mockActiveTasks([{ ...runningTask, bookTitle: "Naruto Vol. 12" }]);

    renderWithProviders(<TasksSettings />);

    expect(await screen.findByText("Naruto Vol. 12")).toBeInTheDocument();
    expect(screen.queryByText("Processing...")).not.toBeInTheDocument();
  });

  it("prefers the live progress message over the resolved target", async () => {
    mockActiveTasks([
      {
        ...runningTask,
        bookTitle: "Naruto Vol. 12",
        progress: { current: 0, total: 1, message: "Analyzing i1.cbz" },
      },
    ]);

    renderWithProviders(<TasksSettings />);

    expect(await screen.findByText("Analyzing i1.cbz")).toBeInTheDocument();
  });

  it("does not show terminal tasks in the active panel", async () => {
    // The shared manager keeps completed tasks around for a short linger so the
    // global indicator can show their final state; this panel only lists work
    // that is still in flight.
    mockActiveTasks([
      { ...runningTask, status: "completed", bookTitle: "Naruto Vol. 12" },
    ]);

    renderWithProviders(<TasksSettings />);

    expect(await screen.findByText("Tasks")).toBeInTheDocument();
    expect(screen.queryByText("Active Tasks")).not.toBeInTheDocument();
  });
});
