import { useEffect, useRef, useState } from "react";
import {
  fetchPendingTaskCounts,
  fetchTasksByStatus,
  type PendingTaskCounts,
  subscribeToTaskProgress,
} from "@/api/tasks";
import { useAuthStore } from "@/store/authStore";
import type { TaskProgressEvent, TaskStatus } from "@/types";

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
 *
 * @returns Object with active tasks and connection state
 */
export function useTaskProgress() {
  const { isAuthenticated } = useAuthStore();
  const [activeTasks, setActiveTasks] = useState<
    Map<string, TaskProgressEvent>
  >(new Map());
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

    // Prevent duplicate subscriptions from rapid effect re-runs
    if (hasSubscribedRef.current) {
      console.debug("Already subscribed, skipping duplicate subscription");
      return;
    }
    hasSubscribedRef.current = true;

    // Convert API task response to TaskProgressEvent format
    const convertTaskToEvent = (task: {
      id: string;
      task_type: string;
      status: string;
      library_id?: string | null;
      series_id?: string | null;
      book_id?: string | null;
      started_at?: string | null;
    }): TaskProgressEvent => {
      // Map "processing" status to "running" for UI consistency
      const status: TaskStatus =
        task.status === "processing" ? "running" : (task.status as TaskStatus);

      return {
        task_id: task.id,
        task_type: task.task_type,
        status,
        progress: undefined,
        error: undefined,
        started_at: task.started_at ?? new Date().toISOString(),
        completed_at: undefined,
        library_id: task.library_id ?? undefined,
        series_id: task.series_id ?? undefined,
        book_id: task.book_id ?? undefined,
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
            next.set(event.task_id, event);
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
              const existing = next.get(event.task_id);
              if (
                !existing ||
                (existing.status === "running" &&
                  !existing.progress &&
                  !existing.completed_at)
              ) {
                next.set(event.task_id, event);
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
      console.debug("Task progress event received:", event);

      setActiveTasks((prev) => {
        const next = new Map(prev);

        // Remove completed or failed tasks after a delay
        if (event.status === "completed" || event.status === "failed") {
          // Keep the event for 5 seconds so UI can show completion
          setTimeout(() => {
            setActiveTasks((current) => {
              const updated = new Map(current);
              updated.delete(event.task_id);
              return updated;
            });
          }, 5000);
        }

        next.set(event.task_id, event);
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
  }, [isAuthenticated]);

  // Sort helper for consistent ordering (by task_type alphabetically)
  const sortTasks = (tasks: TaskProgressEvent[]): TaskProgressEvent[] =>
    tasks.sort((a, b) => a.task_type.localeCompare(b.task_type));

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
    getTasksByStatus: (status: TaskStatus): TaskProgressEvent[] => {
      return sortTasks(
        Array.from(activeTasks.values()).filter(
          (task) => task.status === status,
        ),
      );
    },
    /**
     * Get all tasks for a specific library (sorted by task_type)
     */
    getTasksByLibrary: (libraryId: string): TaskProgressEvent[] => {
      return sortTasks(
        Array.from(activeTasks.values()).filter(
          (task) => task.library_id === libraryId,
        ),
      );
    },
    /**
     * Get a specific task by ID
     */
    getTask: (taskId: string): TaskProgressEvent | undefined => {
      return activeTasks.get(taskId);
    },
  };
}
