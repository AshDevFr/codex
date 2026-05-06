import { useEffect, useRef, useState } from "react";
import {
  fetchPendingTaskCounts,
  fetchTasksByStatus,
  type PendingTaskCounts,
  subscribeToTaskProgress,
} from "@/api/tasks";
import { usePermissions } from "@/hooks/usePermissions";
import { useAuthStore } from "@/store/authStore";
import type { ActiveTask, TaskProgressEvent, TaskStatus } from "@/types";
import { PERMISSIONS } from "@/types/permissions";

type ConnectionState = "connecting" | "connected" | "disconnected" | "failed";

/**
 * Hook to subscribe to task progress events and track active tasks
 *
 * This hook maintains a map of active tasks and their current progress,
 * automatically subscribing to the task progress SSE stream.
 *
 * Features:
 * - Automatic subscription/cleanup
 * - Connection state tracking
 * - Active task tracking
 * - Task completion/failure cleanup
 * - Permission-aware: skips task API calls if user lacks TASKS_READ permission
 *
 * @returns Object with active tasks and connection state
 */
export function useTaskProgress() {
  const { isAuthenticated } = useAuthStore();
  const { hasPermission } = usePermissions();
  const canReadTasks = hasPermission(PERMISSIONS.TASKS_READ);
  const [activeTasks, setActiveTasks] = useState<Map<string, ActiveTask>>(
    new Map(),
  );
  const [connectionState, setConnectionState] =
    useState<ConnectionState>("disconnected");
  const [pendingCounts, setPendingCounts] = useState<PendingTaskCounts>({});

  // Track if we've already subscribed to prevent duplicate subscriptions
  // when isAuthenticated briefly flips during Zustand hydration
  const hasSubscribedRef = useRef(false);

  useEffect(() => {
    if (!isAuthenticated) {
      console.debug("Not authenticated, skipping task progress subscription");
      hasSubscribedRef.current = false;
      return;
    }

    if (!canReadTasks) {
      console.debug(
        "User lacks TASKS_READ permission, skipping task progress subscription",
      );
      hasSubscribedRef.current = false;
      return;
    }

    // Prevent duplicate subscriptions from rapid effect re-runs
    if (hasSubscribedRef.current) {
      console.debug("Already subscribed, skipping duplicate subscription");
      return;
    }
    hasSubscribedRef.current = true;

    // Convert API task response to ActiveTask format. Titles come from the
    // polling snapshot (`GET /api/v1/tasks`); SSE events do not carry them.
    const convertTaskToEvent = (task: {
      id: string;
      taskType: string;
      status: string;
      libraryId?: string | null;
      seriesId?: string | null;
      bookId?: string | null;
      startedAt?: string | null;
      bookTitle?: string | null;
      seriesTitle?: string | null;
      libraryName?: string | null;
    }): ActiveTask => {
      // Map "processing" status to "running" for UI consistency
      const status: TaskStatus =
        task.status === "processing" ? "running" : (task.status as TaskStatus);

      return {
        taskId: task.id,
        taskType: task.taskType,
        status,
        progress: undefined,
        error: undefined,
        startedAt: task.startedAt ?? new Date().toISOString(),
        completedAt: undefined,
        libraryId: task.libraryId ?? undefined,
        seriesId: task.seriesId ?? undefined,
        bookId: task.bookId ?? undefined,
        bookTitle: task.bookTitle ?? undefined,
        seriesTitle: task.seriesTitle ?? undefined,
        libraryName: task.libraryName ?? undefined,
      };
    };

    // Fetch initial pending task counts
    fetchPendingTaskCounts()
      .then((counts) => {
        console.debug("Initial pending task counts:", counts);
        setPendingCounts(counts);
      })
      .catch((error) => {
        // Only log non-401 errors
        if (!(error instanceof Error && error.message.includes("401"))) {
          console.error("Failed to fetch pending task counts:", error);
        }
      });

    // Fetch initial processing tasks and add them to activeTasks
    fetchTasksByStatus("processing", 100)
      .then((tasks) => {
        console.debug("Initial processing tasks:", tasks);
        setActiveTasks((prev) => {
          const next = new Map(prev);
          // Create a set of current processing task IDs
          const currentProcessingIds = new Set(tasks.map((task) => task.id));

          // Remove tasks that were previously "running" (from processing)
          // but are no longer in the processing list
          // Preserve tasks with "completed" or "failed" status (from SSE)
          for (const [taskId, task] of prev.entries()) {
            if (
              task.status === "running" &&
              !currentProcessingIds.has(taskId)
            ) {
              next.delete(taskId);
            }
          }

          // Add or update tasks that are currently processing
          for (const task of tasks) {
            const event = convertTaskToEvent(task);
            next.set(event.taskId, event);
          }
          return next;
        });
      })
      .catch((error) => {
        // Only log non-401 errors
        if (!(error instanceof Error && error.message.includes("401"))) {
          console.error("Failed to fetch processing tasks:", error);
        }
      });

    // Poll for pending counts every 10 seconds
    const pollInterval = setInterval(() => {
      fetchPendingTaskCounts()
        .then((counts) => {
          setPendingCounts(counts);
        })
        .catch((error) => {
          // Only log non-401 errors
          if (!(error instanceof Error && error.message.includes("401"))) {
            console.error("Failed to fetch pending task counts:", error);
          }
        });

      // Also poll for processing tasks to catch any that weren't sent via SSE
      fetchTasksByStatus("processing", 100)
        .then((tasks) => {
          setActiveTasks((prev) => {
            const next = new Map(prev);
            // Create a set of current processing task IDs
            const currentProcessingIds = new Set(tasks.map((task) => task.id));

            // Remove tasks that were previously "running" (from processing)
            // but are no longer in the processing list
            // Preserve tasks with "completed" or "failed" status (from SSE)
            // These are kept for 5 seconds to show completion state
            for (const [taskId, task] of prev.entries()) {
              if (
                task.status === "running" &&
                !currentProcessingIds.has(taskId)
              ) {
                next.delete(taskId);
              }
              // Explicitly preserve completed/failed tasks (they're removed by setTimeout in handleEvent)
              // Don't remove them here even if they're not in the processing list
            }

            // Add or update tasks that are currently processing
            // SSE events take precedence, so we only update if:
            // - Task doesn't exist yet, OR
            // - Task exists with "running" status and no progress (from previous poll)
            // Don't overwrite if SSE has updated it (has progress or different status)
            for (const task of tasks) {
              const event = convertTaskToEvent(task);
              const existing = next.get(event.taskId);
              if (
                !existing ||
                (existing.status === "running" &&
                  !existing.progress &&
                  !existing.completedAt)
              ) {
                next.set(event.taskId, event);
              }
            }
            return next;
          });
        })
        .catch((error) => {
          // Only log non-401 errors
          if (!(error instanceof Error && error.message.includes("401"))) {
            console.error("Failed to fetch processing tasks:", error);
          }
        });
    }, 10000);

    const handleEvent = (event: TaskProgressEvent) => {
      setActiveTasks((prev) => {
        const next = new Map(prev);

        // Remove completed or failed tasks after a delay
        if (event.status === "completed" || event.status === "failed") {
          // Keep the event for 5 seconds so UI can show completion
          setTimeout(() => {
            setActiveTasks((current) => {
              const updated = new Map(current);
              updated.delete(event.taskId);
              return updated;
            });
          }, 5000);
        }

        // SSE events do not carry resolved target titles. Preserve any titles
        // already stashed on this task from the most recent polling snapshot
        // so the UI keeps showing the human-readable label across progress
        // updates.
        const existing = prev.get(event.taskId);
        next.set(event.taskId, {
          ...event,
          bookTitle: existing?.bookTitle,
          seriesTitle: existing?.seriesTitle,
          libraryName: existing?.libraryName,
        });
        return next;
      });
    };

    const handleError = (error: Error) => {
      console.error("Task progress subscription error:", error);
    };

    const handleConnectionStateChange = (state: ConnectionState) => {
      console.debug("Task progress connection state:", state);
      setConnectionState(state);
    };

    console.debug("Subscribing to task progress events...");
    const unsubscribe = subscribeToTaskProgress(
      handleEvent,
      handleError,
      handleConnectionStateChange,
    );

    return () => {
      console.debug("Unsubscribing from task progress events");
      hasSubscribedRef.current = false;
      clearInterval(pollInterval);
      unsubscribe();
    };
  }, [isAuthenticated, canReadTasks]);

  // Sort helper for consistent ordering (by task_type alphabetically)
  const sortTasks = (tasks: ActiveTask[]): ActiveTask[] =>
    tasks.sort((a, b) => a.taskType.localeCompare(b.taskType));

  return {
    /**
     * Array of active tasks (sorted by task_type for consistent UI ordering)
     */
    activeTasks: sortTasks(Array.from(activeTasks.values())),
    /**
     * Current SSE connection state
     */
    connectionState,
    /**
     * Pending task counts by type
     */
    pendingCounts,
    /**
     * Get all tasks with a specific status (sorted by task_type)
     */
    getTasksByStatus: (status: TaskStatus): ActiveTask[] => {
      return sortTasks(
        Array.from(activeTasks.values()).filter(
          (task) => task.status === status,
        ),
      );
    },
    /**
     * Get all tasks for a specific library (sorted by task_type)
     */
    getTasksByLibrary: (libraryId: string): ActiveTask[] => {
      return sortTasks(
        Array.from(activeTasks.values()).filter(
          (task) => task.libraryId === libraryId,
        ),
      );
    },
    /**
     * Get a specific task by ID
     */
    getTask: (taskId: string): ActiveTask | undefined => {
      return activeTasks.get(taskId);
    },
  };
}
