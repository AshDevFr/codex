import { useEffect, useState } from "react";
import { subscribeToTaskProgress } from "@/api/tasks";
import type { TaskProgressEvent, TaskStatus } from "@/types/events";

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
  const [activeTasks, setActiveTasks] = useState<Map<string, TaskProgressEvent>>(new Map());
  const [connectionState, setConnectionState] = useState<ConnectionState>("disconnected");

  useEffect(() => {
    const token = localStorage.getItem("jwt_token");
    if (!token) {
      console.debug("No auth token, skipping task progress subscription");
      return;
    }

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
      handleConnectionStateChange
    );

    return () => {
      console.debug("Unsubscribing from task progress events");
      unsubscribe();
    };
  }, []);

  return {
    /**
     * Array of active tasks
     */
    activeTasks: Array.from(activeTasks.values()),
    /**
     * Current SSE connection state
     */
    connectionState,
    /**
     * Get all tasks with a specific status
     */
    getTasksByStatus: (status: TaskStatus): TaskProgressEvent[] => {
      return Array.from(activeTasks.values()).filter((task) => task.status === status);
    },
    /**
     * Get all tasks for a specific library
     */
    getTasksByLibrary: (libraryId: string): TaskProgressEvent[] => {
      return Array.from(activeTasks.values()).filter((task) => task.library_id === libraryId);
    },
    /**
     * Get a specific task by ID
     */
    getTask: (taskId: string): TaskProgressEvent | undefined => {
      return activeTasks.get(taskId);
    },
  };
}
