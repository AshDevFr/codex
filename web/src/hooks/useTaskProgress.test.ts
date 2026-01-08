import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import * as tasksApi from "@/api/tasks";
import { useAuthStore } from "@/store/authStore";
import type { TaskProgressEvent } from "@/types/events";
import { useTaskProgress } from "./useTaskProgress";

// Mock the tasks API
vi.mock("@/api/tasks");

// Mock the auth store
vi.mock("@/store/authStore", () => ({
	useAuthStore: vi.fn(() => ({
		isAuthenticated: true,
	})),
}));

describe("useTaskProgress", () => {
	let mockUnsubscribe: ReturnType<typeof vi.fn>;

	beforeEach(() => {
		mockUnsubscribe = vi.fn();

		Storage.prototype.getItem = vi.fn((key) => {
			if (key === "jwt_token") return "test-token";
			return null;
		});

		// Mock fetchPendingTaskCounts to return empty object
		vi.mocked(tasksApi.fetchPendingTaskCounts).mockResolvedValue({});

		vi.useFakeTimers();
	});

	afterEach(() => {
		vi.restoreAllMocks();
		vi.useRealTimers();
	});

	it("should subscribe to task progress on mount", async () => {
		const mockSubscribe = vi
			.spyOn(tasksApi, "subscribeToTaskProgress")
			.mockReturnValue(mockUnsubscribe);

		renderHook(() => useTaskProgress());

		expect(mockSubscribe).toHaveBeenCalled();
	});

	it("should not subscribe if no token is present", () => {
		// Mock auth store to return not authenticated
		vi.mocked(useAuthStore).mockReturnValue({
			isAuthenticated: false,
		} as ReturnType<typeof useAuthStore>);

		const mockSubscribe = vi
			.spyOn(tasksApi, "subscribeToTaskProgress")
			.mockReturnValue(mockUnsubscribe);

		renderHook(() => useTaskProgress());

		expect(mockSubscribe).not.toHaveBeenCalled();
	});

	it("should unsubscribe on unmount", () => {
		const mockSubscribe = vi
			.spyOn(tasksApi, "subscribeToTaskProgress")
			.mockReturnValue(mockUnsubscribe);

		const { unmount } = renderHook(() => useTaskProgress());

		expect(mockSubscribe).toHaveBeenCalled();

		unmount();

		expect(mockUnsubscribe).toHaveBeenCalled();
	});

	it("should track active tasks", () => {
		let capturedCallback: ((event: TaskProgressEvent) => void) | undefined;

		vi.spyOn(tasksApi, "subscribeToTaskProgress").mockImplementation(
			(onProgress) => {
				capturedCallback = onProgress;
				return mockUnsubscribe;
			},
		);

		const { result } = renderHook(() => useTaskProgress());

		expect(capturedCallback).toBeDefined();

		// Initially no tasks
		expect(result.current.activeTasks).toHaveLength(0);

		// Simulate task started
		const taskEvent: TaskProgressEvent = {
			task_id: "task-1",
			task_type: "analyze_book",
			status: "running",
			progress: undefined,
			error: undefined,
			started_at: "2026-01-07T12:00:00Z",
			library_id: "lib-1",
		};

		act(() => {
			capturedCallback!(taskEvent);
		});

		expect(result.current.activeTasks).toHaveLength(1);
		expect(result.current.activeTasks[0]).toEqual(taskEvent);
	});

	it("should remove completed tasks after 5 seconds", () => {
		let capturedCallback: ((event: TaskProgressEvent) => void) | undefined;

		vi.spyOn(tasksApi, "subscribeToTaskProgress").mockImplementation(
			(onProgress) => {
				capturedCallback = onProgress;
				return mockUnsubscribe;
			},
		);

		const { result } = renderHook(() => useTaskProgress());

		expect(capturedCallback).toBeDefined();

		// Simulate task completed
		const completedTask: TaskProgressEvent = {
			task_id: "task-2",
			task_type: "generate_thumbnails",
			status: "completed",
			progress: { current: 10, total: 10, message: "Done" },
			error: undefined,
			started_at: "2026-01-07T12:00:00Z",
			library_id: "lib-2",
		};

		act(() => {
			capturedCallback!(completedTask);
		});

		expect(result.current.activeTasks).toHaveLength(1);

		// Fast-forward 5 seconds
		act(() => {
			vi.advanceTimersByTime(5000);
		});

		expect(result.current.activeTasks).toHaveLength(0);
	});

	it("should remove failed tasks after 5 seconds", () => {
		let capturedCallback: ((event: TaskProgressEvent) => void) | undefined;

		vi.spyOn(tasksApi, "subscribeToTaskProgress").mockImplementation(
			(onProgress) => {
				capturedCallback = onProgress;
				return mockUnsubscribe;
			},
		);

		const { result } = renderHook(() => useTaskProgress());

		expect(capturedCallback).toBeDefined();

		// Simulate task failed
		const failedTask: TaskProgressEvent = {
			task_id: "task-3",
			task_type: "scan_library",
			status: "failed",
			progress: undefined,
			error: "Database connection lost",
			started_at: "2026-01-07T12:00:00Z",
			library_id: "lib-3",
		};

		act(() => {
			capturedCallback!(failedTask);
		});

		expect(result.current.activeTasks).toHaveLength(1);

		// Fast-forward 5 seconds
		act(() => {
			vi.advanceTimersByTime(5000);
		});

		expect(result.current.activeTasks).toHaveLength(0);
	});

	it("should filter tasks by status", () => {
		let capturedCallback: ((event: TaskProgressEvent) => void) | undefined;

		vi.spyOn(tasksApi, "subscribeToTaskProgress").mockImplementation(
			(onProgress) => {
				capturedCallback = onProgress;
				return mockUnsubscribe;
			},
		);

		const { result } = renderHook(() => useTaskProgress());

		expect(capturedCallback).toBeDefined();

		// Add multiple tasks with different statuses
		act(() => {
			capturedCallback!({
				task_id: "task-1",
				task_type: "analyze_book",
				status: "pending",
				progress: undefined,
				error: undefined,
				started_at: "2026-01-07T12:00:00Z",
				library_id: "lib-1",
			});
			capturedCallback!({
				task_id: "task-2",
				task_type: "analyze_book",
				status: "running",
				progress: undefined,
				error: undefined,
				started_at: "2026-01-07T12:01:00Z",
				library_id: "lib-1",
			});
			capturedCallback!({
				task_id: "task-3",
				task_type: "analyze_book",
				status: "completed",
				progress: undefined,
				error: undefined,
				started_at: "2026-01-07T12:02:00Z",
				library_id: "lib-1",
			});
		});

		const runningTasks = result.current.getTasksByStatus("running");
		expect(runningTasks).toHaveLength(1);
		expect(runningTasks[0].task_id).toBe("task-2");

		const pendingTasks = result.current.getTasksByStatus("pending");
		expect(pendingTasks).toHaveLength(1);
		expect(pendingTasks[0].task_id).toBe("task-1");
	});

	it("should filter tasks by library", () => {
		let capturedCallback: ((event: TaskProgressEvent) => void) | undefined;

		vi.spyOn(tasksApi, "subscribeToTaskProgress").mockImplementation(
			(onProgress) => {
				capturedCallback = onProgress;
				return mockUnsubscribe;
			},
		);

		const { result } = renderHook(() => useTaskProgress());

		expect(capturedCallback).toBeDefined();

		// Add tasks for different libraries
		act(() => {
			capturedCallback!({
				task_id: "task-1",
				task_type: "analyze_book",
				status: "running",
				progress: undefined,
				error: undefined,
				started_at: "2026-01-07T12:00:00Z",
				library_id: "lib-1",
			});
			capturedCallback!({
				task_id: "task-2",
				task_type: "analyze_book",
				status: "running",
				progress: undefined,
				error: undefined,
				started_at: "2026-01-07T12:01:00Z",
				library_id: "lib-2",
			});
		});

		const lib1Tasks = result.current.getTasksByLibrary("lib-1");
		expect(lib1Tasks).toHaveLength(1);
		expect(lib1Tasks[0].task_id).toBe("task-1");

		const lib2Tasks = result.current.getTasksByLibrary("lib-2");
		expect(lib2Tasks).toHaveLength(1);
		expect(lib2Tasks[0].task_id).toBe("task-2");
	});

	it("should get specific task by ID", () => {
		let capturedCallback: ((event: TaskProgressEvent) => void) | undefined;

		vi.spyOn(tasksApi, "subscribeToTaskProgress").mockImplementation(
			(onProgress) => {
				capturedCallback = onProgress;
				return mockUnsubscribe;
			},
		);

		const { result } = renderHook(() => useTaskProgress());

		expect(capturedCallback).toBeDefined();

		const taskEvent: TaskProgressEvent = {
			task_id: "task-unique",
			task_type: "analyze_book",
			status: "running",
			progress: undefined,
			error: undefined,
			started_at: "2026-01-07T12:00:00Z",
			library_id: "lib-1",
		};

		act(() => {
			capturedCallback!(taskEvent);
		});

		const task = result.current.getTask("task-unique");
		expect(task).toEqual(taskEvent);
	});

	it("should track connection state", () => {
		let capturedConnectionChange:
			| ((
					state: "connecting" | "connected" | "disconnected" | "failed",
			  ) => void)
			| undefined;

		vi.spyOn(tasksApi, "subscribeToTaskProgress").mockImplementation(
			(_onProgress, _onError, onConnectionChange) => {
				capturedConnectionChange = onConnectionChange;
				return mockUnsubscribe;
			},
		);

		const { result } = renderHook(() => useTaskProgress());

		expect(capturedConnectionChange).toBeDefined();

		// Initially disconnected (no connection established yet)
		expect(result.current.connectionState).toBe("disconnected");

		// Simulate connection established
		act(() => {
			capturedConnectionChange!("connected");
		});

		expect(result.current.connectionState).toBe("connected");
	});

	it("should handle errors gracefully", () => {
		const consoleError = vi
			.spyOn(console, "error")
			.mockImplementation(() => {});
		let capturedErrorHandler: ((error: Error) => void) | undefined;

		vi.spyOn(tasksApi, "subscribeToTaskProgress").mockImplementation(
			(_onProgress, onError) => {
				capturedErrorHandler = onError;
				return mockUnsubscribe;
			},
		);

		renderHook(() => useTaskProgress());

		expect(capturedErrorHandler).toBeDefined();

		// Simulate an error
		const testError = new Error("Connection failed");
		act(() => {
			capturedErrorHandler!(testError);
		});

		expect(consoleError).toHaveBeenCalledWith(
			"Task progress subscription error:",
			testError,
		);

		consoleError.mockRestore();
	});

	it("should update existing tasks on progress events", () => {
		let capturedCallback: ((event: TaskProgressEvent) => void) | undefined;

		vi.spyOn(tasksApi, "subscribeToTaskProgress").mockImplementation(
			(onProgress) => {
				capturedCallback = onProgress;
				return mockUnsubscribe;
			},
		);

		const { result } = renderHook(() => useTaskProgress());

		expect(capturedCallback).toBeDefined();

		// Add initial task
		act(() => {
			capturedCallback!({
				task_id: "task-1",
				task_type: "analyze_book",
				status: "running",
				progress: { current: 5, total: 10, message: "Processing..." },
				error: undefined,
				started_at: "2026-01-07T12:00:00Z",
				library_id: "lib-1",
			});
		});

		expect(result.current.activeTasks[0].progress?.current).toBe(5);

		// Update task progress
		act(() => {
			capturedCallback!({
				task_id: "task-1",
				task_type: "analyze_book",
				status: "running",
				progress: { current: 10, total: 10, message: "Almost done..." },
				error: undefined,
				started_at: "2026-01-07T12:00:00Z",
				library_id: "lib-1",
			});
		});

		expect(result.current.activeTasks[0].progress?.current).toBe(10);
		expect(result.current.activeTasks[0].progress?.message).toBe(
			"Almost done...",
		);
	});
});
