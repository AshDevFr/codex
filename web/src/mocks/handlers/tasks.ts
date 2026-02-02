/**
 * MSW handlers for task queue API endpoints
 */

import { delay, HttpResponse, http } from "msw";
import { createList, createTask, createTaskStats } from "../data/factories";

// Generate mock tasks
let mockTasks = createList(() => createTask(), 50);

export const tasksHandlers = [
  // List tasks (paginated, 1-indexed)
  http.get("/api/v1/tasks", async ({ request }) => {
    await delay(100);
    const url = new URL(request.url);
    const page = Math.max(1, parseInt(url.searchParams.get("page") || "1", 10));
    const pageSize = parseInt(url.searchParams.get("pageSize") || "50", 10);
    const status = url.searchParams.get("status");
    const taskType = url.searchParams.get("taskType");

    let filteredTasks = [...mockTasks];

    if (status) {
      filteredTasks = filteredTasks.filter((t) => t.status === status);
    }
    if (taskType) {
      filteredTasks = filteredTasks.filter((t) => t.task_type === taskType);
    }

    // 1-indexed pagination
    const start = (page - 1) * pageSize;
    const end = start + pageSize;
    const paginatedTasks = filteredTasks.slice(start, end);

    return HttpResponse.json(paginatedTasks);
  }),

  // Get task stats
  http.get("/api/v1/tasks/stats", async () => {
    await delay(50);
    return HttpResponse.json(createTaskStats());
  }),

  // Get single task
  http.get("/api/v1/tasks/:taskId", async ({ params }) => {
    await delay(50);
    const { taskId } = params;
    const task = mockTasks.find((t) => t.id === taskId);

    if (!task) {
      return new HttpResponse(null, { status: 404 });
    }

    return HttpResponse.json(task);
  }),

  // Create task
  http.post("/api/v1/tasks", async ({ request }) => {
    await delay(100);
    const body = (await request.json()) as {
      task_type: string;
      library_id?: string;
      book_id?: string;
    };

    const newTask = createTask({
      task_type: body.task_type,
      library_id: body.library_id,
      book_id: body.book_id,
      status: "pending",
    });

    mockTasks.unshift(newTask);
    return HttpResponse.json({ task_id: newTask.id }, { status: 201 });
  }),

  // Cancel task
  http.post("/api/v1/tasks/:taskId/cancel", async ({ params }) => {
    await delay(100);
    const { taskId } = params;
    const taskIndex = mockTasks.findIndex((t) => t.id === taskId);

    if (taskIndex === -1) {
      return new HttpResponse(null, { status: 404 });
    }

    mockTasks[taskIndex] = {
      ...mockTasks[taskIndex],
      status: "failed",
      last_error: "Cancelled by user",
      completed_at: new Date().toISOString(),
    };

    return HttpResponse.json(mockTasks[taskIndex]);
  }),

  // Retry task
  http.post("/api/v1/tasks/:taskId/retry", async ({ params }) => {
    await delay(100);
    const { taskId } = params;
    const task = mockTasks.find((t) => t.id === taskId);

    if (!task) {
      return new HttpResponse(null, { status: 404 });
    }

    const newTask = createTask({
      task_type: task.task_type,
      library_id: task.library_id,
      book_id: task.book_id,
      status: "pending",
    });

    mockTasks.unshift(newTask);
    return HttpResponse.json({ task_id: newTask.id });
  }),

  // Purge completed tasks
  http.post("/api/v1/tasks/purge/completed", async () => {
    await delay(150);
    const beforeCount = mockTasks.length;
    mockTasks = mockTasks.filter((t) => t.status !== "completed");
    const purgedCount = beforeCount - mockTasks.length;

    return HttpResponse.json({ purged_count: purgedCount });
  }),

  // Purge failed tasks
  http.post("/api/v1/tasks/purge/failed", async () => {
    await delay(150);
    const beforeCount = mockTasks.length;
    mockTasks = mockTasks.filter((t) => t.status !== "failed");
    const purgedCount = beforeCount - mockTasks.length;

    return HttpResponse.json({ purged_count: purgedCount });
  }),

  // Task progress stream (SSE) - handled by eventHandlers in events.ts
  // Keeping this as a fallback that returns a stream that stays open
  http.get("/api/v1/tasks/stream", async () => {
    const encoder = new TextEncoder();
    let intervalId: ReturnType<typeof setInterval> | null = null;

    const stream = new ReadableStream({
      start(controller) {
        controller.enqueue(encoder.encode("data: keep-alive\n\n"));
        // Keep stream open to prevent reconnection spam
        intervalId = setInterval(() => {
          try {
            controller.enqueue(encoder.encode("data: keep-alive\n\n"));
          } catch {
            if (intervalId) {
              clearInterval(intervalId);
              intervalId = null;
            }
          }
        }, 30000);
      },
      cancel() {
        if (intervalId) {
          clearInterval(intervalId);
          intervalId = null;
        }
      },
    });

    return new HttpResponse(stream, {
      headers: {
        "Content-Type": "text/event-stream",
        "Cache-Control": "no-cache",
        Connection: "keep-alive",
      },
    });
  }),
];
