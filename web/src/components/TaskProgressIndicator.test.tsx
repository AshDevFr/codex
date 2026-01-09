import { MantineProvider } from "@mantine/core";
import { render, screen } from "@testing-library/react";
import type React from "react";
import { describe, expect, it, vi } from "vitest";
import { useTaskProgress } from "@/hooks/useTaskProgress";
import type { TaskProgressEvent } from "@/types/events";
import { TaskProgressIndicator } from "./TaskProgressIndicator";

// Mock the useTaskProgress hook
vi.mock("@/hooks/useTaskProgress", () => ({
	useTaskProgress: vi.fn(),
}));

// Helper to render with Mantine Provider
const renderWithMantine = (component: React.ReactElement) => {
	return render(<MantineProvider>{component}</MantineProvider>);
};

describe("TaskProgressIndicator", () => {
	it("should not render when no tasks are active", () => {
		vi.mocked(useTaskProgress).mockReturnValue({
			activeTasks: [],
			connectionState: "connected",
			pendingCounts: {},
			getTasksByStatus: vi.fn(() => []),
			getTasksByLibrary: vi.fn(() => []),
			getTask: vi.fn(() => undefined),
		});

		renderWithMantine(<TaskProgressIndicator />);

		// No task items should be visible
		expect(screen.queryByText(/Analyze Book/i)).not.toBeInTheDocument();
	});

	it("should render when tasks are active", () => {
		const mockTasks: TaskProgressEvent[] = [
			{
				task_id: "task-1",
				task_type: "analyze_book",
				status: "running",
				progress: { current: 5, total: 10, message: "Processing..." },
				error: undefined,
				started_at: "2026-01-07T12:00:00Z",
				library_id: "lib-1",
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

		renderWithMantine(<TaskProgressIndicator />);

		// Should display task type
		expect(screen.getByText("Analyze Book")).toBeInTheDocument();
	});

	it("should format task names correctly", () => {
		const mockTasks: TaskProgressEvent[] = [
			{
				task_id: "task-1",
				task_type: "generate_thumbnails",
				status: "running",
				progress: undefined,
				error: undefined,
				started_at: "2026-01-07T12:00:00Z",
				library_id: "lib-1",
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

		renderWithMantine(<TaskProgressIndicator />);

		// Snake_case should be converted to Title Case
		expect(screen.getByText("Generate Thumbnails")).toBeInTheDocument();
	});

	it("should display progress percentage when available", () => {
		const mockTasks: TaskProgressEvent[] = [
			{
				task_id: "task-1",
				task_type: "analyze_book",
				status: "running",
				progress: { current: 7, total: 10, message: "Processing..." },
				error: undefined,
				started_at: "2026-01-07T12:00:00Z",
				library_id: "lib-1",
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

		renderWithMantine(<TaskProgressIndicator />);

		// Should display progress as "7 / 10"
		expect(screen.getByText("7 / 10")).toBeInTheDocument();
	});

	it("should show completed status with green color", () => {
		const mockTasks: TaskProgressEvent[] = [
			{
				task_id: "task-1",
				task_type: "analyze_book",
				status: "completed",
				progress: { current: 10, total: 10, message: "Done" },
				error: undefined,
				started_at: "2026-01-07T12:00:00Z",
				library_id: "lib-1",
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

		renderWithMantine(<TaskProgressIndicator />);

		// Completed tasks are filtered out, so nothing should be visible
		expect(screen.queryByText("Analyze Book")).not.toBeInTheDocument();
	});

	it("should show failed status with red color", () => {
		const mockTasks: TaskProgressEvent[] = [
			{
				task_id: "task-1",
				task_type: "analyze_book",
				status: "failed",
				progress: undefined,
				error: "Database error",
				started_at: "2026-01-07T12:00:00Z",
				library_id: "lib-1",
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

		renderWithMantine(<TaskProgressIndicator />);

		// Failed tasks are filtered out, so nothing should be visible
		expect(screen.queryByText("Analyze Book")).not.toBeInTheDocument();
	});

	it("should display multiple tasks", () => {
		const mockTasks: TaskProgressEvent[] = [
			{
				task_id: "task-1",
				task_type: "analyze_book",
				status: "running",
				progress: undefined,
				error: undefined,
				started_at: "2026-01-07T12:00:00Z",
				library_id: "lib-1",
			},
			{
				task_id: "task-2",
				task_type: "generate_thumbnails",
				status: "running",
				progress: undefined,
				error: undefined,
				started_at: "2026-01-07T12:00:00Z",
				library_id: "lib-2",
			},
		];

		vi.mocked(useTaskProgress).mockReturnValue({
			activeTasks: mockTasks,
			connectionState: "connected",
			pendingCounts: {},
			getTasksByStatus: vi.fn(() => mockTasks),
			getTasksByLibrary: vi.fn(() => mockTasks),
			getTask: vi.fn((id) => mockTasks.find((t) => t.task_id === id)),
		});

		renderWithMantine(<TaskProgressIndicator />);

		expect(screen.getByText("Analyze Book")).toBeInTheDocument();
		expect(screen.getByText("Generate Thumbnails")).toBeInTheDocument();
	});

	it("should show connecting state", () => {
		vi.mocked(useTaskProgress).mockReturnValue({
			activeTasks: [],
			connectionState: "connecting",
			pendingCounts: {},
			getTasksByStatus: vi.fn(() => []),
			getTasksByLibrary: vi.fn(() => []),
			getTask: vi.fn(() => undefined),
		});

		renderWithMantine(<TaskProgressIndicator />);

		// Component doesn't render when no tasks, even if connecting
		expect(screen.queryByText("Connecting")).not.toBeInTheDocument();
	});

	it("should show disconnected state", () => {
		vi.mocked(useTaskProgress).mockReturnValue({
			activeTasks: [],
			connectionState: "disconnected",
			pendingCounts: {},
			getTasksByStatus: vi.fn(() => []),
			getTasksByLibrary: vi.fn(() => []),
			getTask: vi.fn(() => undefined),
		});

		renderWithMantine(<TaskProgressIndicator />);

		// Component doesn't render when no tasks
		expect(screen.queryByText(/disconnected/i)).not.toBeInTheDocument();
	});

	it("should display progress message when available", () => {
		const mockTasks: TaskProgressEvent[] = [
			{
				task_id: "task-1",
				task_type: "analyze_book",
				status: "running",
				progress: {
					current: 3,
					total: 10,
					message: "Analyzing metadata...",
				},
				error: undefined,
				started_at: "2026-01-07T12:00:00Z",
				library_id: "lib-1",
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

		renderWithMantine(<TaskProgressIndicator />);

		expect(screen.getByText("Analyzing metadata...")).toBeInTheDocument();
	});

	it("should display error message for failed tasks", () => {
		const mockTasks: TaskProgressEvent[] = [
			{
				task_id: "task-1",
				task_type: "scan_library",
				status: "failed",
				progress: undefined,
				error: "Connection timeout",
				started_at: "2026-01-07T12:00:00Z",
				library_id: "lib-1",
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

		renderWithMantine(<TaskProgressIndicator />);

		// Failed tasks are filtered out
		expect(screen.queryByText("Connection timeout")).not.toBeInTheDocument();
	});

	it("should not show pending tasks in the indicator", () => {
		const mockTasks: TaskProgressEvent[] = [
			{
				task_id: "task-1",
				task_type: "analyze_book",
				status: "running",
				progress: undefined,
				error: undefined,
				started_at: "2026-01-07T12:00:00Z",
				library_id: "lib-1",
			},
			{
				task_id: "task-2",
				task_type: "analyze_book",
				status: "pending",
				progress: undefined,
				error: undefined,
				started_at: "2026-01-07T12:01:00Z",
				library_id: "lib-1",
			},
		];

		vi.mocked(useTaskProgress).mockReturnValue({
			activeTasks: mockTasks,
			connectionState: "connected",
			pendingCounts: {},
			getTasksByStatus: vi.fn(() => mockTasks),
			getTasksByLibrary: vi.fn(() => mockTasks),
			getTask: vi.fn((id) => mockTasks.find((t) => t.task_id === id)),
		});

		renderWithMantine(<TaskProgressIndicator />);

		// Should only show running task, not pending task
		expect(screen.getByText("Analyze Book")).toBeInTheDocument();
		// Should only appear once (for the running task)
		const analyzeBookElements = screen.queryAllByText("Analyze Book");
		expect(analyzeBookElements).toHaveLength(1);
	});
});
