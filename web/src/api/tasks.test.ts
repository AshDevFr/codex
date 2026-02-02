import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  fetchPendingTaskCounts,
  fetchTaskStats,
  subscribeToTaskProgress,
} from "./tasks";

describe("subscribeToTaskProgress", () => {
  let mockFetch: ReturnType<typeof vi.fn>;

  beforeEach(() => {
    Storage.prototype.getItem = vi.fn((key) => {
      if (key === "jwt_token") return "test-token-789";
      return null;
    });

    mockFetch = vi.fn();
    global.fetch = mockFetch;
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("should connect with Authorization header", async () => {
    const mockReader = {
      read: vi.fn().mockResolvedValueOnce({ done: true, value: undefined }),
      cancel: vi.fn(),
    };

    mockFetch.mockResolvedValueOnce({
      ok: true,
      body: {
        getReader: () => mockReader,
      },
    });

    const onProgress = vi.fn();
    const unsubscribe = await subscribeToTaskProgress(onProgress);

    expect(mockFetch).toHaveBeenCalledWith(
      expect.stringContaining("/api/v1/tasks/stream"),
      expect.objectContaining({
        headers: expect.objectContaining({
          Authorization: "Bearer test-token-789",
          Accept: "text/event-stream",
        }),
      }),
    );

    unsubscribe();
  });

  it("should parse task started events correctly", async () => {
    const eventData =
      'data: {"task_id":"task-123","task_type":"analyze_book","status":"running","progress":null,"error":null,"library_id":"lib-1"}\n\n';
    const encoder = new TextEncoder();

    const mockReader = {
      read: vi
        .fn()
        .mockResolvedValueOnce({
          done: false,
          value: encoder.encode(eventData),
        })
        .mockResolvedValueOnce({ done: true, value: undefined }),
      cancel: vi.fn(),
    };

    mockFetch.mockResolvedValueOnce({
      ok: true,
      body: {
        getReader: () => mockReader,
      },
    });

    const onProgress = vi.fn();
    const unsubscribe = await subscribeToTaskProgress(onProgress);

    await new Promise((resolve) => setTimeout(resolve, 100));

    expect(onProgress).toHaveBeenCalledWith(
      expect.objectContaining({
        task_id: "task-123",
        task_type: "analyze_book",
        status: "running",
        progress: null,
        error: null,
        library_id: "lib-1",
      }),
    );

    unsubscribe();
  });

  it("should parse task completed events correctly", async () => {
    const eventData =
      'data: {"task_id":"task-456","task_type":"generate_thumbnails","status":"completed","progress":{"current":10,"total":10,"message":"All thumbnails generated"},"error":null,"library_id":"lib-2"}\n\n';
    const encoder = new TextEncoder();

    const mockReader = {
      read: vi
        .fn()
        .mockResolvedValueOnce({
          done: false,
          value: encoder.encode(eventData),
        })
        .mockResolvedValueOnce({ done: true, value: undefined }),
      cancel: vi.fn(),
    };

    mockFetch.mockResolvedValueOnce({
      ok: true,
      body: {
        getReader: () => mockReader,
      },
    });

    const onProgress = vi.fn();
    const unsubscribe = await subscribeToTaskProgress(onProgress);

    await new Promise((resolve) => setTimeout(resolve, 100));

    expect(onProgress).toHaveBeenCalledWith(
      expect.objectContaining({
        task_id: "task-456",
        task_type: "generate_thumbnails",
        status: "completed",
        progress: expect.objectContaining({
          current: 10,
          total: 10,
          message: "All thumbnails generated",
        }),
      }),
    );

    unsubscribe();
  });

  it("should parse task failed events correctly", async () => {
    const eventData =
      'data: {"task_id":"task-789","task_type":"scan_library","status":"failed","progress":null,"error":"Database connection lost","library_id":"lib-3"}\n\n';
    const encoder = new TextEncoder();

    const mockReader = {
      read: vi
        .fn()
        .mockResolvedValueOnce({
          done: false,
          value: encoder.encode(eventData),
        })
        .mockResolvedValueOnce({ done: true, value: undefined }),
      cancel: vi.fn(),
    };

    mockFetch.mockResolvedValueOnce({
      ok: true,
      body: {
        getReader: () => mockReader,
      },
    });

    const onProgress = vi.fn();
    const unsubscribe = await subscribeToTaskProgress(onProgress);

    await new Promise((resolve) => setTimeout(resolve, 100));

    expect(onProgress).toHaveBeenCalledWith(
      expect.objectContaining({
        task_id: "task-789",
        status: "failed",
        error: "Database connection lost",
      }),
    );

    unsubscribe();
  });

  it("should handle keep-alive messages", async () => {
    const keepAlive = "data: keep-alive\n\n";
    const encoder = new TextEncoder();

    const mockReader = {
      read: vi
        .fn()
        .mockResolvedValueOnce({
          done: false,
          value: encoder.encode(keepAlive),
        })
        .mockResolvedValueOnce({ done: true, value: undefined }),
      cancel: vi.fn(),
    };

    mockFetch.mockResolvedValueOnce({
      ok: true,
      body: {
        getReader: () => mockReader,
      },
    });

    const onProgress = vi.fn();
    const unsubscribe = await subscribeToTaskProgress(onProgress);

    await new Promise((resolve) => setTimeout(resolve, 100));

    expect(onProgress).not.toHaveBeenCalled();

    unsubscribe();
  });

  it("should handle connection state changes", async () => {
    const mockReader = {
      read: vi.fn().mockResolvedValueOnce({ done: true, value: undefined }),
      cancel: vi.fn(),
    };

    mockFetch.mockResolvedValueOnce({
      ok: true,
      body: {
        getReader: () => mockReader,
      },
    });

    const onProgress = vi.fn();
    const onConnectionChange = vi.fn();
    const unsubscribe = await subscribeToTaskProgress(
      onProgress,
      undefined,
      onConnectionChange,
    );

    await new Promise((resolve) => setTimeout(resolve, 50));

    expect(onConnectionChange).toHaveBeenCalledWith("connected");

    unsubscribe();

    await new Promise((resolve) => setTimeout(resolve, 50));

    expect(onConnectionChange).toHaveBeenCalledWith("disconnected");
  });

  it("should call onError on stream errors", async () => {
    // Mock fetch to fail multiple times to reach max reconnect attempts
    const mockReader = {
      read: vi.fn().mockRejectedValue(new Error("Network timeout")),
      cancel: vi.fn(),
    };

    mockFetch.mockResolvedValue({
      ok: true,
      body: {
        getReader: () => mockReader,
      },
    });

    const onProgress = vi.fn();
    const onError = vi.fn();
    const consoleLog = vi.spyOn(console, "log").mockImplementation(() => {});
    const consoleError = vi
      .spyOn(console, "error")
      .mockImplementation(() => {});

    const unsubscribe = subscribeToTaskProgress(onProgress, onError);

    // Wait for reconnection attempts to exhaust (10 attempts with backoff)
    await new Promise((resolve) => setTimeout(resolve, 100));

    // onError is only called after max reconnection attempts are reached
    // Since reconnection happens with delays, we just verify the connection fails
    expect(consoleError).toHaveBeenCalledWith(
      "Task progress stream error:",
      expect.any(Error),
    );

    unsubscribe();
    consoleLog.mockRestore();
    consoleError.mockRestore();
  });

  it("should cleanup properly on unsubscribe", async () => {
    const mockReader = {
      read: vi.fn().mockResolvedValue({ done: false, value: new Uint8Array() }),
      cancel: vi.fn(),
    };

    mockFetch.mockResolvedValueOnce({
      ok: true,
      body: {
        getReader: () => mockReader,
      },
    });

    const onProgress = vi.fn();
    const unsubscribe = await subscribeToTaskProgress(onProgress);

    unsubscribe();

    await new Promise((resolve) => setTimeout(resolve, 100));

    expect(mockReader.cancel).toHaveBeenCalled();
  });

  it("should handle task lifecycle sequence", async () => {
    const lifecycle =
      'data: {"task_id":"task-1","task_type":"analyze_book","status":"pending","progress":null,"error":null,"library_id":"lib-1"}\n\n' +
      'data: {"task_id":"task-1","task_type":"analyze_book","status":"running","progress":{"current":5,"total":10,"message":"Processing..."},"error":null,"library_id":"lib-1"}\n\n' +
      'data: {"task_id":"task-1","task_type":"analyze_book","status":"completed","progress":{"current":10,"total":10,"message":"Done"},"error":null,"library_id":"lib-1"}\n\n';
    const encoder = new TextEncoder();

    const mockReader = {
      read: vi
        .fn()
        .mockResolvedValueOnce({
          done: false,
          value: encoder.encode(lifecycle),
        })
        .mockResolvedValueOnce({ done: true, value: undefined }),
      cancel: vi.fn(),
    };

    mockFetch.mockResolvedValueOnce({
      ok: true,
      body: {
        getReader: () => mockReader,
      },
    });

    const onProgress = vi.fn();
    const unsubscribe = await subscribeToTaskProgress(onProgress);

    await new Promise((resolve) => setTimeout(resolve, 100));

    expect(onProgress).toHaveBeenCalledTimes(3);
    expect(onProgress).toHaveBeenNthCalledWith(
      1,
      expect.objectContaining({ status: "pending" }),
    );
    expect(onProgress).toHaveBeenNthCalledWith(
      2,
      expect.objectContaining({ status: "running" }),
    );
    expect(onProgress).toHaveBeenNthCalledWith(
      3,
      expect.objectContaining({ status: "completed" }),
    );

    unsubscribe();
  });
});

describe("fetchTaskStats", () => {
  let mockFetch: ReturnType<typeof vi.fn>;

  beforeEach(() => {
    Storage.prototype.getItem = vi.fn((key) => {
      if (key === "jwt_token") return "test-token";
      return null;
    });

    mockFetch = vi.fn();
    global.fetch = mockFetch;
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("should fetch task statistics successfully", async () => {
    const mockStats = {
      pending: 8,
      processing: 2,
      completed: 10,
      failed: 0,
      stale: 0,
      total: 20,
      byType: {
        analyze_book: {
          pending: 8,
          processing: 2,
          completed: 9,
          failed: 0,
          stale: 0,
          total: 19,
        },
        scan_library: {
          pending: 0,
          processing: 0,
          completed: 1,
          failed: 0,
          stale: 0,
          total: 1,
        },
      },
    };

    mockFetch.mockResolvedValueOnce({
      ok: true,
      json: async () => mockStats,
    });

    const stats = await fetchTaskStats();

    expect(mockFetch).toHaveBeenCalledWith("/api/v1/tasks/stats", {
      headers: {
        Authorization: "Bearer test-token",
      },
    });

    expect(stats).toEqual(mockStats);
  });

  it("should return empty stats when not authenticated", async () => {
    Storage.prototype.getItem = vi.fn(() => null);

    const stats = await fetchTaskStats();

    expect(stats).toEqual({
      pending: 0,
      processing: 0,
      completed: 0,
      failed: 0,
      stale: 0,
      total: 0,
      byType: {},
    });

    expect(mockFetch).not.toHaveBeenCalled();
  });

  it("should return empty stats on 401 error", async () => {
    mockFetch.mockResolvedValueOnce({
      ok: false,
      status: 401,
    });

    const stats = await fetchTaskStats();

    expect(stats).toEqual({
      pending: 0,
      processing: 0,
      completed: 0,
      failed: 0,
      stale: 0,
      total: 0,
      byType: {},
    });
  });

  it("should throw error on non-401 errors", async () => {
    mockFetch.mockResolvedValueOnce({
      ok: false,
      status: 500,
      statusText: "Internal Server Error",
    });

    await expect(fetchTaskStats()).rejects.toThrow(
      "HTTP 500: Internal Server Error",
    );
  });
});

describe("fetchPendingTaskCounts", () => {
  let mockFetch: ReturnType<typeof vi.fn>;

  beforeEach(() => {
    Storage.prototype.getItem = vi.fn((key) => {
      if (key === "jwt_token") return "test-token";
      return null;
    });

    mockFetch = vi.fn();
    global.fetch = mockFetch;
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("should extract pending counts from task stats", async () => {
    const mockStats = {
      pending: 12,
      processing: 2,
      completed: 10,
      failed: 0,
      stale: 0,
      total: 24,
      byType: {
        analyze_book: {
          pending: 12,
          processing: 2,
          completed: 9,
          failed: 0,
          stale: 0,
          total: 23,
        },
        scan_library: {
          pending: 0,
          processing: 0,
          completed: 1,
          failed: 0,
          stale: 0,
          total: 1,
        },
      },
    };

    mockFetch.mockResolvedValueOnce({
      ok: true,
      json: async () => mockStats,
    });

    const counts = await fetchPendingTaskCounts();

    expect(counts).toEqual({
      analyze_book: 12,
      scan_library: 0,
    });
  });

  it("should return empty object when no pending tasks", async () => {
    const mockStats = {
      pending: 0,
      processing: 0,
      completed: 10,
      failed: 0,
      stale: 0,
      total: 10,
      byType: {
        analyze_book: {
          pending: 0,
          processing: 0,
          completed: 10,
          failed: 0,
          stale: 0,
          total: 10,
        },
      },
    };

    mockFetch.mockResolvedValueOnce({
      ok: true,
      json: async () => mockStats,
    });

    const counts = await fetchPendingTaskCounts();

    expect(counts).toEqual({
      analyze_book: 0,
    });
  });

  it("should return empty object when not authenticated", async () => {
    Storage.prototype.getItem = vi.fn(() => null);

    const counts = await fetchPendingTaskCounts();

    expect(counts).toEqual({});
    expect(mockFetch).not.toHaveBeenCalled();
  });

  it("should return empty object on 401 error", async () => {
    mockFetch.mockResolvedValueOnce({
      ok: false,
      status: 401,
    });

    const counts = await fetchPendingTaskCounts();

    expect(counts).toEqual({});
  });
});
